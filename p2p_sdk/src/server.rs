//! WebSocket 服务端实现
//!
//! 提供 ws 和 wss 协议的 WebSocket 服务端

use crate::error::{P2PError, Result};
use crate::message::Message;
use crate::protocol::Protocol;
use crate::state::ConnectionState;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock, broadcast};
use tokio_tungstenite::{
    WebSocketStream, accept_async, tungstenite::protocol::Message as WsMessage,
};
use uuid::Uuid;

/// WebSocket 服务端配置
#[derive(Debug, Clone)]
pub struct WsServerConfig {
    /// 监听地址
    pub addr: SocketAddr,
    /// 协议类型
    pub protocol: Protocol,
    /// TLS 证书路径
    pub cert_path: Option<std::path::PathBuf>,
    /// TLS 私钥路径
    pub key_path: Option<std::path::PathBuf>,
    /// 详细日志
    pub verbose: bool,
}

impl Default for WsServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8080".parse().unwrap(),
            protocol: Protocol::Ws,
            cert_path: None,
            key_path: None,
            verbose: false,
        }
    }
}

/// 服务端会话信息
#[derive(Debug, Clone)]
pub struct ServerSession {
    /// 连接 ID
    pub connection_id: String,
    /// 节点 ID
    pub peer_id: String,
    /// 协议类型
    pub protocol: Protocol,
    /// 连接时间
    pub connected_at: std::time::Instant,
    /// 最后消息时间
    pub last_message_at: Option<std::time::Instant>,
}

/// WebSocket 服务端
pub struct WsServer {
    /// 配置
    config: WsServerConfig,
    /// 会话注册表
    sessions: Arc<RwLock<HashMap<String, ServerSession>>>,
    /// 当前状态
    state: Arc<Mutex<ConnectionState>>,
    /// 消息广播通道
    broadcast_tx: broadcast::Sender<Message>,
    /// 连接数统计
    connection_count: Arc<AtomicUsize>,
}

impl WsServer {
    /// 创建新的服务端
    pub fn new(config: WsServerConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1024);
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            broadcast_tx,
            connection_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// 获取连接计数句柄（用于状态监控）
    pub fn connection_count_handle(&self) -> Arc<AtomicUsize> {
        self.connection_count.clone()
    }

    /// 运行服务端
    pub async fn run(&self) -> Result<()> {
        // 验证配置
        if self.config.protocol == Protocol::Wss {
            if self.config.cert_path.is_none() || self.config.key_path.is_none() {
                return Err(P2PError::ConfigError(
                    "wss 模式需要指定证书和私钥路径".to_string(),
                ));
            }
        }

        // 创建 TLS 接受器（如果需要）
        let tls_acceptor = if self.config.protocol == Protocol::Wss {
            let cert_path = self.config.cert_path.as_ref().unwrap();
            let key_path = self.config.key_path.as_ref().unwrap();
            Some(crate::tls::create_server_tls_acceptor(cert_path, key_path)?)
        } else {
            None
        };

        // 绑定 TCP 监听器
        let listener = TcpListener::bind(&self.config.addr)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("绑定地址失败: {}", e)))?;

        let url = self.config.protocol.build_url(
            &self.config.addr.ip().to_string(),
            self.config.addr.port(),
            "ws",
        );

        println!("[INFO] P2P SDK WebSocket 服务端启动");
        println!("[INFO] 协议: {}", self.config.protocol);
        println!("[INFO] 监听地址: {}", url);
        println!("[INFO] 等待客户端连接...");

        // 设置状态为已连接
        *self.state.lock().await = ConnectionState::Connected;

        // 接受连接循环
        loop {
            let (stream, addr) = listener
                .accept()
                .await
                .map_err(|e| P2PError::ConnectionFailed(format!("接受连接失败: {}", e)))?;

            let tls_acceptor = tls_acceptor.clone();
            let sessions = self.sessions.clone();
            let broadcast_tx = self.broadcast_tx.clone();
            let connection_count = self.connection_count.clone();
            let verbose = self.config.verbose;
            let protocol = self.config.protocol;

            tokio::spawn(async move {
                if let Err(e) = handle_connection(
                    stream,
                    addr,
                    tls_acceptor,
                    sessions,
                    broadcast_tx,
                    connection_count,
                    verbose,
                    protocol,
                )
                .await
                {
                    if verbose {
                        eprintln!("[ERROR] 连接处理错误: {}", e);
                    }
                }
            });
        }
    }

    /// 获取当前连接数
    pub async fn connection_count(&self) -> usize {
        self.connection_count.load(Ordering::Relaxed)
    }

    /// 获取当前状态
    pub async fn state(&self) -> ConnectionState {
        self.state.lock().await.clone()
    }

    /// 订阅消息
    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.broadcast_tx.subscribe()
    }
}

