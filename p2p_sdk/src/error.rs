use std::io;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, P2PError>;

#[derive(Debug, Error)]
pub enum P2PError {
    #[error("I/O error: {0}")]
    Io(#[source] io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid message length: {length} (max: {max})")]
    InvalidMessageLength { length: usize, max: usize },
    #[error("Failed to parse message: {0}")]
    MessageParse(String),
    #[error("Peer not found: {peer_id}")]
    PeerNotFound { peer_id: String },
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("UDP address not available for peer: {peer_id}")]
    UdpAddressNotAvailable { peer_id: String },
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    // WebSocket 相关错误
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Protocol mismatch: {0}")]
    ProtocolMismatch(String),
}

impl From<io::Error> for P2PError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            P2PError::ConnectionClosed
        } else {
            P2PError::Io(e)
        }
    }
}

pub const MAX_MESSAGE_SIZE: usize = 65536;

#[inline]
pub fn validate_message_length(length: usize) -> Result<()> {
    if length == 0 || length > MAX_MESSAGE_SIZE {
        Err(P2PError::InvalidMessageLength {
            length,
            max: MAX_MESSAGE_SIZE,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = P2PError::PeerNotFound {
            peer_id: "test-peer".to_string(),
        };
        assert_eq!(err.to_string(), "Peer not found: test-peer");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "connection refused");
        let p2p_err: P2PError = io_err.into();
        matches!(p2p_err, P2PError::Io(_));
    }

    #[test]
    fn test_unexpected_eof_conversion() {
        let io_err = io::Error::new(io::ErrorKind::UnexpectedEof, "unexpected eof");
        let p2p_err: P2PError = io_err.into();
        matches!(p2p_err, P2PError::ConnectionClosed);
    }

    #[test]
    fn test_validate_message_length() {
        assert!(validate_message_length(100).is_ok());
        assert!(validate_message_length(0).is_err());
        assert!(validate_message_length(MAX_MESSAGE_SIZE + 1).is_err());
    }

    #[test]
    fn test_serialization_error_conversion() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let p2p_err: P2PError = json_err.into();
        matches!(p2p_err, P2PError::Serialization(_));
    }
}
