//! P2P SDK CLI 入口
//!
//! 提供命令行接口启动服务端或客户端

use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;

/// P2P SDK - P2P Communication Server/Client
#[derive(Parser, Debug)]
#[command(name = "p2p_sdk")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

/// 运行模式
#[derive(Subcommand, Debug)]
enum Mode {
    /// 启动 P2P 服务端
    Server {
        /// 协议类型 (ws 或 wss)
        #[arg(long, default_value = "ws")]
        protocol: String,

        /// 监听主机
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// 监听端口
        #[arg(long, default_value_t = 8080)]
        port: u16,

        /// TLS 证书路径 (wss 必需)
        #[arg(long)]
        cert: Option<PathBuf>,

        /// TLS 私钥路径 (wss 必需)
        #[arg(long)]
        key: Option<PathBuf>,

        /// 详细日志
        #[arg(short, long)]
        verbose: bool,
    },

    /// 启动 P2P 客户端
    Client {
        /// 协议类型 (ws 或 wss)
        #[arg(long, default_value = "ws")]
        protocol: String,

        /// 服务端主机
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// 服务端端口
        #[arg(long, default_value_t = 8080)]
        port: u16,

        /// 自定义 CA 证书路径
        #[arg(long)]
        ca: Option<PathBuf>,

        /// 跳过证书验证 (仅开发)
        #[arg(long)]
        insecure: bool,

        /// 详细日志
        #[arg(short, long)]
        verbose: bool,
    },

    /// 显示服务端状态
    Status {
        /// 服务端主机
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// 服务端端口
        #[arg(long, default_value_t = 8080)]
        port: u16,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.mode {
        Mode::Server {
            protocol,
            host,
            port,
            cert,
            key,
            verbose,
        } => run_server(protocol, host, port, cert, key, verbose).await,
        Mode::Client {
            protocol,
            host,
            port,
            ca,
            insecure,
            verbose,
        } => run_client(protocol, host, port, ca, insecure, verbose).await,
        Mode::Status { host, port } => run_status(host, port).await,
    };

    if let Err(e) = result {
        eprintln!("[ERROR] {}", e);
        process::exit(1);
    }
}

/// 运行服务端
async fn run_server(
    protocol: String,
    host: String,
    port: u16,
    cert: Option<PathBuf>,
    key: Option<PathBuf>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use p2p_sdk::websocket::protocol::Protocol;
    use p2p_sdk::websocket::server::{WsServer, WsServerConfig};

    // 解析协议
    let protocol: Protocol = protocol
        .parse()
        .map_err(|e: String| format!("协议错误: {}", e))?;

    // 验证 wss 模式需要证书
    if protocol == Protocol::Wss {
        if cert.is_none() || key.is_none() {
            return Err("wss 模式需要 --cert 和 --key 参数".into());
        }
    }

    // 解析地址
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| format!("无效的地址: {}", e))?;

    // 创建配置
    let config = WsServerConfig {
        addr,
        protocol,
        cert_path: cert,
        key_path: key,
        verbose,
    };

    // 运行服务端
    let server = WsServer::new(config);

    // 启动状态更新任务
    let status_file = get_status_file_path(port);
    let status_file_clone = status_file.clone();
    let connection_count = server.connection_count_handle();
    let host_for_status = host.clone();
    let protocol_str = protocol.to_string();
    tokio::spawn(async move {
        use std::io::Write;
        loop {
            let status = serde_json::json!({
                "host": host_for_status,
                "port": port,
                "protocol": protocol_str,
                "connections": connection_count.load(std::sync::atomic::Ordering::Relaxed),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            if let Ok(content) = serde_json::to_string_pretty(&status) {
                if let Ok(mut file) = std::fs::File::create(&status_file_clone) {
                    let _ = file.write_all(content.as_bytes());
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    server.run().await?;

    // 清理状态文件
    let _ = std::fs::remove_file(&status_file);

    Ok(())
}

/// 获取状态文件路径
fn get_status_file_path(port: u16) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("p2p_sdk_status_{}.json", port))
}

/// 运行状态查询
async fn run_status(_host: String, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let status_file = get_status_file_path(port);

    if !status_file.exists() {
        println!("[状态] 服务端未运行或状态文件不存在");
        println!("[提示] 请先启动服务端: p2p_sdk server --port {}", port);
        return Ok(());
    }

    let content =
        std::fs::read_to_string(&status_file).map_err(|e| format!("读取状态文件失败: {}", e))?;

    let status: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("解析状态文件失败: {}", e))?;

    println!("=== P2P SDK 服务端状态 ===");
    println!("主机: {}", status["host"].as_str().unwrap_or("N/A"));
    println!("端口: {}", status["port"].as_u64().unwrap_or(0));
    println!("协议: {}", status["protocol"].as_str().unwrap_or("N/A"));
    println!(
        "当前连接数: {}",
        status["connections"].as_u64().unwrap_or(0)
    );
    println!(
        "更新时间: {}",
        status["timestamp"].as_str().unwrap_or("N/A")
    );

    Ok(())
}

/// 运行客户端
async fn run_client(
    protocol: String,
    host: String,
    port: u16,
    ca: Option<PathBuf>,
    insecure: bool,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use p2p_sdk::websocket::client::{run_ws_client, WsClientConfig};
    use p2p_sdk::websocket::config::{HeartbeatConfig, ReconnectConfig};
    use p2p_sdk::websocket::protocol::Protocol;

    // 解析协议
    let protocol: Protocol = protocol
        .parse()
        .map_err(|e: String| format!("协议错误: {}", e))?;

    // 创建配置
    let config = WsClientConfig {
        server_addr: host,
        server_port: port,
        protocol,
        ca_path: ca,
        insecure,
        reconnect: ReconnectConfig {
            max_retries: None, // 无限重连
            ..Default::default()
        },
        heartbeat: HeartbeatConfig::default(),
        verbose,
    };

    // 运行客户端（带交互功能）
    run_ws_client(config).await?;

    Ok(())
}
