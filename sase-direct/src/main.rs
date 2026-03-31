use smoltcp::iface::Config;
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_smoltcp::device::ChannelCapture;
use tokio_smoltcp::{Net, NetConfig};
use tun2::Configuration as TunConfig;

/// TUN device wrapper for smoltcp
///
/// Data flow:
/// ```
/// Application (TcpStream)
///       │
///       ▼
/// tokio-smoltcp (Net)
///       │
///       ▼
/// smoltcp (TCP/IP stack)
///       │
///       ▼
/// ChannelCapture
///   ┌───┴───┐
///   │       │
/// recv    send
/// closure closure
///   │       │
///   ▼       ▼
/// TUN device (virtual network interface)
///       │
///       ▼
/// OS Network Stack (routing/NAT)
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== TUN + smoltcp Demo ===\n");

    // Create TUN device
    println!("Creating TUN device...");
    let tun = create_tun_device()?;

    // Create network stack
    println!("Creating network stack...");
    let net = create_net_with_tun(tun);

    // Run TCP client
    run_tcp_client(&net).await?;

    println!("\nDone");
    Ok(())
}

fn create_tun_device() -> Result<tun2::AsyncDevice, Box<dyn std::error::Error>> {
    let mut config = TunConfig::default();
    config.tun_name("tun0");

    // Platform-specific configuration
    #[cfg(target_os = "linux")]
    {
        // Linux: Configure TUN with IP address
        config.address("10.0.0.1");
        config.netmask("255.255.255.0");
        config.destination("10.0.0.2"); // Point-to-point destination
        config.mtu(1500);
        config.up(); // Bring up the interface

        // Note: You need to enable IP forwarding and NAT:
        // sysctl -w net.ipv4.ip_forward=1
        // iptables -t nat -A POSTROUTING -s 10.0.0.0/24 -o eth0 -j MASQUERADE
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: Wintun driver required
        config.mtu(1500);

        // Note: Windows requires manual route configuration:
        // route add 10.0.0.0 mask 255.255.255.0 10.0.0.1
        // Or use: netsh interface ip set address "tun0" static 10.0.0.1 255.255.255.0
    }

    #[cfg(target_os = "macos")]
    {
        config.address("10.0.0.1");
        config.netmask("255.255.255.0");
        config.destination("10.0.0.2");
        config.mtu(1500);

        // Note: macOS requires manual configuration:
        // ifconfig tun0 10.0.0.1 10.0.0.2 up
        // sysctl -w net.inet.ip.forwarding=1
    }

    let tun = tun2::create_as_async(&config)?;
    println!("TUN device created: tun0");
    Ok(tun)
}

fn create_net_with_tun(tun: tun2::AsyncDevice) -> Net {
    let mut caps = DeviceCapabilities::default();
    caps.max_transmission_unit = 1500;
    caps.medium = Medium::Ip;

    // Split TUN device for bidirectional I/O
    let (reader, writer) = tokio::io::split(tun);

    // Create ChannelCapture with TUN I/O
    let device = ChannelCapture::new(
        // Receiver closure: TUN -> smoltcp
        // Reads packets from TUN and sends to smoltcp network stack
        move |sender: Sender<std::io::Result<Vec<u8>>>| {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(async move {
                let mut reader = reader;
                let mut buf = vec![0u8; 1500];

                println!("[TUN->Stack] Receiver thread started");
                loop {
                    match reader.read(&mut buf).await {
                        Ok(n) if n > 0 => {
                            let pkt = buf[..n].to_vec();
                            println!("[TUN->Stack] Received {} bytes from TUN", n);
                            if sender.send(Ok(pkt)).await.is_err() {
                                eprintln!("[TUN->Stack] Channel closed");
                                break;
                            }
                        }
                        Ok(_) => continue,
                        Err(e) => {
                            eprintln!("[TUN->Stack] Read error: {}", e);
                            let _ = sender.send(Err(e)).await;
                            break;
                        }
                    }
                }
            });
        },
        // Sender closure: smoltcp -> TUN
        // Receives packets from smoltcp and writes to TUN
        move |receiver: Receiver<Vec<u8>>| {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(async move {
                let mut writer = writer;
                let mut receiver = receiver;

                println!("[Stack->TUN] Sender thread started");
                while let Some(pkt) = receiver.recv().await {
                    println!("[Stack->TUN] Sending {} bytes to TUN", pkt.len());
                    if let Err(e) = writer.write_all(&pkt).await {
                        eprintln!("[Stack->TUN] Write error: {}", e);
                        break;
                    }
                }
                println!("[Stack->TUN] Sender thread exiting");
            });
        },
        caps,
    );

    // Configure smoltcp interface
    let interface_config = Config::new(HardwareAddress::Ip);
    let net_config = NetConfig::new(
        interface_config,
        IpCidr::new(IpAddress::v4(10, 0, 0, 1), 24), // Local IP
        vec![IpAddress::v4(10, 0, 0, 2)],            // Gateway (point-to-point peer)
    );

    Net::new(device, net_config)
}

async fn run_tcp_client(net: &Net) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== TCP Client ===");

    // Note: The target must be reachable through the TUN interface
    // For point-to-point TUN, use the peer address (10.0.0.2)
    // For external addresses, you need proper routing/NAT configuration

    // Option 1: Connect to peer (point-to-point)
    let remote_addr: SocketAddr = "10.0.0.2:80".parse()?;

    // Option 2: External address (requires routing/NAT)
    // let remote_addr: SocketAddr = "104.18.26.120:80".parse()?;

    println!("Connecting to {}", remote_addr);
    println!("Note: Target must be reachable through TUN interface\n");

    match net.tcp_connect(remote_addr).await {
        Ok(mut stream) => {
            println!("Connected!");

            let request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
            stream.write_all(request).await?;
            println!("Sent {} bytes", request.len());

            let mut buf = [0u8; 4096];
            match stream.read(&mut buf).await {
                Ok(n) => {
                    println!("Received {} bytes", n);
                    if n > 0 {
                        println!("Response:\n{}", String::from_utf8_lossy(&buf[..n.min(500)]));
                    }
                }
                Err(e) => println!("Read error: {}", e),
            }
        }
        Err(e) => println!("Connect failed: {}", e),
    }

    Ok(())
}
