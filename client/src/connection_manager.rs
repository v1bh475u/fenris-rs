use common::{
    DEFAULT_TRANSFER_CHUNK_SIZE, DefaultSecureChannel, FenrisCommand, FenrisError, FenrisOutput,
    ObjectWriteMode, Result, TransferChunk,
};

use std::{io, path::PathBuf};

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tracing::{debug, info};

use crate::response_manager::ResponseManager;
use crate::{
    request_manager::{ClientCommandPlan, RequestManager},
    response_manager::FormattedResponse,
};

const READ_PREVIEW_LIMIT: usize = 500;

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
            .ok_or(FenrisError::NetworkError(io::Error::other(
                "Server info not set",
            )))?
            .to_socket_addr();
        info!("Connecting to server at {}", addr);

        let stream = TcpStream::connect(addr)
            .await
            .map_err(FenrisError::NetworkError)?;

        let channel = DefaultSecureChannel::client_handshake(stream).await?;
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
        let plan = self.request_manager.build_request(command)?;

        let response = self.execute_plan(plan).await?;

        let formatted = self.response_manager.format_response(&response);
        Ok(formatted)
    }

    async fn execute_plan(&mut self, plan: ClientCommandPlan) -> Result<FenrisOutput> {
        match plan {
            ClientCommandPlan::Single(request) => {
                self.send_request_receive_response(&request).await
            }
            ClientCommandPlan::ChunkedRead { path } => self.receive_chunked_read(path).await,
            ClientCommandPlan::ChunkedInlineWrite { path, mode, data } => {
                self.send_inline_write(path, mode, data).await
            }
            ClientCommandPlan::ChunkedUpload {
                source,
                destination,
                total_size,
            } => self.send_upload(source, destination, total_size).await,
        }
    }

    pub async fn send_request_receive_response(
        &mut self,
        request: &FenrisCommand,
    ) -> Result<FenrisOutput> {
        let channel = self.channel.as_mut().ok_or(FenrisError::ConnectionClosed)?;

        channel.send_msg(request).await?;
        debug!("Request sent, awaiting response...");
        channel.recv_msg::<FenrisOutput>().await
    }

    async fn send_inline_write(
        &mut self,
        path: PathBuf,
        mode: ObjectWriteMode,
        data: Vec<u8>,
    ) -> Result<FenrisOutput> {
        let chunk_size = self.begin_transfer(path, mode, data.len() as u64).await?;
        self.send_bytes_as_chunks(&data, chunk_size).await
    }

    async fn send_upload(
        &mut self,
        source: PathBuf,
        destination: PathBuf,
        total_size: u64,
    ) -> Result<FenrisOutput> {
        let chunk_size = self
            .begin_transfer(destination, ObjectWriteMode::Upload, total_size)
            .await?;
        let mut file = tokio::fs::File::open(&source).await.map_err(|e| {
            FenrisError::FileOperationError(format!(
                "Failed to open file {}: {}",
                source.display(),
                e
            ))
        })?;

        if total_size == 0 {
            return self.send_transfer_chunk(0, Vec::new(), true, 0).await;
        }

        let mut offset = 0;
        let mut buffer = vec![0; chunk_size];

        loop {
            let read = file.read(&mut buffer).await.map_err(|e| {
                FenrisError::FileOperationError(format!(
                    "Failed to read file {}: {}",
                    source.display(),
                    e
                ))
            })?;

            if read == 0 {
                return Err(FenrisError::FileOperationError(format!(
                    "File {} ended before {} bytes were read",
                    source.display(),
                    total_size
                )));
            }

            let data = buffer[..read].to_vec();
            let is_last = offset + read as u64 == total_size;
            let output = self
                .send_transfer_chunk(offset, data, is_last, total_size)
                .await?;

            if is_last {
                return Ok(output);
            }

            expect_transfer_progress(output)?;
            offset += read as u64;
        }
    }

    async fn begin_transfer(
        &mut self,
        path: PathBuf,
        mode: ObjectWriteMode,
        total_size: u64,
    ) -> Result<usize> {
        let channel = self.channel.as_mut().ok_or(FenrisError::ConnectionClosed)?;
        channel
            .send_msg(&FenrisCommand::BeginObjectWrite {
                path,
                mode,
                total_size,
            })
            .await?;

        match channel.recv_msg::<FenrisOutput>().await? {
            FenrisOutput::TransferReady { chunk_size } => {
                Ok(chunk_size.clamp(1, DEFAULT_TRANSFER_CHUNK_SIZE))
            }
            FenrisOutput::Error { message } => Err(FenrisError::InvalidRequest(message)),
            output => Err(FenrisError::InvalidRequest(format!(
                "unexpected transfer response: {:?}",
                output
            ))),
        }
    }

    async fn send_bytes_as_chunks(
        &mut self,
        data: &[u8],
        chunk_size: usize,
    ) -> Result<FenrisOutput> {
        if data.is_empty() {
            return self.send_transfer_chunk(0, Vec::new(), true, 0).await;
        }

        let total_size = data.len() as u64;
        let mut offset = 0;

        for chunk in data.chunks(chunk_size) {
            let is_last = offset + chunk.len() as u64 == total_size;
            let output = self
                .send_transfer_chunk(offset, chunk.to_vec(), is_last, total_size)
                .await?;

            if is_last {
                return Ok(output);
            }

            expect_transfer_progress(output)?;
            offset += chunk.len() as u64;
        }

        Err(FenrisError::ConnectionClosed)
    }

    async fn send_transfer_chunk(
        &mut self,
        offset: u64,
        data: Vec<u8>,
        is_last: bool,
        total_size: u64,
    ) -> Result<FenrisOutput> {
        let channel = self.channel.as_mut().ok_or(FenrisError::ConnectionClosed)?;
        channel
            .send_msg(&FenrisCommand::WriteObjectChunk(TransferChunk {
                offset,
                data,
                is_last,
                total_size,
            }))
            .await?;
        channel.recv_msg::<FenrisOutput>().await
    }

    async fn receive_chunked_read(&mut self, path: PathBuf) -> Result<FenrisOutput> {
        let channel = self.channel.as_mut().ok_or(FenrisError::ConnectionClosed)?;
        channel
            .send_msg(&FenrisCommand::ReadObject { path })
            .await?;

        let mut preview = Vec::new();

        loop {
            match channel.recv_msg::<FenrisOutput>().await? {
                FenrisOutput::ObjectContentChunk(chunk) => {
                    if preview.len() < READ_PREVIEW_LIMIT {
                        let remaining = READ_PREVIEW_LIMIT - preview.len();
                        preview.extend_from_slice(&chunk.data[..chunk.data.len().min(remaining)]);
                    }

                    if chunk.is_last {
                        return Ok(FenrisOutput::ObjectContent {
                            truncated: preview.len() as u64 != chunk.total_size,
                            data: preview,
                            total_size: chunk.total_size,
                        });
                    }
                }
                legacy @ FenrisOutput::ObjectContent { .. } => return Ok(legacy),
                FenrisOutput::Error { message } => return Ok(FenrisOutput::Error { message }),
                output => {
                    return Err(FenrisError::InvalidRequest(format!(
                        "unexpected read response: {:?}",
                        output
                    )));
                }
            }
        }
    }

    pub fn set_server_info(&mut self, server_info: ServerInfo) -> Result<()> {
        if self.is_connected() {
            tracing::error!("Cannot change server info while connected");
            return Err(FenrisError::NetworkError(io::Error::other(
                "Cannot change server info while connected",
            )));
        }

        self.server_info = Some(server_info);
        Ok(())
    }
}

