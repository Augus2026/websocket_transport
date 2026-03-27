# WebSocket 文件传输系统

一个完整的基于 WebSocket 的文件传输系统，包含 Rust 后端和 JavaScript 前端。

## 项目结构

```
file_transport_test/
├── websocket_server/          # Rust WebSocket 服务器
│   ├── Cargo.toml
│   ├── src/main.rs
│   ├── start.sh             # 启动服务器脚本
│   └── README.md
├── wwwroot/               # 前端项目
│   ├── package.json
│   ├── vite.config.js
│   ├── index.html           # 主页面
│   ├── test.html           # 测试页面
│   ├── src/
│   │   ├── main.js
│   │   ├── websocket-client.js
│   │   ├── file-uploader.js
│   │   ├── file-downloader.js
│   │   └── ui.js
│   └── public/
│       └── StreamSaver.min.js
├── run_all.sh            # 同时启动前后端
└── README.md             # 本文件
```

## 功能特性

### 服务器端
- ✅ 基于 Rust 和 Axum 框架
- ✅ WebSocket 双向通信
- ✅ 文件上传（分块传输）
- ✅ 文件下载（流式传输）
- ✅ 文件列表查看
- ✅ 传输进度跟踪
- ✅ 错误处理

### 前端
- ✅ 纯 JavaScript，无框架依赖
- ✅ 文件上传（拖拽支持）
- ✅ 文件下载（使用 StreamSaver.js）
- ✅ 实时进度显示
- ✅ 美观的 UI 设计
- ✅ 自动重连机制

## 快速开始

### 方式一：使用自动启动脚本（推荐）

```bash
cd /root/workspace/file_transport_test
./run_all.sh
```

这将同时启动 WebSocket 服务器和前端开发服务器。

### 方式二：手动启动

1. **启动 WebSocket 服务器**：

```bash
cd /root/workspace/file_transport_test/websocket_server
./start.sh
```

服务器将在 `ws://localhost:8080/ws` 启动。

2. **启动前端开发服务器**：

```bash
cd /root/workspace/file_transport_test/wwwroot
npm run dev
```

前端将在 `http://localhost:3000`（或 3001）启动。

### 方式三：生产构建

1. **构建 WebSocket 服务器**：

```bash
cd websocket_server
cargo build --release
```

2. **构建前端**：

```bash
cd ../wwwroot
npm run build
```

构建的文件将输出到 `wwwroot/dist/` 目录。

## 访问地址

- **前端页面**: `http://localhost:3000`（或 3001）
- **测试页面**: `http://localhost:3000/test.html`
- **WebSocket**: `ws://localhost:8080/ws`
- **文件列表**: `http://localhost:8080/files`

## 使用说明

### 上传文件

1. 打开前端页面
2. 等待 WebSocket 连接建立
3. 拖拽文件到上传区域或点击选择文件
4. 查看上传进度

### 下载文件

1. 在"下载文件"区域输入文件名
2. 点击"下载"按钮
3. 查看下载进度

### 查看服务器文件

访问 `http://localhost:8080/files` 查看所有已上传的文件。

## WebSocket 协议

### 上传协议

1. 开始上传：
```json
{
  "op": "upload_start",
  "filename": "example.txt",
  "size": 1024,
  "chunks": 10,
  "fileId": "upload_xxx"
}
```

2. 发送块：
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

3. 结束上传：
```json
{
  "op": "upload_end",
  "filename": "example.txt",
  "fileId": "upload_xxx"
}
```

### 下载协议

1. 下载请求：
```json
{
  "op": "download_request",
  "filename": "example.txt",
  "fileId": "download_xxx"
}
```

2. 下载开始（服务器响应）：
```json
{
  "op": "download_start",
  "filename": "example.txt",
  "size": 1024,
  "chunks": 10,
  "fileId": "download_xxx"
}
```

3. 发送块（服务器发送）：
```json
{
  "op": "download_chunk",
  "fileId": "download_xxx",
  "index": 0,
  "totalChunks": 10
}
```
（随后发送二进制数据）

4. 下载结束（服务器响应）：
```json
{
  "op": "download_end",
  "fileId": "download_xxx"
}
```

## 配置

### 服务器配置

编辑 `websocket_server/src/main.rs` 修改：

- `SERVER_ADDR`: 服务器监听地址（默认：`0.0.0.0:8080`）
- `STORAGE_DIR`: 文件存储目录（默认：`./uploads`）

### 前端配置

编辑 `wwwroot/src/main.js` 修改：

- WebSocket 地址会自动根据当前页面 URL 生成

## 故障排除

### 端口被占用

如果 8080 或 3000 端口被占用，启动脚本会自动停止占用进程。

### 文件存储目录

确保 `uploads` 目录有写入权限。

### WebSocket 连接失败

1. 检查服务器是否正常运行
2. 检查防火墙设置
3. 查看浏览器控制台错误信息

### StreamSaver.js 不工作

StreamSaver.js 需要：
- 现代 Web 浏览器
- HTTPS 或 localhost
- Service Worker 支持

如果不可用，系统会自动降级到 Blob 下载方式。

## 依赖

### 服务器端

- Rust 1.84+
- Tokio
- Axum
- Serde
- Tungstenite

### 前端

- Node.js
- Vite
- StreamSaver.js

## 安全提示

⚠️ **警告**: 此项目是演示版本，生产环境使用前请添加：

- 用户认证和授权
- 文件大小限制
- 文件类型验证
- 速率限制
- CORS 配置
- HTTPS 支持
- 输入验证和清理

## 许可证

MIT License
