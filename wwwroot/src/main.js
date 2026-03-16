import WebSocketClient from './websocket-client.js';
import FileUploader from './file-uploader.js';
import FileDownloader from './file-downloader.js';
import UIManager from './ui.js';

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
        const port = ':9090';
        return `${protocol}//${host}${port}/ws`;
    }

    setupUIHandlers() {
        this.wsClient.onConnectionChange = (connected) => {
            this.ui.updateConnectionStatus(connected);
            if (connected) {
                this.setupBinaryMessageHandler();
            }
        };

        this.ui.setFileSelectHandler((files) => {
            files.forEach(file => this.uploadFile(file));
        });

        this.ui.setDownloadRequestHandler((filename) => {
            this.downloadFile(filename);
        });

        this.ui.setCancelHandler((fileId, type) => {
            if (type === 'upload') {
                this.uploader.cancelUpload(fileId);
            } else {
                this.downloader.cancelDownload(fileId);
            }
        });
    }

    setupWebSocketHandlers() {
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
        if (this.wsClient.ws) {
            this.wsClient.ws.onmessage = (event) => {
                if (typeof event.data === 'string') {
                    try {
                        const message = JSON.parse(event.data);
                        this.wsClient.handleMessage(event.data);
                    } catch (e) {
                        console.log('Non-JSON message:', event.data);
                    }
                } else {
                    console.log('Binary data, length:', event.data.byteLength);
                    const downloadInfo = Array.from(this.downloader.activeDownloads.values())
                        .find(info => info.status === 'downloading');
                    if (downloadInfo) {
                        this.downloader.handleChunk({ op: 'download_chunk', file_id: downloadInfo.file_id }, event.data);
                    } else {
                        console.warn('No active download task for binary data');
                    }
                }
            };
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

        if (info.status === 'completed') {
            setTimeout(() => {
                this.ui.removeProgressItem(fileId);
                this.uploader.cleanup(fileId);
            }, 5000);
        }
    }

    onUploadComplete(fileId, file) {
        console.log('Upload complete:', file.name);
    }

    onUploadError(fileId, error) {
        console.error('Upload error:', error);
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

        if (info.status === 'completed') {
            setTimeout(() => {
                this.ui.removeProgressItem(fileId);
                this.downloader.cleanup(fileId);
            }, 5000);
        }
    }

    onDownloadComplete(fileId, filename) {
        console.log('Download complete:', filename);
    }

    onDownloadError(fileId, error) {
        console.error('Download error:', error);
    }

    start() {
        console.log('Starting WebSocket file transfer app...');
        console.log('Connecting to:', this.wsUrl);
        this.wsClient.connect();
    }
}

document.addEventListener('DOMContentLoaded', () => {
    const app = new App();
    app.start();
});
