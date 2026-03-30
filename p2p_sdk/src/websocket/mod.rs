//! WebSocket 模块
//!
//! 提供 WebSocket 通信功能，支持 ws 和 wss 协议

pub mod client;
pub mod config;
pub mod protocol;
pub mod reconnect;
pub mod server;
pub mod state;
pub mod tls;

pub use client::{WsClient, WsClientConfig};
pub use config::{HeartbeatConfig, ReconnectConfig, WsProtocolConfig};
pub use protocol::Protocol;
pub use reconnect::{ReconnectAttempt, ReconnectState};
pub use server::{ServerSession, WsServer, WsServerConfig};
pub use state::{ConnectionState, StateChangeEvent, StateEmitter};
