use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub max_connections: usize,

    pub handshake_timeout: Duration,

    pub idle_timeout: Option<Duration>,

    pub reject_when_full: bool,

    pub tcp_keepalive: Option<Duration>,
}

impl ServerConfig {
    pub fn builder() -> ServerConfigBuilder {
        ServerConfigBuilder::default()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_connections: 1024,
            handshake_timeout: Duration::from_secs(10),
            idle_timeout: Some(Duration::from_secs(300)),
            reject_when_full: true,
            tcp_keepalive: Some(Duration::from_secs(60)),
        }
    }
}

#[derive(Default)]
pub struct ServerConfigBuilder {
    max_connections: Option<usize>,
    handshake_timeout: Option<Duration>,
    idle_timeout: Option<Duration>,
    reject_when_full: Option<bool>,
    tcp_keepalive: Option<Duration>,
}

impl ServerConfigBuilder {
    pub fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = Some(max);
        self
    }

    pub fn handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = Some(timeout);
        self
    }

    pub fn idle_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.idle_timeout = Some(timeout.unwrap_or(Duration::from_secs(0)));
        self
    }

    pub fn reject_when_full(mut self, reject: bool) -> Self {
        self.reject_when_full = Some(reject);
        self
    }

    pub fn tcp_keepalive(mut self, keepalive: Option<Duration>) -> Self {
        self.tcp_keepalive = Some(keepalive.unwrap_or(Duration::from_secs(0)));
        self
    }

    pub fn build(self) -> ServerConfig {
        let defaults = ServerConfig::default();
        ServerConfig {
            max_connections: self.max_connections.unwrap_or(defaults.max_connections),
            handshake_timeout: self.handshake_timeout.unwrap_or(defaults.handshake_timeout),
            idle_timeout: self.idle_timeout.or(defaults.idle_timeout),
            reject_when_full: self.reject_when_full.unwrap_or(defaults.reject_when_full),
            tcp_keepalive: self.tcp_keepalive.or(defaults.tcp_keepalive),
        }
    }
}
