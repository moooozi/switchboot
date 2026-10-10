use crate::build_info;
use pipeguard::{NamedPipeClientStruct, NamedPipeServerStruct};
use std::io::Write;

macro_rules! ipc_debug {
    ($($args:tt)*) => {
        if std::env::var_os("SWITCHBOOT_DEBUG_IPC").is_some() {
            eprintln!($($args)*);
        }
    };
}

pub const PIPE_NAME: &str = build_info::APP_IDENTIFIER_VERSION;
const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn with_deadline<T>(
    duration: std::time::Duration,
    operation: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(duration, operation)
        .await
        .map_err(|_| "IPC operation timed out; command outcome may be unknown".to_string())?
}

/// User instance: run the pipe server, forwarding stdin commands to the elevated
/// instance and writing responses to stdout. Blocks until the connection closes.
#[cfg(windows)]
pub fn run_unelevated_pipe_server(
    timeout: Option<u64>,
    _wait_for_new_client: bool,
) -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    ipc_debug!("[PIPE_SERVER] Starting unelevated pipe server...");
    ipc_debug!("[PIPE_SERVER] Pipe name: {}", PIPE_NAME);

    rt.block_on(run_unelevated_pipe_server_async(timeout))
}

#[cfg(windows)]
async fn run_unelevated_pipe_server_async(timeout: Option<u64>) -> Result<(), String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::sync::mpsc;

    let (connection_tx, mut connection_rx) = mpsc::channel::<pipeguard::NamedPipeConnection>(1);

    let mut server = NamedPipeServerStruct::new_encrypted(PIPE_NAME, None);

    ipc_debug!("[PIPE_SERVER] Pipe server created, waiting for elevated client to connect...");

    let server_handle = tokio::spawn(async move {
        server
            .start(move |connection| {
                let connection_tx = connection_tx.clone();
                async move {
                    ipc_debug!(
                        "[PIPE_SERVER] Elevated client connected with ID: {}",
                        connection.id()
                    );

                    if connection_tx.send(connection).await.is_err() {
                        eprintln!("[PIPE_SERVER ERROR] Failed to send connection to main loop");
                        return Err(pipeguard::NamedPipeError::Io(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "Channel closed",
                        )));
                    }

                    // Keep this handler alive for the connection's lifetime.
                    std::future::pending::<()>().await;
                    Ok(())
                }
            })
            .await
    });

    let _server_guard = AbortOnDrop(server_handle.abort_handle());
    ipc_debug!("[PIPE_SERVER] Waiting for elevated client connection...");
    let incoming = async {
        connection_rx
            .recv()
            .await
            .ok_or_else(|| "Server closed without accepting connection".to_string())
    };
    let connected = if let Some(seconds) = timeout {
        with_deadline(std::time::Duration::from_secs(seconds), incoming).await?
    } else {
        incoming.await?
    };
    let mut connection = connected;

    ipc_debug!("[PIPE_SERVER] Starting command processing loop");

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line_buffer = String::new();

    loop {
        line_buffer.clear();

        match reader.read_line(&mut line_buffer).await {
            Ok(0) => {
                ipc_debug!("[PIPE_SERVER] EOF on stdin, disconnecting...");
                break;
            }
            Ok(n) => {
                ipc_debug!("[PIPE_SERVER] Read {} bytes from stdin", n);
                let line = line_buffer.trim();
                if line.is_empty() {
                    ipc_debug!("[PIPE_SERVER] Empty line, skipping");
                    continue;
                }

                ipc_debug!("[PIPE_SERVER] Processing command: {}", line);
                match with_deadline(
                    COMMAND_TIMEOUT,
                    send_command_and_get_response(&mut connection, line),
                )
                .await
                {
                    Ok(response) => {
                        ipc_debug!("[PIPE_SERVER] Received response, outputting to stdout");
                        let mut stdout = std::io::stdout().lock();
                        writeln!(stdout, "{response}").map_err(|e| e.to_string())?;
                        stdout.flush().map_err(|e| e.to_string())?;
                    }
                    Err(e) => {
                        return Err(format!("Failed to process command: {e}"));
                    }
                }
            }
            Err(e) => {
                return Err(format!("Failed to read input: {e}"));
            }
        }
    }

    ipc_debug!("[PIPE_SERVER] Disconnecting and cleaning up");
    drop(connection);
    server_handle.abort();

    ipc_debug!("[PIPE_SERVER] Pipe server stopped");
    Ok(())
}

