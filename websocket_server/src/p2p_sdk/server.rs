use std::net::SocketAddr;
use std::sync::Arc;
use tcp_p2p_server::{
    config,
    error::Result,
    message::Message,
    network::{receive_udp, send_udp},
    registry::{PeerRegistry, RelaySessionRegistry},
    RelayTask,
};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, Mutex};

async fn handle_udp_messages(
    socket: Arc<UdpSocket>,
    registry: Arc<Mutex<PeerRegistry>>,
    broadcast_tx: broadcast::Sender<Message>,
    relay_tx: mpsc::Sender<RelayTask>,
    server_addr: SocketAddr,
) {
    loop {
        match receive_udp(&socket).await {
            Ok((message, addr)) => {
                // Process message based on sender
                if addr == server_addr {
                    // Ignore messages from ourselves
                    continue;
                }

                match &message {
                    Message::PeerJoin { peer_id, peer_addr: _ } => {
                        println!("PeerJoin from {}: {}", addr, peer_id);

                        let mut reg = registry.lock().await;

                        if !reg.contains_peer(peer_id) {
                            // Store the client's address directly
                            reg.add_peer(peer_id.clone(), addr);

                            // Send peer list to new peer
                            let peer_list = Message::PeerListReady {
                                peers: reg.get_peer_list(),
                            };
                            if let Err(e) = send_udp(&*socket, &addr, &peer_list).await {
                                eprintln!("Send peer list error: {}", e);
                            }

                            // Broadcast join message to all peers
                            let _ = broadcast_tx.send(message.clone());
                        }
                    }

                    Message::PeerListRequest => {
                        println!("PeerListRequest from {}", addr);
                        let reg = registry.lock().await;
                        let peer_list = Message::PeerListReady {
                            peers: reg.get_peer_list(),
                        };

                        // Find peer by address and send peer list
                        for peer in reg.get_peer_list() {
                            let peer_addr: std::result::Result<SocketAddr, _> = peer.peer_addr.parse();

                            if let Ok(parsed_addr) = peer_addr {
                                if parsed_addr == addr {
                                    if let Err(e) = send_udp(&*socket, &parsed_addr, &peer_list).await {
                                        eprintln!("Send peer list error: {}", e);
                                    }
                                    break;
                                }
                            }
                        }
                    }

                    Message::PunchRequest { from_peer, to_peer } => {
                        println!("PunchRequest: {} -> {}", from_peer, to_peer);
                        handle_punch_request(&registry, from_peer, to_peer, &socket).await;
                    }

                    Message::RelayRequest { from_peer, to_peer } => {
                        println!("RelayRequest: {} -> {}", from_peer, to_peer);
                        let _ = relay_tx.send(RelayTask { from_peer: from_peer.clone(), to_peer: to_peer.clone() }).await;
                    }

                    Message::Chat { .. } => {
                        println!("Chat from {}", addr);
                        let _ = broadcast_tx.send(message);
                    }

                    Message::PrivateMessage { from_peer, to_peer, content } => {
                        println!("Private message: {} -> {}: '{}'", from_peer, to_peer, content);
                        // Forward private message to the target peer
                        let reg = registry.lock().await;
                        if let Some(target_peer) = reg.get_peer(to_peer) {
                            let target_addr = target_peer.addr;
                            drop(reg); // Release lock before sending
                            if let Err(e) = send_udp(&*socket, &target_addr, &message).await {
                                eprintln!("Failed to send private message to {}: {}", to_peer, e);
                            } else {
                                println!("Private message sent to {}", to_peer);
                            }
                        } else {
                            println!("Target peer {} not found for private message", to_peer);
                        }
                    }

                    _ => {
                        println!("Unknown message from {}: {:?}", addr, message);
                    }
                }
            }
            Err(e) => {
                eprintln!("UDP recv error: {}", e);
            }
        }
    }
}

async fn handle_punch_request(
    registry: &Arc<Mutex<PeerRegistry>>,
    from_peer: &str,
    to_peer: &str,
    udp_socket: &Arc<UdpSocket>,
) {
    let reg = registry.lock().await;

    let from_info = reg.get_peer(from_peer);
    let to_info = reg.get_peer(to_peer);

    if let (Some(from), Some(to)) = (from_info, to_info) {
        let ready_msg_a = Message::PunchReady {
            peer_a: from.info.clone(),
            peer_a_udp: from.addr.to_string(),
            peer_b: to.info.clone(),
            peer_b_udp: to.addr.to_string(),
        };
        let ready_msg_b = Message::PunchReady {
            peer_a: to.info.clone(),
            peer_a_udp: to.addr.to_string(),
            peer_b: from.info.clone(),
            peer_b_udp: from.addr.to_string(),
        };

        let _ = send_udp(&*udp_socket, &from.addr, &ready_msg_a).await;
        let _ = send_udp(&*udp_socket, &to.addr, &ready_msg_b).await;
    } else {
        println!("UDP not available, fallback relay");
    }
}

