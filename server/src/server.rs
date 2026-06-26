use common::{FenrisError, Result, ServerIdentityKey, StorageBackend};
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

pub struct Server<B: StorageBackend> {
    listener: TcpListener,
    handler: Arc<RequestHandler<B>>,
    config: Arc<ServerConfig>,
    shutdown: CancellationToken,
    connection_limiter: Arc<Semaphore>,
    next_id: Arc<AtomicU64>,
    identity_key: Option<Arc<ServerIdentityKey>>,
}

impl<B: StorageBackend> Server<B> {
    pub async fn bind(
        addr: &str,
        storage: Arc<B>,
        config: ServerConfig,
    ) -> Result<(Self, ServerHandle)> {
        Self::bind_with_identity(addr, storage, config, None).await
    }

    pub async fn bind_authenticated(
        addr: &str,
        storage: Arc<B>,
        identity_key: Arc<ServerIdentityKey>,
        config: ServerConfig,
    ) -> Result<(Self, ServerHandle)> {
        Self::bind_with_identity(addr, storage, config, Some(identity_key)).await
    }

    async fn bind_with_identity(
        addr: &str,
        storage: Arc<B>,
        config: ServerConfig,
        identity_key: Option<Arc<ServerIdentityKey>>,
    ) -> Result<(Self, ServerHandle)> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(FenrisError::NetworkError)?;

        let config = Arc::new(config);
        let shutdown = CancellationToken::new();
        let connection_limiter = Arc::new(Semaphore::new(config.max_connections));

        let server = Self {
            listener,
            handler: Arc::new(RequestHandler::new(storage)),
            config,
            shutdown: shutdown.clone(),
            connection_limiter,
            next_id: Arc::new(AtomicU64::new(1)),
            identity_key,
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
        let identity_key = self.identity_key.clone();
        let shutdown = self.shutdown.clone();

        tasks.spawn(async move {
            let _permit = permit;

            let connection = if let Some(identity_key) = identity_key {
                Connection::accept_authenticated(id, stream, addr, handler, config, identity_key)
                    .await?
            } else {
                Connection::accept(id, stream, addr, handler, config).await?
            };
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
