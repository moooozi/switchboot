//! Windows service entry point and lifecycle handling.
//! Install/uninstall/start/stop live in the `service_management` module.

use super::pipe::run_elevated_connector_async;
use super::service_management::{self, ServiceConfig};
use std::ffi::OsString;
use std::sync::Arc;
use std::sync::mpsc;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

use crate::constants::{SERVICE_DISPLAY_NAME, SERVICE_NAME};

define_windows_service!(ffi_service_main, service_main);

/// Entry point invoked by the SCM when running as a service.
pub fn launch_windows_service_connector() {
    if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        eprintln!("[SERVICE ERROR] Failed to start service dispatcher: {}", e);
    }
}

fn service_main(_arguments: Vec<OsString>) {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("[SERVICE ERROR] Failed to register control handler: {}", e);
            return;
        }
    };

    // Report StartPending to the SCM.
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(1),
        process_id: None,
    });

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("[SERVICE ERROR] Failed to create tokio runtime: {}", e);
            let _ = status_handle.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: ServiceState::Stopped,
                controls_accepted: ServiceControlAccept::empty(),
                exit_code: ServiceExitCode::Win32(1),
                checkpoint: 0,
                wait_hint: std::time::Duration::default(),
                process_id: None,
            });
            return;
        }
    };

    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let shutdown_notify_clone = shutdown_notify.clone();

    let connector_handle = rt.spawn(async move {
        if let Err(e) = run_elevated_connector_async(Some(shutdown_notify_clone)).await {
            eprintln!("[SERVICE ERROR] Elevated connector failed: {}", e);
        }
    });

    // Report Running to the SCM.
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    });

    let _ = shutdown_rx.recv();

    // Report StopPending to the SCM.
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(5),
        process_id: None,
    });

    shutdown_notify.notify_one();

    rt.block_on(async {
        let timeout = tokio::time::timeout(tokio::time::Duration::from_secs(5), connector_handle);
        if timeout.await.is_err() {
            eprintln!("[SERVICE WARN] Connector shutdown timed out");
        }
    });

    // Report Stopped to the SCM.
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    });
}

/// Start the service and run the unelevated pipe server (non-portable mode).
pub fn run_service_manager() {
    use super::pipe::run_unelevated_pipe_server;
    use crate::constants::PIPE_SERVER_WAIT_TIMEOUT;

    eprintln!("[SERVICE_MANAGER] Starting service manager...");

    if !is_service_installed() {
        eprintln!("[SERVICE_MANAGER ERROR] Service is not installed!");
        eprintln!("[SERVICE_MANAGER] Please run: switchboot.exe --cli /install_service");
        eprintln!("[SERVICE_MANAGER] (This requires administrator privileges)");
        std::process::exit(1);
    }

    // Starting an already-running service is not an error.
    match service_management::start_service(SERVICE_NAME, Some(5)) {
        Ok(_) => {
            eprintln!("[SERVICE_MANAGER] Service started successfully");
        }
        Err(e) => {
            if format!("{:?}", e).contains("Access is denied") {
                eprintln!("[SERVICE_MANAGER ERROR] Access denied when starting service");
                eprintln!(
                    "[SERVICE_MANAGER] The service may need to be started with administrator privileges"
                );
                std::process::exit(1);
            }
            eprintln!("[SERVICE_MANAGER] Warning: Could not start service: {}", e);
            eprintln!("[SERVICE_MANAGER] The service may already be running");
        }
    }

    eprintln!("[SERVICE_MANAGER] Starting pipe server...");
    run_unelevated_pipe_server(Some(PIPE_SERVER_WAIT_TIMEOUT), false);

    // Stop the service when the user-facing app (and thus the pipe server) exits.
    eprintln!("[SERVICE_MANAGER] Pipe server exited, stopping service...");
    match service_management::stop_service(SERVICE_NAME) {
        Ok(_) => {
            eprintln!("[SERVICE_MANAGER] Service stopped successfully");
        }
        Err(e) => {
            eprintln!("[SERVICE_MANAGER] Warning: Could not stop service: {}", e);
        }
    }
}

fn is_service_installed() -> bool {
    use windows_service::service::ServiceAccess;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let manager = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
    {
        Ok(m) => m,
        Err(_) => return false,
    };

    manager
        .open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        .is_ok()
}

pub fn install_service() {
    let executable_path = std::env::current_exe().expect("Failed to get current executable path");
    let launch_arguments = vec![
        OsString::from("--cli"),
        OsString::from("/service_connector"),
    ];

    let config = ServiceConfig {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        executable_path,
        launch_arguments,
        grant_start_to_everyone: true,
    };

    match service_management::install_service(config) {
        Ok(_) => {
            println!("Service installed successfully.");
        }
        Err(e) => {
            eprintln!("[ERROR] Failed to install service: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn uninstall_service() {
    match service_management::uninstall_service(SERVICE_NAME, true) {
        Ok(_) => println!("Service uninstalled successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to uninstall service: {}", e);
            std::process::exit(1);
        }
    }
}
