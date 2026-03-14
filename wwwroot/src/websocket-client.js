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
        if (this.isConnecting || (this.ws && this.ws.readyState === WebSocket.OPEN)) return;

        this.isConnecting = true;

        try {
            this.ws = new WebSocket(this.url);
            this.ws.binaryType = 'arraybuffer';

            this.ws.onopen = () => {
                this.reconnectAttempts = 0;
                this.isConnecting = false;
                if (this.onConnectionChange) this.onConnectionChange(true);
            };

            this.ws.onmessage = (event) => {
                this.handleMessage(event.data);
            };

            this.ws.onerror = () => {};

            this.ws.onclose = () => {
                this.isConnecting = false;
                if (this.onConnectionChange) this.onConnectionChange(false);
                this.attemptReconnect();
            };
        } catch (error) {
            this.isConnecting = false;
            this.attemptReconnect();
        }
    }

    attemptReconnect() {
        if (this.reconnectAttempts < this.maxReconnectAttempts) {
            this.reconnectAttempts++;
            setTimeout(() => this.connect(), this.reconnectDelay);
        }
    }

    handleMessage(data) {
        try {
            const message = JSON.parse(data);
            const handler = this.messageHandlers.get(message.op);
            if (handler) handler(message);
        } catch (error) {}
    }

    send(message) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(message);
            return true;
        }
        return false;
    }

    sendBinary(buffer) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(buffer);
            return true;
        }
        return false;
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
        this.reconnectAttempts = this.maxReconnectAttempts;
    }

    isConnected() {
        return this.ws && this.ws.readyState === WebSocket.OPEN;
    }
}

export default WebSocketClient;
