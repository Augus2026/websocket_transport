//! P2P SDK - P2P 通信库
//!
//! 提供 WebSocket 服务端和客户端功能

pub mod error;
pub mod message;
pub mod config;
pub mod protocol;
pub mod state;
pub mod reconnect;
pub mod server;
pub mod client;
pub mod tls;

// 错误处理
pub use error::{P2PError, Result};

// 消息类型
pub use message::{Message, PeerInfo, parse_message, serialize_message};

// 协议类型
pub use protocol::Protocol;

// 状态类型
pub use state::ConnectionState;

// 重连工具
pub use reconnect::{ReconnectConfig, calculate_wait_time, should_retry};

// 配置类型
pub use config::{
    ServerConfig, ClientConfig, ConfigManager, ConfigFile,
    WsProtocolConfig, HeartbeatConfig,
};

// 服务端类型
pub use server::{WsServer, WsServerConfig, ServerSession, run_ws_server};

// 客户端类型
pub use client::{WsClient, WsClientConfig, run_ws_client};

// TLS 工具
pub use tls::{
    load_certs, load_private_key,
    create_server_tls_acceptor,
    create_client_tls_connector,
    create_client_tls_connector_with_ca,
};

/// 配置常量
pub mod constants {
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
    fn test_config_constants() {
        assert!(!constants::DEFAULT_TCP_ADDR.is_empty());
        assert!(constants::BROADCAST_CAPACITY > 0);
    }
}
