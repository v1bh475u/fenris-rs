use crate::{
    CompressionOf, Config, CryptoOf, ProtocolCodec, ProtocolCodecOf, Result, SecureChannelConfig,
    network,
};
use tokio::net::TcpStream;
use tracing::debug;

pub const DEFAULT_KDF_CONTEXT: &[u8] = b"fenris-aes-key";

pub type DefaultSecureChannel = SecureChannel<Config>;

pub struct SecureChannel<Cfg: SecureChannelConfig> {
    stream: TcpStream,
    key: Vec<u8>,
    crypto: CryptoOf<Cfg>,
    compressor: CompressionOf<Cfg>,
}

impl<Cfg: SecureChannelConfig> SecureChannel<Cfg> {
    pub fn new(
        stream: TcpStream,
        key: Vec<u8>,
        crypto: CryptoOf<Cfg>,
        compressor: CompressionOf<Cfg>,
    ) -> Self {
        Self {
            stream,
            key,
            crypto,
            compressor,
        }
    }

    pub async fn client_handshake(stream: TcpStream) -> Result<Self> {
        Self::client_handshake_with_context(stream, DEFAULT_KDF_CONTEXT).await
    }

    pub async fn client_handshake_with_context(
        mut stream: TcpStream,
        context: &[u8],
    ) -> Result<Self> {
        debug!("Starting client handshake");

        let crypto = Cfg::crypto();
        let compressor = Cfg::compression();

        let (private_key, public_key) = crypto.generate_keypair();
        network::send_prefixed(&mut stream, &public_key).await?;

        let server_public_key = network::receive_prefixed(&mut stream).await?;
        let shared_secret = crypto.compute_shared_secret(&private_key, &server_public_key)?;
        let key = crypto.derive_key(&shared_secret, context)?;

        Ok(Self::new(stream, key, crypto, compressor))
    }

    pub async fn server_handshake(stream: TcpStream) -> Result<Self> {
        Self::server_handshake_with_context(stream, DEFAULT_KDF_CONTEXT).await
    }

    pub async fn server_handshake_with_context(
        mut stream: TcpStream,
        context: &[u8],
    ) -> Result<Self> {
        debug!("Starting server key exchange");

        let client_public_key = network::receive_prefixed(&mut stream).await?;

        let crypto = Cfg::crypto();
        let compressor = Cfg::compression();

        let (private_key, public_key) = crypto.generate_keypair();
        network::send_prefixed(&mut stream, &public_key).await?;

        let shared_secret = crypto.compute_shared_secret(&private_key, &client_public_key)?;
        let key = crypto.derive_key(&shared_secret, context)?;

        Ok(Self::new(stream, key, crypto, compressor))
    }

    pub async fn send_msg<M>(&mut self, msg: &M) -> Result<()>
    where
        ProtocolCodecOf<Cfg>: ProtocolCodec<M>,
    {
        let buf = <ProtocolCodecOf<Cfg> as ProtocolCodec<M>>::encode(msg)?;
        debug!("Serialized outgoing message: {} bytes", buf.len());

        // Compress -> Seal (iv||ciphertext) -> Frame+Send
        let compressed = self.compressor.compress(&buf)?;
        let packet = self.crypto.seal(&compressed, &self.key)?;
        network::send_prefixed(&mut self.stream, &packet).await?;
        Ok(())
    }

    pub async fn recv_msg<M>(&mut self) -> Result<M>
    where
        ProtocolCodecOf<Cfg>: ProtocolCodec<M>,
    {
        let packet = network::receive_prefixed(&mut self.stream).await?;
        debug!("Received encrypted packet: {} bytes", packet.len());

        // Open -> Decompress -> Deserialize
        let decrypted = self.crypto.open(&packet, &self.key)?;
        let decompressed = self.compressor.decompress(&decrypted)?;

        <ProtocolCodecOf<Cfg> as ProtocolCodec<M>>::decode(decompressed.as_slice())
    }

    pub fn into_inner(self) -> TcpStream {
        self.stream
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DefaultSuite, FenrisError, KEY_SIZE, ProtocolConfig};
    use tokio::net::{TcpListener, TcpStream};

    #[derive(Debug, PartialEq, Eq)]
    struct TestMessage {
        value: u8,
    }

    struct TestCodec;

    impl ProtocolCodec<TestMessage> for TestCodec {
        fn encode(message: &TestMessage) -> Result<Vec<u8>> {
            Ok(vec![message.value])
        }

        fn decode(data: &[u8]) -> Result<TestMessage> {
            match data {
                [value] => Ok(TestMessage { value: *value }),
                _ => Err(FenrisError::SerializationError(
                    "invalid test message".to_string(),
                )),
            }
        }
    }

    struct TestProtocol;

    impl ProtocolConfig for TestProtocol {
        type Codec = TestCodec;
    }

    struct TestConfig;

    impl SecureChannelConfig for TestConfig {
        type CryptoConfig = DefaultSuite;
        type CompressionConfig = DefaultSuite;
        type ProtocolConfig = TestProtocol;
    }

    async fn setup_connection() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });

        let (server, _) = listener.accept().await.unwrap();
        let client = client.await.unwrap();

        (client, server)
    }

    #[tokio::test]
    async fn secure_channel_uses_configured_non_protobuf_codec() {
        let (client_stream, server_stream) = setup_connection().await;
        let key = vec![9u8; KEY_SIZE];

        let mut client = SecureChannel::<TestConfig>::new(
            client_stream,
            key.clone(),
            TestConfig::crypto(),
            TestConfig::compression(),
        );
        let mut server = SecureChannel::<TestConfig>::new(
            server_stream,
            key,
            TestConfig::crypto(),
            TestConfig::compression(),
        );

        let send_task = tokio::spawn(async move {
            client.send_msg(&TestMessage { value: 42 }).await
        });

        let received: TestMessage = server.recv_msg().await.unwrap();
        send_task.await.unwrap().unwrap();

        assert_eq!(received, TestMessage { value: 42 });
    }
}
