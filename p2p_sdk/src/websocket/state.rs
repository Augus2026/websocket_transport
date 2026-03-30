//! 连接状态定义
//!
//! 定义 WebSocket 连接的状态机和状态转换

use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::sync::broadcast;

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

/// 状态变更事件
#[derive(Debug, Clone)]
pub struct StateChangeEvent {
    /// 旧状态
    pub from: ConnectionState,
    /// 新状态
    pub to: ConnectionState,
    /// 时间戳
    pub timestamp: std::time::Instant,
}

/// 状态发射器
#[derive(Clone)]
pub struct StateEmitter {
    /// 状态变更广播发送器
    sender: broadcast::Sender<StateChangeEvent>,
    /// 当前状态
    current_state: ConnectionState,
}

impl Default for StateEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl StateEmitter {
    /// 创建新的状态发射器
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(16);
        Self {
            sender,
            current_state: ConnectionState::Disconnected,
        }
    }

    /// 获取当前状态
    pub fn current(&self) -> &ConnectionState {
        &self.current_state
    }

    /// 订阅状态变更事件
    pub fn subscribe(&self) -> broadcast::Receiver<StateChangeEvent> {
        self.sender.subscribe()
    }

    /// 转换到新状态
    pub fn transition(&mut self, new_state: ConnectionState) {
        let event = StateChangeEvent {
            from: self.current_state.clone(),
            to: new_state.clone(),
            timestamp: std::time::Instant::now(),
        };

        self.current_state = new_state;

        // 忽略发送错误（没有订阅者时）
        let _ = self.sender.send(event);
    }

    /// 设置为连接中
    pub fn set_connecting(&mut self) {
        self.transition(ConnectionState::Connecting);
    }

    /// 设置为已连接
    pub fn set_connected(&mut self) {
        self.transition(ConnectionState::Connected);
    }

    /// 设置为断开
    pub fn set_disconnected(&mut self) {
        self.transition(ConnectionState::Disconnected);
    }

    /// 设置为重连中
    pub fn set_reconnecting(&mut self, attempt: u32, wait_seconds: u64) {
        self.transition(ConnectionState::Reconnecting {
            attempt,
            wait_seconds,
        });
    }

    /// 设置为错误
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.transition(ConnectionState::Error {
            message: message.into(),
        });
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
    fn test_state_emitter_transitions() {
        let mut emitter = StateEmitter::new();
        assert_eq!(emitter.current(), &ConnectionState::Disconnected);

        emitter.set_connecting();
        assert_eq!(emitter.current(), &ConnectionState::Connecting);

        emitter.set_connected();
        assert_eq!(emitter.current(), &ConnectionState::Connected);

        emitter.set_disconnected();
        assert_eq!(emitter.current(), &ConnectionState::Disconnected);
    }

    #[test]
    fn test_state_emitter_error() {
        let mut emitter = StateEmitter::new();
        emitter.set_error("测试错误");
        assert!(matches!(emitter.current(), ConnectionState::Error { .. }));
    }
}
