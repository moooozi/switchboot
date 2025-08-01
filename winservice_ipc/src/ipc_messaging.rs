use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::ipc_server::IPC;

use serde::{Deserialize, Serialize};

pub fn pipe_server<H>(should_stop: Arc<AtomicBool>, ipc: Arc<IPC>, handle_client_request: H)
where
    H: Fn(&IPC, &[u8]),
{
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

        let mut buffer = Vec::new();
        if ipc.receive_message(&mut buffer) {
            handle_client_request(&ipc, &buffer);
        }
        sleep(Duration::from_millis(20));
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientRequest {
    pub id: String,
    pub payload: Vec<u8>, // or serde_json::Value, or a String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerResponse {
    pub id: String,
    pub status: String,
    pub result: Option<Vec<u8>>, // or Option<serde_json::Value>
    pub error: Option<String>,
}
