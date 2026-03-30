//! WebSocket 客户端实现
//!
//! 提供 ws 和 wss 协议的 WebSocket 客户端

use crate::error::{P2PError, Result};
use crate::message::Message;
use crate::websocket::config::{HeartbeatConfig, ReconnectConfig};
use crate::websocket::protocol::Protocol;
use crate::websocket::reconnect::ReconnectState;
use crate::websocket::state::{ConnectionState, StateEmitter};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering as AtomicOrdering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

/// WebSocket 客户端配置
#[derive(Debug, Clone)]
pub struct WsClientConfig {
    /// 服务端地址
    pub server_addr: String,
    /// 服务端端口
    pub server_port: u16,
    /// 协议类型
    pub protocol: Protocol,
    /// 自定义 CA 路径
    pub ca_path: Option<std::path::PathBuf>,
    /// 跳过证书验证
    pub insecure: bool,
    /// 重连配置
    pub reconnect: ReconnectConfig,
    /// 心跳配置
    pub heartbeat: HeartbeatConfig,
    /// 详细日志
    pub verbose: bool,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1".to_string(),
            server_port: 8080,
            protocol: Protocol::Ws,
            ca_path: None,
            insecure: false,
            reconnect: ReconnectConfig::default(),
            heartbeat: HeartbeatConfig::default(),
            verbose: false,
        }
    }
}

impl WsClientConfig {
    /// 构建 WebSocket URL
    pub fn build_url(&self, path: &str) -> String {
        self.protocol
            .build_url(&self.server_addr, self.server_port, path)
    }
}

/// WebSocket 客户端
pub struct WsClient {
    /// 配置
    pub config: WsClientConfig,
    /// 状态发射器
    state: StateEmitter,
    /// 重连状态
    reconnect_state: ReconnectState,
    /// 消息发送通道（外部使用）
    input_tx: broadcast::Sender<WsMessage>,
    /// 接收消息广播
    message_tx: broadcast::Sender<Message>,
    /// 连接状态标志
    connected: Arc<AtomicBool>,
}

impl WsClient {
    /// 创建新的客户端
    pub fn new(config: WsClientConfig) -> Self {
        let (message_tx, _) = broadcast::channel(1024);
        let (input_tx, _) = broadcast::channel(256);
        let reconnect_state = ReconnectState::new(config.reconnect.clone());
        let connected = Arc::new(AtomicBool::new(false));
        Self {
            config,
            state: StateEmitter::new(),
            reconnect_state,
            input_tx,
            message_tx,
            connected,
        }
    }

    /// 获取输入发送器（用于发送消息）
    pub fn input_sender(&self) -> broadcast::Sender<WsMessage> {
        self.input_tx.clone()
    }

