use tcp_p2p_server::{
    config,
    error::Result,
    message::Message,
    network::{receive_udp, send_udp},
};
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use uuid::Uuid;

struct ClientState {
    peer_id: String,
    udp_socket: Arc<UdpSocket>,
}

impl ClientState {
    async fn new() -> Result<Self> {
        let peer_id = Uuid::new_v4().to_string();

        let udp_socket = UdpSocket::bind("0.0.0.0:0").await?;
        let local_udp_addr = udp_socket.local_addr()?;
        println!("UDP bound to {}", local_udp_addr);

        let server_addr: std::net::SocketAddr = config::DEFAULT_UDP_ADDR.parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let join_msg = Message::PeerJoin {
            peer_id: peer_id.clone(),
            peer_addr: local_udp_addr.to_string(),
        };
        send_udp(&udp_socket, &server_addr, &join_msg).await?;

        Ok(Self {
            peer_id,
            udp_socket: Arc::new(udp_socket),
        })
    }

}


async fn handle_message(
    message: Message,
    addr: std::net::SocketAddr,
    peer_id: &str,
    server_addr: &std::net::SocketAddr,
    display_tx: mpsc::Sender<String>,
    cmd_tx: mpsc::Sender<Message>,
    _udp_socket: &UdpSocket,
) {
    // Only handle messages from server for control messages, all peers for P2P
    let is_from_server = addr == *server_addr;

    match message {
        Message::PeerJoin { peer_id, peer_addr } => {
            let _ = display_tx.send(format!("New peer: {} ({})", peer_id, peer_addr)).await;
        }
        Message::PeerLeave { peer_id } => {
            let _ = display_tx.send(format!("Peer left: {}", peer_id)).await;
        }
        Message::PeerListReady { peers } => {
            let mut msg = String::from("Connected peers:\n");
            for peer in &peers {
                msg.push_str(&format!("   - {} ({})\n", peer.peer_id, peer.peer_addr));
            }
            let _ = display_tx.send(msg).await;
        }
        Message::Chat { sender_id, content } => {
            if is_from_server {
                let _ = display_tx.send(format!("{}: {}", sender_id, content)).await;
            } else {
                let _ = display_tx.send(format!("[P2P] {}: {}", sender_id, content)).await;
            }
        }
        Message::PrivateMessage { from_peer, to_peer, content } => {
            let is_for_me = to_peer == peer_id;
            if is_for_me {
                if is_from_server {
                    let _ = display_tx.send(format!("[Private from {}]: {}", from_peer, content)).await;
                } else {
                    let _ = display_tx.send(format!("[P2P Private from {}]: {}", from_peer, content)).await;
                }
            }
        }
        Message::PunchReady { peer_a, peer_b, .. } => {
            let _ = display_tx.send(format!("NAT punch ready: {} <-> {}", peer_a.peer_id, peer_b.peer_id)).await;

            let target_peer = if peer_a.peer_id == peer_id {
                peer_b.peer_id.clone()
            } else {
                peer_a.peer_id.clone()
            };

            let _ = cmd_tx.send(Message::Chat {
                sender_id: peer_id.to_string(),
                content: format!("Connecting to {}", target_peer),
            }).await;
        }
        Message::RelayReady { from_peer, to_peer } => {
            let _ = display_tx.send(format!("Relay ready: {} <-> {}", from_peer, to_peer)).await;
        }
        _ => {}
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting P2P client...");

    let ClientState {
        peer_id,
        udp_socket,
    } = ClientState::new().await?;

    let (display_tx, mut display_rx) = mpsc::channel::<String>(config::DISPLAY_CHANNEL_CAPACITY);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<Message>(config::DISPLAY_CHANNEL_CAPACITY);

    let server_addr: std::net::SocketAddr = config::DEFAULT_UDP_ADDR.parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let display_tx_udp = display_tx.clone();
    let cmd_tx_udp = cmd_tx.clone();
    let peer_id_udp = peer_id.clone();
    let udp_socket_arc = udp_socket.clone();

    tokio::spawn(async move {
        loop {
            match receive_udp(&*udp_socket_arc).await {
                Ok((message, addr)) => {
                    // Handle server messages and P2P messages
                    handle_message(message, addr, &peer_id_udp, &server_addr, display_tx_udp.clone(), cmd_tx_udp.clone(), &*udp_socket_arc).await;
                }
                Err(e) => {
                    let _ = display_tx_udp.send(format!("UDP error: {}", e)).await;
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(msg) = display_rx.recv().await {
            println!("{}", msg);
        }
    });

    let cmd_socket = udp_socket.clone();
    tokio::spawn(async move {
        while let Some(message) = cmd_rx.recv().await {
            if let Err(e) = send_udp(&*cmd_socket, &server_addr, &message).await {
                eprintln!("Send error: {}", e);
                break;
            }
        }
    });

    println!("\nCommands:");
    println!("   /peers          - List peers");
    println!("   /punch <peer>   - NAT traversal");
    println!("   /relay <peer>   - Relay connection");
    println!("   /msg <peer> <text> - Relay message");
    println!("   /quit           - Exit");
    println!("   <message>       - Broadcast chat\n");

    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('/') {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            match parts.get(0).map(|s| *s) {
                Some("/quit") => {
                    println!("Bye!");
                    break;
                }
                Some("/punch") => {
                    if let Some(target_peer) = parts.get(1) {
                        let msg = Message::PunchRequest {
                            from_peer: peer_id.clone(),
                            to_peer: target_peer.to_string(),
                        };
                        let _ = cmd_tx.send(msg).await;
                    } else {
                        println!("Usage: /punch <peer_id>");
                    }
                }
                Some("/relay") => {
                    if let Some(target_peer) = parts.get(1) {
                        let msg = Message::RelayRequest {
                            from_peer: peer_id.clone(),
                            to_peer: target_peer.to_string(),
                        };
                        let _ = cmd_tx.send(msg).await;
                    } else {
                        println!("Usage: /relay <peer_id>");
                    }
                }
                Some("/peers") => {
                    let msg = Message::PeerListRequest;
                    let _ = cmd_tx.send(msg).await;
                }
                Some("/msg") => {
                    if parts.len() >= 3 {
                        let target_peer = parts[1].to_string();
                        let message_content = parts[2..].join(" ");
                        println!("Sending private message to {}: {}", target_peer, message_content);
                        let msg = Message::PrivateMessage {
                            from_peer: peer_id.clone(),
                            to_peer: target_peer,
                            content: message_content,
                        };
                        let _ = cmd_tx.send(msg).await;
                    } else {
                        println!("Usage: /msg <peer_id> <message>");
                    }
                }
                _ => {
                    println!("Unknown: {}", parts[0]);
                }
            }
        } else {
            let msg = Message::Chat {
                sender_id: peer_id.clone(),
                content: trimmed.to_string(),
            };
            let _ = cmd_tx.send(msg).await;
        }
    }

    Ok(())
}
