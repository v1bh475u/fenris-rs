use common::{
    DefaultSecureChannel, FenrisError, Result, SecureChannel, default_compression, default_crypto,
    proto::{Request, Response},
};

use std::io::{self, ErrorKind};

use tokio::net::TcpStream;
use tracing::{debug, info};

use crate::response_manager::ResponseManager;
use crate::{request_manager::RequestManager, response_manager::FormattedResponse};

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub address: String,
    pub port: u16,
}

impl ServerInfo {
    pub fn new(address: String, port: u16) -> Self {
        Self { address, port }
    }

    pub fn to_socket_addr(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}
pub struct ConnectionManager {
    server_info: Option<ServerInfo>,
    channel: Option<DefaultSecureChannel>,
    request_manager: RequestManager,
    response_manager: ResponseManager,
}

impl ConnectionManager {
    pub fn new(request_manager: RequestManager, response_manager: ResponseManager) -> Self {
        Self {
            server_info: None,
            channel: None,
            request_manager,
            response_manager,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.channel.is_some()
    }

    pub async fn connect(&mut self) -> Result<()> {
        let addr = self
            .server_info
            .as_ref()
            .ok_or(FenrisError::NetworkError(io::Error::new(
                ErrorKind::Other,
                "Server info not set",
            )))?
            .to_socket_addr();
        info!("Connecting to server at {}", addr);

        let stream = TcpStream::connect(addr)
            .await
            .map_err(|e| FenrisError::NetworkError(e))?;

        let crypto = default_crypto();
        let compressor = default_compression();

        let channel = SecureChannel::client_handshake(stream, crypto, compressor).await?;
        self.channel = Some(channel);

        info!("Successfully connected to server");

        Ok(())
    }

    pub async fn disconnect(&mut self) {
        self.channel.take();
        info!("Disconnected from server");
    }

    pub async fn send_command(&mut self, command: &str) -> Result<FormattedResponse> {
        if !self.is_connected() {
            return Err(FenrisError::ConnectionClosed);
        }
        debug!("Sending command: {}", command);
        let request = self.request_manager.build_request(command)?;

        let response = self.send_request_receive_response(&request).await?;

        let formatted = self.response_manager.format_response(&response);
        Ok(formatted)
    }

    pub async fn send_request_receive_response(&mut self, request: &Request) -> Result<Response> {
        let channel = self.channel.as_mut().ok_or(FenrisError::ConnectionClosed)?;

        channel.send_msg(request).await?;
        debug!("Request sent, awaiting response...");
        channel.recv_msg::<Response>().await
    }

    pub fn server_info(&self) -> Result<&ServerInfo> {
        self.server_info
            .as_ref()
            .ok_or(FenrisError::NetworkError(io::Error::new(
                ErrorKind::Other,
                "Server info not set",
            )))
    }

    pub fn set_server_info(&mut self, server_info: ServerInfo) -> Result<()> {
        if self.is_connected() {
            tracing::error!("Cannot change server info while connected");
            return Err(FenrisError::NetworkError(io::Error::new(
                ErrorKind::Other,
                "Cannot change server info while connected",
            )));
        }

        self.server_info = Some(server_info);
        Ok(())
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new(RequestManager::default(), ResponseManager::default())
    }
}

#[cfg(test)]
mod tests {
    use crate::{request_manager, response_manager};

    use super::*;

    #[test]
    fn test_connection_manager_creation() {
        let server_info = ServerInfo::new("127.0.0.1".to_string(), 8080);
        let request_manager =
            RequestManager::new(Box::new(request_manager::DefaultRequestManager {}));
        let response_manager =
            ResponseManager::new(Box::new(response_manager::DefaultResponseFormatter {}));

        let mut manager = ConnectionManager::new(request_manager, response_manager);
        manager.set_server_info(server_info.clone()).unwrap();

        assert!(!manager.is_connected());
        assert_eq!(manager.server_info().unwrap().address, "127.0.0.1");
        assert_eq!(manager.server_info().unwrap().port, 8080);
    }

    #[test]
    fn test_server_info_to_socket_addr() {
        let info = ServerInfo::new("localhost".to_string(), 8080);
        assert_eq!(info.to_socket_addr(), "localhost:8080");
    }

    #[tokio::test]
    async fn test_send_command_when_disconnected() {
        let mut manager =
            ConnectionManager::new(RequestManager::default(), ResponseManager::default());

        let result = manager.send_command("ping").await;

        assert!(result.is_err());
        match result {
            Err(FenrisError::ConnectionClosed) => {} // Expected
            _ => panic!("Expected ConnectionClosed error"),
        }
    }
}
