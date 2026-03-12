# WebSocket 文件传输前端项目

## 项目结构

```
wwwroot/
├── package.json           # npm 配置
├── vite.config.js         # Vite 构建配置
├── index.html             # 主页面
├── src/
│   ├── main.js            # 应用入口
│   ├── websocket-client.js # WebSocket 客户端
│   ├── file-uploader.js   # 文件上传器
│   ├── file-downloader.js # 文件下载器
│   └── ui.js              # UI 管理器
├── public/
│   └── StreamSaver.min.js # StreamSaver 库
└── dist/                  # 构建输出目录
```

## 功能特性

- **文件上传**: 支持拖拽或点击选择文件上传
- **文件下载**: 通过文件名下载服务器上的文件
- **实时进度**: 显示上传/下载进度和传输速度
- **断点续传**: 自动重连机制
- **大文件支持**: 使用分块传输和 StreamSaver.js
- **纯 JavaScript**: 不依赖任何前端框架

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

2. 下载开始:
```json
{
  "op": "download_start",
  "filename": "example.txt",
  "size": 1024,
  "chunks": 10,
  "fileId": "download_xxx"
}
```

3. 发送块:
```json
{
  "op": "download_chunk",
  "fileId": "download_xxx",
  "index": 0,
  "totalChunks": 10
}
```
（随后发送二进制数据）

4. 下载结束:
```json
{
  "op": "download_end",
  "fileId": "download_xxx"
}
```

## 开发

```bash
# 安装依赖
npm install

# 启动开发服务器
npm run dev

# 构建生产版本
npm run build

# 预览构建结果
npm run preview
```

## 使用说明

1. 启动前端开发服务器: `npm run dev`
2. 打开浏览器访问 `http://localhost:3000`
3. 等待 WebSocket 连接建立
4. 上传文件: 拖拽文件到上传区域或点击选择
5. 下载文件: 输入文件名并点击下载

## 注意事项

- WebSocket 服务器需要在 8080 端口运行
- 前端默认连接到 `ws://localhost:8080`
- 需要后端 WebSocket 服务器支持上述协议
- StreamSaver.js 需要浏览器支持 Service Worker