async fn notify_relay_rejected(
    _reg: &PeerRegistry,
    from_peer: &str,
    to_peer: &str,
    reason: &str,
) {
    println!("Relay rejected: {} -> {} ({})", from_peer, to_peer, reason);
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Server starting...");
    println!("UDP: {}", config::DEFAULT_UDP_ADDR);

    let udp_socket = UdpSocket::bind(config::DEFAULT_UDP_ADDR).await?;
    let server_addr = udp_socket.local_addr()?;

    let registry = Arc::new(Mutex::new(PeerRegistry::new()));
    let (broadcast_tx, _) = broadcast::channel(config::BROADCAST_CAPACITY);
    let (relay_tx, mut relay_rx): (mpsc::Sender<RelayTask>, mpsc::Receiver<RelayTask>) = mpsc::channel(config::RELAY_CHANNEL_CAPACITY);

    let udp_socket_arc = Arc::new(udp_socket);

    let registry_clone = registry.clone();
    let broadcast_clone = broadcast_tx.clone();
    let relay_clone = relay_tx.clone();
    let socket_clone1 = udp_socket_arc.clone();
    let server_addr_clone = server_addr;

    tokio::spawn(async move {
        handle_udp_messages(
            socket_clone1,
            registry_clone,
            broadcast_clone,
            relay_clone,
            server_addr_clone,
        ).await;
    });

    // Broadcast handler - forwards messages to all peers
    let mut broadcast_rx = broadcast_tx.subscribe();
    let broadcast_registry = registry.clone();
    let broadcast_socket = udp_socket_arc.clone();

    tokio::spawn(async move {
        while let Ok(message) = broadcast_rx.recv().await {
            let reg = broadcast_registry.lock().await;
            let peers = reg.get_peer_list();

            if let Message::Chat { sender_id, content } = &message {
                println!("Broadcasting chat from {}: '{}'", sender_id, content);
                // Forward chat message to all peers except sender
                for peer in peers {
                    if peer.peer_id != *sender_id {
                        let peer_addr: std::result::Result<SocketAddr, _> = peer.peer_addr.parse();
                        if let Ok(addr) = peer_addr {
                            println!("Forwarding to {} at {}", peer.peer_id, addr);
                            if let Err(e) = send_udp(&*broadcast_socket, &addr, &message).await {
                                eprintln!("Failed to forward message to {}: {}", peer.peer_id, e);
                            }
                        }
                    }
                }
            }
        }
    });

    let relay_sessions = Arc::new(Mutex::new(RelaySessionRegistry::new()));

    let relay_registry = registry.clone();
    let relay_sessions_clone = relay_sessions.clone();

    tokio::spawn(async move {
        while let Some(task) = relay_rx.recv().await {
            let reg = relay_registry.lock().await;
            let mut sessions = relay_sessions_clone.lock().await;

            let peer_a_exists = reg.contains_peer(&task.from_peer);
            let peer_b_exists = reg.contains_peer(&task.to_peer);

            if !peer_a_exists || !peer_b_exists {
                let reason = if !peer_b_exists {
                    "Target not found"
                } else {
                    "Error"
                };
                notify_relay_rejected(&reg, &task.from_peer, &task.to_peer, reason).await;
                continue;
            }

            let session_id = if let Some(existing) = sessions.get_session_for_peers(&task.from_peer, &task.to_peer) {
                existing.session_id.clone()
            } else {
                let sid = sessions.create_session(task.from_peer.clone(), task.to_peer.clone());
                println!("Session created: {}", sid);
                sid
            };

            sessions.activate_session(&session_id);

            // Relay ready notifications sent via broadcast
            println!("Relay ready: {} <-> {}", task.from_peer, task.to_peer);

            println!("Relay: {} <-> {} ({})",
                task.from_peer, task.to_peer, session_id);
        }
    });

    println!("Server ready!");

    // Keep the main task alive
    tokio::signal::ctrl_c().await?;

    println!("Server shutting down...");
    Ok(())
}
