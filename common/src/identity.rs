use crate::{FenrisError, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use std::path::Path;

pub const SERVER_IDENTITY_KEY_SIZE: usize = 32;
pub const SERVER_IDENTITY_SIGNATURE_SIZE: usize = 64;

const SERVER_IDENTITY_TRANSCRIPT_LABEL: &[u8] = b"fenris-server-identity-v1";

#[derive(Clone)]
pub struct ServerIdentityKey {
    signing_key: SigningKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerIdentityPublicKey {
    bytes: [u8; SERVER_IDENTITY_KEY_SIZE],
}

impl ServerIdentityKey {
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let bytes = key_bytes(bytes)?;
        Ok(Self {
            signing_key: SigningKey::from_bytes(&bytes),
        })
    }

    pub fn from_hex(encoded: &str) -> Result<Self> {
        let bytes = decode_hex(encoded)?;
        Self::from_slice(&bytes)
    }

    pub fn to_bytes(&self) -> [u8; SERVER_IDENTITY_KEY_SIZE] {
        self.signing_key.to_bytes()
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    pub fn public_key(&self) -> ServerIdentityPublicKey {
        ServerIdentityPublicKey {
            bytes: self.signing_key.verifying_key().to_bytes(),
        }
    }

    pub fn sign_transcript(&self, transcript: &[u8]) -> [u8; SERVER_IDENTITY_SIGNATURE_SIZE] {
        self.signing_key.sign(transcript).to_bytes()
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let encoded = std::fs::read_to_string(path).map_err(|e| {
            FenrisError::FileOperationError(format!(
                "Failed to read server identity key {}: {}",
                path.display(),
                e
            ))
        })?;

        Self::from_hex(encoded.trim())
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                FenrisError::FileOperationError(format!(
                    "Failed to create server identity key directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        std::fs::write(path, format!("{}\n", self.to_hex())).map_err(|e| {
            FenrisError::FileOperationError(format!(
                "Failed to write server identity key {}: {}",
                path.display(),
                e
            ))
        })
    }

    pub fn load_or_generate(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if path.exists() {
            return Self::load_from_file(path);
        }

        let key = Self::generate();
        key.save_to_file(path)?;
        Ok(key)
    }
}

impl ServerIdentityPublicKey {
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let bytes = key_bytes(bytes)?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| FenrisError::InvalidProtocolMessage)?;
        Ok(Self { bytes })
    }

    pub fn from_hex(encoded: &str) -> Result<Self> {
        let bytes = decode_hex(encoded)?;
        Self::from_slice(&bytes)
    }

    pub fn from_hex_or_file(input: &str) -> Result<Self> {
        let path = Path::new(input);

        if path.exists() {
            return Self::load_from_file(path);
        }

        Self::from_hex(input)
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let encoded = std::fs::read_to_string(path).map_err(|e| {
            FenrisError::FileOperationError(format!(
                "Failed to read pinned server identity {}: {}",
                path.display(),
                e
            ))
        })?;

        Self::from_hex(encoded.trim())
    }

    pub fn as_bytes(&self) -> &[u8; SERVER_IDENTITY_KEY_SIZE] {
        &self.bytes
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.bytes)
    }

    pub fn verify_transcript(&self, transcript: &[u8], signature: &[u8]) -> Result<()> {
        let signature = signature_bytes(signature)?;
        let signature = Signature::from_bytes(&signature);
        let verifying_key = VerifyingKey::from_bytes(&self.bytes)
            .map_err(|_| FenrisError::InvalidProtocolMessage)?;

        verifying_key.verify(transcript, &signature).map_err(|_| {
            FenrisError::AuthenticationError(
                "server identity signature verification failed".to_string(),
            )
        })
    }
}

pub(crate) fn server_identity_transcript(
    client_ephemeral_public_key: &[u8],
    server_ephemeral_public_key: &[u8],
    server_identity_public_key: &ServerIdentityPublicKey,
    kdf_context: &[u8],
) -> Vec<u8> {
    let mut transcript = Vec::new();
    append_transcript_part(&mut transcript, SERVER_IDENTITY_TRANSCRIPT_LABEL);
    append_transcript_part(&mut transcript, client_ephemeral_public_key);
    append_transcript_part(&mut transcript, server_ephemeral_public_key);
    append_transcript_part(&mut transcript, server_identity_public_key.as_bytes());
    append_transcript_part(&mut transcript, kdf_context);
    transcript
}

fn append_transcript_part(out: &mut Vec<u8>, part: &[u8]) {
    out.extend_from_slice(&(part.len() as u64).to_be_bytes());
    out.extend_from_slice(part);
}

fn key_bytes(bytes: &[u8]) -> Result<[u8; SERVER_IDENTITY_KEY_SIZE]> {
    bytes
        .try_into()
        .map_err(|_| FenrisError::InvalidProtocolMessage)
}

