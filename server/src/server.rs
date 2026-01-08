use common::{FenrisError, FileOperations, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::config::ServerConfig;
use crate::connection::Connection;
use crate::request_handler::RequestHandler;

pub struct Server {
    listener: TcpListener,
    handler: Arc<RequestHandler>,
    config: Arc<ServerConfig>,
    shutdown: CancellationToken,
    connection_limiter: Arc<Semaphore>,
    next_id: Arc<AtomicU64>,
}

impl Server {
    pub async fn bind(
        addr: &str,
        file_ops: Arc<dyn FileOperations>,
        config: ServerConfig,
    ) -> Result<(Self, ServerHandle)> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(FenrisError::NetworkError)?;

        let config = Arc::new(config);
        let shutdown = CancellationToken::new();
        let connection_limiter = Arc::new(Semaphore::new(config.max_connections));

        let server = Self {
            listener,
            handler: Arc::new(RequestHandler::new(file_ops)),
            config,
            shutdown: shutdown.clone(),
            connection_limiter,
            next_id: Arc::new(AtomicU64::new(1)),
        };

        let handle = ServerHandle {
            shutdown: shutdown.clone(),
        };

        Ok((server, handle))
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener
            .local_addr()
            .map_err(FenrisError::NetworkError)
    }

    pub async fn run(self) -> Result<()> {
        info!("Server listening on {}", self.local_addr()?);

        let mut tasks = JoinSet::new();

        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => {
                    info!("Shutdown signal received");
                    break;
                }

                Some(result) = tasks.join_next() => {
                    if let Err(e) = result {
                        warn!("Task panicked: {}", e);
                    }
                }

                accept_result = self.listener.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            self.spawn_connection(stream, addr, &mut tasks).await;
                        }
                        Err(e) => {
                            warn!("Accept error: {}", e);
                        }
                    }
                }
            }
        }

        info!("Shutting down server...");
        self.shutdown.cancel();

        while let Some(result) = tasks.join_next().await {
            if let Err(e) = result {
                warn!("Task shutdown error: {}", e);
            }
        }

        info!("Server stopped");
        Ok(())
    }

    async fn spawn_connection(
        &self,
        stream: TcpStream,
        addr: SocketAddr,
        tasks: &mut JoinSet<Result<()>>,
    ) {
        let permit = match self.connection_limiter.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                warn!("Connection limit reached, rejecting {}", addr);
                return;
            }
        };

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handler = Arc::clone(&self.handler);
        let config = Arc::clone(&self.config);
        let shutdown = self.shutdown.clone();

        tasks.spawn(async move {
            let _permit = permit;

            let connection = Connection::accept(id, stream, addr, handler, config).await?;
            connection.run(shutdown).await
        });
    }
}

#[derive(Clone)]
pub struct ServerHandle {
    shutdown: CancellationToken,
}

impl ServerHandle {
    pub fn shutdown(&self) {
        self.shutdown.cancel();
    }
}
