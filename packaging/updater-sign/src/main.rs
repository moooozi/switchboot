use base64::{Engine, engine::general_purpose::STANDARD};
use minisign::{PublicKey, SecretKey, SecretKeyBox};
use std::{error::Error, fs, io::Cursor, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn decode_secret(encoded: &str, password: String) -> Result<SecretKey> {
    let text = String::from_utf8(STANDARD.decode(encoded.trim())?)?;
    Ok(SecretKeyBox::from_string(&text)?.into_secret_key(Some(password))?)
}

fn encode_signature(secret: &SecretKey, data: &[u8]) -> Result<String> {
    let public = PublicKey::from_secret_key(secret)?;
    let signature = minisign::sign(Some(&public), secret, Cursor::new(data), None, None)?;
    minisign::verify(&public, &signature, Cursor::new(data), true, false, false)?;
    Ok(STANDARD.encode(signature.to_string()))
}

fn run() -> Result<()> {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    if paths.is_empty() {
        return Err("Usage: switchboot-updater-sign <artifact> [...]".into());
    }
    let encoded = std::env::var("TAURI_SIGNING_PRIVATE_KEY")?;
    let password = std::env::var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD").unwrap_or_default();
    let secret = decode_secret(&encoded, password)?;
    for path in paths {
        let bytes = fs::read(Path::new(&path))?;
        if bytes.is_empty() {
            return Err("Cannot sign an empty artifact".into());
        }
        let signature = encode_signature(&secret, &bytes)?;
        let mut output = path;
        output.push(".sig");
        fs::write(output, signature)?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Updater signing failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minisign::{KeyPair, SignatureBox};

    #[test]
    fn encrypted_legacy_keys_and_base64_signatures_remain_compatible() {
        let pair = KeyPair::generate_encrypted_keypair(Some("fixture".into())).unwrap();
        let encoded = STANDARD.encode(pair.sk.to_box(None).unwrap().to_string());
        let secret = decode_secret(&encoded, "fixture".into()).unwrap();
        let encoded_sig = encode_signature(&secret, b"installer bytes").unwrap();
        let text = String::from_utf8(STANDARD.decode(encoded_sig).unwrap()).unwrap();
        let signature = SignatureBox::from_string(&text).unwrap();
        minisign::verify(
            &pair.pk,
            &signature,
            Cursor::new(b"installer bytes"),
            true,
            false,
            false,
        )
        .unwrap();
        assert!(
            minisign::verify(
                &pair.pk,
                &signature,
                Cursor::new(b"tampered bytes"),
                true,
                false,
                false
            )
            .is_err()
        );
        assert!(decode_secret(&encoded, "incorrect".into()).is_err());
    }

    #[test]
    fn invalid_key_is_rejected() {
        assert!(decode_secret("invalid", String::new()).is_err());
    }
}