fn signature_bytes(bytes: &[u8]) -> Result<[u8; SERVER_IDENTITY_SIGNATURE_SIZE]> {
    bytes
        .try_into()
        .map_err(|_| FenrisError::InvalidProtocolMessage)
}

fn decode_hex(encoded: &str) -> Result<Vec<u8>> {
    hex::decode(encoded.trim()).map_err(|_| FenrisError::InvalidProtocolMessage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript_for(key: &ServerIdentityPublicKey) -> Vec<u8> {
        server_identity_transcript(b"client", b"server", key, b"context")
    }

    #[test]
    fn generated_identity_signs_and_verifies_transcript() {
        let key = ServerIdentityKey::generate();
        let public_key = key.public_key();
        let transcript = transcript_for(&public_key);
        let signature = key.sign_transcript(&transcript);

        public_key
            .verify_transcript(&transcript, &signature)
            .unwrap();
    }

    #[test]
    fn verification_fails_for_wrong_public_key() {
        let key = ServerIdentityKey::generate();
        let wrong_key = ServerIdentityKey::generate().public_key();
        let transcript = transcript_for(&key.public_key());
        let signature = key.sign_transcript(&transcript);

        let result = wrong_key.verify_transcript(&transcript, &signature);

        assert!(matches!(result, Err(FenrisError::AuthenticationError(_))));
    }

    #[test]
    fn verification_fails_for_wrong_transcript() {
        let key = ServerIdentityKey::generate();
        let public_key = key.public_key();
        let signature = key.sign_transcript(&transcript_for(&public_key));

        let result = public_key.verify_transcript(b"wrong transcript", &signature);

        assert!(matches!(result, Err(FenrisError::AuthenticationError(_))));
    }

    #[test]
    fn verification_fails_for_tampered_signature() {
        let key = ServerIdentityKey::generate();
        let public_key = key.public_key();
        let transcript = transcript_for(&public_key);
        let mut signature = key.sign_transcript(&transcript);
        signature[0] ^= 1;

        let result = public_key.verify_transcript(&transcript, &signature);

        assert!(matches!(result, Err(FenrisError::AuthenticationError(_))));
    }

    #[test]
    fn private_key_hex_round_trips() {
        let key = ServerIdentityKey::generate();
        let decoded = ServerIdentityKey::from_hex(&key.to_hex()).unwrap();

        assert_eq!(decoded.to_bytes(), key.to_bytes());
        assert_eq!(decoded.public_key(), key.public_key());
    }

    #[test]
    fn public_key_hex_round_trips() {
        let public_key = ServerIdentityKey::generate().public_key();
        let decoded = ServerIdentityPublicKey::from_hex(&public_key.to_hex()).unwrap();

        assert_eq!(decoded, public_key);
    }

    #[test]
    fn private_and_public_key_files_round_trip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let private_path = temp_dir.path().join("server.key");
        let public_path = temp_dir.path().join("server.pub");
        let key = ServerIdentityKey::generate();
        let public_key = key.public_key();

        key.save_to_file(&private_path).unwrap();
        std::fs::write(&public_path, public_key.to_hex()).unwrap();

        assert_eq!(
            ServerIdentityKey::load_from_file(&private_path)
                .unwrap()
                .to_bytes(),
            key.to_bytes()
        );
        assert_eq!(
            ServerIdentityPublicKey::load_from_file(&public_path).unwrap(),
            public_key
        );
    }

    #[test]
    fn load_or_generate_creates_missing_private_key_and_reuses_existing_key() {
        let temp_dir = tempfile::tempdir().unwrap();
        let private_path = temp_dir.path().join("generated.key");

        let generated = ServerIdentityKey::load_or_generate(&private_path).unwrap();
        let loaded = ServerIdentityKey::load_or_generate(&private_path).unwrap();

        assert_eq!(loaded.to_bytes(), generated.to_bytes());
    }

    #[test]
    fn malformed_key_lengths_return_invalid_protocol_message() {
        assert!(matches!(
            ServerIdentityKey::from_slice(&[0; SERVER_IDENTITY_KEY_SIZE - 1]),
            Err(FenrisError::InvalidProtocolMessage)
        ));
        assert!(matches!(
            ServerIdentityPublicKey::from_slice(&[0; SERVER_IDENTITY_KEY_SIZE - 1]),
            Err(FenrisError::InvalidProtocolMessage)
        ));
        assert!(matches!(
            ServerIdentityPublicKey::from_slice(&[0; SERVER_IDENTITY_KEY_SIZE + 1]),
            Err(FenrisError::InvalidProtocolMessage)
        ));
    }

    #[test]
    fn invalid_hex_returns_invalid_protocol_message() {
        assert!(matches!(
            ServerIdentityKey::from_hex("not hex"),
            Err(FenrisError::InvalidProtocolMessage)
        ));
        assert!(matches!(
            ServerIdentityPublicKey::from_hex("not hex"),
            Err(FenrisError::InvalidProtocolMessage)
        ));
    }
}
