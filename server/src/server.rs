use crate::{
    client_info::{ClientId, ClientInfo},
    request_handler::RequestHandler,
};
use common::{
    DefaultSecureChannel, FenrisError, FileOperations, Request, RequestType, Response,
    ResponseType, Result, default_compression, default_crypto,
};
use dashmap::DashMap;
use std::io;
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub max_connections: usize,
    pub handshake_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub reject_when_full: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_connections: 1024,
            handshake_timeout: Duration::from_secs(10),
            idle_timeout: None,
            reject_when_full: true,
        }
    }
}

struct ServerState {
    clients: DashMap<ClientId, ClientInfo>,
    next_id: AtomicU64,
}

impl ServerState {
    fn new() -> Self {
        Self {
            clients: DashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    fn new_client_id(&self) -> ClientId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
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

    pub fn token(&self) -> CancellationToken {
        self.shutdown.clone()
    }
}

pub struct Server {
    listener: TcpListener,
    handler: Arc<RequestHandler>,
    state: Arc<ServerState>,
    shutdown: CancellationToken,
    permits: Arc<Semaphore>,
    config: ServerConfig,
}

impl Server {
    pub async fn bind(
        bind_addr: &str,
        file_ops: Arc<dyn FileOperations>,
        config: ServerConfig,
    ) -> Result<(Self, ServerHandle)> {
        info!("Binding to {}", bind_addr);
        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(FenrisError::NetworkError)?;

        let server = Self {
            listener,
            handler: Arc::new(RequestHandler::new(file_ops)),
            state: Arc::new(ServerState::new()),
            shutdown: CancellationToken::new(),
            permits: Arc::new(Semaphore::new(config.max_connections)),
            config,
        };

        let handle = ServerHandle {
            shutdown: server.shutdown.clone(),
        };

        Ok((server, handle))
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener
            .local_addr()
            .map_err(FenrisError::NetworkError)
    }

    pub fn active_clients(&self) -> usize {
        self.state.clients.len()
    }

    pub async fn run(self) -> Result<()> {
        info!("Server listening on {}", self.local_addr()?);

        let mut tasks: JoinSet<()> = JoinSet::new();

        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => {
                    info!("Shutdown requested; stopping accept loop");
                    break;
                }

                Some(join_res) = tasks.join_next() => {
                    if let Err(e) = join_res {
                        warn!("Connection task panicked or was cancelled: {}", e);
                    }
                }

                accept_res = self.listener.accept() => {
                    let (socket, addr) = match accept_res {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("Accept failed: {}", e);
                            continue;
                        }
                    };

                    let permit = match self.acquire_permit().await {
                        Ok(p) => p,
                        Err(()) => {
                            warn!("At connection capacity; rejecting {}", addr);
                            continue;
                        }
                    };

                    let state = Arc::clone(&self.state);
                    let handler = Arc::clone(&self.handler);
                    let shutdown = self.shutdown.clone();
                    let config = self.config.clone();

                    tasks.spawn(async move {
                        if let Err(e) = serve_connection(state, handler, shutdown, config, socket, addr, permit).await {
                            debug!("Connection {} ended with error: {}", addr, e);
                        }
                    });
                }
            }
        }

        self.shutdown.cancel();
        while let Some(join_res) = tasks.join_next().await {
            if let Err(e) = join_res {
                warn!(
                    "Connection task panicked or was cancelled during shutdown: {}",
                    e
                );
            }
        }

        info!("Server stopped");
        Ok(())
    }

    async fn acquire_permit(&self) -> std::result::Result<OwnedSemaphorePermit, ()> {
        if self.config.reject_when_full {
            self.permits.clone().try_acquire_owned().map_err(|_| ())
        } else {
            self.permits.clone().acquire_owned().await.map_err(|_| ())
        }
    }
}

async fn serve_connection(
    state: Arc<ServerState>,
    handler: Arc<RequestHandler>,
    shutdown: CancellationToken,
    config: ServerConfig,
    socket: TcpStream,
    addr: SocketAddr,
    _permit: OwnedSemaphorePermit,
) -> Result<()> {
    let client_id = state.new_client_id();
    state
        .clients
        .insert(client_id, ClientInfo::new(client_id, addr));
    info!("Client {} connected from {}", client_id, addr);

    struct Cleanup {
        state: Arc<ServerState>,
        client_id: ClientId,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.state.clients.remove(&self.client_id);
        }
    }
    let _cleanup = Cleanup {
        state: Arc::clone(&state),
        client_id,
    };

    let handshake =
        DefaultSecureChannel::server_handshake(socket, default_crypto(), default_compression());
    let mut channel = match tokio::time::timeout(config.handshake_timeout, handshake).await {
        Ok(Ok(ch)) => ch,
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            return Err(FenrisError::NetworkError(io::Error::new(
                io::ErrorKind::TimedOut,
                "Handshake timed out",
            )));
        }
    };

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("Client {} shutting down", client_id);
                break;
            }

            req_res = recv_request(&mut channel, config.idle_timeout) => {
                let request = match req_res {
                    Ok(r) => r,
                    Err(e) => {
                        debug!("Client {} disconnected or recv failed: {}", client_id, e);
                        break;
                    }
                };

                if let Some(mut info) = state.clients.get_mut(&client_id) {
                    info.update_activity();
                }

               if RequestType::try_from(request.command).ok() == Some(RequestType::Terminate) {
                    let response = Response {
                        r#type: ResponseType::Terminated as i32,
                        success: true,
                        error_message: String::new(),
                        data: vec![],
                        details: None,
                    };
                    let _ = channel.send_msg(&response).await;
                    break;
                }

                let response = handler.process_request(client_id, &request).await;

                if let Err(e) = channel.send_msg(&response).await {
                    debug!("Client {} send failed: {}", client_id, e);
                    break;
                }
            }
        }
    }

    info!("Client {} disconnected", client_id);
    Ok(())
}

async fn recv_request(
    channel: &mut DefaultSecureChannel,
    idle: Option<Duration>,
) -> Result<Request> {
    if let Some(timeout) = idle {
        tokio::time::timeout(timeout, channel.recv_msg::<Request>())
            .await
            .map_err(|_| {
                FenrisError::NetworkError(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Idle timeout reached",
                ))
            })?
    } else {
        channel.recv_msg::<Request>().await
    }
}
