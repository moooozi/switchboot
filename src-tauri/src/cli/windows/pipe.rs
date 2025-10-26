use crate::build_info;
use named_pipe_ipc::{NamedPipeClientStruct, NamedPipeServerStruct};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Notify;

pub const PIPE_NAME: &str = build_info::APP_IDENTIFIER_VERSION;

/// Synchronous wrapper for backwards compatibility - maintains connection but uses sync stdin
#[cfg(windows)]
pub fn run_pipe_client() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // Create client once and maintain connection
    let mut client: NamedPipeClientStruct = rt
        .block_on(async {
            let mut client = NamedPipeClientStruct::new_encrypted(PIPE_NAME, None);
            match client.connect().await {
                Ok(_) => Ok(client),
                Err(e) => Err(format!("Failed to connect to pipe: {}", e)),
            }
        })
        .unwrap_or_else(|e| {
            eprintln!("[ERROR] {}", e);
            std::process::exit(1);
        });

    // Use synchronous stdin reading (like the old client) but maintain async pipe connection
    let stdin = std::io::stdin();
    let mut line_buffer = String::new();

    loop {
        line_buffer.clear();

        // Synchronous read from stdin (no buffering issues)
        match stdin.read_line(&mut line_buffer) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let line = line_buffer.trim();
                if line.is_empty() {
                    continue;
                }

                // Process command using existing connection
                if let Err(e) = rt.block_on(process_command_line(&mut client, line)) {
                    eprintln!("[ERROR] Failed to process command: {}", e);
                    break; // Exit on pipe errors
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to read input: {}", e);
                break;
            }
        }
    }

    // Clean disconnect
    client.disconnect();
}

/// Keep the async version for advanced use cases
#[cfg(windows)]
pub async fn run_pipe_client_async(shutdown_notify: Arc<Notify>) -> Result<(), String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line_buffer = String::new();

    // Create encrypted client with default key (secure and automatic)
    let mut client = NamedPipeClientStruct::new_encrypted(PIPE_NAME, None);

    // Connect to the pipe
    match client.connect().await {
        Ok(_) => println!("[INFO] Connected to encrypted pipe server"),
        Err(e) => {
            return Err(format!("Failed to connect to pipe: {}", e));
        }
    }

    loop {
        line_buffer.clear();
        tokio::select! {
            result = reader.read_line(&mut line_buffer) => {
                match result {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let line = line_buffer.trim();
                        if line.is_empty() {
                            continue;
                        }
                        // Process the command
                        if let Err(e) = process_command_line(&mut client, line).await {
                            eprintln!("[ERROR] Failed to process command: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Failed to read input: {}", e);
                        break;
                    }
                }
            }
            _ = shutdown_notify.notified() => {
                println!("[INFO] Shutdown signal received, stopping client...");
                break;
            }
        }
    }

    client.disconnect();
    println!("[INFO] Client disconnected gracefully");
    Ok(())
}

async fn process_command_line(
    client: &mut NamedPipeClientStruct,
    line: &str,
) -> Result<(), String> {
    use super::{CliCommand, CommandResponse};

    // Parse JSON args
    let args: Vec<String> =
        serde_json::from_str(line).map_err(|e| format!("Invalid JSON input: {}", e))?;

    // Create command
    let command = CliCommand::from_args(&args).map_err(|e| format!("Invalid command: {}", e))?;

    // Serialize command
    let command_bytes =
        bincode::serialize(&command).map_err(|e| format!("Serialization error: {}", e))?;

    // Send encrypted command (encryption is automatic!)
    client
        .send_bytes(&command_bytes)
        .await
        .map_err(|e| format!("Failed to send command: {}", e))?;

    // Receive encrypted response (decryption is automatic!)
    let response_bytes = client
        .receive_bytes()
        .await
        .map_err(|e| format!("Failed to receive response: {}", e))?;

    // Deserialize response
    let response: CommandResponse = bincode::deserialize(&response_bytes)
        .map_err(|e| format!("Failed to deserialize response: {}", e))?;

    // Output response
    let response_json = serde_json::to_string(&response)
        .map_err(|e| format!("Failed to serialize response to JSON: {}", e))?;
    println!("{}", response_json);

    Ok(())
}

