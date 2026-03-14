class UIManager {
    constructor() {
        this.connectionStatus = document.getElementById('connectionStatus');
        this.uploadArea = document.getElementById('uploadArea');
        this.fileInput = document.getElementById('fileInput');
        this.filenameInput = document.getElementById('filenameInput');
        this.downloadBtn = document.getElementById('downloadBtn');
        this.progressContainer = document.getElementById('progressContainer');
        this.progressItems = new Map();
        this.onFileSelect = null;
        this.onDownloadRequest = null;
        this.onCancel = null;
        this.initEventListeners();
    }

    initEventListeners() {
        this.uploadArea.addEventListener('click', () => {
            this.fileInput.click();
        });

        this.fileInput.addEventListener('change', (e) => {
            const files = Array.from(e.target.files);
            if (files.length > 0 && this.onFileSelect) {
                this.onFileSelect(files);
            }
            this.fileInput.value = '';
        });

        this.uploadArea.addEventListener('dragover', (e) => {
            e.preventDefault();
            this.uploadArea.classList.add('dragover');
        });

        this.uploadArea.addEventListener('dragleave', () => {
            this.uploadArea.classList.remove('dragover');
        });

        this.uploadArea.addEventListener('drop', (e) => {
            e.preventDefault();
            this.uploadArea.classList.remove('dragover');
            const files = Array.from(e.dataTransfer.files);
            if (files.length > 0 && this.onFileSelect) {
                this.onFileSelect(files);
            }
        });

        this.downloadBtn.addEventListener('click', () => {
            const filename = this.filenameInput.value.trim();
            if (filename && this.onDownloadRequest) {
                this.onDownloadRequest(filename);
            }
        });

        this.filenameInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                this.filenameInput.blur();
                this.downloadBtn.click();
            }
        });
    }

    updateConnectionStatus(connected) {
        if (connected) {
            this.connectionStatus.textContent = '已连接';
            this.connectionStatus.className = 'connection-status connected';
        } else {
            this.connectionStatus.textContent = '未连接';
            this.connectionStatus.className = 'connection-status disconnected';
        }
    }

    createProgressItem(fileId, filename, type) {
        const item = document.createElement('div');
        item.className = 'progress-item';
        item.id = `progress-${fileId}`;

        item.innerHTML = `
            <div class="progress-item-header">
                <span>${type === 'upload' ? '↑ 上传' : '↓ 下载'}: ${filename}</span>
                <span class="close" data-file-id="${fileId}">×</span>
            </div>
            <div class="progress-bar">
                <div class="progress-bar-inner" style="width: 0%"></div>
            </div>
            <div class="progress-info">
                <span class="progress-text">0%</span>
                <span class="status-text">准备中...</span>
            </div>
            <div class="progress-speed-time">
                <span class="speed">⚡ 0 Mbps</span>
                <span class="time">⏱️ 0s</span>
            </div>
        `;

        const closeBtn = item.querySelector('.close');
        closeBtn.addEventListener('click', () => {
            if (this.onCancel) this.onCancel(fileId, type);
        });

        this.progressContainer.appendChild(item);
        this.progressItems.set(fileId, item);

        return item;
    }

    updateProgress(fileId, progress, status, transferredSize = null, totalSize = null, speed = null, duration = null) {
        const item = this.progressItems.get(fileId);
        if (!item) return;

        const progressBar = item.querySelector('.progress-bar-inner');
        const progressText = item.querySelector('.progress-text');
        const statusText = item.querySelector('.status-text');
        const speedElement = item.querySelector('.speed');
        const timeElement = item.querySelector('.time');

        progressBar.style.width = `${progress}%`;
        progressText.textContent = `${progress}%`;

        let statusMessage = status;
        if (transferredSize !== null && totalSize !== null) {
            statusMessage = `${status} (${this.formatSize(transferredSize)}/${this.formatSize(totalSize)})`;
        }
        statusText.textContent = statusMessage;

        if (speed !== null) speedElement.textContent = `⚡ ${speed.toFixed(2)} Mbps`;
        if (duration !== null) timeElement.textContent = `⏱️ ${this.formatDuration(duration)}`;

        const progressInfo = item.querySelector('.progress-info');
        progressInfo.classList.remove('completed', 'error');

        if (status === '已完成') progressInfo.classList.add('completed');
        else if (status === '失败' || status === '已取消') progressInfo.classList.add('error');
    }

    removeProgressItem(fileId) {
        const item = this.progressItems.get(fileId);
        if (item) {
            item.remove();
            this.progressItems.delete(fileId);
        }
    }

    formatSize(bytes) {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
    }

    formatDuration(seconds) {
        if (seconds < 60) return `${Math.round(seconds)}s`;
        if (seconds < 3600) {
            const minutes = Math.floor(seconds / 60);
            const remainingSeconds = Math.round(seconds % 60);
            return `${minutes}m ${remainingSeconds}s`;
        }
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        return `${hours}h ${minutes}m`;
    }

    setFileSelectHandler(handler) { this.onFileSelect = handler; }
    setDownloadRequestHandler(handler) { this.onDownloadRequest = handler; }
    setCancelHandler(handler) { this.onCancel = handler; }
}

export default UIManager;