    /// 连接到服务端
    pub async fn connect(&mut self) -> Result<()> {
        self.state.set_connecting();
        let url = self.config.build_url("ws");

        if self.config.verbose {
            println!("[状态] 连接中 -> {}", url);
        }

        // 简化连接：直接使用 connect_async
        let (ws_stream, _) = connect_async(&url).await.map_err(|e| {
            self.state.set_error(format!("连接失败: {}", e));
            P2PError::ConnectionFailed(format!("连接失败: {}", e))
        })?;

        if self.config.verbose {
            println!("[状态] 已连接 -> {}", url);
            if self.config.protocol == Protocol::Wss {
                println!("[INFO] TLS 加密已启用");
            }
        }

        self.state.set_connected();
        self.reconnect_state.reset();
        self.connected.store(true, AtomicOrdering::Relaxed);

        // 启动消息循环
        let (mut ws_sink, mut ws_stream) = ws_stream.split();
        let message_tx = self.message_tx.clone();
        let verbose = self.config.verbose;
        let heartbeat_config = self.config.heartbeat.clone();

        // 心跳跟踪
        let missed_heartbeats = Arc::new(AtomicU32::new(0));
        let missed_heartbeats_clone = missed_heartbeats.clone();

        // 外部输入通道
        let mut input_rx = self.input_tx.subscribe();

        // 内部发送通道（用于心跳）
        let (internal_tx, mut internal_rx) = mpsc::channel::<WsMessage>(32);

        // 发送任务
        let send_task = async move {
            loop {
                tokio::select! {
                    // 内部消息（心跳等）
                    msg = internal_rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if let Err(e) = ws_sink.send(msg).await {
                                    if verbose {
                                        eprintln!("[ERROR] 发送消息失败: {}", e);
                                    }
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    // 外部输入
                    msg = input_rx.recv() => {
                        match msg {
                            Ok(msg) => {
                                if let Err(e) = ws_sink.send(msg).await {
                                    if verbose {
                                        eprintln!("[ERROR] 发送消息失败: {}", e);
                                    }
                                    break;
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                }
            }
        };

        // 心跳任务
        let heartbeat_task = async move {
            let interval = Duration::from_secs(heartbeat_config.interval);
            let max_missed = heartbeat_config.max_missed;

            loop {
                tokio::time::sleep(interval).await;

                // 发送 Ping
                let ping = WsMessage::Ping(vec![]);
                if internal_tx.send(ping).await.is_err() {
                    break; // 连接已断开
                }

                // 检查是否超过最大丢失次数
                let missed =
                    missed_heartbeats.fetch_add(1, AtomicOrdering::Relaxed) + 1;
                if missed >= max_missed {
                    if verbose {
                        eprintln!("[WARN] 心跳超时，连续 {} 次未收到响应", missed);
                    }
                    break; // 触发断开
                }

                if verbose {
                    println!("[心跳] 发送 Ping (未响应: {})", missed);
                }
            }
        };

        // 接收任务
        let recv_task = async move {
            while let Some(msg_result) = ws_stream.next().await {
                match msg_result {
                    Ok(WsMessage::Text(text)) => {
                        if verbose {
                            println!("[DEBUG] 收到消息: {}", text);
                        }
                        if let Ok(msg) = serde_json::from_str::<Message>(&text) {
                            let _ = message_tx.send(msg);
                        }
                    }
                    Ok(WsMessage::Ping(data)) => {
                        let _ = data;
                    }
                    Ok(WsMessage::Pong(_)) => {
                        missed_heartbeats_clone.store(0, AtomicOrdering::Relaxed);
                        if verbose {
                            println!("[心跳] 收到 Pong 响应");
                        }
                    }
                    Ok(WsMessage::Close(_)) => {
                        if verbose {
                            println!("[INFO] 服务端关闭连接");
                        }
                        break;
                    }
                    Err(e) => {
                        if verbose {
                            eprintln!("[ERROR] WebSocket 错误: {}", e);
                        }
                        break;
                    }
                    _ => {}
                }
            }
        };

        // 并行运行任务
        tokio::select! {
            _ = send_task => {}
            _ = recv_task => {}
            _ = heartbeat_task => {}
        }

        // 连接断开
        self.state.set_disconnected();
        self.connected.store(false, AtomicOrdering::Relaxed);

        if self.config.verbose {
            println!("[状态] 断开 -> 原因: 连接关闭");
        }

        Ok(())
    }

    /// 订阅消息
    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.message_tx.subscribe()
    }

    /// 获取当前状态
    pub fn state(&self) -> &ConnectionState {
        self.state.current()
    }

    /// 是否已连接
    pub fn is_connected(&self) -> bool {
        self.connected.load(AtomicOrdering::Relaxed)
    }
}

/// 运行 WebSocket 客户端（带交互功能）
pub async fn run_ws_client(config: WsClientConfig) -> Result<()> {
    let mut client = WsClient::new(config.clone());

    // 启动消息打印任务
    let msg_rx = client.subscribe();
    tokio::spawn(async move {
        let mut rx = msg_rx;
        while let Ok(msg) = rx.recv().await {
            match &msg {
                Message::PeerJoin { peer_id, .. } => {
                    println!("[系统] 节点加入: {}", peer_id);
                }
                Message::PeerLeave { peer_id } => {
                    println!("[系统] 节点离开: {}", peer_id);
                }
                Message::Chat { sender_id, content } => {
                    println!("[聊天] {}: {}", sender_id, content);
                }
                Message::PeerListReady { peers } => {
                    println!("[节点列表] 当前在线 {} 个节点:", peers.len());
                    for peer in peers {
                        println!("  - {}", peer.peer_id);
                    }
                }
                Message::PrivateMessage { from_peer, content, .. } => {
                    println!("[私聊] {}: {}", from_peer, content);
                }
                _ => {
                    println!("[消息] {:?}", msg);
                }
            }
        }
    });

    // 获取发送通道
    let input_tx = client.input_sender();

    // 创建标准输入通道
    let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);

    // 在单独的线程中读取标准输入
    std::thread::spawn(move || {
        use std::io::{self, BufRead};
        let stdin = io::stdin();
        println!("[系统] 可用命令: /peers, /msg <peer_id> <message>, /quit");
        println!("[系统] 直接输入文本发送广播消息");
        for line in stdin.lock().lines() {
            if let Ok(line) = line {
                if stdin_tx.blocking_send(line).is_err() {
                    break;
                }
            }
        }
    });

    // 启动输入处理任务
    tokio::spawn(async move {
        while let Some(line) = stdin_rx.recv().await {
            if line == "/quit" {
                let _ = input_tx.send(WsMessage::Close(None));
                break;
            } else if line == "/peers" {
                let msg = Message::PeerListRequest;
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = input_tx.send(WsMessage::Text(json));
                }
            } else if line.starts_with("/msg ") {
                let parts: Vec<&str> = line[5..].splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let msg = Message::PrivateMessage {
                        from_peer: "local".to_string(),
                        to_peer: parts[0].to_string(),
                        content: parts[1].to_string(),
                    };
                    if let Ok(json) = serde_json::to_string(&msg) {
                        let _ = input_tx.send(WsMessage::Text(json));
                    }
                } else {
                    println!("[错误] 用法: /msg <peer_id> <message>");
                }
            } else if !line.is_empty() {
                let msg = Message::Chat {
                    sender_id: "local".to_string(),
                    content: line,
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = input_tx.send(WsMessage::Text(json));
                }
            }
        }
    });

    // 连接（带重连）
    loop {
        println!("[系统] 正在连接...");

        match client.connect().await {
            Ok(()) => {
                if config.verbose {
                    println!("[系统] 连接正常断开");
                }
            }
            Err(e) => {
                if config.verbose {
                    eprintln!("[错误] 连接失败: {}", e);
                }
            }
        }

        // 等待重连
        println!("[系统] 1秒后重连...");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
