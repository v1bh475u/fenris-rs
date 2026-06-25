use crate::{FenrisError, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::{debug, trace};

pub const DEFAULT_MAX_FRAME_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLimits {
    pub max_frame_size: usize,
}

impl Default for FrameLimits {
    fn default() -> Self {
        Self {
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
        }
    }
}

pub struct LengthPrefixedFrame;

impl LengthPrefixedFrame {
    pub fn encode_len(len: usize) -> Result<[u8; 4]> {
        let len = u32::try_from(len).map_err(|_| {
            FenrisError::InvalidFrame("frame length exceeds supported range".to_string())
        })?;
        Ok(len.to_be_bytes())
    }

    pub fn decode_len(header: [u8; 4], limits: FrameLimits) -> Result<usize> {
        let len = usize::try_from(u32::from_be_bytes(header)).map_err(|_| {
            FenrisError::InvalidFrame("frame length exceeds supported range".to_string())
        })?;

        if len > limits.max_frame_size {
            return Err(FenrisError::FrameTooLarge {
                max: limits.max_frame_size,
                got: len,
            });
        }

        Ok(len)
    }

    pub async fn send(stream: &mut TcpStream, data: &[u8], limits: FrameLimits) -> Result<()> {
        if data.len() > limits.max_frame_size {
            return Err(FenrisError::FrameTooLarge {
                max: limits.max_frame_size,
                got: data.len(),
            });
        }

        let length_buf = Self::encode_len(data.len())?;

        trace!("Sending {} bytes", data.len());
        stream.write_all(&length_buf).await?;
        stream.write_all(data).await?;
        debug!("Sent {} bytes", data.len());

        Ok(())
    }

    pub async fn receive(stream: &mut TcpStream, limits: FrameLimits) -> Result<Vec<u8>> {
        let mut length_buf = [0u8; 4];
        stream.read_exact(&mut length_buf).await?;

        let length = Self::decode_len(length_buf, limits)?;
        trace!("Expecting to receive {} bytes", length);

        let mut data = vec![0u8; length];
        stream.read_exact(&mut data).await?;
        debug!("Received {} bytes", length);

        Ok(data)
    }
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

    #[test]
    fn default_limits_use_expected_max_frame_size() {
        assert_eq!(
            FrameLimits::default().max_frame_size,
            DEFAULT_MAX_FRAME_SIZE
        );
    }

    #[test]
    fn encode_len_accepts_valid_length() {
        assert_eq!(
            LengthPrefixedFrame::encode_len(42).unwrap(),
            42u32.to_be_bytes()
        );
    }

    #[test]
    fn encode_len_rejects_length_above_u32_max() {
        let result = LengthPrefixedFrame::encode_len(u32::MAX as usize + 1);
        assert!(matches!(result, Err(FenrisError::InvalidFrame(_))));
    }

    #[test]
    fn decode_len_accepts_zero_length() {
        let len =
            LengthPrefixedFrame::decode_len(0u32.to_be_bytes(), FrameLimits::default()).unwrap();
        assert_eq!(len, 0);
    }

    #[test]
    fn decode_len_accepts_exact_limit() {
        let limits = FrameLimits { max_frame_size: 16 };
        let len = LengthPrefixedFrame::decode_len(16u32.to_be_bytes(), limits).unwrap();
        assert_eq!(len, 16);
    }

    #[test]
    fn decode_len_rejects_over_limit() {
        let limits = FrameLimits { max_frame_size: 16 };
        let result = LengthPrefixedFrame::decode_len(17u32.to_be_bytes(), limits);
        assert!(matches!(
            result,
            Err(FenrisError::FrameTooLarge { max: 16, got: 17 })
        ));
    }

    #[test]
    #[cfg(target_pointer_width = "16")]
    fn decode_len_rejects_u32_value_that_exceeds_usize_when_applicable() {
        let result = LengthPrefixedFrame::decode_len(
            u32::MAX.to_be_bytes(),
            FrameLimits {
                max_frame_size: usize::MAX,
            },
        );
        assert!(matches!(result, Err(FenrisError::InvalidFrame(_))));
    }

    #[test]
    #[cfg(not(target_pointer_width = "16"))]
    fn decode_len_rejects_u32_value_that_exceeds_usize_when_applicable() {
        let len = LengthPrefixedFrame::decode_len(
            u32::MAX.to_be_bytes(),
            FrameLimits {
                max_frame_size: u32::MAX as usize,
            },
        )
        .unwrap();
        assert_eq!(len, u32::MAX as usize);
    }

    #[tokio::test]
    async fn send_receive_empty_frame() {
        let (mut client, mut server) = setup_connection().await;

        tokio::spawn(async move {
            LengthPrefixedFrame::send(&mut client, b"", FrameLimits::default())
                .await
                .unwrap();
        });

        let received = LengthPrefixedFrame::receive(&mut server, FrameLimits::default())
            .await
            .unwrap();

        assert!(received.is_empty());
    }

    #[tokio::test]
    async fn send_receive_normal_frame() {
        let (mut client, mut server) = setup_connection().await;
        let message = b"bounded frame";

        tokio::spawn(async move {
            LengthPrefixedFrame::send(&mut client, message, FrameLimits::default())
                .await
                .unwrap();
        });

        let received = LengthPrefixedFrame::receive(&mut server, FrameLimits::default())
            .await
            .unwrap();

        assert_eq!(received, message);
    }

    #[tokio::test]
    async fn send_rejects_frame_over_configured_limit() {
        let (mut client, _server) = setup_connection().await;
        let limits = FrameLimits { max_frame_size: 4 };

        let result = LengthPrefixedFrame::send(&mut client, b"12345", limits).await;

        assert!(matches!(
            result,
            Err(FenrisError::FrameTooLarge { max: 4, got: 5 })
        ));
    }

    #[tokio::test]
    async fn receive_rejects_frame_over_configured_limit_before_reading_payload() {
        let (mut client, mut server) = setup_connection().await;
        let limits = FrameLimits { max_frame_size: 4 };

        tokio::spawn(async move {
            client.write_all(&5u32.to_be_bytes()).await.unwrap();
        });

        let result = LengthPrefixedFrame::receive(&mut server, limits).await;

        assert!(matches!(
            result,
            Err(FenrisError::FrameTooLarge { max: 4, got: 5 })
        ));
    }
}
