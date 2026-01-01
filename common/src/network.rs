use crate::error::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, trace};

pub async fn send_prefixed(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
    let length = data.len() as u32;

    trace!("Sending {} bytes", length);

    let length_buf = length.to_be_bytes();

    stream.write_all(&length_buf).await?;
    stream.write_all(data).await?;
    debug!("Sent {} bytes", length);

    Ok(())
}

pub async fn receive_prefixed(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut length_buf = [0u8; 4];
    stream.read_exact(&mut length_buf).await?;

    let length = u32::from_be_bytes(length_buf) as usize;
    trace!("Expecting to receive {} bytes", length);

    let mut data = vec![0u8; length];
    stream.read_exact(&mut data).await?;
    debug!("Received {} bytes", length);

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
