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
        return `${protocol}//${host}:8080/ws`;
    }

    setupUIHandlers() {
        this.wsClient.onConnectionChange = (connected) => {
            this.ui.updateConnectionStatus(connected);
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

        if (this.wsClient.ws) {
            this.wsClient.ws.onmessage = (event) => {
                if (typeof event.data === 'string') {
                    try {
                        const message = JSON.parse(event.data);
                        this.wsClient.handleMessage(event.data);
                    } catch (e) {}
                } else {
                    const downloadInfo = Array.from(this.downloader.activeDownloads.values())
                        .find(info => info.status === 'downloading');
                    if (downloadInfo) {
                        this.downloader.handleChunk({ op: 'download_chunk', file_id: downloadInfo.fileId }, event.data);
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
        if (info.status === 'completed') status = '已完成';
        else if (info.status === 'error') status = '失败: ' + (info.error || '未知错误');
        else if (info.status === 'cancelled') status = '已取消';

        this.ui.updateProgress(fileId, info.progress, status, info.uploadedBytes, info.file.size, info.speed, info.duration);

        if (info.status === 'completed') {
            setTimeout(() => {
                this.ui.removeProgressItem(fileId);
                this.uploader.cleanup(fileId);
            }, 5000);
        }
    }

    onUploadComplete(fileId, file) {}

    onUploadError(fileId, error) {}

    onDownloadProgress(fileId, info) {
        let status = '下载中';
        if (info.status === 'completed') status = '已完成';
        else if (info.status === 'error') status = '失败: ' + (info.error || '未知错误');
        else if (info.status === 'cancelled') status = '已取消';
        else if (info.status === 'requesting') status = '请求中...';

        this.ui.updateProgress(fileId, info.progress, status, info.receivedBytes, info.totalSize, info.speed, info.duration);

        if (info.status === 'completed') {
            setTimeout(() => {
                this.ui.removeProgressItem(fileId);
                this.downloader.cleanup(fileId);
            }, 5000);
        }
    }

    onDownloadComplete(fileId, filename) {}

    onDownloadError(fileId, error) {}

    start() {
        this.wsClient.connect();
    }
}

document.addEventListener('DOMContentLoaded', () => {
    const app = new App();
    app.start();
});
