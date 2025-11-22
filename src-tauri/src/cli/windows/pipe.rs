use crate::build_info;
use pipeguard::{NamedPipeClientStruct, NamedPipeServerStruct};

pub const PIPE_NAME: &str = build_info::APP_IDENTIFIER_VERSION;

/// User instance creates the pipe server and sends a single command to the elevated instance.
/// This function is synchronous and blocks until the command is executed and response is received.
#[cfg(windows)]
pub fn run_unelevated_pipe_server(timeout: Option<u64>, _wait_for_new_client: bool) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    eprintln!("[PIPE_SERVER] Starting unelevated pipe server...");
    eprintln!("[PIPE_SERVER] Pipe name: {}", PIPE_NAME);

    if let Err(e) = rt.block_on(run_unelevated_pipe_server_async(timeout)) {
        eprintln!("[PIPE_SERVER ERROR] Pipe server failed: {}", e);
        std::process::exit(1);
    }

    eprintln!("[PIPE_SERVER] Pipe server exited normally");
}

/// Asynchronous implementation of the unelevated pipe server.
/// This server reads JSON commands from stdin, forwards them to the elevated client,
/// receives responses, and outputs them to stdout.
#[cfg(windows)]
async fn run_unelevated_pipe_server_async(_timeout: Option<u64>) -> Result<(), String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::sync::mpsc;

    // Create a channel to communicate with the connection handler
    let (connection_tx, mut connection_rx) = mpsc::channel::<pipeguard::NamedPipeConnection>(1);

    // Create encrypted server
    let mut server = NamedPipeServerStruct::new_encrypted(PIPE_NAME, None);
    server.enforce_same_path_client(true);

    eprintln!("[PIPE_SERVER] Pipe server created, waiting for elevated client to connect...");

    // Spawn the server task
    let server_handle = tokio::spawn(async move {
        server
            .start(move |connection| {
                let connection_tx = connection_tx.clone();
                async move {
                    eprintln!(
                        "[PIPE_SERVER] Elevated client connected with ID: {}",
                        connection.id()
                    );

                    // Send the connection to the main loop
                    if connection_tx.send(connection).await.is_err() {
                        eprintln!("[PIPE_SERVER ERROR] Failed to send connection to main loop");
                        return Err(pipeguard::NamedPipeError::Io(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "Channel closed",
                        )));
                    }

                    // Keep this handler alive - it will be dropped when the connection is closed
                    tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
                    Ok(())
                }
            })
            .await
    });

    // Wait for the elevated client to connect
    eprintln!("[PIPE_SERVER] Waiting for elevated client connection...");
    let mut connection = match connection_rx.recv().await {
        Some(conn) => {
            eprintln!("[PIPE_SERVER] Elevated client connected successfully");
            conn
        }
        None => {
            eprintln!("[PIPE_SERVER ERROR] Server closed without accepting connection");
            return Err("Server closed without accepting connection".to_string());
        }
    };

    eprintln!("[PIPE_SERVER] Starting command processing loop");

    // Read commands from stdin and forward them to the elevated client
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line_buffer = String::new();

    loop {
        line_buffer.clear();

        // Read command from stdin
        match reader.read_line(&mut line_buffer).await {
            Ok(0) => {
                eprintln!("[PIPE_SERVER] EOF on stdin, disconnecting...");
                break; // EOF
            }
            Ok(n) => {
                eprintln!("[PIPE_SERVER] Read {} bytes from stdin", n);
                let line = line_buffer.trim();
                if line.is_empty() {
                    eprintln!("[PIPE_SERVER] Empty line, skipping");
                    continue;
                }

                eprintln!("[PIPE_SERVER] Processing command: {}", line);
                // Parse and send command to elevated client
                match send_command_and_get_response(&mut connection, line).await {
                    Ok(response) => {
                        eprintln!("[PIPE_SERVER] Received response, outputting to stdout");
                        // Output response to stdout
                        println!("{}", response);
                    }
                    Err(e) => {
                        eprintln!("[PIPE_SERVER ERROR] Failed to process command: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("[PIPE_SERVER ERROR] Failed to read input: {}", e);
                break;
            }
        }
    }

    eprintln!("[PIPE_SERVER] Disconnecting and cleaning up");
    drop(connection);
    server_handle.abort();

    eprintln!("[PIPE_SERVER] Pipe server stopped");
    Ok(())
}

/// Send a command to the elevated client and wait for response
async fn send_command_and_get_response(
    connection: &mut pipeguard::NamedPipeConnection,
    line: &str,
) -> Result<String, String> {
    use super::CliCommand;
    use crate::types::CommandResponse;

    eprintln!("[PIPE_SERVER] Parsing JSON args from: {}", line);
    // Parse JSON args
    let args: Vec<String> =
        serde_json::from_str(line).map_err(|e| format!("Invalid JSON input: {}", e))?;

    eprintln!("[PIPE_SERVER] Creating command from args: {:?}", args);
    // Create command
    let command = CliCommand::from_args(&args).map_err(|e| format!("Invalid command: {}", e))?;

    eprintln!("[PIPE_SERVER] Serializing command");
    // Serialize command
    let command_bytes =
        bincode::serialize(&command).map_err(|e| format!("Serialization error: {}", e))?;

    eprintln!(
        "[PIPE_SERVER] Sending {} bytes to elevated client",
        command_bytes.len()
    );
    // Send encrypted command to elevated client
    connection
        .send_bytes(&command_bytes)
        .await
        .map_err(|e| format!("Failed to send command: {}", e))?;

    eprintln!("[PIPE_SERVER] Waiting for response from elevated client...");
    // Receive encrypted response from elevated client
    let response_bytes = connection
        .receive_bytes()
        .await
        .map_err(|e| format!("Failed to receive response: {}", e))?;

    eprintln!(
        "[PIPE_SERVER] Received {} bytes response",
        response_bytes.len()
    );
    // Deserialize response
    let response: CommandResponse = bincode::deserialize(&response_bytes)
        .map_err(|e| format!("Failed to deserialize response: {}", e))?;

    eprintln!("[PIPE_SERVER] Response deserialized successfully");
    // Convert response to JSON string
    let response_json = serde_json::to_string(&response)
        .map_err(|e| format!("Failed to serialize response to JSON: {}", e))?;

    Ok(response_json)
}

/// Elevated instance connects to the unelevated pipe server and executes commands.
/// This is the client that waits for commands from the server (unelevated instance).
#[cfg(windows)]
pub fn run_elevated_connector() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    eprintln!("[PIPE_CLIENT] Starting elevated pipe client (connector)...");
    eprintln!("[PIPE_CLIENT] Pipe name: {}", PIPE_NAME);

    if let Err(e) = rt.block_on(run_elevated_connector_async(None)) {
        eprintln!("[PIPE_CLIENT ERROR] Elevated connector failed: {}", e);
        std::process::exit(1);
    }

    eprintln!("[PIPE_CLIENT] Elevated connector exited normally");
}

/// Asynchronous implementation of the elevated connector.
/// Connects to the unelevated pipe server and waits for commands to execute.
///
/// # Arguments
/// * `shutdown_notify` - Optional shutdown notification. If provided, the connector will
///   gracefully shutdown when notified. If None, it will run until the connection is closed.
#[cfg(windows)]
pub async fn run_elevated_connector_async(
    shutdown_notify: Option<std::sync::Arc<tokio::sync::Notify>>,
) -> Result<(), String> {
    use super::dispatch_command;
    use crate::types::CommandResponse;

    eprintln!("[PIPE_CLIENT] Creating encrypted client");
    // Create encrypted client
    let mut client = NamedPipeClientStruct::new_encrypted(PIPE_NAME, None);
    client.enforce_same_path_server(true);

    eprintln!("[PIPE_CLIENT] Attempting to connect to unelevated pipe server...");

    // Connect to the pipe server with retries (the server might not be ready immediately)
    let max_retries = 10;
    let mut retry_count = 0;
    loop {
        match client.connect().await {
            Ok(_) => {
                break;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    eprintln!("[PIPE_CLIENT ERROR] Connection failed: {}", e);
                    return Err(format!("Failed to connect to pipe server: {}", e));
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }

    eprintln!("[PIPE_CLIENT] Connected successfully to unelevated pipe server");
    eprintln!("[PIPE_CLIENT] Entering command receive loop...");

    // Loop: receive commands, execute them, send responses back
    loop {
        eprintln!("[PIPE_CLIENT] Waiting for command from server...");
        // Receive command from unelevated server or wait for shutdown signal
        let command_bytes = if let Some(ref notify) = shutdown_notify {
            tokio::select! {
                result = client.receive_bytes() => {
                    match result {
                        Ok(bytes) => {
                            eprintln!("[PIPE_CLIENT] Received {} bytes from server", bytes.len());
                            bytes
                        },
                        Err(e) => {
                            // Connection closed or error - this is normal when the user instance exits
                            eprintln!("[PIPE_CLIENT] Connection closed: {}", e);
                            break;
                        }
                    }
                }
                _ = notify.notified() => {
                    eprintln!("[PIPE_CLIENT] Shutdown signal received, stopping elevated connector...");
                    break;
                }
            }
        } else {
            match client.receive_bytes().await {
                Ok(bytes) => {
                    eprintln!("[PIPE_CLIENT] Received {} bytes from server", bytes.len());
                    bytes
                }
                Err(e) => {
                    // Connection closed or error - this is normal when the user instance exits
                    eprintln!("[PIPE_CLIENT] Connection closed: {}", e);
                    break;
                }
            }
        };

        eprintln!("[PIPE_CLIENT] Deserializing command...");
        // Deserialize command
        let command = match bincode::deserialize(&command_bytes) {
            Ok(cmd) => {
                eprintln!("[PIPE_CLIENT] Command deserialized successfully");
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

        eprintln!("[PIPE_CLIENT] Executing command with elevated privileges...");

        // Execute command with elevated privileges
        let response = dispatch_command(command);

        eprintln!("[PIPE_CLIENT] Command executed, code: {}", response.code);
        eprintln!("[PIPE_CLIENT] Serializing response...");
        // Serialize response
        let response_bytes = match bincode::serialize(&response) {
            Ok(bytes) => {
                eprintln!("[PIPE_CLIENT] Response serialized: {} bytes", bytes.len());
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

        eprintln!("[PIPE_CLIENT] Sending response back to server...");
        // Send response back to unelevated server
        if let Err(e) = client.send_bytes(&response_bytes).await {
            eprintln!("[PIPE_CLIENT ERROR] Failed to send response: {}", e);
            break;
        }

        eprintln!("[PIPE_CLIENT] Response sent successfully, ready for next command");
    }

    client.disconnect();
    eprintln!("[PIPE_CLIENT] Elevated connector disconnected");
    Ok(())
}
