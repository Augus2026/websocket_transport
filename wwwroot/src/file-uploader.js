class FileUploader {
    constructor(wsClient, onProgress, onComplete, onError) {
        this.wsClient = wsClient;
        this.onProgress = onProgress;
        this.onComplete = onComplete;
        this.onError = onError;
        this.chunkSize = 512 * 1024;
        this.activeUploads = new Map();
    }

    async uploadFile(file, fileId = null) {
        const finalFileId = fileId || this.generateFileId();
        const fileSize = file.size;
        const totalChunks = Math.ceil(fileSize / this.chunkSize);

        const uploadInfo = {
            file: file,
            fileId: finalFileId,
            totalChunks: totalChunks,
            uploadedChunks: 0,
            uploadedBytes: 0,
            progress: 0,
            status: 'uploading',
            startTime: Date.now(),
            lastUpdateTime: Date.now(),
            lastUploadedBytes: 0,
            speed: 0,
            duration: 0
        };

        this.activeUploads.set(finalFileId, uploadInfo);

        try {
            const startMessage = {
                op: 'upload_start',
                filename: file.name,
                size: fileSize,
                chunks: totalChunks,
                file_id: finalFileId
            };

            this.wsClient.send(JSON.stringify(startMessage));

            await this.sendFileChunks(file, finalFileId, totalChunks);

            const endMessage = {
                op: 'upload_end',
                filename: file.name,
                file_id: finalFileId
            };

            this.wsClient.send(JSON.stringify(endMessage));

            uploadInfo.status = 'completed';
            uploadInfo.progress = 100;
            if (this.onProgress) {
                this.onProgress(finalFileId, uploadInfo);
            }
            if (this.onComplete) {
                this.onComplete(finalFileId, file);
            }

        } catch (error) {
            console.error('Upload failed:', error);
            uploadInfo.status = 'error';
            uploadInfo.error = error.message;
            if (this.onProgress) {
                this.onProgress(finalFileId, uploadInfo);
            }
            if (this.onError) {
                this.onError(finalFileId, error);
            }
        }
    }

    async sendFileChunks(file, fileId, totalChunks) {
        const uploadInfo = this.activeUploads.get(fileId);
        let currentIndex = 0;

        while (currentIndex < totalChunks) {
            const start = currentIndex * this.chunkSize;
            const end = Math.min(start + this.chunkSize, file.size);
            const chunk = file.slice(start, end);

            const buffer = await this.readFileAsArrayBuffer(chunk);

            const metadata = {
                op: 'upload_chunk',
                filename: file.name,
                file_id: fileId,
                index: currentIndex,
                total_chunks: totalChunks,
                size: buffer.byteLength
            };

            this.wsClient.send(JSON.stringify(metadata));
            this.wsClient.sendBinary(buffer);

            currentIndex++;

            uploadInfo.uploadedBytes = Math.min(currentIndex * this.chunkSize, file.size);
            uploadInfo.uploadedChunks = currentIndex;

            uploadInfo.progress = Math.round((uploadInfo.uploadedBytes / file.size) * 100);

            const now = Date.now();
            const elapsedTime = (now - uploadInfo.startTime) / 1000;
            uploadInfo.duration = elapsedTime;

            if (now - uploadInfo.lastUpdateTime >= 1000) {
                const timeDiff = (now - uploadInfo.lastUpdateTime) / 1000;
                const bytesDiff = uploadInfo.uploadedBytes - uploadInfo.lastUploadedBytes;
                uploadInfo.speed = (bytesDiff * 8) / (timeDiff * 1000000);
                uploadInfo.lastUpdateTime = now;
                uploadInfo.lastUploadedBytes = uploadInfo.uploadedBytes;
            }

            if (this.onProgress) {
                this.onProgress(fileId, uploadInfo);
            }

            await this.delay(1);
        }
    }

    readFileAsArrayBuffer(file) {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result);
            reader.onerror = reject;
            reader.readAsArrayBuffer(file);
        });
    }

    generateFileId() {
        return 'upload_' + Date.now() + '_' + Math.random().toString(36).substring(2, 11);
    }

    delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    cancelUpload(fileId) {
        const uploadInfo = this.activeUploads.get(fileId);
        if (uploadInfo) {
            uploadInfo.status = 'cancelled';
            this.activeUploads.delete(fileId);

            const cancelMessage = {
                op: 'upload_cancel',
                file_id: fileId
            };
            this.wsClient.send(JSON.stringify(cancelMessage));
        }
    }

    cleanup(fileId) {
        this.activeUploads.delete(fileId);
    }
}

export default FileUploader;