/// Forward one command to the elevated client and return its JSON-encoded response.
async fn send_command_and_get_response(
    connection: &mut pipeguard::NamedPipeConnection,
    line: &str,
) -> Result<String, String> {
    use super::CliCommand;
    use crate::types::CommandResponse;

    ipc_debug!("[PIPE_SERVER] Parsing JSON args from: {}", line);
    let args: Vec<String> =
        serde_json::from_str(line).map_err(|e| format!("Invalid JSON input: {}", e))?;

    ipc_debug!("[PIPE_SERVER] Creating command from args: {:?}", args);
    let command = CliCommand::from_args(&args).map_err(|e| format!("Invalid command: {}", e))?;

    ipc_debug!("[PIPE_SERVER] Serializing command");
    let command_bytes =
        bincode::serialize(&command).map_err(|e| format!("Serialization error: {}", e))?;

    ipc_debug!(
        "[PIPE_SERVER] Sending {} bytes to elevated client",
        command_bytes.len()
    );
    connection
        .send_bytes(&command_bytes)
        .await
        .map_err(|e| format!("Failed to send command: {}", e))?;

    ipc_debug!("[PIPE_SERVER] Waiting for response from elevated client...");
    let response_bytes = connection
        .receive_bytes()
        .await
        .map_err(|e| format!("Failed to receive response: {}", e))?;

    ipc_debug!(
        "[PIPE_SERVER] Received {} bytes response",
        response_bytes.len()
    );
    let response: CommandResponse = bincode::deserialize(&response_bytes)
        .map_err(|e| format!("Failed to deserialize response: {}", e))?;

    ipc_debug!("[PIPE_SERVER] Response deserialized successfully");
    let response_json = serde_json::to_string(&response)
        .map_err(|e| format!("Failed to serialize response to JSON: {}", e))?;

    Ok(response_json)
}

/// Elevated instance: connect to the unelevated pipe server and run received commands.
#[cfg(windows)]
pub fn run_elevated_connector() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    ipc_debug!("[PIPE_CLIENT] Starting elevated pipe client (connector)...");
    ipc_debug!("[PIPE_CLIENT] Pipe name: {}", PIPE_NAME);

    if let Err(e) = rt.block_on(run_elevated_connector_async(None)) {
        eprintln!("[PIPE_CLIENT ERROR] Elevated connector failed: {}", e);
        std::process::exit(1);
    }

    ipc_debug!("[PIPE_CLIENT] Elevated connector exited normally");
}

