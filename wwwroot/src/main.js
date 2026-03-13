import WebSocketClient from './websocket-client.js';
import FileUploader from './file-uploader.js';
import FileDownloader from './file-downloader.js';
import UIManager from './ui.js';

// 应用主类
class App {
    constructor() {
        this.wsUrl = this.getWebSocketUrl();
        this.wsClient = new WebSocketClient(this.wsUrl);
        this.uploader = new FileUploader(
            this.wsClient,
            this.onUploadProgress.bind(this),
            this.onUploadComplete.bind(this),
            this.onUploadError.bind(this)
        );
        this.downloader = new FileDownloader(
            this.wsClient,
            this.onDownloadProgress.bind(this),
            this.onDownloadComplete.bind(this),
            this.onDownloadError.bind(this)
        );
        this.ui = new UIManager();
        this.setupUIHandlers();
        this.setupWebSocketHandlers();
    }

    getWebSocketUrl() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const host = window.location.hostname;
        // WebSocket 服务器总是在 9090 端口，不管前端页面在哪个端口
        const port = ':9090';
        return `${protocol}//${host}${port}/ws`;
    }

    setupUIHandlers() {
        // 连接状态更新
        this.wsClient.onConnectionChange = (connected) => {
            this.ui.updateConnectionStatus(connected);
            // 连接建立后设置二进制消息处理器
            if (connected) {
                this.setupBinaryMessageHandler();
            }
        };

        // 文件选择
        this.ui.setFileSelectHandler((files) => {
            files.forEach(file => this.uploadFile(file));
        });

        // 下载请求
        this.ui.setDownloadRequestHandler((filename) => {
            this.downloadFile(filename);
        });

        // 取消操作
        this.ui.setCancelHandler((fileId, type) => {
            if (type === 'upload') {
                this.uploader.cancelUpload(fileId);
            } else {
                this.downloader.cancelDownload(fileId);
            }
        });
    }

    setupWebSocketHandlers() {
        // 监听下载相关的消息
        this.wsClient.on('download_start', (message) => {
            this.downloader.handleChunk(message, null);
        });

        this.wsClient.on('download_chunk', (message, data) => {
            this.downloader.handleChunk(message, data);
        });

        this.wsClient.on('download_end', (message) => {
            this.downloader.handleChunk(message, null);
        });

        this.wsClient.on('download_error', (message) => {
            this.downloader.handleChunk(message, null);
        });
    }

    setupBinaryMessageHandler() {
        console.log('[Main] 设置二进制消息处理器...');
        // 监听 WebSocket 二进制消息
        if (this.wsClient.ws) {
            console.log('[Main] WebSocket 实例存在，设置 onmessage 处理器');
            this.wsClient.ws.onmessage = (event) => {
                if (typeof event.data === 'string') {
                    try {
                        const message = JSON.parse(event.data);
                        this.wsClient.handleMessage(event.data);
                    } catch (e) {
                        console.log('非 JSON 消息:', event.data);
                    }
                } else {
                    // 二进制数据，找到对应的下载任务
                    console.log('[Main] 处理二进制数据，长度:', event.data.byteLength);
                    const downloadInfo = Array.from(this.downloader.activeDownloads.values())
                        .find(info => info.status === 'downloading');
                    if (downloadInfo) {
                        this.downloader.handleChunk({ op: 'download_chunk', file_id: downloadInfo.file_id }, event.data);
                    } else {
                        console.warn('[Main] 没有找到正在进行的下载任务');
                    }
                }
            };
        } else {
            console.error('[Main] WebSocket 实例不存在，无法设置二进制消息处理器');
        }
    }

    uploadFile(file) {
        const fileId = this.uploader.generateFileId();
        this.ui.createProgressItem(fileId, file.name, 'upload');
        this.uploader.uploadFile(file, fileId);
    }

    downloadFile(filename) {
        const fileId = this.downloader.generateFileId();
        this.ui.createProgressItem(fileId, filename, 'download');
        this.downloader.downloadFile(filename, fileId);
    }

    onUploadProgress(fileId, info) {
        let status = '上传中';
        if (info.status === 'completed') {
            status = '已完成';
        } else if (info.status === 'error') {
            status = '失败: ' + (info.error || '未知错误');
        } else if (info.status === 'cancelled') {
            status = '已取消';
        }

        this.ui.updateProgress(
            fileId,
            info.progress,
            status,
            info.uploadedBytes,
            info.file.size,
            info.speed,
            info.duration
        );

        // 5秒后移除已完成的进度条
        if (info.status === 'completed') {
            setTimeout(() => {
                this.ui.removeProgressItem(fileId);
                this.uploader.cleanup(fileId);
            }, 5000);
        }
    }

    onUploadComplete(fileId, file) {
        console.log('上传完成:', file.name);
    }

    onUploadError(fileId, error) {
        console.error('上传错误:', error);
    }

    onDownloadProgress(fileId, info) {
        let status = '下载中';
        if (info.status === 'completed') {
            status = '已完成';
        } else if (info.status === 'error') {
            status = '失败: ' + (info.error || '未知错误');
        } else if (info.status === 'cancelled') {
            status = '已取消';
        } else if (info.status === 'requesting') {
            status = '请求中...';
        }

        this.ui.updateProgress(
            fileId,
            info.progress,
            status,
            info.receivedBytes,
            info.totalSize,
            info.speed,
            info.duration
        );

        // 5秒后移除已完成的进度条
        if (info.status === 'completed') {
            setTimeout(() => {
                this.ui.removeProgressItem(fileId);
                this.downloader.cleanup(fileId);
            }, 5000);
        }
    }

    onDownloadComplete(fileId, filename) {
        console.log('下载完成:', filename);
    }

    onDownloadError(fileId, error) {
        console.error('下载错误:', error);
    }

    start() {
        console.log('启动 WebSocket 文件传输应用...');
        console.log('连接到:', this.wsUrl);
        this.wsClient.connect();
    }
}

// 启动应用
document.addEventListener('DOMContentLoaded', () => {
    const app = new App();
    app.start();
});
