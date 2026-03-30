use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
        let config_dir = dirs::config_local()
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
        let content = toml::to_string_pretty(config)?;
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