fn expect_transfer_progress(output: FenrisOutput) -> Result<()> {
    match output {
        FenrisOutput::TransferProgress { .. } => Ok(()),
        FenrisOutput::Error { message } => Err(FenrisError::InvalidRequest(message)),
        output => Err(FenrisError::InvalidRequest(format!(
            "unexpected transfer response: {:?}",
            output
        ))),
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new(RequestManager, ResponseManager)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use tokio::net::{TcpListener, TcpStream};

    async fn connected_manager_and_server() -> (ConnectionManager, DefaultSecureChannel) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_stream = TcpStream::connect(addr);
        let server_stream = listener.accept();
        let (client_stream, server_stream) = tokio::join!(client_stream, server_stream);
        let client_stream = client_stream.unwrap();
        let (server_stream, _) = server_stream.unwrap();

        let client = DefaultSecureChannel::client_handshake(client_stream);
        let server = DefaultSecureChannel::server_handshake(server_stream);
        let (client, server) = tokio::join!(client, server);

        let manager = ConnectionManager {
            server_info: None,
            channel: Some(client.unwrap()),
            request_manager: RequestManager,
            response_manager: ResponseManager,
        };

        (manager, server.unwrap())
    }

    #[test]
    fn test_connection_manager_creation() {
        let server_info = ServerInfo::new("127.0.0.1".to_string(), 8080);
        let request_manager = RequestManager;
        let response_manager = ResponseManager;

        let mut manager = ConnectionManager::new(request_manager, response_manager);
        manager.set_server_info(server_info.clone()).unwrap();

        assert!(!manager.is_connected());
        let info = manager.server_info.unwrap();
        assert_eq!(info.address, "127.0.0.1");
        assert_eq!(info.port, 8080);
    }

    #[test]
    fn test_server_info_to_socket_addr() {
        let info = ServerInfo::new("localhost".to_string(), 8080);
        assert_eq!(info.to_socket_addr(), "localhost:8080");
    }

    #[tokio::test]
    async fn test_send_command_when_disconnected() {
        let mut manager = ConnectionManager::new(RequestManager, ResponseManager);

        let result = manager.send_command("ping").await;

        assert!(result.is_err());
        match result {
            Err(FenrisError::ConnectionClosed) => {} // Expected
            _ => panic!("Expected ConnectionClosed error"),
        }
    }

    #[tokio::test]
    async fn test_execute_plan_sends_chunked_inline_write() {
        let (mut manager, mut server) = connected_manager_and_server().await;

        let server_task = tokio::spawn(async move {
            let command: FenrisCommand = server.recv_msg().await.unwrap();
            assert_eq!(
                command,
                FenrisCommand::BeginObjectWrite {
                    path: PathBuf::from("data.txt"),
                    mode: ObjectWriteMode::Write,
                    total_size: 6,
                }
            );
            server
                .send_msg(&FenrisOutput::TransferReady { chunk_size: 4 })
                .await
                .unwrap();

            let command: FenrisCommand = server.recv_msg().await.unwrap();
            assert_eq!(
                command,
                FenrisCommand::WriteObjectChunk(TransferChunk {
                    offset: 0,
                    data: b"abcd".to_vec(),
                    is_last: false,
                    total_size: 6,
                })
            );
            server
                .send_msg(&FenrisOutput::TransferProgress { offset: 4 })
                .await
                .unwrap();

            let command: FenrisCommand = server.recv_msg().await.unwrap();
            assert_eq!(
                command,
                FenrisCommand::WriteObjectChunk(TransferChunk {
                    offset: 4,
                    data: b"ef".to_vec(),
                    is_last: true,
                    total_size: 6,
                })
            );
            server
                .send_msg(&FenrisOutput::Success {
                    message: "done".to_string(),
                })
                .await
                .unwrap();
        });

        let output = manager
            .execute_plan(ClientCommandPlan::ChunkedInlineWrite {
                path: PathBuf::from("data.txt"),
                mode: ObjectWriteMode::Write,
                data: b"abcdef".to_vec(),
            })
            .await
            .unwrap();

        assert_eq!(
            output,
            FenrisOutput::Success {
                message: "done".to_string()
            }
        );
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_execute_plan_collects_chunked_read_preview() {
        let (mut manager, mut server) = connected_manager_and_server().await;

        let server_task = tokio::spawn(async move {
            let command: FenrisCommand = server.recv_msg().await.unwrap();
            assert_eq!(
                command,
                FenrisCommand::ReadObject {
                    path: PathBuf::from("large.txt")
                }
            );

            server
                .send_msg(&FenrisOutput::ObjectContentChunk(TransferChunk {
                    offset: 0,
                    data: vec![b'a'; 300],
                    is_last: false,
                    total_size: 700,
                }))
                .await
                .unwrap();
            server
                .send_msg(&FenrisOutput::ObjectContentChunk(TransferChunk {
                    offset: 300,
                    data: vec![b'b'; 400],
                    is_last: true,
                    total_size: 700,
                }))
                .await
                .unwrap();
        });

        let output = manager
            .execute_plan(ClientCommandPlan::ChunkedRead {
                path: PathBuf::from("large.txt"),
            })
            .await
            .unwrap();

        assert_eq!(
            output,
            FenrisOutput::ObjectContent {
                data: [vec![b'a'; 300], vec![b'b'; 200]].concat(),
                total_size: 700,
                truncated: true,
            }
        );
        server_task.await.unwrap();
    }
}
