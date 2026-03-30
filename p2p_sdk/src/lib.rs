pub mod client;
pub mod error;
pub mod message;
pub mod network;
pub mod registry;
pub mod server;
pub mod websocket;

pub use error::{P2PError, Result};
pub use message::{Message, PeerInfo};
pub use registry::{PeerConnection, PeerRegistry, RelaySession, RelaySessionRegistry, RelayState};

// 重新导出 WebSocket 相关类型
pub use websocket::{
    ConnectionState, HeartbeatConfig, Protocol, ReconnectConfig, StateEmitter, WsProtocolConfig,
};

#[derive(Debug, Clone)]
pub struct RelayTask {
    pub from_peer: String,
    pub to_peer: String,
}

pub mod config {
    pub const DEFAULT_TCP_ADDR: &str = "127.0.0.1:8080";
    pub const BROADCAST_CAPACITY: usize = 1000;
    pub const RELAY_CHANNEL_CAPACITY: usize = 100;
    pub const DISPLAY_CHANNEL_CAPACITY: usize = 100;
    pub const MAX_MESSAGE_SIZE: usize = 65536;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_task_creation() {
        let task = RelayTask {
            from_peer: "peer-a".to_string(),
            to_peer: "peer-b".to_string(),
        };
        assert_eq!(task.from_peer, "peer-a");
        assert_eq!(task.to_peer, "peer-b");
    }

    #[test]
    fn test_config_constants() {
        assert!(!config::DEFAULT_TCP_ADDR.is_empty());
        assert!(config::BROADCAST_CAPACITY > 0);
    }
}
