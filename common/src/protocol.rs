use crate::{FenrisError, Result};
use prost::Message;

pub trait ProtocolCodec<M> {
    fn encode(message: &M) -> Result<Vec<u8>>;

    fn decode(data: &[u8]) -> Result<M>;
}

pub struct ProtobufCodec;

impl<M> ProtocolCodec<M> for ProtobufCodec
where
    M: Message + Default,
{
    fn encode(message: &M) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        message
            .encode(&mut buf)
            .map_err(|e| FenrisError::SerializationError(e.to_string()))?;
        Ok(buf)
    }

    fn decode(data: &[u8]) -> Result<M> {
        M::decode(data).map_err(|e| FenrisError::SerializationError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Request, RequestType, Response, ResponseType};

    #[test]
    fn protobuf_codec_round_trips_request() {
        let request = Request {
            command: RequestType::Ping as i32,
            filename: "test.txt".to_string(),
            ip_addr: 0,
            data: vec![1, 2, 3],
        };

        let encoded = ProtobufCodec::encode(&request).unwrap();
        let decoded: Request = ProtobufCodec::decode(&encoded).unwrap();

        assert_eq!(decoded.command, request.command);
        assert_eq!(decoded.filename, request.filename);
        assert_eq!(decoded.ip_addr, request.ip_addr);
        assert_eq!(decoded.data, request.data);
    }

    #[test]
    fn protobuf_codec_round_trips_response() {
        let response = Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: vec![4, 5, 6],
            details: None,
        };

        let encoded = ProtobufCodec::encode(&response).unwrap();
        let decoded: Response = ProtobufCodec::decode(&encoded).unwrap();

        assert_eq!(decoded.r#type, response.r#type);
        assert_eq!(decoded.success, response.success);
        assert_eq!(decoded.error_message, response.error_message);
        assert_eq!(decoded.data, response.data);
        assert_eq!(decoded.details, response.details);
    }

    #[test]
    fn protobuf_codec_rejects_invalid_bytes() {
        let result = <ProtobufCodec as ProtocolCodec<Request>>::decode(&[0xff]);

        assert!(matches!(result, Err(FenrisError::SerializationError(_))));
    }
}
