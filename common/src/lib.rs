pub mod compression;
pub mod config;
pub mod crypto;
pub mod error;
pub mod file_ops;
pub mod framing;
pub mod network;
pub mod proto;
pub mod protocol;
pub mod secure_channel;

pub use compression::{CompressionManager, ZlibCompressor};
pub use config::{
    CompressionConfig, CompressionOf, Config, CryptoConfig, CryptoOf, DefaultSuite, Protobuf,
    ProtocolCodecOf, ProtocolConfig, SecureChannelConfig, Zlib, ZlibWithLevel,
};
pub use crypto::{CryptoManager, IV_SIZE, KEY_SIZE, TAG_SIZE};
pub use error::{FenrisError, Result};
pub use file_ops::{DefaultFileOperations, FileMetadata, FileOperations};
pub use framing::{DEFAULT_MAX_FRAME_SIZE, FrameLimits, LengthPrefixedFrame};
pub use network::{
    receive_prefixed, receive_prefixed_with_limits, send_prefixed, send_prefixed_with_limits,
};
pub use proto::{Request, RequestType, Response, ResponseType};
pub use protocol::{ProtobufCodec, ProtocolCodec};
pub use secure_channel::{DefaultSecureChannel, SecureChannel};
