const SERVICE_NAME: &str = "swboot-cli";
const SERVICE_DISPLAY_NAME: &str = "Switchboot System Service";
const PIPE_NAME: &str = r"\\.\pipe\ca9ba1f9-4aaa-486f-8ce4-f69453af0c6c";

use crate::logic::{dispatch_command, CliCommand, CommandResponse};
use std::sync::Arc;
use winservice_ipc::{pipe_server, run_service, start_service, IPC};
use winservice_ipc::{ClientRequest, ServerResponse};

#[cfg(windows)]
pub fn launch_windows_service() {
    winservice_ipc::run_windows_service(SERVICE_NAME, my_service_main);
}

#[cfg(windows)]
pub fn my_service_main(arguments: Vec<std::ffi::OsString>) {
    println!("Service main started with arguments: {:?}", arguments);
    let pipe_name_owned = PIPE_NAME.to_owned();
    if let Err(e) = run_service(SERVICE_NAME, move |ctx| {
        let ipc = Arc::new(IPC::new(&pipe_name_owned));
        ipc.set_non_blocking();
        pipe_server(ctx.stop_flag, ipc, handle_client_request);
    }) {
        println!("Error running service: {:?}", e);
    }
}

#[cfg(windows)]
fn handle_client_request(ipc: &IPC, request: &[u8]) {
    // Deserialize the request using bincode

    let client_req: Result<ClientRequest, _> = bincode::deserialize(request);
    let response = match client_req {
        Ok(req) => {
            let command: CliCommand = bincode::deserialize(&req.payload).unwrap();
            let result = dispatch_command(command);
            let result = bincode::serialize(&result).unwrap_or_default();
            ServerResponse {
                id: req.id,
                status: "ok".to_string(),
                result: Some(result),
                error: None,
            }
        }
        Err(e) => ServerResponse {
            id: "".to_string(),
            status: "error".to_string(),
            result: None,
            error: Some(format!("Deserialization error: {}", e)),
        },
    };

    // Serialize the response and send it back
    if let Ok(resp_bytes) = bincode::serialize(&response) {
        ipc.send_message(&resp_bytes);
    }
}

/// Try to send the command to the Windows service via IPC.
/// Returns Some(CommandResponse) if successful, None if IPC fails.
#[cfg(windows)]
pub fn run_pipe_client() {
    use std::io::{BufRead, BufReader, Write};
    use winservice_ipc::IPCClient;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin);

    // Connect to the pipe once and keep the connection open
    let client = match IPCClient::connect(PIPE_NAME) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[ERROR] Failed to connect to pipe: {}", e);
            std::process::exit(1);
        }
    };

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let args: Vec<String> = match serde_json::from_str(&line) {
            Ok(a) => a,
            Err(e) => {
                let resp = CommandResponse {
                    code: 1,
                    message: format!("Invalid input: {}", e),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        let command = CliCommand::from_args(&args);
        // Use the same client for all requests
        let payload = match bincode::serialize(&command) {
            Ok(p) => p,
            Err(e) => {
                let resp = CommandResponse {
                    code: 1,
                    message: format!("Serialization error: {}", e),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };
        use rand::Rng;
        let req = ClientRequest {
            id: rand::rng().random::<u128>().to_string(),
            payload,
        };
        let req_bytes = match bincode::serialize(&req) {
            Ok(b) => b,
            Err(e) => {
                let resp = CommandResponse {
                    code: 1,
                    message: format!("Serialization error: {}", e),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };
        let response = match client.send_request(req_bytes) {
            Ok(resp) => {
                if resp.status == "ok" {
                    if let Some(result_bytes) = resp.result {
                        bincode::deserialize(&result_bytes).unwrap_or(CommandResponse {
                            code: 1,
                            message: "Failed to decode response".to_string(),
                        })
                    } else {
                        CommandResponse {
                            code: 1,
                            message: "No result in response".to_string(),
                        }
                    }
                } else {
                    CommandResponse {
                        code: 1,
                        message: resp.error.unwrap_or_else(|| "Unknown error".to_string()),
                    }
                }
            }
            Err(e) => CommandResponse {
                code: 1,
                message: format!("IPC communication failed: {}", e),
            },
        };
        let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
        let _ = stdout.flush();
    }
}

#[cfg(windows)]
pub fn run_pipe_server() {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use winservice_ipc::{pipe_server, IPC};
    println!("[INFO] Starting pipe server (not as a Windows service)...");
    let should_stop = Arc::new(AtomicBool::new(false));
    let ipc = Arc::new(IPC::new(PIPE_NAME));
    ipc.set_non_blocking();
    pipe_server(should_stop, ipc, handle_client_request);
}

#[cfg(windows)]
pub fn run_service_client() {
    if let Err(e) = start_service(SERVICE_NAME) {
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
    match winservice_ipc::install_service(SERVICE_NAME, SERVICE_DISPLAY_NAME, &bin_path) {
        Ok(_) => println!("Service installed successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to install service: {}", e.message());
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
pub fn uninstall_service() {
    match winservice_ipc::uninstall_service(SERVICE_NAME) {
        Ok(_) => println!("Service uninstalled successfully."),
        Err(e) => {
            eprintln!("[ERROR] Failed to uninstall service: {}", e.message());
            std::process::exit(1);
        }
    }
}
