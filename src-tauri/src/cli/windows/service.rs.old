pub const SERVICE_NAME: &str = "swboot-cli";
pub const SERVICE_DISPLAY_NAME: &str = "Switchboot System Service";
use super::pipe::{run_pipe_client, run_pipe_server_async_with_ready};
use std::sync::Arc;
use win_service;
const SERVICE_START_TIMEOUT: u64 = 5; // seconds

#[cfg(windows)]
pub fn launch_windows_service() {
    win_service::service::run_windows_service(SERVICE_NAME, my_service_main);
}

#[cfg(windows)]
pub fn my_service_main(arguments: Vec<std::ffi::OsString>) {
    println!("Service main started with arguments: {:?}", arguments);

    use crate::PIPE_SERVER_WAIT_TIMEOUT;
    use win_service::service::run_service_with_readiness;

    if let Err(e) = run_service_with_readiness(
        SERVICE_NAME,
        move |ctx| {
            // Use the runtime handle provided by the service supervisor so
            // readiness signaling and task execution occur on the same runtime.
            let shutdown_notify = ctx
                .stop_notify
                .clone()
                .unwrap_or_else(|| Arc::new(tokio::sync::Notify::new()));
            let ready_notify = ctx.ready_notify.clone();

            let rt_handle = ctx
                .runtime_handle
                .expect("runtime_handle must be provided when wait_for_ready=true");

            println!("[SERVICE] Starting tokio-based pipe server (shared runtime)...");

            // Spawn the async pipe server onto the shared runtime
            let server_join = rt_handle.spawn(run_pipe_server_async_with_ready(
                shutdown_notify,
                Some(PIPE_SERVER_WAIT_TIMEOUT),
                false, // Don't wait for new clients in service mode
                ready_notify,
            ));

            println!("[SERVICE] Waiting for pipe server to complete...");

            // Use the same runtime to await the server task so it actually runs
            match rt_handle.block_on(server_join) {
                Ok(Ok(())) => {
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
    use win_service::service::start_service;
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
    match win_service::service::install_service(SERVICE_NAME, SERVICE_DISPLAY_NAME, &bin_path) {
        Ok(_) => println!("Service installed successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to install service: {}", e.message());
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
pub fn uninstall_service() {
    match win_service::service::uninstall_service(SERVICE_NAME, true) {
        Ok(_) => println!("Service uninstalled successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to uninstall service: {}", e.message());
            std::process::exit(1);
        }
    }
}