/// 处理单个连接
async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    tls_acceptor: Option<tokio_rustls::TlsAcceptor>,
    sessions: Arc<RwLock<HashMap<String, ServerSession>>>,
    broadcast_tx: broadcast::Sender<Message>,
    connection_count: Arc<AtomicUsize>,
    verbose: bool,
    protocol: Protocol,
) -> Result<()> {
    // WebSocket 握手（区分 TLS 和 非 TLS）
    if let Some(acceptor) = tls_acceptor {
        // TLS 连接
        let tls_stream = acceptor
            .accept(stream)
            .await
            .map_err(|e| P2PError::Tls(format!("TLS 握手失败: {}", e)))?;
        handle_ws_connection(
            accept_async(tls_stream)
                .await
                .map_err(|e| P2PError::WebSocket(format!("WebSocket 握手失败: {}", e)))?,
            addr,
            sessions,
            broadcast_tx,
            connection_count,
            verbose,
            protocol,
        )
        .await
    } else {
        // 非 TLS 连接
        handle_ws_connection(
            accept_async(stream)
                .await
                .map_err(|e| P2PError::WebSocket(format!("WebSocket 握手失败: {}", e)))?,
            addr,
            sessions,
            broadcast_tx,
            connection_count,
            verbose,
            protocol,
        )
        .await
    }
}

/// 处理 WebSocket 连接（通用）
async fn handle_ws_connection<S>(
    ws_stream: WebSocketStream<S>,
    addr: SocketAddr,
    sessions: Arc<RwLock<HashMap<String, ServerSession>>>,
    broadcast_tx: broadcast::Sender<Message>,
    connection_count: Arc<AtomicUsize>,
    verbose: bool,
    protocol: Protocol,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // 生成节点 ID
    let peer_id = Uuid::new_v4().to_string();
    let connection_id = Uuid::new_v4().to_string();

    if verbose {
        println!("[INFO] 客户端连接: {} from {}", peer_id, addr);
    }

    // 创建会话
    let session = ServerSession {
        connection_id: connection_id.clone(),
        peer_id: peer_id.clone(),
        protocol,
        connected_at: std::time::Instant::now(),
        last_message_at: None,
    };

    // 注册会话
    {
        let mut sessions = sessions.write().await;
        sessions.insert(connection_id.clone(), session);
        connection_count.fetch_add(1, Ordering::Relaxed);
    }

    // 发送节点加入通知
    let join_msg = Message::PeerJoin {
        peer_id: peer_id.clone(),
        peer_addr: addr.to_string(),
    };
    let _ = broadcast_tx.send(join_msg);

    // 分离 WebSocket 流
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    // 消息循环
    while let Some(msg_result) = ws_stream.next().await {
        match msg_result {
            Ok(WsMessage::Text(text)) => {
                if verbose {
                    println!("[DEBUG] 收到消息: {}", text);
                }

                // 解析消息
                match serde_json::from_str::<Message>(&text) {
                    Ok(msg) => {
                        // 更新最后消息时间
                        {
                            let mut sessions = sessions.write().await;
                            if let Some(session) = sessions.get_mut(&connection_id) {
                                session.last_message_at = Some(std::time::Instant::now());
                            }
                        }

                        // 处理特殊消息
                        match &msg {
                            Message::PeerListRequest => {
                                // 构建节点列表
                                let peers: Vec<crate::message::PeerInfo> = {
                                    let sessions = sessions.read().await;
                                    sessions
                                        .values()
                                        .map(|s| crate::message::PeerInfo {
                                            peer_id: s.peer_id.clone(),
                                            peer_addr: String::new(), // 不暴露地址
                                        })
                                        .collect()
                                };

                                // 发送节点列表
                                let response = Message::PeerListReady { peers };
                                let json = serde_json::to_string(&response)
                                    .map_err(|e| P2PError::WebSocket(format!("序列化失败: {}", e)))?;
                                ws_sink
                                    .send(WsMessage::Text(json))
                                    .await
                                    .map_err(|e| P2PError::WebSocket(format!("发送失败: {}", e)))?;
                            }
                            Message::Chat { sender_id: _, content } => {
                                // 广播聊天消息（替换 sender_id 为实际 peer_id）
                                let chat_msg = Message::Chat {
                                    sender_id: peer_id.clone(),
                                    content: content.clone(),
                                };
                                let _ = broadcast_tx.send(chat_msg);
                            }
                            _ => {
                                // 广播其他消息
                                let _ = broadcast_tx.send(msg);
                            }
                        }
                    }
                    Err(e) => {
                        if verbose {
                            eprintln!("[WARN] 消息解析失败: {}", e);
                        }
                    }
                }
            }
            Ok(WsMessage::Ping(data)) => {
                ws_sink
                    .send(WsMessage::Pong(data))
                    .await
                    .map_err(|e| P2PError::WebSocket(format!("发送 Pong 失败: {}", e)))?;
            }
            Ok(WsMessage::Pong(_)) => {
                // 心跳响应，忽略
            }
            Ok(WsMessage::Close(_)) => {
                if verbose {
                    println!("[INFO] 客户端关闭连接: {}", peer_id);
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

    // 清理会话
    {
        let mut sessions = sessions.write().await;
        sessions.remove(&connection_id);
        connection_count.fetch_sub(1, Ordering::Relaxed);
    }

    // 发送离开通知
    let leave_msg = Message::PeerLeave {
        peer_id: peer_id.clone(),
    };
    let _ = broadcast_tx.send(leave_msg);

    if verbose {
        println!("[INFO] 客户端断开: {}", peer_id);
    }

    Ok(())
}

/// 运行 WebSocket 服务端（简化入口）
pub async fn run_ws_server(config: WsServerConfig) -> Result<()> {
    let server = WsServer::new(config);
    server.run().await
}
