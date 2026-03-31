# Rust TUN + smoltcp 网络栈示例 (my-project)

这是一个基于 Rust 开发的示例项目，展示了如何通过 `tun2` 库创建虚拟网卡（TUN 设备），并使用 `smoltcp` (通过 `tokio-smoltcp`) 在用户态实现一个完整的 TCP/IP 协议栈。

## 核心特性

- **TUN 设备集成**：使用 `tun2` 创建虚拟网络接口，将 OS 流量重定向到用户态。
- **用户态协议栈**：集成 `smoltcp`，绕过内核协议栈直接处理 IP 数据包。
- **异步 IO**：完全基于 `tokio` 异步运行时，支持并发的包处理。
- **TCP 客户端实现**：展示了如何通过用户态协议栈发起 TCP 连接。

## 网络拓扑

为了避免 IP 冲突并实现通信，本项目采用了以下配置：

| 实体 | IP 地址 | 说明 |
| :--- | :--- | :--- |
| **Windows OS (tun0)** | `10.0.0.1` | 虚拟网卡在操作系统侧的地址 |
| **User-space Stack** | `10.0.0.2` | `smoltcp` 协议栈内部使用的地址 |
| **Gateway** | `10.0.0.1` | 协议栈将 OS 侧视为网关 |

## 环境要求

### 1. 管理员权限
创建 TUN 设备涉及操作系统的网卡驱动，必须以**管理员/Root 身份**运行程序。

### 2. Windows 驱动 (Wintun)
在 Windows 上运行需要 `wintun.dll`：
- 请从 [wintun.net](https://www.wintun.net/) 下载。
- 将对应架构的 `wintun.dll` 放置在项目根目录或生成的 `.exe` 同级目录下。

## 快速开始

### 编译
```powershell
cargo build
```

### 运行
1. 以管理员权限打开终端。
2. 运行程序：
   ```powershell
   cargo run
   ```

## 验证连接

由于 `smoltcp` 栈运行在用户态，它无法直接通过操作系统的默认路由访问公网。最简单的验证方式是让协议栈连接 OS 侧的服务：

1. **在本地 OS 启动监听服务**（在另一个终端运行）：
   ```powershell
   # 假设已安装 Python
   python -m http.server 80 --bind 10.0.0.1
   ```
2. **观察程序输出**：
   程序会尝试连接 `10.0.0.1:80`，如果一切正常，你会看到：
   - `[TUN->Stack] 接收任务启动`
   - `[Stack->TUN] 发送任务启动`
   - `连接成功！`
   - 收到来自 Python 服务的 HTTP 响应头。

## 依赖库

- [tokio](https://github.com/tokio-rs/tokio): 异步运行时。
- [smoltcp](https://github.com/smoltcp-rs/smoltcp): 纯 Rust 编写的 TCP/IP 栈。
- [tun2](https://github.com/tun2/tun2): 跨平台的 TUN 设备操作库。
- [tokio-smoltcp](https://github.com/m-ou-se/tokio-smoltcp): 为 smoltcp 提供的 Tokio 适配器。
