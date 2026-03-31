use smoltcp::iface::Config;
use smoltcp::phy::Medium;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_smoltcp::device::{ChannelCapture, DeviceCapabilities};
use tokio_smoltcp::{Net, NetConfig};
use tun2::Configuration as TunConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_tcp_client().await?;
    println!("\nDone");
    Ok(())
}

fn create_device_capabilities() -> DeviceCapabilities {
    let mut caps = DeviceCapabilities::default();
    caps.max_transmission_unit = 1500;
    caps.medium = Medium::Ip;
    caps
}

async fn run_tcp_client() -> Result<(), Box<dyn std::error::Error>> {
    println!("TCP Client");

    let caps = create_device_capabilities();
    let device = ChannelCapture::new(
        |_sender| {
            // In demo mode, no external data source
            // In production: read from TUN and send to sender
        },
        |_receiver| {
            // In demo mode, no external data destination
            // In production: read from receiver and write to TUN
        },
        caps,
    );

    let interface_config = Config::new(HardwareAddress::Ip);
    let net_config = NetConfig::new(
        interface_config,
        IpCidr::new(IpAddress::v4(192, 168, 1, 100), 24),
        vec![IpAddress::v4(192, 168, 1, 1)],
    );

    let net = Net::new(device, net_config);

    let remote_addr: SocketAddr = "110.242.74.102:80".parse()?;
    println!("Connecting to {} (will timeout - no real network)", remote_addr);

    // This will timeout because ChannelCapture has no real I/O
    tokio::select! {
        result = net.tcp_connect(remote_addr) => {
            match result {
                Ok(mut stream) => {
                    println!("Connected");
                    let request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
                    stream.write_all(request).await?;
                    println!("Sent {} bytes", request.len());
                    let mut buf = [0u8; 1024];
                    match stream.read(&mut buf).await {
                        Ok(n) => println!("Received {} bytes", n),
                        Err(e) => println!("Read error: {}", e),
                    }
                }
                Err(e) => println!("Connect failed: {}", e),
            }
        }
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
            println!("Connection timeout (expected in demo mode)");
        }
    }

    Ok(())
}

fn create_tun_device() -> Result<tun2::AsyncDevice, Box<dyn std::error::Error>> {
    let mut config = TunConfig::default();

    // Set TUN device configuration
    config.tun_name("tun0");
    config.tun_name("tun0");

    // Platform-specific configuration
    #[cfg(target_os = "linux")]
    {
        config.address("10.0.0.1");
        config.netmask("255.255.255.0");
        config.destination("10.0.0.2");
        config.mtu(1500);
        config.up(); // Bring up the interface
    }

    #[cfg(target_os = "windows")]
    {
        // Windows uses Wintun driver
        // Download from: https://www.wintun.net/
        config.tun_name("tun0");
        config.mtu(1500);
    }

    #[cfg(target_os = "macos")]
    {
        config.address("10.0.0.1");
        config.netmask("255.255.255.0");
        config.destination("10.0.0.2");
        config.mtu(1500);
    }

    let tun = tun2::create_as_async(&config)?;
    println!("TUN device created");

    Ok(tun)
}

fn create_net_with_tun(tun: tun2::AsyncDevice) -> Net {
    let mut caps = DeviceCapabilities::default();
    caps.max_transmission_unit = 1500;
    caps.medium = Medium::Ip;

    // Split TUN into reader and writer
    let (mut reader, mut writer) = tokio::io::split(tun);

    // Create ChannelCapture with TUN I/O
    let device = ChannelCapture::new(
        // Receiver: read packets from TUN -> send to network stack
        move |sender| {
            tokio::spawn(async move {
                let mut buf = vec![0u8; 1500];
                loop {
                    match reader.read(&mut buf).await {
                        Ok(n) if n > 0 => {
                            let pkt = buf[..n].to_vec();
                            if sender.send(Ok(pkt)).await.is_err() {
                                break;
                            }
                        }
                        Ok(_) => continue,
                        Err(e) => {
                            eprintln!("TUN read error: {}", e);
                            break;
                        }
                    }
                }
            });
        },
        // Sender: receive packets from network stack -> write to TUN
        move |mut receiver| {
            tokio::spawn(async move {
                while let Some(pkt) = receiver.recv().await {
                    if let Err(e) = writer.write_all(&pkt).await {
                        eprintln!("TUN write error: {}", e);
                        break;
                    }
                }
            });
        },
        caps,
    );

    let interface_config = Config::new(HardwareAddress::Ip);
    let net_config = NetConfig::new(
        interface_config,
        IpCidr::new(IpAddress::v4(10, 0, 0, 1), 24),
        vec![IpAddress::v4(10, 0, 0, 2)],
    );

    Net::new(device, net_config)
}
