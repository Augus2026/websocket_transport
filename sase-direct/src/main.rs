use smoltcp::iface::Config;
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_smoltcp::device::ChannelCapture;
use tokio_smoltcp::{Net, NetConfig};
use tun2::Configuration as TunConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating TUN device...");
    let tun = create_tun_device()?;

    println!("Creating network stack...");
    let net = create_net_with_tun(tun);

    run_tcp_client(&net).await?;

    println!("\nDone");
    Ok(())
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

    let (reader, writer) = tokio::io::split(tun);

    let device = ChannelCapture::new(
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

    let interface_config = Config::new(HardwareAddress::Ip);
    let net_config = NetConfig::new(
        interface_config,
        IpCidr::new(IpAddress::v4(10, 0, 0, 1), 24),
        vec![IpAddress::v4(10, 0, 0, 0)],
    );

    Net::new(device, net_config)
}

async fn run_tcp_client(net: &Net) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== TCP Client ===");

    // let remote_addr: SocketAddr = "10.0.0.2:80".parse()?;
    let remote_addr: SocketAddr = "104.18.26.120:80".parse()?;

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
