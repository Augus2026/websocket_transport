# WebSocket 文件传输服务器

基于 Rust 和 Axum 框架的 WebSocket 文件传输服务器。

## 功能特性

- 支持 WebSocket 双向文件传输
- 文件上传（分块传输）
- 文件下载（流式传输）
- 文件列表查看
- 传输进度跟踪
- 支持大文件传输（64KB 分块）

## 技术栈

- **Tokio** - 异步运行时
- **Axum** - Web 框架
- **Tokio-tungstenite** - WebSocket 支持
- **Serde** - JSON 序列化/反序列化
- **Tracing** - 日志记录

## 构建和运行

### 开发环境

```bash
# 进入服务器目录
cd websocket_server

# 构建（开发版本）
cargo build

# 运行
cargo run

# 或者使用 debug 模式运行
cargo run --bin websocket_server
```

### 生产环境

```bash
# 构建优化版本
cargo build --release

# 运行优化版本
./target/release/websocket_server
```

## 配置

服务器默认配置：
- **监听地址**: `0.0.0.0:8080`
- **WebSocket 路径**: `/ws`
- **文件列表路径**: `/files`
- **文件存储目录**: `./uploads`

## WebSocket 协议

### 上传协议

1. 开始上传:
```json
{
  "op": "upload_start",
  "filename": "example.txt",
  "size": 1024,
  "chunks": 10,
  "fileId": "upload_xxx"
}
```

2. 发送块:
```json
{
  "op": "upload_chunk",
  "filename": "example.txt",
  "fileId": "upload_xxx",
  "index": 0,
  "totalChunks": 10,
  "size": 65536
}
```
（随后发送二进制数据）

3. 结束上传:
```json
{
  "op": "upload_end",
  "filename": "example.txt",
  "fileId": "upload_xxx"
}
```

### 下载协议

1. 下载请求:
```json
{
  "op": "download_request",
  "filename": "example.txt",
  "fileId": "download_xxx"
}
```

2. 下载开始 (服务器响应):
```json
{
  "op": "download_start",
  "filename": "example.txt",
  "size": 1024,
  "chunks": 10,
  "fileId": "download_xxx"
}
```

3. 发送块 (服务器发送):
```json
{
  "op": "download_chunk",
  "fileId": "download_xxx",
  "index": 0,
  "totalChunks": 10
}
```
（随后发送二进制数据）

4. 下载结束 (服务器响应):
```json
{
  "op": "download_end",
  "fileId": "download_xxx"
}
```

## 使用示例

### 1. 启动服务器

```bash
cd websocket_server
cargo run
```

服务器将输出：
```
WebSocket 文件传输服务器启动于: 0.0.0.0:8080
文件存储目录: ./uploads
文件列表页面: http://0.0.0.0:8080/files
```

### 2. 访问文件列表

在浏览器中打开: `http://localhost:8080/files`

### 3. 使用前端

在 `wwwroot` 目录中启动前端：

```bash
cd ../wwwroot
npm run dev
```

然后在浏览器中打开前端页面进行文件传输。

## 目录结构

```
websocket_server/
├── Cargo.toml          # Rust 项目配置
├── src/
│   └── main.rs         # 主程序
├── uploads/            # 文件存储目录（自动创建）
└── README.md           # 本文件
```

## 日志

服务器使用 `tracing` 库记录日志，默认输出 INFO 级别日志。

## 性能优化

- 使用 64KB 分块大小以平衡内存使用和传输效率
- 异步 I/O 处理多个并发连接
- 互斥锁保护共享状态

## 安全注意事项

⚠️ **注意**: 此服务器示例不包含以下安全特性，生产环境使用时应添加：

- 用户认证和授权
- 文件大小限制
- 文件类型验证
- 速率限制
- CORS 配置
- HTTPS 支持
- 输入验证和清理

## 故障排除

### 端口占用

如果 8080 端口被占用，修改 `src/main.rs` 中的 `SERVER_ADDR` 常量。

### 文件权限

确保 `uploads` 目录有写入权限。

### 连接问题

检查防火墙设置，确保 WebSocket 端口可访问。
