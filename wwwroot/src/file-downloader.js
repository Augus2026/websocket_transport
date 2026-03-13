// 文件下载器
class FileDownloader {
    constructor(wsClient, onProgress, onComplete, onError) {
        this.wsClient = wsClient;
        this.onProgress = onProgress;
        this.onComplete = onComplete;
        this.onError = onError;
        this.activeDownloads = new Map();
        this.chunkBufferSize = 1024 * 1024; // 1MB 缓冲区大小
        this.progressUpdateInterval = 200; // 进度更新间隔 (ms)
    }

    // 请求下载文件
    async downloadFile(filename, fileId = null) {
        const finalFileId = fileId || this.generateFileId();

        console.log(`[下载] 请求下载文件: ${filename}, 文件ID: ${finalFileId}`);

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
            lastProgressUpdate: 0 // 优化进度更新频率
        };

        this.activeDownloads.set(finalFileId, downloadInfo);

        try {
            // 发送下载请求
            const requestMessage = {
                op: 'download_request',
                filename: filename,
                file_id: finalFileId
            };

            this.wsClient.send(JSON.stringify(requestMessage));

        } catch (error) {
            console.error('下载请求失败:', error);
            downloadInfo.status = 'error';
            downloadInfo.error = error.message;
            if (this.onProgress) {
                this.onProgress(finalFileId, downloadInfo);
            }
            if (this.onError) {
                this.onError(finalFileId, error);
            }
        }
    }

    // 处理下载块
    handleChunk(message, data) {
        console.log('[下载] 收到消息:', message.op, '文件ID:', message.file_id, '数据长度:', data?.byteLength || 0);

        const fileId = message.file_id || message.fileId || message.downloadId;
        const downloadInfo = this.activeDownloads.get(fileId);

        if (!downloadInfo) {
            console.warn('未找到对应的下载信息:', fileId);
            return;
        }

        if (message.op === 'download_start') {
            console.log('[下载] download_start - 文件:', downloadInfo.filename, '大小:', message.size, '块数:', message.chunks);
            // 下载开始，初始化
            downloadInfo.totalChunks = message.chunks || 0;
            downloadInfo.totalSize = message.size || 0;
            downloadInfo.status = 'downloading';
            downloadInfo.chunks = new Array(downloadInfo.totalChunks);

            // 初始化时间
            downloadInfo.startTime = Date.now();
            downloadInfo.lastUpdateTime = Date.now();
            downloadInfo.lastReceivedBytes = 0;

            // 更新初始进度
            if (this.onProgress) {
                this.onProgress(fileId, downloadInfo);
            }

        } else if (message.op === 'download_chunk') {
            console.log('[下载] download_chunk - 索引:', message.index, '总块数:', message.total_chunks, '文件ID:', fileId);
            // 接收到数据块
            const index = message.index;
            if (index >= 0 && index < downloadInfo.chunks.length) {
                downloadInfo.chunks[index] = data;
                downloadInfo.receivedChunks++;

                if (data) {
                    downloadInfo.receivedBytes += data.byteLength || data.length || 0;
                }

                // 更新进度（优化：减少更新频率）
                const now = Date.now();
                const progressUpdateNeeded = (now - downloadInfo.lastProgressUpdate) >= this.progressUpdateInterval;

                if (progressUpdateNeeded) {
                    if (downloadInfo.totalSize > 0) {
                        downloadInfo.progress = Math.round((downloadInfo.receivedBytes / downloadInfo.totalSize) * 100);
                    } else {
                        downloadInfo.progress = Math.round((downloadInfo.receivedChunks / downloadInfo.totalChunks) * 100);
                    }

                    // 计算速度和时长
                    if (downloadInfo.startTime) {
                        const elapsedTime = (now - downloadInfo.startTime) / 1000; // 秒
                        downloadInfo.duration = elapsedTime;

                        // 计算速度（每秒更新一次）
                        if (now - downloadInfo.lastUpdateTime >= 1000) {
                            const timeDiff = (now - downloadInfo.lastUpdateTime) / 1000;
                            const bytesDiff = downloadInfo.receivedBytes - downloadInfo.lastReceivedBytes;
                            downloadInfo.speed = (bytesDiff * 8) / (timeDiff * 1000000); // Mbps
                            downloadInfo.lastUpdateTime = now;
                            downloadInfo.lastReceivedBytes = downloadInfo.receivedBytes;
                        }
                    }

                    // 更新进度显示
                    console.log('[下载] 进度更新 - 文件:', downloadInfo.filename, '进度:', downloadInfo.progress, '%');
                    if (this.onProgress) {
                        this.onProgress(fileId, downloadInfo);
                    }

                    downloadInfo.lastProgressUpdate = now;
                }

                // 检查是否完成
                if (downloadInfo.receivedChunks === downloadInfo.totalChunks) {
                    this.completeDownload(fileId);
                }
            }

        } else if (message.op === 'download_end') {
            console.log('[下载] download_end - 文件:', downloadInfo.filename, '已完成');
            // 下载完成
            this.completeDownload(fileId);
        } else if (message.op === 'download_error') {
            console.error('[下载] download_error - 错误:', message.error, '文件ID:', fileId);
            // 下载错误
            downloadInfo.status = 'error';
            downloadInfo.error = message.error || '下载失败';
            if (this.onProgress) {
                this.onProgress(fileId, downloadInfo);
            }
            if (this.onError) {
                this.onError(fileId, new Error(downloadInfo.error));
            }
        }
    }

    // 完成下载
    completeDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);

        if (!downloadInfo) {
            return;
        }

        if (downloadInfo.status === 'completed') {
            return; // 已经完成
        }

        downloadInfo.status = 'completed';
        downloadInfo.progress = 100;

        // 使用 Blob 方式下载
        if (downloadInfo.chunks.length > 0) {
            this.downloadAsBlob(downloadInfo);
        }

        if (this.onProgress) {
            this.onProgress(fileId, downloadInfo);
        }
        if (this.onComplete) {
            this.onComplete(fileId, downloadInfo.filename);
        }
    }

    // 使用 Blob 下载
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
        } catch (error) {
            console.error('Blob 下载失败:', error);
        }
    }

    // 生成文件ID
    generateFileId() {
        return 'download_' + Date.now() + '_' + Math.random().toString(36).substring(2, 11);
    }

    // 取消下载
    cancelDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);
        if (downloadInfo) {
            downloadInfo.status = 'cancelled';

            this.activeDownloads.delete(fileId);

            const cancelMessage = {
                op: 'download_cancel',
                file_id: fileId
            };
            this.wsClient.send(JSON.stringify(cancelMessage));
        }
    }

    // 清理已完成的下载
    cleanup(fileId) {
        this.activeDownloads.delete(fileId);
    }
}

export default FileDownloader;
