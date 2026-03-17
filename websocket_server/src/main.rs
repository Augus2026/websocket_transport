use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::header::{ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue},
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

const SERVER_ADDR: &str = "0.0.0.0:9090";
const STORAGE_DIR: &str = "./uploads";

#[derive(Clone)]
struct UploadStatus {
    filename: String,
    total_chunks: usize,
    received_chunks: usize,
    total_size: u64,
    received_size: u64,
    chunks: HashMap<usize, Vec<u8>>,
}

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
}

#[derive(Clone)]
struct AppState {
    active_uploads: Arc<Mutex<HashMap<String, UploadStatus>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            active_uploads: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

async fn handle_socket(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_connection(socket, state))
}

async fn handle_connection(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    while let Some(result) = receiver.next().await {
        match result {
            Ok(msg) => {
                if let Err(e) = handle_message(msg, &state, &mut sender).await {
                    error!("{}", e);
                }
            }
            Err(e) => {
                warn!("{}", e);
                break;
            }
        }
    }
}

async fn handle_message(
    msg: Message,
    state: &AppState,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> Result<()> {
    match msg {
        Message::Text(text) => {
            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                match client_msg {
                    ClientMessage::UploadStart {
                        filename,
                        size,
                        chunks,
                        file_id,
                    } => {
                        info!("upload: {} ({} bytes, {} chunks)", filename, size, chunks);

                        let upload_status = UploadStatus {
                            filename: filename.clone(),
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
                        debug!("chunk: {} [{}], {} bytes", filename, index, size);
                    }

                    ClientMessage::UploadEnd { filename, file_id } => {
                        let result = save_file(&file_id, &state).await;

                        if let Err(e) = result {
                            error!("save failed: {}", e);
                            let response = ServerMessage::UploadError {
                                file_id: file_id.clone(),
                                error: e.to_string(),
                            };
                            sender
                                .send(Message::Text(serde_json::to_string(&response)?))
                                .await?;
                        } else {
                            info!("saved: {}", filename);
                            let response = ServerMessage::UploadComplete {
                                file_id: file_id.clone(),
                                filename: filename.clone(),
                            };
                            sender
                                .send(Message::Text(serde_json::to_string(&response)?))
                                .await?;

                            state.active_uploads.lock().unwrap().remove(&file_id);
                        }
                    }

                    ClientMessage::UploadCancel { file_id } => {
                        let response = ServerMessage::UploadError {
                            file_id: file_id.clone(),
                            error: "cancelled".to_string(),
                        };
                        sender
                            .send(Message::Text(serde_json::to_string(&response)?))
                            .await?;

                        state.active_uploads.lock().unwrap().remove(&file_id);
                    }

                    ClientMessage::DownloadRequest { filename, file_id } => {
                        info!("download: {} ({})", filename, file_id);

                        let file_path = PathBuf::from(STORAGE_DIR).join(&filename);

                        if !file_path.exists() {
                            warn!("not found: {}", filename);
                            let response = ServerMessage::DownloadError {
                                file_id: file_id.clone(),
                                error: "not found".to_string(),
                            };
                            sender
                                .send(Message::Text(serde_json::to_string(&response)?))
                                .await?;
                        } else {
                            match tokio::fs::read(&file_path).await {
                                Ok(data) => {
                                    let file_size = data.len() as u64;
                                    let chunk_size = 64 * 1024;
                                    let total_chunks = (data.len() + chunk_size - 1) / chunk_size;

                                    let response = ServerMessage::DownloadStart {
                                        filename: filename.clone(),
                                        size: file_size,
                                        chunks: total_chunks,
                                        file_id: file_id.clone(),
                                    };
                                    sender
                                        .send(Message::Text(serde_json::to_string(&response)?))
                                        .await?;

                                    send_file_chunks(&data, chunk_size, &file_id, sender).await?;

                                    let response = ServerMessage::DownloadEnd {
                                        file_id: file_id.clone(),
                                    };
                                    sender
                                        .send(Message::Text(serde_json::to_string(&response)?))
                                        .await?;

                                    info!("complete: {}", filename);
                                }
                                Err(e) => {
                                    error!("read failed: {}", e);
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
                        file_id: _,
                    } => {
                        // Download cancelled - no state to clean up
                    }
                }
            } else {
                warn!("parse error");
            }
        }

        Message::Binary(data) => {
            let file_id = {
                let uploads = state.active_uploads.lock().unwrap();
                if let Some((file_id, _)) = uploads.iter().next() {
                    Some(file_id.clone())
                } else {
                    None
                }
            };

            if let Some(file_id) = file_id {
                let mut uploads = state.active_uploads.lock().unwrap();
                if let Some(upload) = uploads.get_mut(&file_id) {
                    let index = upload.received_chunks;
                    upload.chunks.insert(index, data.to_vec());
                    upload.received_chunks += 1;
                    upload.received_size += data.len() as u64;
                    debug!(
                        "chunk: {}/{} ({} bytes)",
                        upload.received_chunks,
                        upload.total_chunks,
                        data.len()
                    );
                }
            }
        }

        Message::Close(_) => {
            info!("disconnect");
            return Ok(());
        }

        _ => {}
    }

    Ok(())
}

async fn save_file(file_id: &str, state: &AppState) -> Result<()> {
    let upload = {
        let uploads = state.active_uploads.lock().unwrap();
        uploads.get(file_id).cloned()
    };

    if let Some(upload) = upload {
        tokio::fs::create_dir_all(STORAGE_DIR).await?;

        let mut file_data = Vec::with_capacity(upload.total_size as usize);
        for i in 0..upload.total_chunks {
            if let Some(chunk) = upload.chunks.get(&i) {
                file_data.extend_from_slice(chunk);
            } else {
                return Err(anyhow::anyhow!("missing chunk: {}", i));
            }
        }

        let file_path = PathBuf::from(STORAGE_DIR).join(&upload.filename);
        tokio::fs::write(&file_path, file_data).await?;

        info!("saved: {}", file_path.display());
    }

    Ok(())
}

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

        let response = ServerMessage::DownloadChunk {
            file_id: file_id.to_string(),
            index: i,
            total_chunks,
        };
        sender
            .send(Message::Text(serde_json::to_string(&response)?))
            .await?;

        sender.send(Message::Binary(Bytes::copy_from_slice(chunk).to_vec())).await?;
    }

    Ok(())
}

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

    let mut response = axum::Json(files).into_response();
    response.headers_mut().insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    response.headers_mut().insert(ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, OPTIONS"));
    response.headers_mut().insert(ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("*"));
    response
}

#[derive(Serialize)]
struct FileInfo {
    name: String,
    size: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tokio::fs::create_dir_all(STORAGE_DIR).await?;

    let state = AppState::new();

    let app = Router::new()
        .route("/ws", get(handle_socket))
        .route("/files", get(list_files))
        .with_state(state);

    info!("Server: {}", SERVER_ADDR);
    info!("Files: {}", STORAGE_DIR);
    info!("List: http://{}/files", SERVER_ADDR);

    let addr: std::net::SocketAddr = SERVER_ADDR.parse()?;
    let socket = TcpSocket::new_v4()?;
    socket.bind(addr)?;
    let listener = socket.listen(1024)?;
    axum::serve(listener, app).await?;

    Ok(())
}