/// Connect to the unelevated pipe server and execute commands until the connection
/// closes or `shutdown_notify` fires (when provided).
#[cfg(windows)]
pub async fn run_elevated_connector_async(
    shutdown_notify: Option<std::sync::Arc<tokio::sync::Notify>>,
) -> Result<(), String> {
    use super::dispatch_command;
    use crate::types::CommandResponse;

    ipc_debug!("[PIPE_CLIENT] Creating encrypted client");
    let mut client = NamedPipeClientStruct::new_encrypted(PIPE_NAME, None);
    client.enforce_same_path_server(true);

    ipc_debug!("[PIPE_CLIENT] Attempting to connect to unelevated pipe server...");

    // Retry only the connection handshake, never an EFI command.
    let connect = with_deadline(
        std::time::Duration::from_secs(crate::constants::SERVICE_START_TIMEOUT),
        async {
            loop {
                if client.connect().await.is_ok() {
                    return Ok(());
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        },
    );
    if let Some(ref notify) = shutdown_notify {
        tokio::select! {
            result = connect => result?,
            _ = notify.notified() => return Ok(()),
        }
    } else {
        connect.await?;
    }

    ipc_debug!("[PIPE_CLIENT] Connected successfully to unelevated pipe server");
    ipc_debug!("[PIPE_CLIENT] Entering command receive loop...");

    loop {
        ipc_debug!("[PIPE_CLIENT] Waiting for command from server...");
        let command_bytes = if let Some(ref notify) = shutdown_notify {
            tokio::select! {
                result = client.receive_bytes() => {
                    match result {
                        Ok(bytes) => {
                            ipc_debug!("[PIPE_CLIENT] Received {} bytes from server", bytes.len());
                            bytes
                        },
                        Err(e) => {
                            // Normal when the user instance exits.
                            ipc_debug!("[PIPE_CLIENT] Connection closed: {}", e);
                            break;
                        }
                    }
                }
                _ = notify.notified() => {
                    ipc_debug!("[PIPE_CLIENT] Shutdown signal received, stopping elevated connector...");
                    break;
                }
            }
        } else {
            match client.receive_bytes().await {
                Ok(bytes) => {
                    ipc_debug!("[PIPE_CLIENT] Received {} bytes from server", bytes.len());
                    bytes
                }
                Err(e) => {
                    // Normal when the user instance exits.
                    ipc_debug!("[PIPE_CLIENT] Connection closed: {}", e);
                    break;
                }
            }
        };

        ipc_debug!("[PIPE_CLIENT] Deserializing command...");
        let command = match bincode::deserialize(&command_bytes) {
            Ok(cmd) => {
                ipc_debug!("[PIPE_CLIENT] Command deserialized successfully");
                cmd
            }
            Err(e) => {
                eprintln!("[PIPE_CLIENT ERROR] Failed to deserialize command: {}", e);
                let error_response = CommandResponse {
                    code: 1,
                    message: format!("Deserialization error: {}", e),
                };
                if let Ok(error_bytes) = bincode::serialize(&error_response) {
                    let _ = client.send_bytes(&error_bytes).await;
                }
                continue;
            }
        };

        ipc_debug!("[PIPE_CLIENT] Executing command with elevated privileges...");

        let response = dispatch_command(command);

        ipc_debug!("[PIPE_CLIENT] Command executed, code: {}", response.code);
        ipc_debug!("[PIPE_CLIENT] Serializing response...");
        let response_bytes = match bincode::serialize(&response) {
            Ok(bytes) => {
                ipc_debug!("[PIPE_CLIENT] Response serialized: {} bytes", bytes.len());
                bytes
            }
            Err(e) => {
                eprintln!("[PIPE_CLIENT ERROR] Failed to serialize response: {}", e);
                let error_response = CommandResponse {
                    code: 1,
                    message: format!("Serialization error: {}", e),
                };
                bincode::serialize(&error_response).unwrap_or_default()
            }
        };

        ipc_debug!("[PIPE_CLIENT] Sending response back to server...");
        if let Err(e) = client.send_bytes(&response_bytes).await {
            eprintln!("[PIPE_CLIENT ERROR] Failed to send response: {}", e);
            break;
        }

        ipc_debug!("[PIPE_CLIENT] Response sent successfully, ready for next command");
    }

    client.disconnect();
    ipc_debug!("[PIPE_CLIENT] Elevated connector disconnected");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadlines_bound_pending_operations_and_keep_successes() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let short = std::time::Duration::from_millis(10);
            let result: Result<(), String> = with_deadline(short, std::future::pending()).await;
            assert!(result.unwrap_err().contains("timed out"));
            assert_eq!(with_deadline(short, async { Ok(42) }).await.unwrap(), 42);
        });
    }

    #[test]
    fn server_task_is_aborted_on_early_exit() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let task = tokio::spawn(std::future::pending::<()>());
            {
                let _guard = AbortOnDrop(task.abort_handle());
            }
            assert!(task.await.unwrap_err().is_cancelled());
        });
    }
}
