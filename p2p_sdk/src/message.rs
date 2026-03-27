use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerInfo {
    pub peer_id: String,
    pub peer_addr: String,
}

impl PeerInfo {
    #[inline]
    pub fn new(peer_id: String, peer_addr: String) -> Self {
        Self { peer_id, peer_addr }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    PeerJoin { peer_id: String, peer_addr: String },
    PeerLeave { peer_id: String },
    PeerListRequest,
    PeerListReady { peers: Vec<PeerInfo> },
    Chat { sender_id: String, content: String },
    PunchRequest { from_peer: String, to_peer: String },
    PunchReady {
        peer_a: PeerInfo,
        peer_a_udp: String,
        peer_b: PeerInfo,
        peer_b_udp: String,
    },
    RelayRequest { from_peer: String, to_peer: String },
    RelayReady { from_peer: String, to_peer: String },
    PrivateMessage { from_peer: String, to_peer: String, content: String },
}

impl Message {
    pub fn should_filter_for_sender(&self, sender_peer_id: &str) -> bool {
        match self {
            Message::Chat { sender_id, .. } => sender_id == sender_peer_id,
            Message::PeerJoin { peer_id, .. } => peer_id == sender_peer_id,
            Message::PeerLeave { peer_id } => peer_id == sender_peer_id,
            _ => false,
        }
    }

    pub fn sender_id(&self) -> Option<&str> {
        match self {
            Message::Chat { sender_id, .. } => Some(sender_id),
            Message::PunchRequest { from_peer, .. } => Some(from_peer),
            Message::RelayRequest { from_peer, .. } => Some(from_peer),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_info_creation() {
        let peer = PeerInfo::new("peer-123".to_string(), "127.0.0.1:8080".to_string());
        assert_eq!(peer.peer_id, "peer-123");
        assert_eq!(peer.peer_addr, "127.0.0.1:8080");
    }

    #[test]
    fn test_peer_info_serialization() {
        let peer = PeerInfo::new("peer-123".to_string(), "127.0.0.1:8080".to_string());
        let json = serde_json::to_string(&peer).unwrap();
        let decoded: PeerInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, peer);
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::Chat {
            sender_id: "peer-1".to_string(),
            content: "Hello!".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: Message = serde_json::from_str(&json).unwrap();
        matches!(decoded, Message::Chat { .. });
    }

    #[test]
    fn test_message_should_filter_for_sender() {
        let msg = Message::Chat {
            sender_id: "peer-1".to_string(),
            content: "Hello!".to_string(),
        };
        assert!(msg.should_filter_for_sender("peer-1"));
        assert!(!msg.should_filter_for_sender("peer-2"));
    }

    #[test]
    fn test_message_sender_id() {
        let msg = Message::Chat {
            sender_id: "peer-1".to_string(),
            content: "Hello!".to_string(),
        };
        assert_eq!(msg.sender_id(), Some("peer-1"));

        let msg = Message::PeerLeave { peer_id: "peer-1".to_string() };
        assert_eq!(msg.sender_id(), None);
    }
}
