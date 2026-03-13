use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::net::TcpSocket;
use tracing::{debug, error, info, warn};
use tracing_subscriber;

// 服务器配置
const SERVER_ADDR: &str = "0.0.0.0:9090";
const STORAGE_DIR: &str = "./uploads";

// TCP 缓冲区配置 (单位: 字节)
const TCP_RECV_BUFFER_SIZE: u32 = 12 * 1024 * 1024; // 12MB 接收缓冲区
const TCP_SEND_BUFFER_SIZE: u32 = 12 * 1024 * 1024; // 12MB 发送缓冲区

// 文件块大小配置 (单位: 字节)
const DEFAULT_CHUNK_SIZE: usize = 512 * 1024; // 512KB 块大小

// 文件上传状态
#[derive(Clone)]
struct UploadStatus {
    filename: String,
    file_id: String,
    total_chunks: usize,
    received_chunks: usize,
    total_size: u64,
    received_size: u64,
    chunks: HashMap<usize, Vec<u8>>,
}

// 文件下载状态
#[derive(Clone)]
struct DownloadStatus {
    filename: String,
    file_id: String,
    total_chunks: usize,
    sent_chunks: usize,
    chunk_size: usize,
}

// WebSocket 消息类型
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "op")]
enum ClientMessage {
    #[serde(rename = "upload_start")]
    UploadStart {
        filename: String,
        size: u64,
        chunks: usize,
        file_id: String,
    },
    #[serde(rename = "upload_chunk")]
    UploadChunk {
        filename: String,
        file_id: String,
        index: usize,
        total_chunks: usize,
        size: usize,
    },
    #[serde(rename = "upload_end")]
    UploadEnd {
        filename: String,
        file_id: String,
    },
    #[serde(rename = "upload_cancel")]
    UploadCancel {
        file_id: String,
    },
    #[serde(rename = "download_request")]
    DownloadRequest {
        filename: String,
        file_id: String,
    },
    #[serde(rename = "download_cancel")]
    DownloadCancel {
        file_id: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "op")]
enum ServerMessage {
    #[serde(rename = "upload_started")]
    UploadStarted {
        file_id: String,
    },
    #[serde(rename = "upload_complete")]
    UploadComplete {
        file_id: String,
        filename: String,
    },
    #[serde(rename = "upload_error")]
    UploadError {
        file_id: String,
        error: String,
    },
    #[serde(rename = "download_start")]
    DownloadStart {
        filename: String,
        size: u64,
        chunks: usize,
        file_id: String,
    },
    #[serde(rename = "download_chunk")]
    DownloadChunk {
        file_id: String,
        index: usize,
        total_chunks: usize,
    },
    #[serde(rename = "download_end")]
    DownloadEnd {
        file_id: String,
    },
    #[serde(rename = "download_error")]
    DownloadError {
        file_id: String,
        error: String,
    },
    #[serde(rename = "error")]
    Error {
        error: String,
    },
}

// 共享状态
#[derive(Clone)]
struct AppState {
    active_uploads: Arc<Mutex<HashMap<String, UploadStatus>>>,
    active_downloads: Arc<Mutex<HashMap<String, DownloadStatus>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            active_uploads: Arc::new(Mutex::new(HashMap::new())),
            active_downloads: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// 处理 WebSocket 连接
async fn handle_socket(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_connection(socket, state))
}

async fn handle_connection(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // 处理接收到的消息
    while let Some(result) = receiver.next().await {
        match result {
            Ok(msg) => {
                if let Err(e) = handle_message(msg, &state, &mut sender).await {
                    error!("处理消息错误: {}", e);
                }
            }
            Err(e) => {
                warn!("WebSocket 接收错误: {}", e);
                break;
            }
        }
    }
}

