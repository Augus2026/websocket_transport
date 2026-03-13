// 文件下载器
class FileDownloader {
    constructor(wsClient, onProgress, onComplete, onError) {
        this.wsClient = wsClient;
        this.onProgress = onProgress;
        this.onComplete = onComplete;
        this.onError = onError;
        this.activeDownloads = new Map();
        this.streamSaver = window.streamSaver || null;
        this.chunkBufferSize = 1024 * 1024; // 1MB 缓冲区大小
        this.progressUpdateInterval = 10; // 进度更新间隔 (ms)
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
    async handleChunk(message, data) {
        // console.log('[下载] 收到消息:', message.op, '文件ID:', message.file_id, '数据长度:', data?.byteLength || 0);

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

            // 初始化时间
            downloadInfo.startTime = Date.now();
            downloadInfo.lastUpdateTime = Date.now();
            downloadInfo.lastReceivedBytes = 0;

            // 强制使用 StreamSaver
            try {
                const fileStream = this.streamSaver.createWriteStream(downloadInfo.filename);
                downloadInfo.fileStream = fileStream.getWriter ? fileStream.getWriter() : fileStream;
                console.log('[下载] 文件流创建成功');
            } catch (error) {
                console.error('[下载] 创建文件流失败:', error);
                throw error;
            }

            // 更新初始进度
            if (this.onProgress) {
                this.onProgress(fileId, downloadInfo);
            }

        } else if (message.op === 'download_chunk') {
            // console.log('[下载] download_chunk - 索引:', message.index, '总块数:', message.total_chunks, '文件ID:', fileId);

            // 接收数据块
            downloadInfo.receivedChunks++;
            if (data) {
                downloadInfo.receivedBytes += data.byteLength || data.length || 0;
            }

            // 更新进度（优化：减少更新频率）
            const now = Date.now();
            const progressUpdateNeeded = (now - downloadInfo.lastProgressUpdate) >= this.progressUpdateInterval;
            // console.log('[下载] 进度更新检查 ', now - downloadInfo.lastProgressUpdate, 'ms 过去', "now ", now, "lastProgressUpdate ", downloadInfo.lastProgressUpdate);

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

            // 写入数据到文件流
            if (downloadInfo.fileStream) {
                try {
                    if (typeof downloadInfo.fileStream.write === 'function') {
                        await downloadInfo.fileStream.write(data);
                    } else {
                        console.error('[下载] StreamSaver 文件流不支持 write 方法');
                    }
                } catch (error) {
                    console.error('[下载] 写入文件流失败:', error);
                }
            }

            // 检查是否完成
            if (downloadInfo.receivedChunks === downloadInfo.totalChunks) {
                this.completeDownload(fileId);
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
    async completeDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);

        if (!downloadInfo) {
            return;
        }

        if (downloadInfo.status === 'completed') {
            return; // 已经完成
        }

        downloadInfo.status = 'completed';
        downloadInfo.progress = 100;

        // 关闭 StreamSaver 文件流
        if (downloadInfo.fileStream) {
            try {
                if (typeof downloadInfo.fileStream.close === 'function') {
                    await downloadInfo.fileStream.close();
                } else {
                    console.error('[下载] StreamSaver 文件流不支持 close 方法');
                }
            } catch (error) {
                console.error('[下载] 关闭文件流失败:', error);
            }
        }

        if (this.onProgress) {
            this.onProgress(fileId, downloadInfo);
        }
        if (this.onComplete) {
            this.onComplete(fileId, downloadInfo.filename);
        }
    }

    // 生成文件ID
    generateFileId() {
        return 'download_' + Date.now() + '_' + Math.random().toString(36).substring(2, 11);
    }

    // 取消下载
    async cancelDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);
        if (downloadInfo) {
            downloadInfo.status = 'cancelled';

            // 关闭 StreamSaver 文件流
            if (downloadInfo.fileStream) {
                try {
                    if (typeof downloadInfo.fileStream.close === 'function') {
                        await downloadInfo.fileStream.close();
                    } else {
                        console.error('[下载] StreamSaver 文件流不支持 close 方法');
                    }
                } catch (error) {
                    console.error('[下载] 关闭文件流失败:', error);
                }
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