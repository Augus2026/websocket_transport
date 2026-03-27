use crate::error::{validate_message_length, P2PError, Result, MAX_MESSAGE_SIZE};
use crate::message::Message;
use tokio::net::UdpSocket;

pub async fn send_udp(socket: &UdpSocket, addr: &std::net::SocketAddr, message: &Message) -> Result<()> {
    let message_bytes = serde_json::to_vec(message)?;
    validate_message_length(message_bytes.len())?;
    socket.send_to(&message_bytes, addr).await?;
    Ok(())
}

pub async fn receive_udp(socket: &UdpSocket) -> Result<(Message, std::net::SocketAddr)> {
    let mut buf = vec![0u8; MAX_MESSAGE_SIZE];
    let (len, addr) = socket.recv_from(&mut buf).await?;
    validate_message_length(len)?;

    let message = serde_json::from_slice(&buf[..len])?;
    Ok((message, addr))
}

#[inline]
pub async fn receive_udp_into(
    socket: &UdpSocket,
    buf: &mut [u8],
) -> Result<(usize, std::net::SocketAddr)> {
    let (len, addr) = socket.recv_from(buf).await?;
    Ok((len, addr))
}

#[inline]
pub fn parse_message(data: &[u8]) -> Result<Message> {
    serde_json::from_slice(data).map_err(P2PError::from)
}

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

    #[test]
    fn test_validate_message_length() {
        assert!(validate_message_length(100).is_ok());
        assert!(validate_message_length(0).is_err());
        assert!(validate_message_length(MAX_MESSAGE_SIZE + 1).is_err());
    }
}
