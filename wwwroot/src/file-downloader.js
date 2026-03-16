class FileDownloader {
    constructor(wsClient, onProgress, onComplete, onError) {
        this.wsClient = wsClient;
        this.onProgress = onProgress;
        this.onComplete = onComplete;
        this.onError = onError;
        this.activeDownloads = new Map();
        this.streamSaver = window.streamSaver || null;
        this.chunkBufferSize = 1024 * 1024;
        this.progressUpdateInterval = 10;
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
            startTime: null,
            lastUpdateTime: null,
            lastReceivedBytes: 0,
            speed: 0,
            duration: 0,
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
            console.error('Download request failed:', error);
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

    async handleChunk(message, data) {
        const fileId = message.file_id || message.fileId || message.downloadId;
        const downloadInfo = this.activeDownloads.get(fileId);

        if (!downloadInfo) {
            console.warn('Download info not found:', fileId);
            return;
        }

        if (message.op === 'download_start') {
            downloadInfo.totalChunks = message.chunks || 0;
            downloadInfo.totalSize = message.size || 0;
            downloadInfo.status = 'downloading';

            downloadInfo.startTime = Date.now();
            downloadInfo.lastUpdateTime = Date.now();
            downloadInfo.lastReceivedBytes = 0;

            try {
                const fileStream = streamSaver.createWriteStream(downloadInfo.filename);
                downloadInfo.fileStream = fileStream.getWriter();
            } catch (error) {
                console.error('Failed to create file stream:', error);
                throw error;
            }

            if (this.onProgress) {
                this.onProgress(fileId, downloadInfo);
            }

        } else if (message.op === 'download_chunk') {
            downloadInfo.receivedChunks++;
            if (data) {
                downloadInfo.receivedBytes += data.byteLength || data.length || 0;
            }

            const now = Date.now();
            const progressUpdateNeeded = (now - downloadInfo.lastProgressUpdate) >= this.progressUpdateInterval;

            if (progressUpdateNeeded) {
                if (downloadInfo.totalSize > 0) {
                    downloadInfo.progress = Math.round((downloadInfo.receivedBytes / downloadInfo.totalSize) * 100);
                } else {
                    downloadInfo.progress = Math.round((downloadInfo.receivedChunks / downloadInfo.totalChunks) * 100);
                }

                if (downloadInfo.startTime) {
                    const elapsedTime = (now - downloadInfo.startTime) / 1000;
                    downloadInfo.duration = elapsedTime;

                    if (now - downloadInfo.lastUpdateTime >= 1000) {
                        const timeDiff = (now - downloadInfo.lastUpdateTime) / 1000;
                        const bytesDiff = downloadInfo.receivedBytes - downloadInfo.lastReceivedBytes;
                        downloadInfo.speed = (bytesDiff * 8) / (timeDiff * 1000000);
                        downloadInfo.lastUpdateTime = now;
                        downloadInfo.lastReceivedBytes = downloadInfo.receivedBytes;
                    }
                }

                console.log('Download progress:', downloadInfo.filename, downloadInfo.progress + '%');
                if (this.onProgress) {
                    this.onProgress(fileId, downloadInfo);
                }

                downloadInfo.lastProgressUpdate = now;
            }

            if (downloadInfo.fileStream) {
                try {
                    if(data instanceof ArrayBuffer) {
                        const chunk_data = new Uint8Array(data);
                        const writer = downloadInfo.fileStream;
                        await writer.write(chunk_data);
                    }
                } catch (error) {
                    console.error('Failed to write chunk to file stream:', error);
                }
            }

            // 不要在这里自动完成下载，等待 download_end 消息

        } else if (message.op === 'download_end') {
            console.log('Download end received for:', downloadInfo.filename);
            this.completeDownload(fileId);

        } else if (message.op === 'download_error') {
            console.error('Download error:', message.error, 'File ID:', fileId);
            downloadInfo.status = 'error';
            downloadInfo.error = message.error || 'Download failed';
            if (this.onProgress) {
                this.onProgress(fileId, downloadInfo);
            }
            if (this.onError) {
                this.onError(fileId, new Error(downloadInfo.error));
            }
        }
    }

    async completeDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);

        if (!downloadInfo) {
            return;
        }

        if (downloadInfo.status === 'completed') {
            return;
        }

        downloadInfo.status = 'completed';
        downloadInfo.progress = 100;

        if (downloadInfo.fileStream) {
            try {
                const writer = downloadInfo.fileStream;
                // 等待所有写入操作完成
                await writer.ready;
                // 关闭流，StreamSaver 会触发文件下载
                await writer.close();
                console.log('File stream closed and saved:', downloadInfo.filename);
            } catch (error) {
                console.error('Failed to close file stream:', error);
            }
        }

        if (this.onProgress) {
            this.onProgress(fileId, downloadInfo);
        }
        if (this.onComplete) {
            this.onComplete(fileId, downloadInfo.filename);
        }
    }

    generateFileId() {
        return 'download_' + Date.now() + '_' + Math.random().toString(36).substring(2, 11);
    }

    async cancelDownload(fileId) {
        const downloadInfo = this.activeDownloads.get(fileId);
        if (downloadInfo) {
            downloadInfo.status = 'cancelled';

            if (downloadInfo.fileStream) {
                try {
                    const writer = downloadInfo.fileStream;
                    await writer.close();
                } catch (error) {
                    console.error('Failed to close file stream:', error);
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

    cleanup(fileId) {
        this.activeDownloads.delete(fileId);
    }
}

export default FileDownloader;