// 处理客户端消息
async fn handle_message(
    msg: Message,
    state: &AppState,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> Result<()> {
    match msg {
        Message::Text(text) => {
            // 尝试解析为 JSON 消息
            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                match client_msg {
                    ClientMessage::UploadStart {
                        filename,
                        size,
                        chunks,
                        file_id,
                    } => {
                        info!("开始上传: {} (大小: {}, 块数: {})", filename, size, chunks);

                        // 创建上传状态
                        let upload_status = UploadStatus {
                            filename: filename.clone(),
                            file_id: file_id.clone(),
                            total_chunks: chunks,
                            received_chunks: 0,
                            total_size: size,
                            received_size: 0,
                            chunks: HashMap::new(),
                        };

                        state
                            .active_uploads
                            .lock()
                            .unwrap()
                            .insert(file_id.clone(), upload_status);

                        // 发送确认消息
                        let response = ServerMessage::UploadStarted {
                            file_id: file_id.clone(),
                        };
                        sender
                            .send(Message::Text(serde_json::to_string(&response)?))
                            .await?;
                    }

                    ClientMessage::UploadChunk {
                        filename,
                        file_id: _,
                        index,
                        total_chunks: _,
                        size,
                    } => {
                        // 等待下一个消息（二进制数据）
                        debug!("接收文件块: {} (索引: {}, 大小: {})", filename, index, size);
                    }

                    ClientMessage::UploadEnd { filename, file_id } => {
                        info!("上传结束: {} ({})", filename, file_id);

                        // 保存文件
                        let result = save_file(&file_id, &state).await;

                        if let Err(e) = result {
                            error!("保存文件失败: {}", e);
                            let response = ServerMessage::UploadError {
                                file_id: file_id.clone(),
                                error: e.to_string(),
                            };
                            sender
                                .send(Message::Text(serde_json::to_string(&response)?))
                                .await?;
                        } else {
                            info!("文件保存成功: {}", filename);
                            let response = ServerMessage::UploadComplete {
                                file_id: file_id.clone(),
                                filename: filename.clone(),
                            };
                            sender
                                .send(Message::Text(serde_json::to_string(&response)?))
                                .await?;

                            // 清理上传状态
                            state.active_uploads.lock().unwrap().remove(&file_id);
                        }
                    }

                    ClientMessage::UploadCancel { file_id } => {
                        info!("取消上传: {}", file_id);

                        let response = ServerMessage::UploadError {
                            file_id: file_id.clone(),
                            error: "上传已取消".to_string(),
                        };
                        sender
                            .send(Message::Text(serde_json::to_string(&response)?))
                            .await?;

                        // 清理上传状态
                        state.active_uploads.lock().unwrap().remove(&file_id);
                    }

                    ClientMessage::DownloadRequest { filename, file_id } => {
                        info!("下载请求: {} ({})", filename, file_id);

                        // 检查文件是否存在
                        let file_path = PathBuf::from(STORAGE_DIR).join(&filename);

                        if !file_path.exists() {
                            warn!("文件不存在: {}", filename);
                            let response = ServerMessage::DownloadError {
                                file_id: file_id.clone(),
                                error: "文件不存在".to_string(),
                            };
                            sender
                                .send(Message::Text(serde_json::to_string(&response)?))
                                .await?;
                        } else {
                            // 读取文件
                            match tokio::fs::read(&file_path).await {
                                Ok(data) => {
                                    let file_size = data.len() as u64;
                                    let chunk_size = 64 * 1024; // 64KB
                                    let total_chunks = (data.len() + chunk_size - 1) / chunk_size;

                                    // 创建下载状态
                                    let download_status = DownloadStatus {
                                        filename: filename.clone(),
                                        file_id: file_id.clone(),
                                        total_chunks,
                                        sent_chunks: 0,
                                        chunk_size,
                                    };

                                    state
                                        .active_downloads
                                        .lock()
                                        .unwrap()
                                        .insert(file_id.clone(), download_status);

                                    // 发送下载开始消息
                                    let response = ServerMessage::DownloadStart {
                                        filename: filename.clone(),
                                        size: file_size,
                                        chunks: total_chunks,
                                        file_id: file_id.clone(),
                                    };
                                    sender
                                        .send(Message::Text(serde_json::to_string(&response)?))
                                        .await?;

                                    // 发送文件块
                                    send_file_chunks(&data, chunk_size, &file_id, sender).await?;

                                    // 发送下载结束消息
                                    let response = ServerMessage::DownloadEnd {
                                        file_id: file_id.clone(),
                                    };
                                    sender
                                        .send(Message::Text(serde_json::to_string(&response)?))
                                        .await?;

                                    // 清理下载状态
                                    state.active_downloads.lock().unwrap().remove(&file_id);

                                    info!("下载完成: {}", filename);
                                }
                                Err(e) => {
                                    error!("读取文件失败: {}", e);
                                    let response = ServerMessage::DownloadError {
                                        file_id: file_id.clone(),
                                        error: e.to_string(),
                                    };
                                    sender
                                        .send(Message::Text(serde_json::to_string(&response)?))
                                        .await?;
                                }
                            }
                        }
                    }

                    ClientMessage::DownloadCancel {
                        file_id,
                    } => {
                        info!("取消下载: {}", file_id);

                        let response = ServerMessage::DownloadError {
                            file_id: file_id.clone(),
                            error: "下载已取消".to_string(),
                        };
                        sender
                            .send(Message::Text(serde_json::to_string(&response)?))
                            .await?;

                        // 清理下载状态
                        state.active_downloads.lock().unwrap().remove(&file_id);
                    }
                }
            } else {
                warn!("无法解析消息: {}", text);
            }
        }

        Message::Binary(data) => {
            // 处理二进制数据（文件块）
            // 查找最近的上传请求
            let file_id = {
                let uploads = state.active_uploads.lock().unwrap();
                if let Some((file_id, _)) = uploads.iter().next() {
                    Some(file_id.clone())
                } else {
                    None
                }
            };

            if let Some(file_id) = file_id {
                // 更新上传状态
                let mut uploads = state.active_uploads.lock().unwrap();
                if let Some(upload) = uploads.get_mut(&file_id) {
                    // 找到当前块索引
                    let index = upload.received_chunks;
                    upload.chunks.insert(index, data.to_vec());
                    upload.received_chunks += 1;
                    upload.received_size += data.len() as u64;
                    debug!(
                        "接收块: {}/{} (大小: {})",
                        upload.received_chunks,
                        upload.total_chunks,
                        data.len()
                    );
                }
            }
        }

        Message::Close(_) => {
            info!("客户端断开连接");
            return Ok(());
        }

        _ => {}
    }

    Ok(())
}

