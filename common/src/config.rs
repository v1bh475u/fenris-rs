//! Compile-time configuration for crypto and compression policies.
//!
//! To experiment with different combinations:
//! 1. Change the type aliases below
//! 2. Recompile
//! 3. Test performance/behavior
//!

use crate::{
    CompressionManager, CryptoManager,
    compression::{NullCompressor, ZlibCompressor},
    crypto::{AesGcmEncryptor, HkdfSha256Deriver, X25519KeyExchanger},
};

/// The encryption algorithm used for AEAD sealing/opening.
/// Options: AesGcmEncryptor (production)
pub type DefaultEncryptor = AesGcmEncryptor;

/// The key exchange algorithm for ECDH.
/// Options: X25519KeyExchanger (production)
pub type DefaultKeyExchanger = X25519KeyExchanger;

/// The key derivation function for HKDF.
/// Options: HkdfSha256Deriver (production)
pub type DefaultKeyDeriver = HkdfSha256Deriver;

/// The compression algorithm.
/// Options:
/// - NullCompressor (no compression, lowest CPU)
/// - ZlibCompressor (balanced compression)
pub type DefaultCompressor = NullCompressor;

/// The configured crypto manager type. Change the inner types above to swap algorithms.
pub type DefaultCrypto = CryptoManager<DefaultEncryptor, DefaultKeyExchanger, DefaultKeyDeriver>;

/// The configured compression manager type. Change DefaultCompressor above to swap.
pub type DefaultCompression = CompressionManager<DefaultCompressor>;

pub fn default_crypto() -> DefaultCrypto {
    CryptoManager::new(
        DefaultEncryptor::default(),
        DefaultKeyExchanger::default(),
        DefaultKeyDeriver::default(),
    )
}

pub fn default_compression() -> DefaultCompression {
    CompressionManager::new(DefaultCompressor::default())
}

pub fn zlib_compression(level: u32) -> CompressionManager<ZlibCompressor> {
    CompressionManager::new(ZlibCompressor::with_level(level))
}
