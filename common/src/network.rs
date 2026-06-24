use crate::{
    error::Result,
    framing::{FrameLimits, LengthPrefixedFrame},
};
use tokio::net::TcpStream;

pub async fn send_prefixed(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
    send_prefixed_with_limits(stream, data, FrameLimits::default()).await
}

pub async fn receive_prefixed(stream: &mut TcpStream) -> Result<Vec<u8>> {
    receive_prefixed_with_limits(stream, FrameLimits::default()).await
}

pub async fn send_prefixed_with_limits(
    stream: &mut TcpStream,
    data: &[u8],
    limits: FrameLimits,
) -> Result<()> {
    LengthPrefixedFrame::send(stream, data, limits).await
}

pub async fn receive_prefixed_with_limits(
    stream: &mut TcpStream,
    limits: FrameLimits,
) -> Result<Vec<u8>> {
    LengthPrefixedFrame::receive(stream, limits).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FenrisError, FrameLimits};
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, TcpStream};

    async fn setup_connection() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });

        let (server, _) = listener.accept().await.unwrap();
        let client = client.await.unwrap();

        (client, server)
    }

    #[tokio::test]
    async fn test_send_receive_prefixed() {
        let (mut client, mut server) = setup_connection().await;

        let message = b"Hello, Network!";

        tokio::spawn(async move {
            send_prefixed(&mut client, message).await.unwrap();
        });

        let received = receive_prefixed(&mut server).await.unwrap();

        assert_eq!(received, message);
    }

    #[tokio::test]
    async fn test_empty_message() {
        let (mut client, mut server) = setup_connection().await;

        let empty = b"";

        tokio::spawn(async move {
            send_prefixed(&mut client, empty).await.unwrap();
        });

        let received = receive_prefixed(&mut server).await.unwrap();

        assert_eq!(received, empty);
    }

    #[tokio::test]
    async fn send_prefixed_with_limits_rejects_oversized_payload() {
        let (mut client, _server) = setup_connection().await;
        let limits = FrameLimits { max_frame_size: 4 };

        let result = send_prefixed_with_limits(&mut client, b"12345", limits).await;

        assert!(matches!(
            result,
            Err(FenrisError::FrameTooLarge { max: 4, got: 5 })
        ));
    }

    #[tokio::test]
    async fn receive_prefixed_with_limits_rejects_oversized_payload() {
        let (mut client, mut server) = setup_connection().await;
        let limits = FrameLimits { max_frame_size: 4 };

        tokio::spawn(async move {
            client.write_all(&5u32.to_be_bytes()).await.unwrap();
        });

        let result = receive_prefixed_with_limits(&mut server, limits).await;

        assert!(matches!(
            result,
            Err(FenrisError::FrameTooLarge { max: 4, got: 5 })
        ));
    }
}
