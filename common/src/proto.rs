include!(concat!(env!("OUT_DIR"), "/fenris.rs"));

use crate::error::{FenrisError, Result};
use prost::Message;

impl Request {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.encode(&mut buf)
            .map_err(|e| FenrisError::SerializationError(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::decode(data).map_err(|e| FenrisError::SerializationError(e.to_string()))
    }
}

impl Response {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.encode(&mut buf)
            .map_err(|e| FenrisError::SerializationError(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::decode(data).map_err(|e| FenrisError::SerializationError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let request = Request {
            command: RequestType::Ping as i32,
            filename: "test.txt".to_string(),
            ip_addr: 0,
            data: vec![1, 2, 3],
        };

        let bytes = request.to_bytes().unwrap();
        let decoded = Request::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.command, request.command);
        assert_eq!(decoded.filename, request.filename);
        assert_eq!(decoded.data, request.data);
    }

    #[test]
    fn test_response_serialization() {
        let response = Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: vec![4, 5, 6],
            details: None,
        };

        let bytes = response.to_bytes().unwrap();
        let decoded = Response::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.r#type, response.r#type);
        assert_eq!(decoded.success, response.success);
        assert_eq!(decoded.data, response.data);
    }
}
