use crate::{
    CompressionManager, FenrisError, Result,
    compression::Compressor,
    config::{DefaultCompressor, DefaultEncryptor, DefaultKeyDeriver, DefaultKeyExchanger},
    crypto::{CryptoManager, Encryptor, KeyDeriver, KeyExchanger},
    network,
};
use prost::Message;
use tokio::net::TcpStream;
use tracing::debug;

pub const DEFAULT_KDF_CONTEXT: &[u8] = b"fenris-aes-key";

pub type DefaultSecureChannel =
    SecureChannel<DefaultEncryptor, DefaultKeyExchanger, DefaultKeyDeriver, DefaultCompressor>;

pub struct SecureChannel<E: Encryptor, K: KeyExchanger, D: KeyDeriver, C: Compressor> {
    stream: TcpStream,
    key: Vec<u8>,
    crypto: CryptoManager<E, K, D>,
    compressor: CompressionManager<C>,
}

impl<E: Encryptor, K: KeyExchanger, D: KeyDeriver, C: Compressor> SecureChannel<E, K, D, C> {
    pub fn new(
        stream: TcpStream,
        key: Vec<u8>,
        crypto: CryptoManager<E, K, D>,
        compressor: CompressionManager<C>,
    ) -> Self {
        Self {
            stream,
            key,
            crypto,
            compressor,
        }
    }

    pub async fn client_handshake(
        stream: TcpStream,
        crypto: CryptoManager<E, K, D>,
        compressor: CompressionManager<C>,
    ) -> Result<Self> {
        Self::client_handshake_with_context(stream, crypto, compressor, DEFAULT_KDF_CONTEXT).await
    }

    pub async fn client_handshake_with_context(
        mut stream: TcpStream,
        crypto: CryptoManager<E, K, D>,
        compressor: CompressionManager<C>,
        context: &[u8],
    ) -> Result<Self> {
        debug!("Starting client handshake");

        let (private_key, public_key) = crypto.generate_keypair();
        network::send_prefixed(&mut stream, &public_key).await?;

        let server_public_key = network::receive_prefixed(&mut stream).await?;
        let shared_secret = crypto.compute_shared_secret(&private_key, &server_public_key)?;
        let key = crypto.derive_key(&shared_secret, context)?;

        Ok(Self::new(stream, key, crypto, compressor))
    }

    pub async fn server_handshake(
        stream: TcpStream,
        crypto: CryptoManager<E, K, D>,
        compressor: CompressionManager<C>,
    ) -> Result<Self> {
        Self::server_handshake_with_context(stream, crypto, compressor, DEFAULT_KDF_CONTEXT).await
    }

    pub async fn server_handshake_with_context(
        mut stream: TcpStream,
        crypto: CryptoManager<E, K, D>,
        compressor: CompressionManager<C>,
        context: &[u8],
    ) -> Result<Self> {
        debug!("Starting server key exchange");

        let client_public_key = network::receive_prefixed(&mut stream).await?;

        let (private_key, public_key) = crypto.generate_keypair();
        network::send_prefixed(&mut stream, &public_key).await?;

        let shared_secret = crypto.compute_shared_secret(&private_key, &client_public_key)?;
        let key = crypto.derive_key(&shared_secret, context)?;

        Ok(Self::new(stream, key, crypto, compressor))
    }

    pub async fn send_msg<M: Message>(&mut self, msg: &M) -> Result<()> {
        let mut buf = Vec::new();
        msg.encode(&mut buf)
            .map_err(|e| FenrisError::SerializationError(e.to_string()))?;
        debug!("Serialized outgoing message: {} bytes", buf.len());

        // Compress -> Seal (iv||ciphertext) -> Frame+Send
        let compressed = self.compressor.compress(&buf)?;
        let packet = self.crypto.seal(&compressed, &self.key)?;
        network::send_prefixed(&mut self.stream, &packet).await?;
        Ok(())
    }

    pub async fn recv_msg<M: Message + Default>(&mut self) -> Result<M> {
        let packet = network::receive_prefixed(&mut self.stream).await?;
        debug!("Received encrypted packet: {} bytes", packet.len());

        // Open -> Decompress -> Deserialize
        let decrypted = self.crypto.open(&packet, &self.key)?;
        let decompressed = self.compressor.decompress(&decrypted)?;

        M::decode(decompressed.as_slice())
            .map_err(|e| FenrisError::SerializationError(e.to_string()))
    }

    pub fn into_inner(self) -> TcpStream {
        self.stream
    }
}
