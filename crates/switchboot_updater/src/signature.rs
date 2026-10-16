use crate::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};

fn err(error: impl std::fmt::Display) -> Error {
    Error::Signature(error.to_string())
}

pub(crate) fn verify(bytes: &[u8], signature: &str, public_key: &str) -> Result<()> {
    let key_text = String::from_utf8(STANDARD.decode(public_key).map_err(err)?).map_err(err)?;
    let sig_text = String::from_utf8(STANDARD.decode(signature).map_err(err)?).map_err(err)?;
    PublicKey::decode(&key_text)
        .map_err(err)?
        .verify(bytes, &Signature::decode(&sig_text).map_err(err)?, false)
        .map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_signer_output_verifies_and_tampering_is_rejected() {
        use minisign::KeyPair;
        let pair = KeyPair::generate_unencrypted_keypair().unwrap();
        let bytes = b"MZ fixture installer";
        let signature = minisign::sign(
            Some(&pair.pk),
            &pair.sk,
            std::io::Cursor::new(bytes),
            None,
            None,
        )
        .unwrap();
        let key = STANDARD.encode(pair.pk.to_box().unwrap().to_string());
        let encoded = STANDARD.encode(signature.to_string());
        verify(bytes, &encoded, &key).unwrap();
        assert!(verify(b"MZ tampered installer", &encoded, &key).is_err());
        let other = KeyPair::generate_unencrypted_keypair().unwrap();
        assert!(
            verify(
                bytes,
                &encoded,
                &STANDARD.encode(other.pk.to_box().unwrap().to_string())
            )
            .is_err()
        );
    }

    #[test]
    fn legacy_public_key_is_valid_and_malformed_signatures_fail() {
        let key = String::from_utf8(STANDARD.decode(crate::PUBLIC_KEY).unwrap()).unwrap();
        PublicKey::decode(&key).unwrap();
        assert!(verify(b"MZ", "invalid", crate::PUBLIC_KEY).is_err());
    }
}
