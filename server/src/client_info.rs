use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;

pub type ClientId = u64;

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub id: ClientId,

    pub addr: SocketAddr,

    pub current_dir: PathBuf,

    pub connected_at: Instant,

    pub last_activity: Instant,
}

impl ClientInfo {
    pub fn new(id: ClientId, addr: SocketAddr) -> Self {
        Self {
            id,
            addr,
            current_dir: PathBuf::from("/"),
            connected_at: Instant::now(),
            last_activity: Instant::now(),
        }
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn idle_duration(&self) -> std::time::Duration {
        self.last_activity.elapsed()
    }

    pub fn connection_duration(&self) -> std::time::Duration {
        self.connected_at.elapsed()
    }
}
