# P2P SDK

一个功能完整的 Rust P2P 通信 SDK，支持 UDP 点对点通信、NAT 穿透和中继转发。

## 项目概述

P2P SDK 提供了一套完整的点对点通信解决方案，包括：
- 服务端：管理对等节点、处理连接请求、提供中继服务
- 客户端：自动发现其他节点、建立 P2P 连接、发送/接收消息
- 配置管理：灵活的 TOML 配置文件支持
- 错误处理：完善的错误类型和异常处理机制

## 功能特性

### 核心功能
- ✅ UDP 点对点通信
- ✅ 对等节点自动发现和管理
- ✅ NAT 穿透支持（打洞）
- ✅ 消息中继转发
- ✅ 广播消息
- ✅ 私有消息
- ✅ 配置文件持久化
- ✅ 连接状态跟踪

### 技术特性
- 🚀 基于 Tokio 异步运行时
- 🔒 完善的错误处理机制
- 📦 模块化设计，易于集成
- ⚙️ 灵活的配置系统
- 🧪 完整的单元测试覆盖

## 项目结构

```text
p2p_sdk/
├── Cargo.toml              # 项目配置和依赖
├── src/
│   ├── lib.rs             # 库入口，导出公共 API
│   ├── config.rs          # 配置管理模块
│   ├── error.rs           # 错误类型定义
│   ├── message.rs         # 消息协议定义
│   ├── network.rs         # 网络通信工具
│   ├── registry.rs        # 对等节点和会话注册表
│   ├── server.rs          # 服务端实现
│   └── client.rs          # 客户端实现
└── README.md              # 本文件
```

## 快速开始

### 安装依赖

```bash
cargo build --release
```

### 命令行使用

P2P SDK 提供了简单的命令行接口来启动服务端或客户端：

```bash
# 启动服务端
cargo run --release -- server

# 启动客户端
cargo run --release -- client

# 查看帮助信息
cargo run --release -- --help
```

### 服务端使用

```rust
use p2p_sdk::server::run_server;
use p2p_sdk::config::ServerConfig;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig {
        tcp_addr: "127.0.0.1:8080".to_string(),
        udp_addr: "127.0.0.1:8081".to_string(),
        broadcast_capacity: 1000,
        relay_channel_capacity: 100,
        max_message_size: 65536,
        verbose: true,
    };

    let udp_addr: SocketAddr = config.udp_addr.parse()?;
    run_server(udp_addr, config).await?;

    Ok(())
}
```

### 客户端使用

```rust
use p2p_sdk::client::run_client;
use p2p_sdk::config::ClientConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientConfig {
        server_tcp_addr: "127.0.0.1:8080".to_string(),
        server_udp_addr: "127.0.0.1:8081".to_string(),
        local_udp_addr: "0.0.0.0:0".to_string(),
        display_channel_capacity: 100,
        max_message_size: 65536,
        verbose: true,
        auto_connect: true,
        enable_p2p: true,
    };

    run_client(config).await?;

    Ok(())
}
```

### 客户端命令

启动客户端后，可以使用以下命令：

- `/peers` - 列出所有连接的对等节点
- `/punch <peer_id>` - 尝试与指定节点进行 NAT 穿透
- `/relay <peer_id>` - 通过服务器与指定节点建立中继连接
- `/msg <peer_id> <message>` - 发送私有消息给指定节点
- `/quit` - 退出客户端
- 直接输入文本 - 向所有节点广播聊天消息

## 消息协议

SDK 定义了以下消息类型：

```rust
pub enum Message {
    PeerJoin { peer_id: String, peer_addr: String },      // 节点加入
    PeerLeave { peer_id: String },                        // 节点离开
    PeerListRequest,                                       // 请求节点列表
    PeerListReady { peers: Vec<PeerInfo> },                // 节点列表响应
    Chat { sender_id: String, content: String },           // 聊天消息
    PunchRequest { from_peer: String, to_peer: String },  // 打洞请求
    PunchReady { peer_a: PeerInfo, peer_a_udp: String, peer_b: PeerInfo, peer_b_udp: String }, // 打洞准备
    RelayRequest { from_peer: String, to_peer: String },  // 中继请求
    RelayReady { from_peer: String, to_peer: String },   // 中继准备
    PrivateMessage { from_peer: String, to_peer: String, content: String }, // 私有消息
}
```

## 配置说明

### 默认配置

```toml
[server]
tcp_addr = "127.0.0.1:8080"
udp_addr = "127.0.0.1:8081"
broadcast_capacity = 1000
relay_channel_capacity = 100
max_message_size = 65536
verbose = false

[client]
server_tcp_addr = "127.0.0.1:8080"
server_udp_addr = "127.0.0.1:8081"
local_udp_addr = "0.0.0.0:0"
display_channel_capacity = 100
max_message_size = 65536
verbose = false
auto_connect = true
enable_p2p = true
```

### 配置文件管理

