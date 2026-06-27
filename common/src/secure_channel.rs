use crate::{
    CompressionOf, Config, CryptoOf, ProtocolCodec, ProtocolCodecOf, Result, SecureChannelConfig,
    identity::{
        ServerIdentityKey, ServerIdentityPublicKey, authenticated_kdf_context,
        server_identity_transcript,
    },
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

    pub async fn client_handshake_authenticated(
        stream: TcpStream,
        expected_server_identity: ServerIdentityPublicKey,
    ) -> Result<Self> {
        Self::client_handshake_authenticated_with_context(
            stream,
            expected_server_identity,
            DEFAULT_KDF_CONTEXT,
        )
        .await
    }

    pub async fn client_handshake_authenticated_with_context(
        mut stream: TcpStream,
        expected_server_identity: ServerIdentityPublicKey,
        context: &[u8],
    ) -> Result<Self> {
        debug!("Starting authenticated client handshake");

        let crypto = Cfg::crypto();
        let compressor = Cfg::compression();

        let (private_key, public_key) = crypto.generate_keypair();
        network::send_prefixed(&mut stream, &public_key).await?;

        let server_public_key = network::receive_prefixed(&mut stream).await?;
        let server_identity = ServerIdentityPublicKey::from_slice(
            network::receive_prefixed(&mut stream).await?.as_slice(),
        )?;
        let signature = network::receive_prefixed(&mut stream).await?;

        if server_identity != expected_server_identity {
            return Err(crate::FenrisError::AuthenticationError(
                "server identity did not match pinned key".to_string(),
            ));
        }

        let transcript =
            server_identity_transcript(&public_key, &server_public_key, &server_identity, context);
        server_identity.verify_transcript(&transcript, &signature)?;

        let shared_secret = crypto.compute_shared_secret(&private_key, &server_public_key)?;
        let key = crypto.derive_key(&shared_secret, &authenticated_kdf_context(&transcript))?;

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

    pub async fn server_handshake_authenticated(
        stream: TcpStream,
        server_identity_key: &ServerIdentityKey,
    ) -> Result<Self> {
        Self::server_handshake_authenticated_with_context(
            stream,
            server_identity_key,
            DEFAULT_KDF_CONTEXT,
        )
        .await
    }

    pub async fn server_handshake_authenticated_with_context(
        mut stream: TcpStream,
        server_identity_key: &ServerIdentityKey,
        context: &[u8],
    ) -> Result<Self> {
        debug!("Starting authenticated server key exchange");

        let client_public_key = network::receive_prefixed(&mut stream).await?;

        let crypto = Cfg::crypto();
        let compressor = Cfg::compression();

        let (private_key, public_key) = crypto.generate_keypair();
        let server_identity = server_identity_key.public_key();
        let transcript =
            server_identity_transcript(&client_public_key, &public_key, &server_identity, context);
        let signature = server_identity_key.sign_transcript(&transcript);

        network::send_prefixed(&mut stream, &public_key).await?;
        network::send_prefixed(&mut stream, server_identity.as_bytes()).await?;
        network::send_prefixed(&mut stream, &signature).await?;

        let shared_secret = crypto.compute_shared_secret(&private_key, &client_public_key)?;
        let key = crypto.derive_key(&shared_secret, &authenticated_kdf_context(&transcript))?;

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
    use crate::{DefaultSuite, FenrisError, KEY_SIZE, ProtocolConfig, ServerIdentityKey};
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

        let send_task =
            tokio::spawn(async move { client.send_msg(&TestMessage { value: 42 }).await });

        let received: TestMessage = server.recv_msg().await.unwrap();
        send_task.await.unwrap().unwrap();

        assert_eq!(received, TestMessage { value: 42 });
    }

    #[tokio::test]
    async fn authenticated_handshake_sends_and_receives_with_matching_pinned_key() {
        let (client_stream, server_stream) = setup_connection().await;
        let identity_key = ServerIdentityKey::generate();
        let expected_identity = identity_key.public_key();

        let client = SecureChannel::<TestConfig>::client_handshake_authenticated(
            client_stream,
            expected_identity,
        );
        let server = SecureChannel::<TestConfig>::server_handshake_authenticated(
            server_stream,
            &identity_key,
        );
        let (client, server) = tokio::join!(client, server);
        let mut client = client.unwrap();
        let mut server = server.unwrap();

        let send_task =
            tokio::spawn(async move { client.send_msg(&TestMessage { value: 7 }).await });

        let received: TestMessage = server.recv_msg().await.unwrap();
        send_task.await.unwrap().unwrap();

        assert_eq!(received, TestMessage { value: 7 });
    }

    #[tokio::test]
    async fn authenticated_handshake_rejects_wrong_pinned_server_identity() {
        let (client_stream, server_stream) = setup_connection().await;
        let identity_key = ServerIdentityKey::generate();
        let wrong_identity = ServerIdentityKey::generate().public_key();

        let client = SecureChannel::<TestConfig>::client_handshake_authenticated(
            client_stream,
            wrong_identity,
        );
        let server = SecureChannel::<TestConfig>::server_handshake_authenticated(
            server_stream,
            &identity_key,
        );
        let (client, server) = tokio::join!(client, server);

        assert!(matches!(
            client,
            Err(FenrisError::AuthenticationError(message))
                if message.contains("pinned key")
        ));
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn authenticated_handshake_rejects_tampered_server_signature() {
        let (client_stream, mut server_stream) = setup_connection().await;
        let identity_key = ServerIdentityKey::generate();
        let expected_identity = identity_key.public_key();

        let client_task = tokio::spawn(async move {
            SecureChannel::<TestConfig>::client_handshake_authenticated(
                client_stream,
                expected_identity,
            )
            .await
        });

        let client_public_key = network::receive_prefixed(&mut server_stream).await.unwrap();
        let crypto = TestConfig::crypto();
        let (_private_key, server_public_key) = crypto.generate_keypair();
        let transcript = server_identity_transcript(
            &client_public_key,
            &server_public_key,
            &identity_key.public_key(),
            DEFAULT_KDF_CONTEXT,
        );
        let mut signature = identity_key.sign_transcript(&transcript);
        signature[0] ^= 1;

        network::send_prefixed(&mut server_stream, &server_public_key)
            .await
            .unwrap();
        network::send_prefixed(&mut server_stream, identity_key.public_key().as_bytes())
            .await
            .unwrap();
        network::send_prefixed(&mut server_stream, &signature)
            .await
            .unwrap();

        let result = client_task.await.unwrap();

        assert!(matches!(
            result,
            Err(FenrisError::AuthenticationError(message))
                if message.contains("signature verification")
        ));
    }
}
