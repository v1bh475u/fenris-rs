pub mod compression;
pub mod crypto;
pub mod error;
pub mod network;
pub mod proto;

pub use compression::CompressionManager;
pub use crypto::{CryptoManager, IV_SIZE, KEY_SIZE, TAG_SIZE};
pub use error::{FenrisError, Result};
pub use network::{receive_prefixed, send_prefixed};
pub use proto::{Request, RequestType, Response, ResponseType};
