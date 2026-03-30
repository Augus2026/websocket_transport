//! WebSocket 协议类型定义
//!
//! 定义支持的 WebSocket 协议类型：ws（非加密）和 wss（TLS 加密）

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// WebSocket 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// 非加密 WebSocket
    #[default]
    Ws,
    /// TLS 加密 WebSocket
    Wss,
}

impl Protocol {
    /// 返回协议的默认端口
    pub fn default_port(&self) -> u16 {
        match self {
            Protocol::Ws => 80,
            Protocol::Wss => 443,
        }
    }

    /// 返回协议的 URL 前缀
    pub fn scheme(&self) -> &'static str {
        match self {
            Protocol::Ws => "ws",
            Protocol::Wss => "wss",
        }
    }

    /// 是否为加密协议
    pub fn is_secure(&self) -> bool {
        matches!(self, Protocol::Wss)
    }

    /// 构建 WebSocket URL
    pub fn build_url(&self, host: &str, port: u16, path: &str) -> String {
        format!("{}://{}:{}/{}", self.scheme(), host, port, path)
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Ws => write!(f, "ws"),
            Protocol::Wss => write!(f, "wss"),
        }
    }
}

impl FromStr for Protocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ws" => Ok(Protocol::Ws),
            "wss" => Ok(Protocol::Wss),
            _ => Err(format!("无效的协议类型: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_default() {
        let protocol = Protocol::default();
        assert_eq!(protocol, Protocol::Ws);
    }

    #[test]
    fn test_protocol_default_port() {
        assert_eq!(Protocol::Ws.default_port(), 80);
        assert_eq!(Protocol::Wss.default_port(), 443);
    }

    #[test]
    fn test_protocol_is_secure() {
        assert!(!Protocol::Ws.is_secure());
        assert!(Protocol::Wss.is_secure());
    }

    #[test]
    fn test_protocol_from_str() {
        assert_eq!(Protocol::from_str("ws").unwrap(), Protocol::Ws);
        assert_eq!(Protocol::from_str("WSS").unwrap(), Protocol::Wss);
        assert!(Protocol::from_str("invalid").is_err());
    }

    #[test]
    fn test_protocol_build_url() {
        let url = Protocol::Ws.build_url("localhost", 8080, "ws");
        assert_eq!(url, "ws://localhost:8080/ws");

        let url = Protocol::Wss.build_url("example.com", 443, "ws");
        assert_eq!(url, "wss://example.com:443/ws");
    }
}
