use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::Duration;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use chacha20poly1305::aead::{Aead, OsRng};
use chacha20poly1305::aead::rand_core::RngCore;

mod ipc_client;
mod ipc_messaging;
mod ipc_server;

use crate::ipc_client::IPCClient;
use crate::ipc_messaging::{pipe_server, ClientRequest, ServerResponse};
use crate::ipc_server::IPC;

const PSK: &[u8; 32] = b"0123456789abcdef0123456789abcdef";

fn encrypt_message(plaintext: &[u8]) -> Vec<u8> {
    let key = Key::from_slice(PSK);
    let cipher = ChaCha20Poly1305::new(key);
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut ciphertext = cipher.encrypt(nonce, plaintext).expect("encryption failed");
    let mut out = nonce_bytes.to_vec();
    out.append(&mut ciphertext);
    out
}

fn decrypt_message(ciphertext: &[u8]) -> Vec<u8> {
    if ciphertext.len() < 12 {
        panic!("Ciphertext too short");
    }
    let (nonce_bytes, ct) = ciphertext.split_at(12);
    let key = Key::from_slice(PSK);
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ct).expect("decryption failed")
}

fn main() {
    let pipe_name = r"\\.\pipe\test_pipe";
    let should_stop = Arc::new(AtomicBool::new(false));
    let ipc = Arc::new(IPC::new(pipe_name));

    // Start server in a separate thread
    let server_stop = should_stop.clone();
    let server_ipc = ipc.clone();
    thread::spawn(move || {
        pipe_server(server_stop, server_ipc, |ipc, buf| {
            // Decrypt incoming message
            let decrypted = decrypt_message(buf);
            let req: ClientRequest = bincode::deserialize(&decrypted).unwrap();
            println!("Server received: {:?}", req);

            // Respond
            let resp = ServerResponse {
                id: req.id,
                status: "ok".to_string(),
                result: Some(b"pong".to_vec()),
                error: None,
            };
            let resp_bytes = bincode::serialize(&resp).unwrap();
            let encrypted = encrypt_message(&resp_bytes);
            println!("[SERVER] Sending encrypted response ({} bytes): {:02x?}", encrypted.len(), encrypted);
            // Send only the encrypted response (the protocol already sends the length)
            ipc.send_message(&encrypted);
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
    let encrypted = encrypt_message(&req_bytes);
    // Send encrypted request
    let handle_arc = client.get_handle();
    let handle = handle_arc.lock().unwrap();
    let len = (encrypted.len() as u32).to_le_bytes();
    let mut bytes_written = 0;
    unsafe {
        use windows::Win32::Storage::FileSystem::WriteFile;
        WriteFile(
            *handle,
            len.as_ptr() as *const _,
            len.len() as u32,
            &mut bytes_written,
            std::ptr::null_mut(),
        );
        WriteFile(
            *handle,
            encrypted.as_ptr() as *const _,
            encrypted.len() as u32,
            &mut bytes_written,
            std::ptr::null_mut(),
        );
    }
    // Read response length
    let mut len_buf = [0u8; 4];
    let mut bytes_read = 0;
    unsafe {
        use windows::Win32::Storage::FileSystem::ReadFile;
        ReadFile(
            *handle,
            len_buf.as_mut_ptr() as *mut _,
            4,
            &mut bytes_read,
            std::ptr::null_mut(),
        );
    }
    let resp_len = u32::from_le_bytes(len_buf) as usize;
    let mut resp_buf = vec![0u8; resp_len];
    let mut bytes_read = 0;
    unsafe {
        use windows::Win32::Storage::FileSystem::ReadFile;
        ReadFile(
            *handle,
            resp_buf.as_mut_ptr() as *mut _,
            resp_len as u32,
            &mut bytes_read,
            std::ptr::null_mut(),
        );
    }
    resp_buf.truncate(bytes_read as usize);
    println!("[CLIENT] Received encrypted response ({} bytes): {:02x?}", resp_buf.len(), resp_buf);
    let decrypted = decrypt_message(&resp_buf);
    println!("[CLIENT] Decrypted response ({} bytes): {:02x?}", decrypted.len(), decrypted);
    let resp: ServerResponse = bincode::deserialize(&decrypted).unwrap();
    println!("Client got response: {:?}", resp);

    should_stop.store(true, Ordering::SeqCst);
}
