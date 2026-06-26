use common::{
    DEFAULT_TRANSFER_CHUNK_SIZE, DefaultSecureChannel, FenrisCommand, FenrisError, FenrisOutput,
    Result, StorageBackend,
};
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::config::ServerConfig;
use crate::request_handler::{ActiveWriteTransfer, RequestHandler};

pub struct Connection<B: StorageBackend> {
    id: u64,
    channel: DefaultSecureChannel,
    current_dir: PathBuf,
    handler: Arc<RequestHandler<B>>,
    config: Arc<ServerConfig>,
    active_write: Option<ActiveWriteTransfer>,
}

impl<B: StorageBackend> Connection<B> {
    pub async fn accept(
        id: u64,
        stream: TcpStream,
        addr: SocketAddr,
        handler: Arc<RequestHandler<B>>,
        config: Arc<ServerConfig>,
    ) -> Result<Self> {
        let handshake = DefaultSecureChannel::server_handshake(stream);

        let channel = tokio::time::timeout(config.handshake_timeout, handshake)
            .await
            .map_err(|_| {
                FenrisError::NetworkError(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Handshake timeout",
                ))
            })??;

        info!("Client {} connected from {}", id, addr);

        Ok(Self {
            id,
            channel,
            current_dir: PathBuf::from("/"),
            handler,
            config,
            active_write: None,
        })
    }

    pub async fn run(mut self, shutdown: CancellationToken) -> Result<()> {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("Client {} shutting down", self.id);
                    break;
                }

                result = self.receive_command() => {
                    match result {
                        Ok(command) => {
                            if Self::is_terminate(&command) {
                                self.send_terminate_response().await?;
                                break;
                            }

                            if let Err(e) = self.handle_command(command).await {
                                debug!("Client {} send error: {}", self.id, e);
                                break;
                            }
                        }
                        Err(e) => {
                            debug!("Client {} recv error: {}", self.id, e);
                            break;
                        }
                    }
                }
            }
        }

        info!("Client {} disconnected", self.id);
        Ok(())
    }

    async fn receive_command(&mut self) -> Result<FenrisCommand> {
        if let Some(timeout) = self.config.idle_timeout {
            tokio::time::timeout(timeout, self.channel.recv_msg())
                .await
                .map_err(|_| {
                    FenrisError::NetworkError(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Idle timeout",
                    ))
                })?
        } else {
            self.channel.recv_msg().await
        }
    }

    fn is_terminate(command: &FenrisCommand) -> bool {
        matches!(command, FenrisCommand::Terminate)
    }

    async fn handle_command(&mut self, command: FenrisCommand) -> Result<()> {
        match command {
            FenrisCommand::ReadObject { path } => self.send_object_content_chunks(path).await,
            FenrisCommand::BeginObjectWrite {
                path,
                mode,
                total_size,
            } => self.begin_object_write(path, mode, total_size).await,
            FenrisCommand::WriteObjectChunk(chunk) => self.write_object_chunk(chunk).await,
            command => {
                let response = self
                    .handler
                    .process_command(self.id, &command, &mut self.current_dir)
                    .await;
                self.channel.send_msg(&response).await
            }
        }
    }

    async fn begin_object_write(
        &mut self,
        path: PathBuf,
        mode: common::ObjectWriteMode,
        total_size: u64,
    ) -> Result<()> {
        if self.active_write.is_some() {
            return self
                .channel
                .send_msg(&FenrisOutput::Error {
                    message: "Transfer already active".to_string(),
                })
                .await;
        }

        match self
            .handler
            .begin_object_write(&path, mode, total_size, &self.current_dir)
            .await
        {
            Ok(transfer) => {
                self.active_write = Some(transfer);
                self.channel
                    .send_msg(&FenrisOutput::TransferReady {
                        chunk_size: DEFAULT_TRANSFER_CHUNK_SIZE,
                    })
                    .await
            }
            Err(e) => {
                self.channel
                    .send_msg(&FenrisOutput::Error {
                        message: e.to_string(),
                    })
                    .await
            }
        }
    }

    async fn write_object_chunk(&mut self, chunk: common::TransferChunk) -> Result<()> {
        let Some(mut transfer) = self.active_write.take() else {
            return self
                .channel
                .send_msg(&FenrisOutput::Error {
                    message: "No active transfer".to_string(),
                })
                .await;
        };

        match self
            .handler
            .write_object_chunk(&mut transfer, &chunk, DEFAULT_TRANSFER_CHUNK_SIZE)
            .await
        {
            Ok(output) => {
                if !matches!(output, FenrisOutput::Success { .. }) {
                    self.active_write = Some(transfer);
                }
                self.channel.send_msg(&output).await
            }
            Err(e) => {
                self.channel
                    .send_msg(&FenrisOutput::Error {
                        message: e.to_string(),
                    })
                    .await
            }
        }
    }

    async fn send_object_content_chunks(&mut self, path: PathBuf) -> Result<()> {
        let mut offset = 0;

        loop {
            match self
                .handler
                .read_object_chunk(&path, &self.current_dir, offset)
                .await
            {
                Ok(chunk) => {
                    offset = chunk.offset + chunk.data.len() as u64;
                    let is_last = chunk.is_last;
                    self.channel
                        .send_msg(&FenrisOutput::ObjectContentChunk(chunk))
                        .await?;

                    if is_last {
                        return Ok(());
                    }
                }
                Err(e) => {
                    return self
                        .channel
                        .send_msg(&FenrisOutput::Error {
                            message: e.to_string(),
                        })
                        .await;
                }
            }
        }
    }

    async fn send_terminate_response(&mut self) -> Result<()> {
        self.channel.send_msg(&FenrisOutput::Terminated).await
    }
}
