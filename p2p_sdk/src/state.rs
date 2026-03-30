//! 连接状态定义
//!
//! 定义 WebSocket 连接的状态枚举

use serde::{Deserialize, Serialize};
use std::fmt;

/// 连接状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// 未连接
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 重连中
    Reconnecting {
        /// 当前重试次数
        attempt: u32,
        /// 等待时间（秒）
        wait_seconds: u64,
    },
    /// 错误状态
    Error {
        /// 错误信息
        message: String,
    },
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState::Disconnected
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "未连接"),
            ConnectionState::Connecting => write!(f, "连接中"),
            ConnectionState::Connected => write!(f, "已连接"),
            ConnectionState::Reconnecting {
                attempt,
                wait_seconds,
            } => {
                write!(
                    f,
                    "重连中（第 {} 次尝试，等待 {} 秒）",
                    attempt, wait_seconds
                )
            }
            ConnectionState::Error { message } => write!(f, "错误: {}", message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_default() {
        let state = ConnectionState::default();
        assert_eq!(state, ConnectionState::Disconnected);
    }

    #[test]
    fn test_state_display() {
        assert_eq!(ConnectionState::Disconnected.to_string(), "未连接");
        assert_eq!(ConnectionState::Connecting.to_string(), "连接中");
        assert_eq!(ConnectionState::Connected.to_string(), "已连接");
    }

    #[test]
    fn test_state_serialization() {
        let state = ConnectionState::Connected;
        let json = serde_json::to_string(&state).unwrap();
        let decoded: ConnectionState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, state);
    }
}