// 保存文件
async fn save_file(file_id: &str, state: &AppState) -> Result<()> {
    let upload = {
        let uploads = state.active_uploads.lock().unwrap();
        uploads.get(file_id).cloned()
    };

    if let Some(upload) = upload {
        // 创建存储目录
        tokio::fs::create_dir_all(STORAGE_DIR).await?;

        // 收集所有块
        let mut file_data = Vec::with_capacity(upload.total_size as usize);
        for i in 0..upload.total_chunks {
            if let Some(chunk) = upload.chunks.get(&i) {
                file_data.extend_from_slice(chunk);
            } else {
                return Err(anyhow::anyhow!("缺少块: {}", i));
            }
        }

        // 写入文件
        let file_path = PathBuf::from(STORAGE_DIR).join(&upload.filename);
        tokio::fs::write(&file_path, file_data).await?;

        info!("文件已保存: {}", file_path.display());
    }

    Ok(())
}

// 发送文件块 - 优化版本
async fn send_file_chunks(
    data: &[u8],
    chunk_size: usize,
    file_id: &str,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> Result<()> {
    let total_chunks = (data.len() + chunk_size - 1) / chunk_size;

    for i in 0..total_chunks {
        let start = i * chunk_size;
        let end = std::cmp::min(start + chunk_size, data.len());
        let chunk = &data[start..end];

        // 发送块元数据
        let response = ServerMessage::DownloadChunk {
            file_id: file_id.to_string(),
            index: i,
            total_chunks,
        };
        sender
            .send(Message::Text(serde_json::to_string(&response)?))
            .await?;

        // 发送二进制数据
        sender.send(Message::Binary(Bytes::copy_from_slice(chunk).to_vec())).await?;

        // 移除延迟以提高传输速度
        // tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }

    Ok(())
}

// 列出所有文件的路由
async fn list_files(State(_state): State<AppState>) -> impl IntoResponse {
    let mut files = Vec::new();

    if let Ok(mut entries) = tokio::fs::read_dir(STORAGE_DIR).await {
        while let Some(entry) = entries.next_entry().await.ok().flatten() {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_file() {
                    files.push(FileInfo {
                        name: entry.file_name().to_string_lossy().to_string(),
                        size: metadata.len(),
                    });
                }
            }
        }
    }

    axum::response::Html(format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>文件列表</title>
    <style>
        body {{ font-family: Arial, sans-serif; padding: 20px; }}
        h1 {{ color: #333; }}
        ul {{ list-style: none; padding: 0; }}
        li {{
            padding: 10px;
            margin: 5px 0;
            background: #f5f5f5;
            border-radius: 5px;
            display: flex;
            justify-content: space-between;
        }}
        .size {{ color: #666; }}
    </style>
</head>
<body>
    <h1>服务器文件列表</h1>
    <ul>
        {}
    </ul>
</body>
</html>"#,
        files
            .iter()
            .map(|f| format!(
                "<li><strong>{}</strong><span class='size'>{}</span></li>",
                f.name,
                format_size(f.size)
            ))
            .collect::<Vec<_>>()
            .join("")
    ))
}

#[derive(Serialize)]
struct FileInfo {
    name: String,
    size: u64,
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志 - 设置为 INFO 级别，调试信息使用 DEBUG 级别
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // 创建存储目录
    tokio::fs::create_dir_all(STORAGE_DIR).await?;

    // 创建应用状态
    let state = AppState::new();

    // 创建路由
    let app = Router::new()
        .route("/ws", get(handle_socket))
        .route("/files", get(list_files))
        .with_state(state);

    // 启动服务器
    info!("WebSocket 文件传输服务器启动于: {}", SERVER_ADDR);
    info!("文件存储目录: {}", STORAGE_DIR);
    info!("文件列表页面: http://{}/files", SERVER_ADDR);
    info!("TCP 接收缓冲区: {} MB", TCP_RECV_BUFFER_SIZE / 1024 / 1024);
    info!("TCP 发送缓冲区: {} MB", TCP_SEND_BUFFER_SIZE / 1024 / 1024);

    // 创建 TCP socket 并设置缓冲区大小
    let addr: std::net::SocketAddr = SERVER_ADDR.parse()?;
    let socket = TcpSocket::new_v4()?;

    // // 设置接收缓冲区大小
    // if let Err(e) = socket.set_recv_buffer_size(TCP_RECV_BUFFER_SIZE) {
    //     warn!("设置 TCP 接收缓冲区失败: {}, 将使用系统默认值", e);
    // }

    // // 设置发送缓冲区大小
    // if let Err(e) = socket.set_send_buffer_size(TCP_SEND_BUFFER_SIZE) {
    //     warn!("设置 TCP 发送缓冲区失败: {}, 将使用系统默认值", e);
    // }

    // 绑定并监听
    socket.bind(addr)?;
    let listener = socket.listen(1024)?; // 1024 是连接队列长度
    axum::serve(listener, app).await?;

    Ok(())
}