```rust
use p2p_sdk::config::ConfigManager;

// 创建配置管理器
let config_manager = ConfigManager::new("my_app");

// 加载配置
let config = config_manager.load();

// 保存配置
config_manager.save(&config)?;

// 更新服务器配置
config_manager.update_server(|server_config| {
    server_config.verbose = true;
    server_config.broadcast_capacity = 2000;
})?;
```

## 核心模块

### 1. 错误处理 (`error.rs`)

提供完整的错误类型定义和转换：

```rust
pub enum P2PError {
    Io(io::Error),
    Serialization(serde_json::Error),
    InvalidMessageLength { length: usize, max: usize },
    MessageParse(String),
    PeerNotFound { peer_id: String },
    ConnectionClosed,
    UdpAddressNotAvailable { peer_id: String },
    RelaySessionError { reason: String },
    ChannelError(String),
    ConfigError(String),
}
```

### 2. 消息协议 (`message.rs`)

定义 P2P 通信的消息结构和 Peer 信息：

```rust
pub struct PeerInfo {
    pub peer_id: String,
    pub peer_addr: String,
}

pub enum Message {
    // 各种消息类型...
}
```

### 3. 网络通信 (`network.rs`)

提供 UDP 通信的工具函数：

```rust
pub async fn send_udp(socket: &UdpSocket, addr: &SocketAddr, message: &Message) -> Result<()>
pub async fn receive_udp(socket: &UdpSocket) -> Result<(Message, SocketAddr)>
pub fn parse_message(data: &[u8]) -> Result<Message>
pub fn serialize_message(message: &Message) -> Result<Vec<u8>>
```

### 4. 注册表管理 (`registry.rs`)

管理对等节点和中继会话：

```rust
pub struct PeerRegistry {
    pub peers: HashMap<String, PeerConnection>,
}

pub struct RelaySessionRegistry {
    sessions: HashMap<String, RelaySession>,
    peer_sessions: HashMap<String, Vec<String>>,
}
```

## NAT 穿透机制

SDK 实现了完整的 NAT 穿透流程：

1. **节点注册**: 客户端连接服务器并注册自己的 UDP 地址
2. **节点发现**: 服务器广播新加入的节点信息
3. **打洞请求**: 客户端向服务器请求与目标节点建立连接
4. **地址交换**: 服务器交换双方的 UDP 地址
5. **P2P 连接**: 客户端尝试直接连接对方
6. **中继回退**: 如果 P2P 连接失败，使用服务器中继

## API 文档

### 公共类型

```rust
// 重新导出的公共类型
pub use error::{P2PError, Result};
pub use message::{Message, PeerInfo};
pub use registry::{PeerConnection, PeerRegistry, RelaySession, RelaySessionRegistry, RelayState};
```

### 服务端函数

```rust
pub async fn run_server(udp_addr: SocketAddr, config: ServerConfig) -> Result<()>
```

启动 P2P 服务器，监听指定的 UDP 地址。

### 客户端函数

```rust
pub async fn run_client(config: ClientConfig) -> Result<()>
```

启动 P2P 客户端，连接到服务器并开始 P2P 通信。

## 测试

运行单元测试：

```bash
cargo test
```

运行特定测试：

```bash
cargo test test_relay_state
```

## 性能优化

SDK 包含以下性能优化：

- 异步 I/O：使用 Tokio 实现高并发
- 零拷贝：消息序列化优化
- 连接池：重用 UDP 连接
- 批量处理：支持批量消息处理
- 内存管理：合理的缓冲区大小配置

## 安全考虑

⚠️ **安全提示**：

- 在生产环境中使用前，请添加身份验证机制
- 实现消息加密以保护通信隐私
- 添加速率限制防止 DDoS 攻击
- 验证所有输入数据
- 使用安全的随机数生成器（SDK 已使用 UUID v4）

## 故障排除

### 常见问题

1. **连接失败**
   - 检查防火墙设置
   - 确认 UDP 端口未被占用
   - 验证服务器地址配置

2. **NAT 穿透失败**
   - 检查 NAT 类型
   - 确保 STUN/TURN 服务器可用
   - 启用详细日志查看问题

3. **消息丢失**
   - 调整 `max_message_size` 配置
   - 检查网络稳定性
   - 增加缓冲区容量

### 调试模式

启用详细日志：

```rust
let config = ServerConfig {
    verbose: true,
    // ... 其他配置
};
```

## 依赖项

- `serde` - 序列化/反序列化
- `serde_json` - JSON 格式支持
- `tokio` - 异步运行时
- `thiserror` - 错误处理
- `uuid` - 唯一标识符生成
- `dirs` - 系统目录管理
- `toml` - 配置文件解析

## 版本历史

### v0.1.0

- 初始版本
- 完整的 P2P 通信功能
- NAT 穿透支持
- 配置管理系统
- 完善的错误处理

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

MIT License

## 联系方式

如有问题或建议，请通过 GitHub Issues 联系。