/// Synchronous wrapper for backwards compatibility
#[cfg(windows)]
pub fn run_pipe_server(timeout: Option<u64>, wait_for_new_client: bool) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let shutdown_notify = Arc::new(Notify::new());

    if let Err(e) = rt.block_on(run_pipe_server_async(
        shutdown_notify,
        timeout,
        wait_for_new_client,
    )) {
        eprintln!("[ERROR] Pipe server failed: {}", e);
        std::process::exit(1);
    }
}

/// Asynchronous pipe server implementation with readiness signaling
#[cfg(windows)]
pub async fn run_pipe_server_async(
    shutdown_notify: Arc<Notify>,
    timeout: Option<u64>,
    wait_for_new_client: bool,
) -> Result<(), String> {
    run_pipe_server_async_with_ready(shutdown_notify, timeout, wait_for_new_client, None).await
}

/// Asynchronous pipe server implementation with optional readiness notification
#[cfg(windows)]
pub async fn run_pipe_server_async_with_ready(
    shutdown_notify: Arc<Notify>,
    timeout: Option<u64>,
    wait_for_new_client: bool,
    ready_signal: Option<mpsc::Sender<()>>,
) -> Result<(), String> {
    use std::sync::atomic::AtomicUsize;
    use tokio::select;
    use tokio::time::{sleep, Duration, Instant};

    println!("[INFO] Starting encrypted pipe server (tokio-based)...");

    // Track active connections
    let active_connections = Arc::new(AtomicUsize::new(0));
    let last_activity = Arc::new(std::sync::Mutex::new(Instant::now()));
    let first_client_served = Arc::new(AtomicBool::new(false));

    // Create encrypted server with default key (secure and automatic)
    let mut server = NamedPipeServerStruct::new_encrypted(PIPE_NAME, None);

    // Helper for shutdown
    let shutdown_notify_server = shutdown_notify.clone();

    // Timeout logic: spawn a task that will notify shutdown_notify if needed
    if let Some(timeout_secs) = timeout {
        let shutdown_notify_timeout = shutdown_notify.clone();
        let active_connections = active_connections.clone();
        let last_activity = last_activity.clone();
        let first_client_served = first_client_served.clone();
        tokio::spawn(async move {
            let timeout_duration = Duration::from_secs(timeout_secs);
            loop {
                // Wait either for a notify (shutdown) or a short sleep interval to check state
                select! {
                    _ = shutdown_notify_timeout.notified() => {
                        break;
                    }
                    _ = sleep(Duration::from_secs(1)) => {
                        // continue to check conditions below
                    }
                }

                let connection_count = active_connections.load(Ordering::Relaxed);
                let has_served_client = first_client_served.load(Ordering::Relaxed);
                if !wait_for_new_client && has_served_client && connection_count == 0 {
                    println!("[INFO] First client served and disconnected, shutting down server (wait_for_new_client=false)...");
                    shutdown_notify_timeout.notify_waiters();
                    break;
                }
                if connection_count == 0 {
                    let last_activity_time = { last_activity.lock().unwrap().clone() };
                    if last_activity_time.elapsed() >= timeout_duration {
                        println!("[INFO] Timeout reached with no active connections, shutting down server...");
                        shutdown_notify_timeout.notify_waiters();
                        break;
                    }
                }
            }
        });
    } else if !wait_for_new_client {
        let shutdown_notify_no_timeout = shutdown_notify.clone();
        let active_connections = active_connections.clone();
        let first_client_served = first_client_served.clone();
        tokio::spawn(async move {
            loop {
                select! {
                    _ = shutdown_notify_no_timeout.notified() => break,
                    _ = sleep(Duration::from_millis(100)) => {}
                }
                let connection_count = active_connections.load(Ordering::Relaxed);
                let has_served_client = first_client_served.load(Ordering::Relaxed);
                if has_served_client && connection_count == 0 {
                    println!("[INFO] First client served and disconnected, shutting down server (wait_for_new_client=false)...");
                    shutdown_notify_no_timeout.notify_waiters();
                    break;
                }
            }
        });
    }

    // Start server with connection handler in a spawned task
    let ready_signal_clone = ready_signal.clone();
    let server_task = tokio::spawn(async move {
        if let Some(ready) = ready_signal_clone {
            let _ = ready.send(());
            println!("[INFO] Pipe server is ready to accept connections");
        }
        server.start(move |mut connection| {
            let shutdown_notify_conn = shutdown_notify_server.clone();
            let conn_counter = active_connections.clone();
            let activity_time = last_activity.clone();
            let client_served = first_client_served.clone();
            async move {
                conn_counter.fetch_add(1, Ordering::SeqCst);
                client_served.store(true, Ordering::SeqCst);
                {
                    let mut last_activity = activity_time.lock().unwrap();
                    *last_activity = Instant::now();
                }
                println!("[SERVER] Encrypted client connected with ID: {}", connection.id());
                loop {
                    select! {
                        result = connection.receive_bytes() => {
                            match result {
                                Ok(command_bytes) => {
                                    {
                                        let mut last_activity = activity_time.lock().unwrap();
                                        *last_activity = Instant::now();
                                    }
                                    match handle_client_command(&command_bytes).await {
                                        Ok(response_bytes) => {
                                            if let Err(e) = connection.send_bytes(&response_bytes).await {
                                                eprintln!("[SERVER] Failed to send response: {}", e);
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[SERVER] Failed to handle command: {}", e);
                                            let error_response = create_error_response(&e);
                                            if let Ok(error_bytes) = bincode::serialize(&error_response) {
                                                let _ = connection.send_bytes(&error_bytes).await;
                                            }
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[SERVER] Failed to receive data: {}", e);
                                    break;
                                }
                            }
                        }
                        _ = shutdown_notify_conn.notified() => {
                            println!("[SERVER] Shutdown signal received for connection {}", connection.id());
                            break;
                        }
                    }
                }
                conn_counter.fetch_sub(1, Ordering::SeqCst);
                {
                    let mut last_activity = activity_time.lock().unwrap();
                    *last_activity = Instant::now();
                }
                println!("[SERVER] Client {} disconnected", connection.id());
                Ok(())
            }
        }).await
    });

    // Wait for either the server task to complete or shutdown signal
    let server_task = server_task;
    tokio::pin!(server_task);
    select! {
        _ = shutdown_notify.notified() => {
            println!("[INFO] Shutdown signal received, stopping server...");
            server_task.abort();
        }
        result = &mut server_task => {
            match result {
                Ok(inner) => {
                    inner.map_err(|e| format!("Server error: {}", e))?;
                }
                Err(e) => {
                    if !e.is_cancelled() {
                        return Err(format!("Server task failed: {}", e));
                    }
                }
            }
        }
    }

    println!("[INFO] Pipe server stopped gracefully");
    Ok(())
}

async fn handle_client_command(command_bytes: &[u8]) -> Result<Vec<u8>, String> {
    use super::{dispatch_command, CliCommand};

    // Deserialize command
    let command: CliCommand = bincode::deserialize(command_bytes)
        .map_err(|e| format!("Failed to deserialize command: {}", e))?;

    // Execute command
    let response = dispatch_command(command);

    // Serialize response
    let response_bytes = bincode::serialize(&response)
        .map_err(|e| format!("Failed to serialize response: {}", e))?;

    Ok(response_bytes)
}

fn create_error_response(error_message: &str) -> super::CommandResponse {
    use super::CommandResponse;
    CommandResponse {
        code: 1,
        message: error_message.to_string(),
    }
}
