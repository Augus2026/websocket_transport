//! 消息序列化工具函数
//!
//! 提供 JSON 序列化和反序列化功能

use crate::error::{P2PError, Result};
use crate::message::Message;

/// 解析消息
#[inline]
pub fn parse_message(data: &[u8]) -> Result<Message> {
    serde_json::from_slice(data).map_err(P2PError::from)
}

/// 序列化消息
#[inline]
pub fn serialize_message(message: &Message) -> Result<Vec<u8>> {
    serde_json::to_vec(message).map_err(P2PError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::PeerInfo;

    #[test]
    fn test_serialize_message() {
        let msg = Message::Chat {
            sender_id: "peer-1".to_string(),
            content: "Hello".to_string(),
        };
        let bytes = serialize_message(&msg).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_parse_message() {
        let msg = Message::PeerListReady {
            peers: vec![PeerInfo::new("p1".to_string(), "addr1".to_string())],
        };
        let bytes = serde_json::to_vec(&msg).unwrap();
        let parsed = parse_message(&bytes).unwrap();
        matches!(parsed, Message::PeerListReady { .. });
    }
}
