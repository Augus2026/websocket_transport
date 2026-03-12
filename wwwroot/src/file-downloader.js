// 文件下载器
class FileDownloader {
    constructor(wsClient, onProgress, onComplete, onError) {
        this.wsClient = wsClient;
        this.onProgress = onProgress;
        this.onComplete = onComplete;
        this.onError = onError;
        this.activeDownloads = new Map();
        this.streamSaver = null;
    }

    // 初始化 StreamSaver
    async initStreamSaver() {
        try {
            // StreamSaver 已经通过 script 标签加载为全局对象
            if (window.streamSaver && window.streamSaver.createWriteStream) {
                this.streamSaver = window.streamSaver;
                return true;
            } else {
                console.warn('StreamSaver 不可用，将使用 Blob 下载');
                return false;
            }
        } catch (error) {
            console.error('加载 StreamSaver 失败:', error);
            return false;
        }
    }

    // 请求下载文件
    async downloadFile(filename, fileId = null) {
        const finalFileId = fileId || this.generateFileId();

        console.log(`请求下载: ${filename}`);

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
            duration: 0
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
        const fileId = message.file_id || message.fileId || message.downloadId;
        const downloadInfo = this.activeDownloads.get(fileId);

        if (!downloadInfo) {
            console.warn('未找到对应的下载信息:', fileId);
            return;
        }

        if (message.op === 'download_start') {
            // 下载开始，初始化
            downloadInfo.totalChunks = message.chunks || 0;
            downloadInfo.totalSize = message.size || 0;
            downloadInfo.status = 'downloading';
            downloadInfo.chunks = new Array(downloadInfo.totalChunks);

            // 初始化时间
            downloadInfo.startTime = Date.now();
            downloadInfo.lastUpdateTime = Date.now();
            downloadInfo.lastReceivedBytes = 0;

            // 初始化 StreamSaver（使用 Blob 方式下载，不依赖 StreamSaver）
            downloadInfo.useBlob = true;

        } else if (message.op === 'download_chunk') {
            // 接收到数据块
            const index = message.index;
            if (index >= 0 && index < downloadInfo.chunks.length) {
                downloadInfo.chunks[index] = data;
                downloadInfo.receivedChunks++;

                if (data) {
                    downloadInfo.receivedBytes += data.byteLength || data.length || 0;
                }

                // 更新进度
                if (downloadInfo.totalSize > 0) {
                    downloadInfo.progress = Math.round((downloadInfo.receivedBytes / downloadInfo.totalSize) * 100);
                } else {
                    downloadInfo.progress = Math.round((downloadInfo.receivedChunks / downloadInfo.totalChunks) * 100);
                }

                // 计算速度和时长
                const now = Date.now();
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

                if (this.onProgress) {
                    this.onProgress(fileId, downloadInfo);
                }

                        // 写入文件流（StreamSaver 不可用时，直接存储到 chunks）
                if (!downloadInfo.useBlob && downloadInfo.fileStream) {
                    downloadInfo.fileStream.write(data);
                }

                // 检查是否完成
                if (downloadInfo.receivedChunks === downloadInfo.totalChunks) {
                    this.completeDownload(fileId);
                }
            }

        } else if (message.op === 'download_end') {
            // 下载完成
            this.completeDownload(fileId);
        } else if (message.op === 'download_error') {
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

    // 创建文件流
    createFileStream(downloadInfo) {
        try {
            if (this.streamSaver && this.streamSaver.createWriteStream) {
                const fileStream = this.streamSaver.createWriteStream(downloadInfo.filename);
                downloadInfo.fileStream = fileStream;
            } else {
                // StreamSaver 不可用，使用 Blob 下载
                downloadInfo.useBlob = true;
            }
        } catch (error) {
            console.error('创建文件流失败:', error);
            // 如果 StreamSaver 不可用，使用 Blob 下载
            downloadInfo.useBlob = true;
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

        // 关闭文件流
        if (downloadInfo.fileStream) {
            downloadInfo.fileStream.close();
        }

        // 如果使用 Blob 方式下载
        if (downloadInfo.useBlob && downloadInfo.chunks.length > 0) {
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
        return 'download_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
    }

    // 取消下载
    cancelDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);
        if (downloadInfo) {
            downloadInfo.status = 'cancelled';

            // 关闭文件流
            if (downloadInfo.fileStream) {
                downloadInfo.fileStream.close();
            }

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
