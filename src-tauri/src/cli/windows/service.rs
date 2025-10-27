//! Windows service implementation for switchboot
//!
//! This module provides the service main loop and entry points for running as a Windows service.
//! Service management (install/uninstall/start/stop) is handled by the `service_management` module.

use super::pipe::run_elevated_connector_async;
use super::service_management::{self, ServiceConfig};
use std::ffi::OsString;
use std::sync::mpsc;
use std::sync::Arc;
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

// Define the service entry point function
define_windows_service!(ffi_service_main, service_main);

/// Launch the Windows service (called when running as a service)
pub fn launch_windows_service_connector() {
    // Run the service dispatcher, which will call our service_main function
    if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        eprintln!("[SERVICE ERROR] Failed to start service dispatcher: {}", e);
    }
}

/// Service main function - executed when the service starts
fn service_main(_arguments: Vec<OsString>) {
    // Create a channel for receiving service control events
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    // Define the service control handler
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

    // Register the service control handler
    let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("[SERVICE ERROR] Failed to register control handler: {}", e);
            return;
        }
    };

    // Tell SCM that the service is starting
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(1),
        process_id: None,
    });

    // Create a tokio runtime for running the async pipe connector
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

    // Create shutdown notification for the elevated connector
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let shutdown_notify_clone = shutdown_notify.clone();

    // Spawn the elevated connector in the background
    let connector_handle = rt.spawn(async move {
        if let Err(e) = run_elevated_connector_async(Some(shutdown_notify_clone)).await {
            eprintln!("[SERVICE ERROR] Elevated connector failed: {}", e);
        }
    });

    // Tell SCM that the service is now running
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    });

    // Wait for shutdown signal
    let _ = shutdown_rx.recv();

    // Tell SCM that the service is stopping
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(5),
        process_id: None,
    });

    // Notify the elevated connector to shut down
    shutdown_notify.notify_one();

    // Wait for the connector to finish with timeout
    rt.block_on(async {
        let timeout = tokio::time::timeout(tokio::time::Duration::from_secs(5), connector_handle);
        if timeout.await.is_err() {
            eprintln!("[SERVICE WARN] Connector shutdown timed out");
        }
    });

    // Tell SCM that the service has stopped
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

/// Run the service manager - starts the service and creates a pipe server
/// This is called from the unelevated user instance in non-portable mode
pub fn run_service_manager() {
    use super::pipe::run_unelevated_pipe_server;
    use crate::constants::PIPE_SERVER_WAIT_TIMEOUT;

    eprintln!("[SERVICE_MANAGER] Starting service manager...");

    // Check if service is installed first
    if !is_service_installed() {
        eprintln!("[SERVICE_MANAGER ERROR] Service is not installed!");
        eprintln!("[SERVICE_MANAGER] Please run: switchboot.exe --cli /install_service");
        eprintln!("[SERVICE_MANAGER] (This requires administrator privileges)");
        std::process::exit(1);
    }

    // Try to start the service (it may already be running, which is fine)
    match service_management::start_service(SERVICE_NAME, Some(5)) {
        Ok(_) => {
            eprintln!("[SERVICE_MANAGER] Service started successfully");
        }
        Err(e) => {
            // Check if it's an access denied error
            if format!("{:?}", e).contains("Access is denied") {
                eprintln!("[SERVICE_MANAGER ERROR] Access denied when starting service");
                eprintln!("[SERVICE_MANAGER] The service may need to be started with administrator privileges");
                std::process::exit(1);
            }
            eprintln!("[SERVICE_MANAGER] Warning: Could not start service: {}", e);
            eprintln!("[SERVICE_MANAGER] The service may already be running");
            // Continue anyway - the service might already be running
        }
    }

    // Now run the unelevated pipe server
    eprintln!("[SERVICE_MANAGER] Starting pipe server...");
    run_unelevated_pipe_server(Some(PIPE_SERVER_WAIT_TIMEOUT), false);

    // When the pipe server exits (user app closed), stop the service
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

/// Check if the service is installed
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

/// Install the service
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
            println!("\nVerifying permissions...");
            if let Err(e) = verify_service_permissions() {
                eprintln!("Warning: Could not verify permissions: {}", e);
            }
        }
        Err(e) => {
            eprintln!("[ERROR] Failed to install service: {}", e);
            std::process::exit(1);
        }
    }
}

/// Verify that the service has the correct permissions for non-elevated start
fn verify_service_permissions() -> Result<(), String> {
    use std::process::Command;

    let output = Command::new("sc.exe")
        .args(&["sdshow", SERVICE_NAME])
        .output()
        .map_err(|e| format!("Failed to run sc.exe: {}", e))?;

    if !output.status.success() {
        return Err("Failed to query service security descriptor".to_string());
    }

    let sddl = String::from_utf8_lossy(&output.stdout);
    let sddl = sddl.trim();

    println!("Service security descriptor: {}", sddl);

    if sddl.contains(";;;WD)") {
        println!("✓ Service has Everyone (WD) permissions - non-elevated start should work");
        Ok(())
    } else {
        println!("✗ Service does NOT have Everyone (WD) permissions");
        println!("  Non-elevated processes may not be able to start the service");
        Err("Missing WD permissions".to_string())
    }
}

/// Uninstall the service
pub fn uninstall_service() {
    match service_management::uninstall_service(SERVICE_NAME, true) {
        Ok(_) => println!("Service uninstalled successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to uninstall service: {}", e);
            std::process::exit(1);
        }
    }
}
