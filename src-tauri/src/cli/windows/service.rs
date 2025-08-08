pub const SERVICE_NAME: &str = "swboot-cli";
pub const SERVICE_DISPLAY_NAME: &str = "Switchboot System Service";
use super::pipe::PIPE_NAME;
use super::pipe::{handle_client_request, run_pipe_client};
use std::sync::Arc;

const SERVICE_START_TIMEOUT: u64 = 5; // seconds

#[cfg(windows)]
pub fn launch_windows_service() {
    winservice_ipc::service::run_windows_service(SERVICE_NAME, my_service_main);
}

#[cfg(windows)]
pub fn my_service_main(arguments: Vec<std::ffi::OsString>) {
    use winservice_ipc::service::run_service;
    println!("Service main started with arguments: {:?}", arguments);
    let pipe_name_owned = PIPE_NAME.to_owned();
    if let Err(e) = run_service(SERVICE_NAME, move |ctx| {
        use winservice_ipc::ipc_server::{pipe_server, IPCServer};

        use crate::PIPE_SERVER_WAIT_TIMEOUT;
        use std::time::Duration;

        let ipc = Arc::new(IPCServer::new(&pipe_name_owned));
        ipc.set_non_blocking();
        pipe_server(
            ctx.stop_flag,
            ipc,
            handle_client_request,
            Some(Duration::from_secs(PIPE_SERVER_WAIT_TIMEOUT)),
            false,
        );
    }) {
        println!("Error running service: {:?}", e);
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
