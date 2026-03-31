use smoltcp::iface::Config;
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_smoltcp::device::ChannelCapture;
use tokio_smoltcp::{Net, NetConfig};
use tun2::Configuration as TunConfig;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("用法: cargo run [server|client]");
        return Ok(());
    }

    let mode = args[1].to_lowercase();

    println!("Creating TUN device...");
    let tun = create_tun_device()?;

    println!("Creating network stack...");
    let net = create_net_with_tun(tun);

    match mode.as_str() {
        "server" => {
            println!("启动 TCP HTTP 服务器模式 (smoltcp)...");
            run_tcp_server(&net).await?;
        }
        "client" => {
            println!("启动 TCP 客户端模式 (smoltcp)...");
            run_tcp_client(&net).await?;
        }
        _ => {
            println!("未知模式: {}。请使用 'server' 或 'client'。", mode);
        }
    }

    println!("\nDone");
    Ok(())
}

async fn run_tcp_server(net: &Net) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = "10.0.0.2:80".parse()?;
    let mut listener = net.tcp_bind(addr).await?;
    println!("HTTP Server 正在 smoltcp 栈运行: http://{}", addr);
    println!("您可以尝试从宿主机访问: http://10.0.0.1 (如果已配置转发) 或通过 client 模式连接 10.0.0.2");

    loop {
        let (mut socket, peer_addr) = listener.accept().await?;
        println!("收到来自 {} 的连接", peer_addr);

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            match socket.read(&mut buf).await {
                Ok(n) if n > 0 => {
                    let request = String::from_utf8_lossy(&buf[..n]);
                    println!("收到来自 {} 的请求:\n{}", peer_addr, request.trim());

                    let body = "<h1>Hello from smoltcp HTTP Server!</h1><p>IP: 10.0.0.2</p>";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\n\
                        Content-Type: text/html; charset=utf-8\r\n\
                        Content-Length: {}\r\n\
                        Connection: close\r\n\
                        \r\n\
                        {}",
                        body.len(),
                        body
                    );

                    if let Err(e) = socket.write_all(response.as_bytes()).await {
                        eprintln!("发送响应失败: {}", e);
                    }
                }
                _ => {}
            }
        });
    }
}

fn create_tun_device() -> Result<tun2::AsyncDevice, Box<dyn std::error::Error>> {
    let mut config = TunConfig::default();
    config.tun_name("tun0");
    config.address("10.0.0.1");
    config.netmask("255.255.255.0");
    config.mtu(1500);
    config.up();

    let tun = tun2::create_as_async(&config)?;
    println!("TUN device created: tun0");
    Ok(tun)
}

fn create_net_with_tun(tun: tun2::AsyncDevice) -> Net {
    let mut caps = DeviceCapabilities::default();
    caps.max_transmission_unit = 1500;
    caps.medium = Medium::Ip;

    let (mut reader, mut writer) = tokio::io::split(tun);
    // 获取当前 Tokio 运行时的句柄
    let handle = tokio::runtime::Handle::current();

    let device = ChannelCapture::new(
        {
            let handle = handle.clone();
            move |sender: Sender<std::io::Result<Vec<u8>>>| {
                handle.spawn(async move {
                    let mut buf = vec![0u8; 1500];
                    println!("[TUN->Stack] 接收任务启动");
                    loop {
                        match reader.read(&mut buf).await {
                            Ok(n) if n > 0 => {
                                if sender.send(Ok(buf[..n].to_vec())).await.is_err() { break; }
                            }
                            Ok(_) => continue,
                            Err(e) => {
                                let _ = sender.send(Err(e)).await;
                                break;
                            }
                        }
                    }
                });
            }
        },
        {
            let handle = handle.clone();
            move |mut receiver: Receiver<Vec<u8>>| {
                handle.spawn(async move {
                    println!("[Stack->TUN] 发送任务启动");
                    while let Some(pkt) = receiver.recv().await {
                        if let Err(_) = writer.write_all(&pkt).await { break; }
                    }
                });
            }
        },
        caps,
    );

    let interface_config = Config::new(HardwareAddress::Ip);
    let net_config = NetConfig::new(
        interface_config,
        IpCidr::new(IpAddress::v4(10, 0, 0, 2), 24),
        vec![IpAddress::v4(10, 0, 0, 1)],
    );

    Net::new(device, net_config)
}

async fn run_tcp_client(net: &Net) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== TCP 客户端尝试连接 ===");
    
    // 如果要连接互联网 IP，OS 必须开启 NAT 或路由转发。
    // 为了演示连接成功，建议先在本地 OS 开启一个监听 10.0.0.1:80 的服务。
    let remote_addr: SocketAddr = "10.0.0.1:80".parse()?; 

    println!("正在连接目标: {}", remote_addr);
    
    match net.tcp_connect(remote_addr).await {
        Ok(mut stream) => {
            println!("连接成功！");
            let request = b"GET / HTTP/1.1\r\nHost: local\r\n\r\n";
            stream.write_all(request).await?;
            let mut buf = [0u8; 1024];
            if let Ok(n) = stream.read(&mut buf).await {
                println!("收到响应:\n{}", String::from_utf8_lossy(&buf[..n]));
            }
        }
        Err(e) => {
            eprintln!("连接失败: {}. \n提示: 请确保 OS 端在 10.0.0.1:80 有服务在运行，或者已配置 NAT 转发。", e);
        }
    }
    Ok(())
}
