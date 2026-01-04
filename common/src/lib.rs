pub mod compression;
pub mod config;
pub mod crypto;
pub mod error;
pub mod file_ops;
pub mod network;
pub mod proto;
pub mod secure_channel;

pub use compression::CompressionManager;
pub use config::{
    DefaultCompression, DefaultCompressor, DefaultCrypto, DefaultEncryptor, DefaultKeyDeriver,
    DefaultKeyExchanger, default_compression, default_crypto,
};
pub use crypto::{CryptoManager, IV_SIZE, KEY_SIZE, TAG_SIZE};
pub use error::{FenrisError, Result};
pub use file_ops::{DefaultFileOperations, FileMetadata, FileOperations};
pub use network::{receive_prefixed, send_prefixed};
pub use proto::{Request, RequestType, Response, ResponseType};
pub use secure_channel::{DefaultSecureChannel, SecureChannel};
