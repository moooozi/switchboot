use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};

include!("psk.rs");

pub fn encrypt_message(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let key = Key::from_slice(&PSK);
    let cipher = ChaCha20Poly1305::new(key);
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let mut ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {:?}", e))?;
    let mut out = nonce_bytes.to_vec();
    out.append(&mut ciphertext);
    Ok(out)
}

pub fn decrypt_message(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if ciphertext.len() < 12 {
        return Err("Ciphertext too short".to_string());
    }
    let (nonce_bytes, ct) = ciphertext.split_at(12);
    let key = Key::from_slice(&PSK);
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ct)
        .map_err(|e| format!("Decryption failed: {:?}", e))
}

pub trait MessageCrypto {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, String>;
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String>;
}

pub struct ChaChaCrypto;

impl MessageCrypto for ChaChaCrypto {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        encrypt_message(plaintext)
    }
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        decrypt_message(ciphertext)
    }
}

pub struct NoCrypto;

impl MessageCrypto for NoCrypto {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        Ok(plaintext.to_vec())
    }
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        Ok(ciphertext.to_vec())
    }
}
