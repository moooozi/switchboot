use named_pipe_ipc::{NamedPipeClientStruct, NamedPipeServerStruct};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::Duration;

pub const PIPE_NAME: &str = "ca9ba1f9-4aaa-486f-8ce4-f69453af0c6c";

/// Synchronous wrapper for backwards compatibility - maintains connection but uses sync stdin
#[cfg(windows)]
pub fn run_pipe_client() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    
    // Create client once and maintain connection
    let mut client = rt.block_on(async {
        let mut client = NamedPipeClientStruct::new_encrypted(PIPE_NAME, None);
        match client.connect().await {
            Ok(_) => Ok(client),
            Err(e) => Err(format!("Failed to connect to pipe: {}", e)),
        }
    }).unwrap_or_else(|e| {
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
pub async fn run_pipe_client_async(shutdown_signal: Arc<AtomicBool>) -> Result<(), String> {
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
        // Check shutdown signal
        if shutdown_signal.load(Ordering::Relaxed) {
            println!("[INFO] Shutdown signal received, stopping client...");
            break;
        }
        
        line_buffer.clear();
        
        // Read line with timeout to allow shutdown signal checking
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
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Timeout to check shutdown signal
                continue;
            }
        }
    }
    
    client.disconnect();
    println!("[INFO] Client disconnected gracefully");
    Ok(())
}

async fn process_command_line(
    client: &mut NamedPipeClientStruct, 
    line: &str
) -> Result<(), String> {
    use super::{CliCommand, CommandResponse};
    
    // Parse JSON args
    let args: Vec<String> = serde_json::from_str(line)
        .map_err(|e| format!("Invalid JSON input: {}", e))?;
    
    // Create command
    let command = CliCommand::from_args(&args)
        .map_err(|e| format!("Invalid command: {}", e))?;
    
    // Serialize command
    let command_bytes = bincode::serialize(&command)
        .map_err(|e| format!("Serialization error: {}", e))?;
    
    // Send encrypted command (encryption is automatic!)
    client.send_bytes(&command_bytes).await
        .map_err(|e| format!("Failed to send command: {}", e))?;
    
    // Receive encrypted response (decryption is automatic!)
    let response_bytes = client.receive_bytes().await
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
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    
    if let Err(e) = rt.block_on(run_pipe_server_async(shutdown_signal, timeout, wait_for_new_client)) {
        eprintln!("[ERROR] Pipe server failed: {}", e);
        std::process::exit(1);
    }
}

/// Asynchronous pipe server implementation with readiness signaling
#[cfg(windows)]
pub async fn run_pipe_server_async(
    shutdown_signal: Arc<AtomicBool>,
    timeout: Option<u64>,
    wait_for_new_client: bool
) -> Result<(), String> {
    run_pipe_server_async_with_ready(shutdown_signal, timeout, wait_for_new_client, None).await
}

