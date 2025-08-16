pub const SERVICE_NAME: &str = "swboot-cli";
pub const SERVICE_DISPLAY_NAME: &str = "Switchboot System Service";
use super::pipe::{run_pipe_client, run_pipe_server_async_with_ready};

const SERVICE_START_TIMEOUT: u64 = 5; // seconds

#[cfg(windows)]
pub fn launch_windows_service() {
    winservice_ipc::service::run_windows_service(SERVICE_NAME, my_service_main);
}

#[cfg(windows)]
pub fn my_service_main(arguments: Vec<std::ffi::OsString>) {
    println!("Service main started with arguments: {:?}", arguments);

    use crate::PIPE_SERVER_WAIT_TIMEOUT;
    use winservice_ipc::service::run_service_with_readiness;

    if let Err(e) = run_service_with_readiness(
        SERVICE_NAME,
        move |ctx| {
            // Create tokio runtime for async operations
            let rt =
                tokio::runtime::Runtime::new().expect("Failed to create tokio runtime in service");

            // Convert Windows service stop flag to our pipe server format
            let shutdown_signal = ctx.stop_flag.clone();

            // Use the service's ready signal for pipe server readiness
            let ready_signal = ctx.ready_signal.clone();

            println!("[SERVICE] Starting tokio-based pipe server...");

            // Run the async pipe server with Windows service shutdown integration and readiness signaling
            let server_task = rt.spawn(run_pipe_server_async_with_ready(
                shutdown_signal,
                Some(PIPE_SERVER_WAIT_TIMEOUT),
                false, // Don't wait for new clients in service mode
                ready_signal,
            ));

            println!("[SERVICE] Waiting for pipe server to complete...");

            // Wait for the server task to complete
            match rt.block_on(server_task) {
                Ok(Ok(())) => {
                    // Success case
                    println!("[SERVICE] Pipe server completed successfully");
                }
                Ok(Err(server_error)) => {
                    eprintln!("[SERVICE ERROR] Pipe server failed: {}", server_error);
                }
                Err(join_error) => {
                    if !join_error.is_cancelled() {
                        eprintln!("[SERVICE ERROR] Server task failed: {}", join_error);
                    }
                }
            }

            println!("[SERVICE] Service stopped gracefully");
        },
        true,
    ) {
        // Enable readiness waiting
        eprintln!("Error running service: {:?}", e);
    }
}

#[cfg(windows)]
pub fn run_service_client() {
    use winservice_ipc::service::start_service;
    if let Err(e) = start_service(SERVICE_NAME, Some(SERVICE_START_TIMEOUT)) {
        eprintln!("[ERROR] Failed to start service: {}", e);
        std::process::exit(1);
    }
    run_pipe_client();
}

#[cfg(windows)]
pub fn install_service() {
    // the current executable path
    let executable_path = std::env::current_exe().expect("Failed to get current executable path");
    let executable_path_str = executable_path
        .to_str()
        .expect("Executable path is not valid UTF-8");
    let bin_path = format!("\"{}\" /service", executable_path_str);
    match winservice_ipc::service::install_service(SERVICE_NAME, SERVICE_DISPLAY_NAME, &bin_path) {
        Ok(_) => println!("Service installed successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to install service: {}", e.message());
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
pub fn uninstall_service() {
    match winservice_ipc::service::uninstall_service(SERVICE_NAME, true) {
        Ok(_) => println!("Service uninstalled successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to uninstall service: {}", e.message());
            std::process::exit(1);
        }
    }
}
