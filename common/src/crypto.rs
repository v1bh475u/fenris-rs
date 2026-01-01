use crate::error::{FenrisError, Result};

pub const KEY_SIZE: usize = 32;

pub const IV_SIZE: usize = 12;

pub const TAG_SIZE: usize = 16;

pub const ECDH_KEY_SIZE: usize = 32;

pub trait Encryptor: Send + Sync {
    fn encrypt(&self, plaintext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>>;

    fn decrypt(&self, ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>>;

    fn generate_iv(&self) -> Vec<u8>;

    fn key_size(&self) -> usize;

    fn iv_size(&self) -> usize;
}

pub trait KeyExchanger: Send + Sync {
    fn generate_keypair(&self) -> (Vec<u8>, Vec<u8>);

    fn compute_shared_secret(&self, private_key: &[u8], peer_public_key: &[u8]) -> Result<Vec<u8>>;

    fn key_size(&self) -> usize;
}

pub trait KeyDeriver: Send + Sync {
    fn derive_key(
        &self,
        shared_secret: &[u8],
        context: &[u8],
        output_size: usize,
    ) -> Result<Vec<u8>>;
}

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Debug, Clone, Default)]
pub struct AesGcmEncryptor;

impl Encryptor for AesGcmEncryptor {
    fn encrypt(&self, plaintext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
        if key.len() != self.key_size() {
            return Err(FenrisError::InvalidKeySize {
                expected: self.key_size(),
                got: key.len(),
            });
        }

        if iv.len() != self.iv_size() {
            return Err(FenrisError::InvalidIvSize {
                expected: self.iv_size(),
                got: iv.len(),
            });
        }

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| FenrisError::EncryptionError(e.to_string()))?;

        let nonce = Nonce::from_slice(iv);

        cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| FenrisError::EncryptionError(e.to_string()))
    }

    fn decrypt(&self, ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
        if key.len() != self.key_size() {
            return Err(FenrisError::InvalidKeySize {
                expected: self.key_size(),
                got: key.len(),
            });
        }

        if iv.len() != self.iv_size() {
            return Err(FenrisError::InvalidIvSize {
                expected: self.iv_size(),
                got: iv.len(),
            });
        }

        if ciphertext.len() < TAG_SIZE {
            return Err(FenrisError::DecryptionError(
                "Ciphertext must contain at least the auth tag".to_string(),
            ));
        }

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| FenrisError::EncryptionError(e.to_string()))?;

        let nonce = Nonce::from_slice(iv);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| FenrisError::DecompressionError(e.to_string()))
    }

    fn generate_iv(&self) -> Vec<u8> {
        let mut iv = vec![0u8; self.iv_size()];
        OsRng.fill_bytes(&mut iv);
        iv
    }

    fn key_size(&self) -> usize {
        KEY_SIZE
    }

    fn iv_size(&self) -> usize {
        IV_SIZE
    }
}

#[derive(Debug, Clone, Default)]
pub struct X25519KeyExchanger;

impl KeyExchanger for X25519KeyExchanger {
    fn generate_keypair(&self) -> (Vec<u8>, Vec<u8>) {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);

        (secret.to_bytes().to_vec(), public.as_bytes().to_vec())
    }

    fn compute_shared_secret(&self, private_key: &[u8], peer_public_key: &[u8]) -> Result<Vec<u8>> {
        if private_key.len() != self.key_size() {
            return Err(FenrisError::InvalidKeySize {
                expected: self.key_size(),
                got: private_key.len(),
            });
        }

        if peer_public_key.len() != self.key_size() {
            return Err(FenrisError::InvalidKeySize {
                expected: self.key_size(),
                got: peer_public_key.len(),
            });
        }

        let secret_bytes: [u8; 32] = private_key.try_into().unwrap();
        let public_bytes: [u8; 32] = peer_public_key.try_into().unwrap();

        let secret = StaticSecret::from(secret_bytes);
        let public = PublicKey::from(public_bytes);

        let shared = secret.diffie_hellman(&public);

        Ok(shared.as_bytes().to_vec())
    }

    fn key_size(&self) -> usize {
        ECDH_KEY_SIZE
    }
}

#[derive(Debug, Clone, Default)]
pub struct HkdfSha256Deriver {
    salt: Vec<u8>,
}

impl HkdfSha256Deriver {
    pub fn with_salt(salt: Vec<u8>) -> Self {
        Self { salt }
    }
}

impl KeyDeriver for HkdfSha256Deriver {
    fn derive_key(
        &self,
        shared_secret: &[u8],
        context: &[u8],
        output_size: usize,
    ) -> Result<Vec<u8>> {
        let salt = if self.salt.is_empty() {
            b"fenris-encryption-salt-v1"
        } else {
            self.salt.as_slice()
        };

        let hkdf = Hkdf::<Sha256>::new(Some(salt), shared_secret);

        let mut key = vec![0u8; output_size];
        hkdf.expand(context, &mut key)
            .map_err(|e| FenrisError::EncryptionError(e.to_string()))?;

        Ok(key)
    }
}

