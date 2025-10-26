//! Windows service implementation for switchboot
//!
//! This module provides the service main loop and entry points for running as a Windows service.
//! Service management (install/uninstall/start/stop) is handled by the `service_management` module.

use super::pipe::run_pipe_server_async_with_ready;
use super::service_management::{self, ServiceConfig};
use std::ffi::OsString;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

pub const SERVICE_NAME: &str = "swboot-cli";
pub const SERVICE_DISPLAY_NAME: &str = "Switchboot System Service";
const SERVICE_START_TIMEOUT: u64 = 5; // seconds

/// Context passed to the service main function
pub struct ServiceContext {
    /// Notifier for when the service should stop
    pub stop_notify: Arc<Notify>,
    /// Sender to signal when the service is ready (optional)
    pub ready_sender: Option<mpsc::Sender<()>>,
}

/// Type alias for the service main function
type ServiceMainFn = Box<dyn FnOnce(ServiceContext) + Send + 'static>;

// Global state for the service
static SERVICE_MAIN: std::sync::Mutex<Option<ServiceMainFn>> = std::sync::Mutex::new(None);

define_windows_service!(ffi_service_main, service_main_wrapper);

/// Internal service main wrapper that the Windows Service dispatcher calls
fn service_main_wrapper(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_impl() {
        log::error!("Service encountered an error: {:?}", e);
    }
}

fn run_service_impl() -> windows_service::Result<()> {
    let stop_notify = Arc::new(Notify::new());
    let (ready_tx, ready_rx) = mpsc::channel();

    let stop_notify_handler = stop_notify.clone();

    // Define the service control handler
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Interrogate => {
                stop_notify_handler.notify_waiters();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register the service control handler
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Set service status to START_PENDING
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(30),
        process_id: None,
    })?;

    // Take the service main function
    let service_main = SERVICE_MAIN
        .lock()
        .unwrap()
        .take()
        .expect("Service main function not set");

    // Spawn the service main function in a thread
    let service_thread = std::thread::spawn(move || {
        service_main(ServiceContext {
            stop_notify: stop_notify.clone(),
            ready_sender: Some(ready_tx),
        });
    });

    // Wait for readiness signal with timeout
    let ready_signaled = ready_rx.recv_timeout(Duration::from_secs(30)).is_ok();

    if ready_signaled {
        // Set service status to RUNNING
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;
        log::info!("Service is now running");
    } else {
        log::warn!("Service did not signal readiness within timeout, setting to running anyway");
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;
    }

    // Wait for the service thread to complete
    service_thread.join().unwrap();

    // Set service status to STOPPED
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    log::info!("Service stopped successfully");
    Ok(())
}

/// Runs a Windows service with the given service main function
fn run_service<F>(service_name: &str, service_main: F) -> windows_service::Result<()>
where
    F: FnOnce(ServiceContext) + Send + 'static,
{
    // Store the service main function in global state
    *SERVICE_MAIN.lock().unwrap() = Some(Box::new(service_main));

    // Start the service dispatcher
    service_dispatcher::start(service_name, ffi_service_main)?;

    Ok(())
}

/// Launch the Windows service (called when running as a service)
#[cfg(windows)]
pub fn launch_windows_service() {
    if let Err(e) = run_service(SERVICE_NAME, my_service_main) {
        eprintln!("Error running service: {:?}", e);
        std::process::exit(1);
    }
}

/// The main service logic
#[cfg(windows)]
fn my_service_main(ctx: ServiceContext) {
    println!("[SERVICE] Starting service main...");

    // Create a tokio runtime for the pipe server
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("[SERVICE ERROR] Failed to create tokio runtime: {}", e);
            return;
        }
    };

    use crate::PIPE_SERVER_WAIT_TIMEOUT;

    // Run the async pipe server
    let result = rt.block_on(run_pipe_server_async_with_ready(
        ctx.stop_notify.clone(),
        Some(PIPE_SERVER_WAIT_TIMEOUT),
        false, // Don't wait for new clients in service mode
        ctx.ready_sender,
    ));

    match result {
        Ok(()) => {
            println!("[SERVICE] Pipe server completed successfully");
        }
        Err(e) => {
            eprintln!("[SERVICE ERROR] Pipe server failed: {}", e);
        }
    }

    println!("[SERVICE] Service stopped gracefully");
}

/// Run as a client (start the service if needed and connect to it)
#[cfg(windows)]
pub fn run_service_client() {
    use super::pipe::run_pipe_client;

    if let Err(e) = service_management::start_service(SERVICE_NAME, Some(SERVICE_START_TIMEOUT)) {
        eprintln!("[ERROR] Failed to start service: {}", e);
        std::process::exit(1);
    }

    run_pipe_client();
}

/// Install the service
#[cfg(windows)]
pub fn install_service() {
    let executable_path = std::env::current_exe().expect("Failed to get current executable path");
    let mut launch_arguments = vec![OsString::from("--cli /service")];

    let config = ServiceConfig {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        executable_path,
        launch_arguments,
        grant_start_to_everyone: true,
    };

    match service_management::install_service(config) {
        Ok(_) => println!("Service installed successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to install service: {}", e);
            std::process::exit(1);
        }
    }
}

/// Uninstall the service
#[cfg(windows)]
pub fn uninstall_service() {
    match service_management::uninstall_service(SERVICE_NAME, true) {
        Ok(_) => println!("Service uninstalled successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to uninstall service: {}", e);
            std::process::exit(1);
        }
    }
}
