use thiserror::Error;

#[derive(Error, Debug)]
pub enum FenrisError {
    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Invalid key size: expected {expected}, got {got}")]
    InvalidKeySize { expected: usize, got: usize },

    #[error("Invalid IV size: expected {expected}, got {got}")]
    InvalidIvSize { expected: usize, got: usize },

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("Decompression error: {0}")]
    DecompressionError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] std::io::Error),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Invalid protocol message")]
    InvalidProtocolMessage,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("File operation failed: {0}")]
    FileOperationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type Result<T> = std::result::Result<T, FenrisError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = FenrisError::InvalidKeySize {
            expected: 32,
            got: 16,
        };
        assert_eq!(format!("{}", err), "Invalid key size: expected 32, got 16");
    }

    #[test]
    fn test_error_type() {
        fn might_fail() -> Result<()> {
            Err(FenrisError::ConnectionClosed)
        }

        let result = might_fail();
        assert!(result.is_err());
    }

    #[test]
    fn test_io_error_conversion() {
        use std::io;

        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let fenris_err: FenrisError = io_err.into();

        match fenris_err {
            FenrisError::NetworkError(_) => {}
            _ => panic!("Wrong error variant"),
        }
    }
}
