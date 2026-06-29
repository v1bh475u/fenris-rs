use crate::error::{FenrisError, Result};

pub trait Compressor: Send + Sync {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>>;

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>>;

    fn name(&self) -> &str;
}

// Default
use flate2::Compression;
use flate2::write::{ZlibDecoder, ZlibEncoder};
use std::io::Write;

#[derive(Debug, Clone)]
pub struct ZlibCompressor {
    level: Compression,
}

impl ZlibCompressor {
    pub fn new() -> Self {
        Self {
            level: Compression::default(),
        }
    }

    pub fn with_level(level: u32) -> Self {
        Self {
            level: Compression::new(level),
        }
    }
}

impl Default for ZlibCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor for ZlibCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = ZlibEncoder::new(Vec::new(), self.level);
        encoder
            .write_all(data)
            .map_err(|e| FenrisError::CompressionError(e.to_string()))?;
        encoder
            .finish()
            .map_err(|e| FenrisError::CompressionError(e.to_string()))
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = ZlibDecoder::new(Vec::new());
        decoder
            .write_all(data)
            .map_err(|e| FenrisError::CompressionError(e.to_string()))?;
        decoder
            .finish()
            .map_err(|e| FenrisError::CompressionError(e.to_string()))
    }

    fn name(&self) -> &str {
        "zlib"
    }
}

#[cfg(feature = "zstd")]
#[derive(Debug, Clone)]
pub struct ZstdCompressor {
    level: i32,
}

#[cfg(feature = "zstd")]
impl ZstdCompressor {
    pub fn new() -> Self {
        Self {
            level: zstd::DEFAULT_COMPRESSION_LEVEL,
        }
    }

    pub fn with_level(level: i32) -> Self {
        Self { level }
    }
}

#[cfg(feature = "zstd")]
impl Default for ZstdCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "zstd")]
impl Compressor for ZstdCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::stream::encode_all(data, self.level)
            .map_err(|e| FenrisError::CompressionError(e.to_string()))
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::stream::decode_all(data).map_err(|e| FenrisError::DecompressionError(e.to_string()))
    }

    fn name(&self) -> &str {
        "zstd"
    }
}

#[derive(Debug, Clone, Default)]
pub struct NullCompressor;

impl Compressor for NullCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn name(&self) -> &str {
        "none"
    }
}

pub struct CompressionManager<C: Compressor> {
    compressor: C,
}

impl<C: Compressor> CompressionManager<C> {
    pub fn new(compressor: C) -> Self {
        Self { compressor }
    }

    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.compressor.compress(data)
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.compressor.decompress(data)
    }

    pub fn compressor_name(&self) -> &str {
        self.compressor.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zlib_compress_decompress() {
        let manager = CompressionManager::new(NullCompressor);

        let original = b"Hello, World!  This is test data.";
        let compressed = manager.compress(original).unwrap();
        let decompressed = manager.decompress(&compressed).unwrap();

        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compression_level() {
        let manager = CompressionManager::new(ZlibCompressor::with_level(9));

        let data = b"AAAAAAAAAA".repeat(1000);
        let compressed = manager.compress(&data).unwrap();

        assert!(compressed.len() < data.len() / 10);
    }

    #[test]
    fn test_null_compressor() {
        let manager = CompressionManager::new(NullCompressor);

        let data = b"Test data";
        let compressed = manager.compress(data).unwrap();

        // Should be unchanged
        assert_eq!(compressed, data);

        let decompressed = manager.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_algorithm_name() {
        let zlib_manager = CompressionManager::new(NullCompressor);
        assert_eq!(zlib_manager.compressor_name(), "none");

        let null_manager = CompressionManager::new(ZlibCompressor::new());
        assert_eq!(null_manager.compressor_name(), "zlib");
    }

    #[cfg(feature = "zstd")]
    #[test]
    fn test_zstd_compress_decompress() {
        let manager = CompressionManager::new(ZstdCompressor::default());

        let data = b"zstd test data ".repeat(128);
        let compressed = manager.compress(&data).unwrap();
        let decompressed = manager.decompress(&compressed).unwrap();

        assert_eq!(decompressed, data);
    }

    #[cfg(feature = "zstd")]
    #[test]
    fn test_zstd_handles_small_payload() {
        let manager = CompressionManager::new(ZstdCompressor::default());

        let data = b"small";
        let compressed = manager.compress(data).unwrap();
        let decompressed = manager.decompress(&compressed).unwrap();

        assert_eq!(decompressed, data);
    }

    #[cfg(feature = "zstd")]
    #[test]
    fn test_zstd_compresses_repeated_data() {
        let manager = CompressionManager::new(ZstdCompressor::with_level(3));

        let data = b"AAAAAAAAAA".repeat(1000);
        let compressed = manager.compress(&data).unwrap();

        assert!(compressed.len() < data.len() / 10);
    }

    #[cfg(feature = "zstd")]
    #[test]
    fn test_zstd_algorithm_name() {
        let manager = CompressionManager::new(ZstdCompressor::default());

        assert_eq!(manager.compressor_name(), "zstd");
    }
}
