#[cfg(windows)]
use winservice_ipc::ipc_server::IPCServer;

pub const PIPE_NAME: &str = r"\\.\pipe\ca9ba1f9-4aaa-486f-8ce4-f69453af0c6c";

#[cfg(feature = "encrypted_pipe")]
use super::crypto::ChaChaCrypto as SelectedCrypto;
#[cfg(not(feature = "encrypted_pipe"))]
use super::crypto::NoCrypto as SelectedCrypto;

/// Try to send the command to the Windows service via IPC.
/// Returns Some(CommandResponse) if successful, None if IPC fails.
#[cfg(windows)]
pub fn run_pipe_client() {
    use super::{CliCommand, CommandResponse};
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

        let command = match CliCommand::from_args(&args) {
            Ok(cmd) => cmd,
            Err(e) => {
                let resp = CommandResponse {
                    code: 1,
                    message: e,
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };
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

        use crate::windows::crypto::MessageCrypto;

        let req = winservice_ipc::ClientRequest {
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
        let crypto = SelectedCrypto;
        // When sending:
        let encrypted = crypto.encrypt(&req_bytes).expect("Encryption failed");
        let response = match client.send_request(encrypted) {
            Ok(resp_bytes) => {
                // Decrypt the entire response buffer
                let decrypted = crypto.decrypt(&resp_bytes).expect("Decryption failed");
                // Deserialize the decrypted bytes into ServerResponse
                match bincode::deserialize::<winservice_ipc::ServerResponse>(&decrypted) {
                    Ok(resp) => {
                        if resp.status == "ok" {
                            if let Some(result_bytes) = resp.result {
                                match bincode::deserialize(&result_bytes) {
                                    Ok(decrypted) => decrypted,
                                    Err(e) => CommandResponse {
                                        code: 1,
                                        message: format!("Failed to decode response: {}", e),
                                    },
                                }
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
                        message: format!("Failed to decode ServerResponse: {}", e),
                    },
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
pub fn run_pipe_server(timeout: Option<u64>, wait_for_new_client: bool) {
    use std::sync::Arc;
    use std::{sync::atomic::AtomicBool, time::Duration};
    use winservice_ipc::ipc_server::pipe_server_blocking;

    println!("[INFO] Starting pipe server (not as a Windows service)...");

    let should_stop = Arc::new(AtomicBool::new(false));
    let ipc = Arc::new(IPCServer::new(PIPE_NAME));

    let duration = timeout.map(Duration::from_secs);

    pipe_server_blocking(
        should_stop,
        ipc,
        handle_client_request,
        duration,
        wait_for_new_client,
    );
}

#[cfg(windows)]
pub fn handle_client_request(ipc: &IPCServer, request: &[u8]) {
    use super::{dispatch_command, CliCommand};
    use crate::windows::crypto::MessageCrypto;
    use winservice_ipc::ClientRequest;
    use winservice_ipc::ServerResponse;

    let crypto = SelectedCrypto;
    let decrypted = match crypto.decrypt(request) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("[SERVER] Decryption failed: {e}");
            // Optionally send an error response here
            return;
        }
    };

    let client_req: Result<ClientRequest, _> = bincode::deserialize(&decrypted);
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

    if let Ok(resp_bytes) = bincode::serialize(&response) {
        if let Ok(enc) = crypto.encrypt(&resp_bytes) {
            ipc.send_message(&enc);
        }
    }
}
