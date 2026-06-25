use crate::{
    CompressionManager, CryptoManager, ProtobufCodec, ZlibCompressor,
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

pub trait ProtocolConfig {
    type Codec;
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

pub type ProtocolCodecOf<Cfg> =
    <<Cfg as SecureChannelConfig>::ProtocolConfig as crate::config::ProtocolConfig>::Codec;

pub trait SecureChannelConfig {
    type CryptoConfig: CryptoConfig;
    type CompressionConfig: CompressionConfig;
    type ProtocolConfig: ProtocolConfig;

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

pub struct ZlibWithLevel<const LEVEL: u32>;

impl<const LEVEL: u32> CompressionConfig for ZlibWithLevel<LEVEL> {
    type Compressor = ZlibCompressor;

    fn compression() -> CompressionManager<Self::Compressor> {
        CompressionManager::new(ZlibCompressor::with_level(LEVEL))
    }
}

pub struct Protobuf;

impl ProtocolConfig for Protobuf {
    type Codec = ProtobufCodec;
}

pub struct Config;

impl SecureChannelConfig for Config {
    type CryptoConfig = DefaultSuite;
    type CompressionConfig = DefaultSuite;
    type ProtocolConfig = Protobuf;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        KEY_SIZE,
        compression::NullCompressor,
        crypto::{AesGcmEncryptor, HkdfSha256Deriver, X25519KeyExchanger},
    };
    use std::marker::PhantomData;

    #[test]
    fn default_suite_crypto_uses_default_crypto_stack() {
        let crypto: CryptoManager<AesGcmEncryptor, X25519KeyExchanger, HkdfSha256Deriver> =
            DefaultSuite::crypto();

        let key = [7u8; KEY_SIZE];
        let iv = crypto.generate_iv();
        let plaintext = b"configured crypto";

        let ciphertext = crypto.encrypt(plaintext, &key, &iv).unwrap();
        let decrypted = crypto.decrypt(&ciphertext, &key, &iv).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn default_suite_compression_uses_null_compressor() {
        let compression: CompressionManager<NullCompressor> = DefaultSuite::compression();
        let data = b"configured compression";

        assert_eq!(compression.compressor_name(), "none");
        assert_eq!(compression.compress(data).unwrap(), data);
        assert_eq!(compression.decompress(data).unwrap(), data);
    }

    #[test]
    fn zlib_compression_uses_zlib_default_settings() {
        let compression: CompressionManager<ZlibCompressor> = Zlib::compression();
        let expected = CompressionManager::new(ZlibCompressor::default());
        let data = b"zlib configured compression ".repeat(32);

        assert_eq!(compression.compressor_name(), "zlib");

        let compressed = compression.compress(&data).unwrap();
        assert_eq!(compressed, expected.compress(&data).unwrap());
        assert_eq!(compression.decompress(&compressed).unwrap(), data);
    }

    #[test]
    fn config_composes_default_suite() {
        let crypto: CryptoOf<Config> = Config::crypto();
        let compression: CompressionOf<Config> = Config::compression();

        let key = [11u8; KEY_SIZE];
        let sealed = crypto.seal(b"composed config", &key).unwrap();
        let opened = crypto.open(&sealed, &key).unwrap();

        assert_eq!(opened, b"composed config");
        assert_eq!(compression.compressor_name(), "none");
    }

    #[test]
    fn config_type_aliases_resolve_to_expected_concrete_types() {
        let _: PhantomData<EncryptorOf<Config>> = PhantomData::<AesGcmEncryptor>;
        let _: PhantomData<KeyExchangerOf<Config>> = PhantomData::<X25519KeyExchanger>;
        let _: PhantomData<KeyDeriverOf<Config>> = PhantomData::<HkdfSha256Deriver>;
        let _: PhantomData<CryptoOf<Config>> =
            PhantomData::<CryptoManager<AesGcmEncryptor, X25519KeyExchanger, HkdfSha256Deriver>>;
        let _: PhantomData<CompressorOf<Config>> = PhantomData::<NullCompressor>;
        let _: PhantomData<CompressionOf<Config>> =
            PhantomData::<CompressionManager<NullCompressor>>;
        let _: PhantomData<ProtocolCodecOf<Config>> = PhantomData::<ProtobufCodec>;
    }
}
