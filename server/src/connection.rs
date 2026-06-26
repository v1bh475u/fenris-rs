use common::{
    DefaultSecureChannel, FenrisCommand, FenrisError, FenrisOutput, Result, StorageBackend,
};
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::config::ServerConfig;
use crate::request_handler::RequestHandler;

pub struct Connection<B: StorageBackend> {
    id: u64,
    channel: DefaultSecureChannel,
    current_dir: PathBuf,
    handler: Arc<RequestHandler<B>>,
    config: Arc<ServerConfig>,
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

                            let response = self.handler.process_command(
                                self.id,
                                &command,
                                &mut self.current_dir,
                            ).await;

                            if let Err(e) = self.channel.send_msg(&response).await {
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

    async fn send_terminate_response(&mut self) -> Result<()> {
        self.channel.send_msg(&FenrisOutput::Terminated).await
    }
}
