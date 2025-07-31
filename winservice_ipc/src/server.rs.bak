use std::ffi::{OsString};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;
use crate::ipc_server::IPC;
mod windpapi;
use crate::winservice;

use winservice::{run_service};

// Add bincode for binary serialization
use bincode;
use serde::{Deserialize, Serialize};

pub fn my_service_main(service_name: &str, pipe_name: &str, arguments: Vec<OsString>) {
    println!("Service main started with arguments: {:?}", arguments);
    let pipe_name_owned = pipe_name.to_owned();
    if let Err(e) = run_service(service_name, move |ctx| {
        let ipc = Arc::new(IPC::new(&pipe_name_owned));
        ipc.set_non_blocking();
        pipe_server(ctx.stop_flag, ipc);
    }) {
        println!("Error running service: {:?}", e);
    }
}

pub fn spawn_server_thread(pipe_name: &str) {
    // function spawn a new thread to run the pipe server
    // AtomicBool is used to communicate between the main thread and the server thread
    // Interrupting the program will set the should_stop flag to true
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = Arc::clone(&should_stop);
    // The IPC struct is used to communicate with the client
    let ipc = Arc::new(IPC::new(pipe_name));
    ipc.set_non_blocking();
    let ipc_clone = Arc::clone(&ipc);
    // Spawn a new thread to run the pipe server
    std::thread::spawn(move || pipe_server(should_stop_clone, ipc_clone));
    // Stopping the program gracefully will set the should_stop flag to true

    // Handle signals to set the should_stop flag to true
    flag::register(SIGTERM, Arc::clone(&should_stop)).expect("Error setting SIGTERM handler");
    flag::register(SIGINT, Arc::clone(&should_stop)).expect("Error setting SIGINT handler");

    // Wait for the server to stop
    while !Arc::clone(&should_stop).load(Ordering::SeqCst) {
        sleep(Duration::from_millis(100));
    }
    println!("Server stopped.");
}

pub fn pipe_server(should_stop: Arc<AtomicBool>, ipc: Arc<IPC>) {
    let timeout_duration = Duration::from_secs(10);
    let mut last_client_connect_attempt = Instant::now();
    println!("Pipe server started.");

    loop {
        if should_stop.load(Ordering::SeqCst) {
            println!("Stopping server as should_stop is set to true.");
            break;
        }

        // Check if the timeout duration has passed
        if last_client_connect_attempt.elapsed() >= timeout_duration {
            println!("No client connected for 10 seconds. Stopping server.");
            should_stop.store(true, Ordering::SeqCst);
            break;
        }

        // Wait for a client is now non-blocking
        if !ipc.wait_for_client() {
            continue;
        }

        // Reset the timer as a client has connected
        last_client_connect_attempt = Instant::now();

        let mut buffer = vec![0u8; 1024];
        if ipc.receive_message(&mut buffer) {
            handle_client_request(&ipc, &buffer);
        }
        sleep(Duration::from_millis(20));
    }
}
/// NEW MESSAGING SYSTEM