pub struct CryptoManager {
    encryptor: Box<dyn Encryptor>,
    key_exchanger: Box<dyn KeyExchanger>,
    key_deriver: Box<dyn KeyDeriver>,
}

impl CryptoManager {
    pub fn new(
        encryptor: Box<dyn Encryptor>,
        key_exchanger: Box<dyn KeyExchanger>,
        key_deriver: Box<dyn KeyDeriver>,
    ) -> Self {
        Self {
            encryptor,
            key_exchanger,
            key_deriver,
        }
    }

    pub fn encrypt(&self, plaintext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
        self.encryptor.encrypt(plaintext, key, iv)
    }

    pub fn decrypt(&self, ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
        self.encryptor.decrypt(ciphertext, key, iv)
    }

    pub fn generate_iv(&self) -> Vec<u8> {
        self.encryptor.generate_iv()
    }

    pub fn generate_keypair(&self) -> (Vec<u8>, Vec<u8>) {
        self.key_exchanger.generate_keypair()
    }

    pub fn compute_shared_secret(
        &self,
        private_key: &[u8],
        peer_public_key: &[u8],
    ) -> Result<Vec<u8>> {
        self.key_exchanger
            .compute_shared_secret(private_key, peer_public_key)
    }

    pub fn derive_key(&self, shared_secret: &[u8], context: &[u8]) -> Result<Vec<u8>> {
        let output_size = self.encryptor.key_size();
        self.key_deriver
            .derive_key(shared_secret, context, output_size)
    }
}

impl Default for CryptoManager {
    fn default() -> Self {
        Self {
            encryptor: Box::new(AesGcmEncryptor),
            key_exchanger: Box::new(X25519KeyExchanger),
            key_deriver: Box::new(HkdfSha256Deriver::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_crypto_manager() {
        let manager = CryptoManager::default();

        let plaintext = b"Hello, Fenris!";
        let key = [42u8; KEY_SIZE];
        let iv = manager.generate_iv();

        let ciphertext = manager.encrypt(plaintext, &key, &iv).unwrap();
        let decrypted = manager.decrypt(&ciphertext, &key, &iv).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_key_exchange() {
        let manager = CryptoManager::default();

        let (alice_priv, alice_pub) = manager.generate_keypair();
        let (bob_priv, bob_pub) = manager.generate_keypair();

        let alice_shared = manager
            .compute_shared_secret(&alice_priv, &bob_pub)
            .unwrap();
        let bob_shared = manager
            .compute_shared_secret(&bob_priv, &alice_pub)
            .unwrap();

        assert_eq!(alice_shared, bob_shared);
    }

    #[test]
    fn test_full_workflow() {
        let manager = CryptoManager::default();

        let (alice_priv, alice_pub) = manager.generate_keypair();
        let (bob_priv, bob_pub) = manager.generate_keypair();

        let shared_secret = manager
            .compute_shared_secret(&alice_priv, &bob_pub)
            .unwrap();

        let key = manager
            .derive_key(&shared_secret, b"fenris-aes-key")
            .unwrap();

        let message = b"Secret message";
        let iv = manager.generate_iv();
        let ciphertext = manager.encrypt(message, &key, &iv).unwrap();

        let bob_shared = manager
            .compute_shared_secret(&bob_priv, &alice_pub)
            .unwrap();
        let bob_key = manager.derive_key(&bob_shared, b"fenris-aes-key").unwrap();

        let decrypted = manager.decrypt(&ciphertext, &bob_key, &iv).unwrap();

        assert_eq!(decrypted, message);
    }

    struct DummyEncryptor;

    impl Encryptor for DummyEncryptor {
        fn encrypt(&self, plaintext: &[u8], _key: &[u8], _iv: &[u8]) -> Result<Vec<u8>> {
            // Just reverse the plaintext
            Ok(plaintext.iter().rev().copied().collect())
        }

        fn decrypt(&self, ciphertext: &[u8], _key: &[u8], _iv: &[u8]) -> Result<Vec<u8>> {
            // Reverse back
            Ok(ciphertext.iter().rev().copied().collect())
        }

        fn generate_iv(&self) -> Vec<u8> {
            vec![0u8; 12]
        }

        fn key_size(&self) -> usize {
            32
        }

        fn iv_size(&self) -> usize {
            12
        }
    }

    #[test]
    fn test_custom_encryptor() {
        let manager = CryptoManager::new(
            Box::new(DummyEncryptor),
            Box::new(X25519KeyExchanger),
            Box::new(HkdfSha256Deriver::default()),
        );

        let plaintext = b"Hello";
        let key = [0u8; 32];
        let iv = [0u8; 12];

        let ciphertext = manager.encrypt(plaintext, &key, &iv).unwrap();

        // Should be reversed
        assert_eq!(ciphertext, b"olleH");

        let decrypted = manager.decrypt(&ciphertext, &key, &iv).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
