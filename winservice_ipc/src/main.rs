use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

mod ipc_client;
mod ipc_messaging;
mod ipc_server;

use crate::ipc_client::IPCClient;
use crate::ipc_messaging::{pipe_server, ClientRequest, ServerResponse};
use crate::ipc_server::IPC;

fn main() {
    let pipe_name = r"\\.\pipe\test_pipe";
    let should_stop = Arc::new(AtomicBool::new(false));
    let ipc = Arc::new(IPC::new(pipe_name));

    // Start server in a separate thread
    let server_stop = should_stop.clone();
    let server_ipc = ipc.clone();
    thread::spawn(move || {
        pipe_server(server_stop, server_ipc, |ipc, buf| {
            // Deserialize request
            let req: ClientRequest = bincode::deserialize(buf).unwrap();
            println!("Server received: {:?}", req);

            // Respond
            let resp = ServerResponse {
                id: req.id,
                status: "ok".to_string(),
                result: Some(b"pong".to_vec()),
                error: None,
            };
            let resp_bytes = bincode::serialize(&resp).unwrap();
            ipc.send_message(&resp_bytes);
        });
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(500));

    // Client
    let client = IPCClient::connect(pipe_name).expect("Client failed to connect");
    let request = ClientRequest {
        id: "1".to_string(),
        payload: b"ping".to_vec(),
    };
    let req_bytes = bincode::serialize(&request).unwrap();
    let resp = client
        .send_request(req_bytes)
        .expect("Failed to send request");
    println!("Client got response: {:?}", resp);

    should_stop.store(true, Ordering::SeqCst);
}
