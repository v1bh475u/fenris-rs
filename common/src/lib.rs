pub mod compression;
pub mod config;
pub mod crypto;
pub mod error;
pub mod network;
pub mod proto;
pub mod secure_channel;

pub use compression::CompressionManager;
pub use config::{DefaultCompression, DefaultCrypto, default_compression, default_crypto};
pub use crypto::{CryptoManager, IV_SIZE, KEY_SIZE, TAG_SIZE};
pub use error::{FenrisError, Result};
pub use network::{receive_prefixed, send_prefixed};
pub use proto::{Request, RequestType, Response, ResponseType};
pub use secure_channel::SecureChannel;
