// WebSocket 客户端类
class WebSocketClient {
    constructor(url) {
        this.url = url;
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        this.reconnectDelay = 3000;
        this.isConnecting = false;
        this.messageHandlers = new Map();
        this.onConnectionChange = null;
    }

    connect() {
        if (this.isConnecting || (this.ws && this.ws.readyState === WebSocket.OPEN)) {
            return;
        }

        this.isConnecting = true;

        try {
            // 优化 WebSocket 连接配置
            this.ws = new WebSocket(this.url);

            // 设置二进制类型为 ArrayBuffer（比 Blob 更高效）
            this.ws.binaryType = 'arraybuffer';

            this.ws.onopen = () => {
                console.log('WebSocket 连接已建立');
                this.reconnectAttempts = 0;
                this.isConnecting = false;
                if (this.onConnectionChange) {
                    this.onConnectionChange(true);
                }
            };

            this.ws.onmessage = (event) => {
                this.handleMessage(event.data);
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket 错误:', error);
            };

            this.ws.onclose = () => {
                console.log('WebSocket 连接已关闭');
                this.isConnecting = false;
                if (this.onConnectionChange) {
                    this.onConnectionChange(false);
                }
                this.attemptReconnect();
            };
        } catch (error) {
            console.error('连接错误:', error);
            this.isConnecting = false;
            this.attemptReconnect();
        }
    }

    attemptReconnect() {
        if (this.reconnectAttempts < this.maxReconnectAttempts) {
            this.reconnectAttempts++;
            console.log(`尝试重新连接 (${this.reconnectAttempts}/${this.maxReconnectAttempts})...`);
            setTimeout(() => this.connect(), this.reconnectDelay);
        } else {
            console.error('达到最大重连次数');
        }
    }

    handleMessage(data) {
        // 尝试解析为 JSON（元数据）
        try {
            const message = JSON.parse(data);
            const handler = this.messageHandlers.get(message.op);
            if (handler) {
                handler(message);
            }
        } catch (error) {
            // 如果不是 JSON，则是二进制数据
            // console.log('收到二进制数据:', data);
        }
    }

    send(message) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(message);
            return true;
        } else {
            console.error('WebSocket 未连接');
            return false;
        }
    }

    sendBinary(buffer) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(buffer);
            return true;
        } else {
            console.error('WebSocket 未连接');
            return false;
        }
    }

    on(operation, handler) {
        this.messageHandlers.set(operation, handler);
    }

    off(operation) {
        this.messageHandlers.delete(operation);
    }

    disconnect() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
        this.reconnectAttempts = this.maxReconnectAttempts; // 防止重连
    }

    isConnected() {
        return this.ws && this.ws.readyState === WebSocket.OPEN;
    }
}

export default WebSocketClient;