/// NEW MESSAGEING SYSTEM

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientRequest {
    pub id: String,
    pub command: ServiceCommand,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerResponse {
    pub id: String,
    pub status: String,
    pub result: Option<Vec<u8>>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServiceCommand {
    Connect(String),
    Disconnect(String),
    Encrypt(Vec<u8>),
    Decrypt(Vec<u8>),
    EncryptFile(Vec<u8>),
    Show,
    Exit,
}

/// OLD MESSAGE SYSTEM

#[derive(Deserialize, Debug)]
struct ClientRequest {
    id: String,
    command: String,
    args: Option<Vec<u8>>,
}

#[derive(Serialize, Debug)]
struct ServerResponse {
    id: String,
    status: String,
    result: Option<Vec<u8>>,
    error: Option<String>,
}


enum ServiceCommand {
    Connect(String),
    Disconnect(String),
    Encrypt(Vec<u8>),
    Decrypt(Vec<u8>),
    EncryptFile(Vec<u8>),
    Show,
    Exit,
    Invalid(String),
}


fn handle_client_request(ipc: &IPC, request: &[u8]) {
    if request.len() < 9 {
        println!("Invalid request format.");
        return;
    }

    let unique_id = &request[0..8];
    let command_bytes = &request[9..];
    let command = parse_service_command(command_bytes);

    println!("Parsed command...");
    let response = match command {
        ServiceCommand::Exit => {
            println!("Received exit command.");
            Vec::new()
        }
        _ => execute_service_command(&command),
    };

    let mut message = Vec::new();
    message.extend_from_slice(b"response:");
    message.extend_from_slice(unique_id);
    message.extend_from_slice(b":");
    message.extend_from_slice(&response);
    ipc.send_message(&message);
}

enum ServiceCommand {
    Connect(String),
    Disconnect(String),
    Encrypt(Vec<u8>),
    Decrypt(Vec<u8>),
    EncryptFile(Vec<u8>),
    Show,
    Exit,
    Invalid(String),
}

fn parse_service_command(bytes: &[u8]) -> ServiceCommand {
    if let Some(pos) = bytes.iter().position(|&b| b == b':') {
        let (command, args) = bytes.split_at(pos);
        let args = &args[1..]; // Skip the colon
        match command {
            b"connect" => ServiceCommand::Connect(String::from_utf8_lossy(args).to_string()),
            b"disconnect" => ServiceCommand::Disconnect(String::from_utf8_lossy(args).to_string()),
            b"encrypt" => ServiceCommand::Encrypt(args.to_vec()),
            b"encrypt_file" => ServiceCommand::EncryptFile(args.to_vec()),
            b"decrypt" => ServiceCommand::Decrypt(args.to_vec()),
            b"show" => ServiceCommand::Show,
            b"exit" => ServiceCommand::Exit,
            _ => ServiceCommand::Invalid(String::from_utf8_lossy(command).to_string()),
        }
    } else {
        ServiceCommand::Invalid(String::from_utf8_lossy(bytes).to_string())
    }
}

fn execute_service_command(command: &ServiceCommand) -> Vec<u8> {
    match command {
        ServiceCommand::Connect(arg) => server_connect_wireguard(arg),
        ServiceCommand::Disconnect(arg) => server_disconnect_wireguard(arg),
        ServiceCommand::Encrypt(data) => encrypt_data(data),
        ServiceCommand::Decrypt(data) => decrypt_data(data),
        ServiceCommand::EncryptFile(data) => encrypt_file(data),
        ServiceCommand::Show => server_show_wireguard(),
        ServiceCommand::Invalid(prompt) => {
            println!("Invalid command: {}", prompt);
            Vec::new()
        }
        ServiceCommand::Exit => Vec::new(),
    }
}

fn encrypt_data(data: &[u8]) -> Vec<u8> {
    println!("Encrypting data.");
    match windpapi::win32_crypt_protect_data(data) {
        Ok(encrypted_data) => encrypted_data,
        Err(_) => b"FAILD%%".to_vec(),
    }
}

fn decrypt_data(data: &[u8]) -> Vec<u8> {
    println!("Decrypting data.");
    match windpapi::win32_crypt_unprotect_data(data) {
        Ok(decrypted_data) => decrypted_data,
        Err(_) => b"FAILD%%".to_vec(),
    }
}

fn encrypt_file(input: &[u8]) -> Vec<u8> {
    println!("Encrypting file.");
    let input_str = match std::str::from_utf8(input) {
        Ok(s) => s.trim_matches(char::from(0)),
        Err(_) => return b"FAILD%%".to_vec(),
    };

    let parts: Vec<&str> = input_str.split("%%").collect();
    if parts.len() != 2 {
        return b"FAILD%%".to_vec();
    }

    let src_file_path = parts[0].trim_matches(char::from(0));
    let dest_file_path = parts[1].trim_matches(char::from(0));

    println!("Encrypting file: {} to {}", src_file_path, dest_file_path);

    // Read the contents of the source file
    let data = match fs::read(src_file_path) {
        Ok(d) => d,
        Err(e) => {
            println!("Failed to read file: {}. Error: {}", src_file_path, e);
            return b"FAILD%%".to_vec();
        }
    };

    // Encrypt the data
    let encrypted_data = match windpapi::win32_crypt_protect_data(&data) {
        Ok(ed) => ed,
        Err(e) => {
            println!("Failed to encrypt data. Error: {}", e);
            return b"FAILD%%".to_vec();
        }
    };

    // Write the encrypted data to the destination file
    if let Err(e) = fs::write(dest_file_path, &encrypted_data) {
        println!("Failed to write to file: {}. Error: {}", dest_file_path, e);
        return b"FAILD%%".to_vec();
    }

    b"SUCCESS%%".to_vec()
}

fn server_connect_wireguard(path: &str) -> Vec<u8> {
    println!("Connecting to WireGuard with path: {}", path);
    let path = path.trim_matches(char::from(0)); // Trim null bytes
    let output = std::process::Command::new("wireguard")
        .arg("/installtunnelservice")
        .arg(path)
        .output()
        .expect("Failed to execute command");
    output.stdout
}

fn server_disconnect_wireguard(connection_name: &str) -> Vec<u8> {
    println!(
        "Disconnecting from WireGuard connection: {}",
        connection_name
    );
    let connection_name = connection_name.trim_matches(char::from(0)); // Trim null bytes

    let output = std::process::Command::new("wireguard")
        .arg("/uninstalltunnelservice")
        .arg(connection_name)
        .output()
        .expect("Failed to execute command");
    output.stdout
}

fn server_show_wireguard() -> Vec<u8> {
    println!("Showing WireGuard status.");
    let output = std::process::Command::new("wg")
        .arg("show")
        .output()
        .expect("Failed to execute command");
    output.stdout
}
