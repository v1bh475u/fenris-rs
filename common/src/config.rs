use crate::{
    CompressionManager, CryptoManager, ZlibCompressor,
    compression::{Compressor, NullCompressor},
    crypto::{
        AesGcmEncryptor, Encryptor, HkdfSha256Deriver, KeyDeriver, KeyExchanger, X25519KeyExchanger,
    },
};

pub trait CryptoConfig {
    type Encryptor: Encryptor;
    type KeyExchanger: KeyExchanger;
    type KeyDeriver: KeyDeriver;

    fn crypto() -> CryptoManager<Self::Encryptor, Self::KeyExchanger, Self::KeyDeriver>;
}

pub trait CompressionConfig {
    type Compressor: Compressor;

    fn compression() -> CompressionManager<Self::Compressor>;
}
pub type EncryptorOf<Cfg> =
    <<Cfg as SecureChannelConfig>::CryptoConfig as crate::config::CryptoConfig>::Encryptor;
pub type KeyExchangerOf<Cfg> =
    <<Cfg as SecureChannelConfig>::CryptoConfig as crate::config::CryptoConfig>::KeyExchanger;
pub type KeyDeriverOf<Cfg> =
    <<Cfg as SecureChannelConfig>::CryptoConfig as crate::config::CryptoConfig>::KeyDeriver;

pub type CryptoOf<Cfg> = CryptoManager<EncryptorOf<Cfg>, KeyExchangerOf<Cfg>, KeyDeriverOf<Cfg>>;

pub type CompressorOf<Cfg> =
    <<Cfg as SecureChannelConfig>::CompressionConfig as crate::config::CompressionConfig>::Compressor;
pub type CompressionOf<Cfg> = CompressionManager<CompressorOf<Cfg>>;

pub trait SecureChannelConfig {
    type CryptoConfig: CryptoConfig;
    type CompressionConfig: CompressionConfig;
    fn crypto() -> CryptoOf<Self> {
        <Self::CryptoConfig as CryptoConfig>::crypto()
    }

    fn compression() -> CompressionOf<Self> {
        <Self::CompressionConfig as CompressionConfig>::compression()
    }
}

pub struct DefaultSuite;

impl CryptoConfig for DefaultSuite {
    type Encryptor = AesGcmEncryptor;
    type KeyExchanger = X25519KeyExchanger;
    type KeyDeriver = HkdfSha256Deriver;

    fn crypto() -> CryptoManager<Self::Encryptor, Self::KeyExchanger, Self::KeyDeriver> {
        CryptoManager::new(
            AesGcmEncryptor,
            X25519KeyExchanger,
            HkdfSha256Deriver::default(),
        )
    }
}

impl CompressionConfig for DefaultSuite {
    type Compressor = NullCompressor;

    fn compression() -> CompressionManager<Self::Compressor> {
        CompressionManager::new(NullCompressor)
    }
}

pub struct Zlib;

impl CompressionConfig for Zlib {
    type Compressor = ZlibCompressor;

    fn compression() -> CompressionManager<Self::Compressor> {
        CompressionManager::new(ZlibCompressor::default())
    }
}

pub struct Config;

impl SecureChannelConfig for Config {
    type CryptoConfig = DefaultSuite;
    type CompressionConfig = DefaultSuite;
}
