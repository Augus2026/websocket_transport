// 文件上传器
class FileUploader {
    constructor(wsClient, onProgress, onComplete, onError) {
        this.wsClient = wsClient;
        this.onProgress = onProgress;
        this.onComplete = onComplete;
        this.onError = onError;
        this.chunkSize = 64 * 1024; // 64KB
        this.activeUploads = new Map();
    }

    // 上传文件
    async uploadFile(file) {
        const fileId = this.generateFileId();
        const fileSize = file.size;
        const totalChunks = Math.ceil(fileSize / this.chunkSize);

        console.log(`开始上传: ${file.name}, 大小: ${fileSize}, 总块数: ${totalChunks}`);

        const uploadInfo = {
            file: file,
            fileId: fileId,
            totalChunks: totalChunks,
            uploadedChunks: 0,
            uploadedBytes: 0,
            progress: 0,
            status: 'uploading'
        };

        this.activeUploads.set(fileId, uploadInfo);

        try {
            // 发送开始上传消息
            const startMessage = {
                op: 'upload_start',
                filename: file.name,
                size: fileSize,
                chunks: totalChunks,
                fileId: fileId
            };

            this.wsClient.send(JSON.stringify(startMessage));

            // 读取并发送文件块
            await this.sendFileChunks(file, fileId, totalChunks);

            // 发送完成消息
            const endMessage = {
                op: 'upload_end',
                filename: file.name,
                fileId: fileId
            };

            this.wsClient.send(JSON.stringify(endMessage));

            uploadInfo.status = 'completed';
            uploadInfo.progress = 100;
            if (this.onProgress) {
                this.onProgress(fileId, uploadInfo);
            }
            if (this.onComplete) {
                this.onComplete(fileId, file);
            }

        } catch (error) {
            console.error('上传失败:', error);
            uploadInfo.status = 'error';
            uploadInfo.error = error.message;
            if (this.onProgress) {
                this.onProgress(fileId, uploadInfo);
            }
            if (this.onError) {
                this.onError(fileId, error);
            }
        }
    }

    // 发送文件块
    async sendFileChunks(file, fileId, totalChunks) {
        for (let i = 0; i < totalChunks; i++) {
            const start = i * this.chunkSize;
            const end = Math.min(start + this.chunkSize, file.size);
            const chunk = file.slice(start, end);

            const buffer = await this.readFileAsArrayBuffer(chunk);

            // 发送元数据头
            const metadata = {
                op: 'upload_chunk',
                filename: file.name,
                fileId: fileId,
                index: i,
                totalChunks: totalChunks,
                size: buffer.byteLength
            };

            this.wsClient.send(JSON.stringify(metadata));
            this.wsClient.sendBinary(buffer);

            // 更新进度
            const uploadInfo = this.activeUploads.get(fileId);
            if (uploadInfo) {
                uploadInfo.uploadedChunks = i + 1;
                uploadInfo.uploadedBytes += buffer.byteLength;
                uploadInfo.progress = Math.round((uploadInfo.uploadedBytes / file.size) * 100);

                if (this.onProgress) {
                    this.onProgress(fileId, uploadInfo);
                }
            }

            // 添加延迟以避免拥塞
            await this.delay(10);
        }
    }

    // 读取文件为 ArrayBuffer
    readFileAsArrayBuffer(file) {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result);
            reader.onerror = reject;
            reader.readAsArrayBuffer(file);
        });
    }

    // 生成文件ID
    generateFileId() {
        return 'upload_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
    }

    // 延迟函数
    delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    // 取消上传
    cancelUpload(fileId) {
        const uploadInfo = this.activeUploads.get(fileId);
        if (uploadInfo) {
            uploadInfo.status = 'cancelled';
            this.activeUploads.delete(fileId);

            const cancelMessage = {
                op: 'upload_cancel',
                fileId: fileId
            };
            this.wsClient.send(JSON.stringify(cancelMessage));
        }
    }

    // 清理已完成的上传
    cleanup(fileId) {
        this.activeUploads.delete(fileId);
    }
}

export default FileUploader;
