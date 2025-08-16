use crate::error::{NamedPipeError, Result};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Key, Nonce,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

// Include the compile-time generated default key
include!(concat!(env!("OUT_DIR"), "/default_key.rs"));

/// A named pipe client for Windows
pub struct NamedPipeClientStruct {
    client: Option<NamedPipeClient>,
    pipe_name: String,
    cipher: Option<ChaCha20Poly1305>,
}

impl NamedPipeClientStruct {
    /// Create a new named pipe client
    pub fn new(pipe_name: &str) -> Self {
        Self {
            client: None,
            pipe_name: Self::format_pipe_name(pipe_name),
            cipher: None,
        }
    }

    /// Create a new named pipe client with encryption.
    /// If key is None, uses a secure compile-time generated default key.
    /// If key is Some(key), uses the provided custom key.
    pub fn new_encrypted(pipe_name: &str, key: Option<&[u8; 32]>) -> Self {
        let key_to_use = key.unwrap_or(&DEFAULT_ENCRYPTION_KEY);
        let key = Key::from_slice(key_to_use);
        let cipher = ChaCha20Poly1305::new(key);

        Self {
            client: None,
            pipe_name: Self::format_pipe_name(pipe_name),
            cipher: Some(cipher),
        }
    }
    /// Connect to the named pipe server
    pub async fn connect(&mut self) -> Result<()> {
        let client = ClientOptions::new()
            .open(&self.pipe_name)
            .map_err(NamedPipeError::Io)?;

        self.client = Some(client);
        Ok(())
    }

    /// Send raw bytes to the server
    pub async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        let client = self.client.as_mut().ok_or(NamedPipeError::NotConnected)?;

        if let Some(ref cipher) = self.cipher {
            // Generate a random nonce
            let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

            // Encrypt the data
            let ciphertext = cipher.encrypt(&nonce, data).map_err(|e| {
                NamedPipeError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Encryption failed: {}", e),
                ))
            })?;

            // Prepare encrypted message: nonce (12 bytes) + ciphertext
            let mut encrypted_message = Vec::with_capacity(12 + ciphertext.len());
            encrypted_message.extend_from_slice(&nonce);
            encrypted_message.extend_from_slice(&ciphertext);

            // Send length-prefixed encrypted message
            let len = encrypted_message.len() as u32;
            client.write_all(&len.to_le_bytes()).await?;
            client.write_all(&encrypted_message).await?;
        } else {
            // Send unencrypted data with length prefix
            let len = data.len() as u32;
            client.write_all(&len.to_le_bytes()).await?;
            client.write_all(data).await?;
        }

        client.flush().await?;
        Ok(())
    }

    /// Receive raw bytes from the server
    pub async fn receive_bytes(&mut self) -> Result<Vec<u8>> {
        let client = self.client.as_mut().ok_or(NamedPipeError::NotConnected)?;

        // Read length first
        let mut len_bytes = [0u8; 4];
        client.read_exact(&mut len_bytes).await?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read data
        let mut buffer = vec![0u8; len];
        client.read_exact(&mut buffer).await?;

        if let Some(ref cipher) = self.cipher {
            // For encrypted data: first 12 bytes are nonce, rest is ciphertext
            if buffer.len() < 12 {
                return Err(NamedPipeError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Encrypted message too short",
                )));
            }

            let (nonce_bytes, ciphertext) = buffer.split_at(12);
            let nonce = Nonce::from_slice(nonce_bytes);

            // Decrypt the data
            let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
                NamedPipeError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Decryption failed: {}", e),
                ))
            })?;

            Ok(plaintext)
        } else {
            Ok(buffer)
        }
    }

    /// Check if the client is connected
    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    /// Disconnect from the server
    pub fn disconnect(&mut self) {
        self.client = None;
    }

    /// Format pipe name to Windows named pipe format
    fn format_pipe_name(name: &str) -> String {
        if name.starts_with("\\\\.\\pipe\\") {
            name.to_string()
        } else {
            format!("\\\\.\\pipe\\{}", name)
        }
    }

    /// Get the pipe name
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }
}

impl Drop for NamedPipeClientStruct {
    fn drop(&mut self) {
        self.disconnect();
    }
}
