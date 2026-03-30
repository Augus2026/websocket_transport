//! WebSocket 配置定义
//!
//! 定义服务端和客户端的 WebSocket 配置

use super::protocol::Protocol;
use crate::error::{P2PError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// WebSocket 协议配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsProtocolConfig {
    /// 协议类型
    #[serde(default)]
    pub protocol: Protocol,
    /// TLS 证书路径（wss 模式必需）
    pub cert_path: Option<PathBuf>,
    /// TLS 私钥路径（wss 模式必需）
    pub key_path: Option<PathBuf>,
    /// 自定义 CA 证书路径（客户端可选）
    pub ca_path: Option<PathBuf>,
    /// 跳过证书验证（仅开发模式）
    #[serde(default)]
    pub insecure: bool,
}

impl Default for WsProtocolConfig {
    fn default() -> Self {
        Self {
            protocol: Protocol::Ws,
            cert_path: None,
            key_path: None,
            ca_path: None,
            insecure: false,
        }
    }
}

impl WsProtocolConfig {
    /// 创建新的配置
    pub fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            ..Default::default()
        }
    }

    /// 设置 TLS 证书路径
    pub fn with_cert(mut self, path: impl Into<PathBuf>) -> Self {
        self.cert_path = Some(path.into());
        self
    }

    /// 设置 TLS 私钥路径
    pub fn with_key(mut self, path: impl Into<PathBuf>) -> Self {
        self.key_path = Some(path.into());
        self
    }

    /// 设置自定义 CA 证书路径
    pub fn with_ca(mut self, path: impl Into<PathBuf>) -> Self {
        self.ca_path = Some(path.into());
        self
    }

    /// 设置跳过证书验证
    pub fn with_insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }

    /// 验证配置是否有效
    pub fn validate(&self) -> Result<()> {
        // wss 模式需要证书和私钥
        if self.protocol == Protocol::Wss {
            if self.cert_path.is_none() {
                return Err(P2PError::ConfigError(
                    "wss 模式需要指定证书路径 (--cert)".to_string(),
                ));
            }
            if self.key_path.is_none() {
                return Err(P2PError::ConfigError(
                    "wss 模式需要指定私钥路径 (--key)".to_string(),
                ));
            }
        }

        // 跳过证书验证时发出警告
        if self.insecure {
            eprintln!("[警告] 已启用 --insecure 选项，将跳过证书验证（仅限开发环境）");
        }

        Ok(())
    }

    /// 是否为安全连接
    pub fn is_secure(&self) -> bool {
        self.protocol.is_secure()
    }
}

/// 重连配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectConfig {
    /// 首次重连等待时间（秒）
    #[serde(default = "default_initial_interval")]
    pub initial_interval: u64,

    /// 最大重连间隔（秒）
    #[serde(default = "default_max_interval")]
    pub max_interval: u64,

    /// 间隔倍增因子
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,

    /// 抖动因子（±百分比）
    #[serde(default = "default_jitter")]
    pub jitter: f64,

    /// 最大重试次数（None 表示无限）
    pub max_retries: Option<u32>,
}

fn default_initial_interval() -> u64 {
    1
}
fn default_max_interval() -> u64 {
    30
}
fn default_multiplier() -> f64 {
    2.0
}
fn default_jitter() -> f64 {
    0.25
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_interval: default_initial_interval(),
            max_interval: default_max_interval(),
            multiplier: default_multiplier(),
            jitter: default_jitter(),
            max_retries: None,
        }
    }
}

impl ReconnectConfig {
    /// 计算第 n 次重试的等待时间（秒）
    pub fn calculate_wait(&self, attempt: u32) -> u64 {
        use rand::Rng;

        // 指数退避
        let base_interval = self.initial_interval as f64 * self.multiplier.powi(attempt as i32 - 1);

        // 应用上限
        let capped = base_interval.min(self.max_interval as f64);

        // 添加抖动
        let jitter_range = capped * self.jitter;
        let jitter: f64 = rand::thread_rng().gen_range(-jitter_range..=jitter_range);

        let final_interval = (capped + jitter).max(0.0) as u64;
        final_interval.max(1) // 至少等待 1 秒
    }

    /// 检查是否应该继续重试
    pub fn should_retry(&self, attempt: u32) -> bool {
        match self.max_retries {
            Some(max) => attempt <= max,
            None => true,
        }
    }
}

/// 心跳配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    /// 心跳发送间隔（秒）
    #[serde(default = "default_heartbeat_interval")]
    pub interval: u64,

    /// 心跳响应超时（秒）
    #[serde(default = "default_heartbeat_timeout")]
    pub timeout: u64,

    /// 最大丢失心跳次数
    #[serde(default = "default_max_missed")]
    pub max_missed: u32,
}

fn default_heartbeat_interval() -> u64 {
    30
}
fn default_heartbeat_timeout() -> u64 {
    60
}
fn default_max_missed() -> u32 {
    3
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: default_heartbeat_interval(),
            timeout: default_heartbeat_timeout(),
            max_missed: default_max_missed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_config_default() {
        let config = WsProtocolConfig::default();
        assert_eq!(config.protocol, Protocol::Ws);
        assert!(!config.is_secure());
    }

    #[test]
    fn test_protocol_config_validation() {
        let config = WsProtocolConfig::new(Protocol::Wss);
        assert!(config.validate().is_err());

        let config = WsProtocolConfig::new(Protocol::Wss)
            .with_cert("cert.pem")
            .with_key("key.pem");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        assert_eq!(config.initial_interval, 1);
        assert_eq!(config.max_interval, 30);
    }

    #[test]
    fn test_reconnect_calculate_wait() {
        let config = ReconnectConfig::default();

        // 第一次重试应该在初始间隔附近
        let wait = config.calculate_wait(1);
        assert!(wait >= 1 && wait <= 2);

        // 第二次应该是第一次的 2 倍左右
        let wait2 = config.calculate_wait(2);
        assert!(wait2 >= 1 && wait2 <= 5);
    }

    #[test]
    fn test_reconnect_should_retry() {
        let config = ReconnectConfig::default();
        assert!(config.should_retry(100));

        let config = ReconnectConfig {
            max_retries: Some(3),
            ..Default::default()
        };
        assert!(config.should_retry(3));
        assert!(!config.should_retry(4));
    }

    #[test]
    fn test_heartbeat_config_default() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.interval, 30);
        assert_eq!(config.timeout, 60);
        assert_eq!(config.max_missed, 3);
    }
}