/// Asynchronous pipe server implementation with optional readiness notification
#[cfg(windows)]
pub async fn run_pipe_server_async_with_ready(
    shutdown_signal: Arc<AtomicBool>,
    timeout: Option<u64>,
    wait_for_new_client: bool,
    ready_signal: Option<Arc<AtomicBool>>
) -> Result<(), String> {
    use std::sync::atomic::AtomicUsize;
    use tokio::time::{sleep, Duration, Instant};
    
    println!("[INFO] Starting encrypted pipe server (tokio-based)...");

    // Track active connections
    let active_connections = Arc::new(AtomicUsize::new(0));
    let last_activity = Arc::new(std::sync::Mutex::new(Instant::now()));
    let first_client_served = Arc::new(AtomicBool::new(false));

    // Create encrypted server with default key (secure and automatic)
    let mut server = NamedPipeServerStruct::new_encrypted(PIPE_NAME, None);
    
    // Clone shutdown signal for the server task
    let server_shutdown = shutdown_signal.clone();
    
    // Start timeout monitoring task if timeout is specified
    if let Some(timeout_secs) = timeout {
        let timeout_shutdown = shutdown_signal.clone();
        let timeout_connections = active_connections.clone();
        let timeout_activity = last_activity.clone();
        let timeout_first_served = first_client_served.clone();
        
        tokio::spawn(async move {
            let timeout_duration = Duration::from_secs(timeout_secs);
            
            loop {
                if timeout_shutdown.load(Ordering::Relaxed) {
                    break;
                }
                
                let connection_count = timeout_connections.load(Ordering::Relaxed);
                let has_served_client = timeout_first_served.load(Ordering::Relaxed);
                
                // If wait_for_new_client is false and we've served at least one client,
                // shutdown immediately when no connections are active
                if !wait_for_new_client && has_served_client && connection_count == 0 {
                    println!("[INFO] First client served and disconnected, shutting down server (wait_for_new_client=false)...");
                    timeout_shutdown.store(true, Ordering::SeqCst);
                    break;
                }
                
                // Check timeout only if there are no active connections
                if connection_count == 0 {
                    let last_activity_time = {
                        timeout_activity.lock().unwrap().clone()
                    };
                    
                    if last_activity_time.elapsed() >= timeout_duration {
                        println!("[INFO] Timeout reached with no active connections, shutting down server...");
                        timeout_shutdown.store(true, Ordering::SeqCst);
                        break;
                    }
                }
                
                sleep(Duration::from_secs(1)).await;
            }
        });
    } else if !wait_for_new_client {
        // If no timeout is specified but wait_for_new_client is false,
        // we still need to monitor for the first client disconnect
        let no_timeout_shutdown = shutdown_signal.clone();
        let no_timeout_connections = active_connections.clone();
        let no_timeout_first_served = first_client_served.clone();
        
        tokio::spawn(async move {
            loop {
                if no_timeout_shutdown.load(Ordering::Relaxed) {
                    break;
                }
                
                let connection_count = no_timeout_connections.load(Ordering::Relaxed);
                let has_served_client = no_timeout_first_served.load(Ordering::Relaxed);
                
                // Shutdown immediately when first client disconnects
                if has_served_client && connection_count == 0 {
                    println!("[INFO] First client served and disconnected, shutting down server (wait_for_new_client=false)...");
                    no_timeout_shutdown.store(true, Ordering::SeqCst);
                    break;
                }
                
                sleep(Duration::from_millis(100)).await;
            }
        });
    }
    
    // Start server with connection handler in a spawned task
    let ready_signal_clone = ready_signal.clone();
    let server_task = tokio::spawn(async move {
        // Signal readiness when server is about to start accepting connections
        if let Some(ready) = ready_signal_clone {
            ready.store(true, Ordering::SeqCst);
            println!("[INFO] Pipe server is ready to accept connections");
        }
        
        server.start(move |mut connection| {
            let shutdown = server_shutdown.clone();
            let conn_counter = active_connections.clone();
            let activity_time = last_activity.clone();
            let client_served = first_client_served.clone();
            
            async move {
                // Increment connection count
                conn_counter.fetch_add(1, Ordering::SeqCst);
                
                // Mark that we've served at least one client
                client_served.store(true, Ordering::SeqCst);
                
                // Update activity time
                {
                    let mut last_activity = activity_time.lock().unwrap();
                    *last_activity = Instant::now();
                }
                
                println!("[SERVER] Encrypted client connected with ID: {}", connection.id());
                
                // Handle client communication loop
                while !shutdown.load(Ordering::Relaxed) {
                    tokio::select! {
                        result = connection.receive_bytes() => {
                            match result {
                                Ok(command_bytes) => {
                                    // Update activity time on successful communication
                                    {
                                        let mut last_activity = activity_time.lock().unwrap();
                                        *last_activity = Instant::now();
                                    }
                                    
                                    // Process the command (decryption is automatic!)
                                    match handle_client_command(&command_bytes).await {
                                        Ok(response_bytes) => {
                                            // Send response (encryption is automatic!)
                                            if let Err(e) = connection.send_bytes(&response_bytes).await {
                                                eprintln!("[SERVER] Failed to send response: {}", e);
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[SERVER] Failed to handle command: {}", e);
                                            // Send error response
                                            let error_response = create_error_response(&e);
                                            if let Ok(error_bytes) = bincode::serialize(&error_response) {
                                                let _ = connection.send_bytes(&error_bytes).await;
                                            }
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    if !shutdown.load(Ordering::Relaxed) {
                                        eprintln!("[SERVER] Failed to receive data: {}", e);
                                    }
                                    break;
                                }
                            }
                        }
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            // Check shutdown periodically
                            if shutdown.load(Ordering::Relaxed) {
                                println!("[SERVER] Shutdown signal received for connection {}", connection.id());
                                break;
                            }
                        }
                    }
                }
                
                // Decrement connection count when connection ends
                conn_counter.fetch_sub(1, Ordering::SeqCst);
                
                // Update activity time when connection closes
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
    loop {
        if shutdown_signal.load(Ordering::Relaxed) {
            println!("[INFO] Shutdown signal received, stopping server...");
            // Optionally stop the server gracefully
            server_task.abort();
            break;
        }
        
        // Check if server task completed
        if server_task.is_finished() {
            match server_task.await {
                Ok(result) => {
                    result.map_err(|e| format!("Server error: {}", e))?;
                }
                Err(e) => {
                    if !e.is_cancelled() {
                        return Err(format!("Server task failed: {}", e));
                    }
                }
            }
            break;
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
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
