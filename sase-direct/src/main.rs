use smoltcp::iface::Config;
use smoltcp::phy::Medium;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_smoltcp::device::{ChannelCapture, DeviceCapabilities};
use tokio_smoltcp::{Net, NetConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_tcp_client().await?;
    run_tcp_server().await?;
    println!("Done");
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
        |_sender| {},
        |_receiver| {},
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
    println!("Connecting to {}", remote_addr);

    match net.tcp_connect(remote_addr).await {
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

    Ok(())
}

async fn run_tcp_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("TCP Server");

    let caps = create_device_capabilities();
    let device = ChannelCapture::new(
        |_sender| {},
        |_receiver| {},
        caps,
    );

    let interface_config = Config::new(HardwareAddress::Ip);
    let net_config = NetConfig::new(
        interface_config,
        IpCidr::new(IpAddress::v4(192, 168, 1, 100), 24),
        vec![IpAddress::v4(192, 168, 1, 1)],
    );

    let net = Net::new(device, net_config);

    let local_addr: SocketAddr = "192.168.1.100:8080".parse()?;
    println!("Binding to {}", local_addr);

    match net.tcp_bind(local_addr).await {
        Ok(mut listener) => {
            println!("Listening");

            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((mut stream, addr)) => {
                            println!("Accepted connection from {}", addr);
                            let mut buf = [0u8; 1024];
                            match stream.read(&mut buf).await {
                                Ok(n) if n > 0 => {
                                    println!("Received {} bytes", n);
                                    stream.write_all(&buf[..n]).await?;
                                    println!("Echoed");
                                }
                                Ok(_) => println!("Connection closed"),
                                Err(e) => println!("Read error: {}", e),
                            }
                        }
                        Err(e) => println!("Accept error: {}", e),
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    println!("Timeout");
                }
            }
        }
        Err(e) => println!("Bind failed: {}", e),
    }

    Ok(())
}
