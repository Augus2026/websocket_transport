class FileDownloader {
    constructor(wsClient, onProgress, onComplete, onError) {
        this.wsClient = wsClient;
        this.onProgress = onProgress;
        this.onComplete = onComplete;
        this.onError = onError;
        this.activeDownloads = new Map();
        this.progressUpdateInterval = 200;
    }

    async downloadFile(filename, fileId = null) {
        const finalFileId = fileId || this.generateFileId();

        const downloadInfo = {
            filename: filename,
            file_id: finalFileId,
            total_chunks: 0,
            receivedChunks: 0,
            receivedBytes: 0,
            progress: 0,
            status: 'requesting',
            chunks: [],
            startTime: null,
            lastUpdateTime: null,
            lastReceivedBytes: 0,
            speed: 0,
            duration: 0,
            useBlob: true,
            lastProgressUpdate: 0
        };

        this.activeDownloads.set(finalFileId, downloadInfo);

        try {
            const requestMessage = {
                op: 'download_request',
                filename: filename,
                file_id: finalFileId
            };

            this.wsClient.send(JSON.stringify(requestMessage));

        } catch (error) {
            downloadInfo.status = 'error';
            downloadInfo.error = error.message;
            if (this.onProgress) this.onProgress(finalFileId, downloadInfo);
            if (this.onError) this.onError(finalFileId, error);
        }
    }

    handleChunk(message, data) {
        const fileId = message.file_id || message.fileId || message.downloadId;
        const downloadInfo = this.activeDownloads.get(fileId);

        if (!downloadInfo) return;

        if (message.op === 'download_start') {
            downloadInfo.totalChunks = message.chunks || 0;
            downloadInfo.totalSize = message.size || 0;
            downloadInfo.status = 'downloading';
            downloadInfo.chunks = new Array(downloadInfo.totalChunks);
            downloadInfo.startTime = Date.now();
            downloadInfo.lastUpdateTime = Date.now();
            downloadInfo.lastReceivedBytes = 0;
            downloadInfo.useBlob = true;

            if (this.onProgress) this.onProgress(fileId, downloadInfo);

        } else if (message.op === 'download_chunk') {
            const index = message.index;
            if (index >= 0 && index < downloadInfo.chunks.length) {
                downloadInfo.chunks[index] = data;
                downloadInfo.receivedChunks++;

                if (data) downloadInfo.receivedBytes += data.byteLength || data.length || 0;

                const now = Date.now();
                const progressUpdateNeeded = (now - downloadInfo.lastProgressUpdate) >= this.progressUpdateInterval;

                if (progressUpdateNeeded) {
                    if (downloadInfo.totalSize > 0) {
                        downloadInfo.progress = Math.round((downloadInfo.receivedBytes / downloadInfo.totalSize) * 100);
                    } else {
                        downloadInfo.progress = Math.round((downloadInfo.receivedChunks / downloadInfo.totalChunks) * 100);
                    }

                    if (downloadInfo.startTime) {
                        downloadInfo.duration = (now - downloadInfo.startTime) / 1000;

                        if (now - downloadInfo.lastUpdateTime >= 1000) {
                            const timeDiff = (now - downloadInfo.lastUpdateTime) / 1000;
                            const bytesDiff = downloadInfo.receivedBytes - downloadInfo.lastReceivedBytes;
                            downloadInfo.speed = (bytesDiff * 8) / (timeDiff * 1000000);
                            downloadInfo.lastUpdateTime = now;
                            downloadInfo.lastReceivedBytes = downloadInfo.receivedBytes;
                        }
                    }

                    if (this.onProgress) this.onProgress(fileId, downloadInfo);
                    downloadInfo.lastProgressUpdate = now;
                }

                if (downloadInfo.receivedChunks === downloadInfo.totalChunks) {
                    this.completeDownload(fileId);
                }
            }

        } else if (message.op === 'download_end') {
            this.completeDownload(fileId);
        } else if (message.op === 'download_error') {
            downloadInfo.status = 'error';
            downloadInfo.error = message.error || '下载失败';
            if (this.onProgress) this.onProgress(fileId, downloadInfo);
            if (this.onError) this.onError(fileId, new Error(downloadInfo.error));
        }
    }

    completeDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);
        if (!downloadInfo || downloadInfo.status === 'completed') return;

        downloadInfo.status = 'completed';
        downloadInfo.progress = 100;

        if (downloadInfo.fileStream) downloadInfo.fileStream.close();

        if (downloadInfo.useBlob && downloadInfo.chunks.length > 0) {
            this.downloadAsBlob(downloadInfo);
        }

        if (this.onProgress) this.onProgress(fileId, downloadInfo);
        if (this.onComplete) this.onComplete(fileId, downloadInfo.filename);
    }

    downloadAsBlob(downloadInfo) {
        try {
            const blob = new Blob(downloadInfo.chunks.filter(chunk => chunk));
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = downloadInfo.filename;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
        } catch (error) {}
    }

    generateFileId() {
        return 'download_' + Date.now() + '_' + Math.random().toString(36).substring(2, 11);
    }

    cancelDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);
        if (downloadInfo) {
            downloadInfo.status = 'cancelled';
            if (downloadInfo.fileStream) downloadInfo.fileStream.close();
            this.activeDownloads.delete(fileId);

            const cancelMessage = {
                op: 'download_cancel',
                file_id: fileId
            };
            this.wsClient.send(JSON.stringify(cancelMessage));
        }
    }

    cleanup(fileId) {
        this.activeDownloads.delete(fileId);
    }
}

export default FileDownloader;
