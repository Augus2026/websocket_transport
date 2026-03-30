use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::protocol::Protocol;

pub mod defaults {
    pub const DEFAULT_TCP_ADDR: &str = "127.0.0.1:8080";
    pub const BROADCAST_CAPACITY: usize = 1000;
    pub const RELAY_CHANNEL_CAPACITY: usize = 100;
    pub const DISPLAY_CHANNEL_CAPACITY: usize = 100;
    pub const MAX_MESSAGE_SIZE: usize = 65536;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub tcp_addr: String,
    pub broadcast_capacity: usize,
    pub relay_channel_capacity: usize,
    pub max_message_size: usize,
    pub verbose: bool,
    // WebSocket 配置
    #[serde(default)]
    pub protocol: String,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            tcp_addr: defaults::DEFAULT_TCP_ADDR.to_string(),
            broadcast_capacity: defaults::BROADCAST_CAPACITY,
            relay_channel_capacity: defaults::RELAY_CHANNEL_CAPACITY,
            max_message_size: defaults::MAX_MESSAGE_SIZE,
            verbose: false,
            protocol: "ws".to_string(),
            cert_path: None,
            key_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub server_tcp_addr: String,
    pub display_channel_capacity: usize,
    pub max_message_size: usize,
    pub verbose: bool,
    pub auto_connect: bool,
    // WebSocket 配置
    #[serde(default)]
    pub protocol: String,
    pub ca_path: Option<String>,
    #[serde(default)]
    pub insecure: bool,
    // 重连配置
    #[serde(default = "default_reconnect_interval")]
    pub reconnect_interval: u64,
    #[serde(default = "default_reconnect_max")]
    pub reconnect_max: u64,
}

fn default_reconnect_interval() -> u64 { 1 }
fn default_reconnect_max() -> u64 { 30 }

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_tcp_addr: defaults::DEFAULT_TCP_ADDR.to_string(),
            display_channel_capacity: defaults::DISPLAY_CHANNEL_CAPACITY,
            max_message_size: defaults::MAX_MESSAGE_SIZE,
            verbose: false,
            auto_connect: true,
            protocol: "ws".to_string(),
            ca_path: None,
            insecure: false,
            reconnect_interval: default_reconnect_interval(),
            reconnect_max: default_reconnect_max(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub client: ClientConfig,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            client: ClientConfig::default(),
        }
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new(config_name: &str) -> Self {
        let config_dir = dirs::config_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tcp-p2p-server");

        fs::create_dir_all(&config_dir).ok();

        let config_path = config_dir.join(format!("{}.toml", config_name));
        Self { config_path }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn load(&self) -> ConfigFile {
        if !self.config_path.exists() {
            return ConfigFile::default();
        }

        match fs::read_to_string(&self.config_path) {
            Ok(content) => {
                match toml::from_str(&content) {
                    Ok(config) => config,
                    Err(e) => {
                        eprintln!("Config parse error: {}", e);
                        ConfigFile::default()
                    }
                }
            }
            Err(e) => {
                eprintln!("Config read error: {}", e);
                ConfigFile::default()
            }
        }
    }

    pub fn save(&self, config: &ConfigFile) -> std::io::Result<()> {
        let content = toml::to_string_pretty(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    pub fn update_server<F>(&self, updater: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut ServerConfig),
    {
        let mut config = self.load();
        updater(&mut config.server);
        self.save(&config)
    }

    pub fn update_client<F>(&self, updater: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut ClientConfig),
    {
        let mut config = self.load();
        updater(&mut config.client);
        self.save(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_server_config() {
        let config = ServerConfig::default();
        assert_eq!(config.tcp_addr, defaults::DEFAULT_TCP_ADDR);
    }

    #[test]
    fn test_default_client_config() {
        let config = ClientConfig::default();
        assert_eq!(config.server_tcp_addr, defaults::DEFAULT_TCP_ADDR);
        assert!(config.auto_connect);
    }
}

// ============================================================================
// WebSocket 协议配置
// ============================================================================

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
    pub fn validate(&self) -> crate::error::Result<()> {
        use crate::error::P2PError;

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

// ============================================================================
// 心跳配置
// ============================================================================

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
