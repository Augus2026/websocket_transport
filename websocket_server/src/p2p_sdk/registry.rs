use crate::message::PeerInfo;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayState {
    Pending,
    Active,
    Closed,
}

impl RelayState {
    #[inline]
    pub fn is_active(&self) -> bool {
        matches!(self, RelayState::Active)
    }
}

#[derive(Debug)]
pub struct RelaySession {
    pub session_id: String,
    pub peer_a: String,
    pub peer_b: String,
    pub state: RelayState,
    pub created_at: Instant,
    pub message_count: u64,
}

impl RelaySession {
    #[inline]
    pub fn duration(&self) -> Duration {
        self.created_at.elapsed()
    }

    pub fn other_peer(&self, peer_id: &str) -> Option<&str> {
        if self.peer_a == peer_id {
            Some(&self.peer_b)
        } else if self.peer_b == peer_id {
            Some(&self.peer_a)
        } else {
            None
        }
    }
}

pub struct RelaySessionRegistry {
    sessions: HashMap<String, RelaySession>,
    peer_sessions: HashMap<String, Vec<String>>,
}

impl RelaySessionRegistry {
    pub fn new() -> Self {
        RelaySessionRegistry {
            sessions: HashMap::new(),
            peer_sessions: HashMap::new(),
        }
    }

    pub fn create_session(&mut self, peer_a: String, peer_b: String) -> String {
        let session_id = format!(
            "relay-{}-{}-{}",
            &peer_a.chars().take(8).collect::<String>(),
            &peer_b.chars().take(8).collect::<String>(),
            &uuid::Uuid::new_v4().to_string().chars().take(8).collect::<String>()
        );

        let session = RelaySession {
            session_id: session_id.clone(),
            peer_a: peer_a.clone(),
            peer_b: peer_b.clone(),
            state: RelayState::Pending,
            created_at: Instant::now(),
            message_count: 0,
        };

        self.sessions.insert(session_id.clone(), session);
        self.peer_sessions
            .entry(peer_a)
            .or_default()
            .push(session_id.clone());
        self.peer_sessions
            .entry(peer_b)
            .or_default()
            .push(session_id.clone());

        session_id
    }

    pub fn activate_session(&mut self, session_id: &str) -> bool {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.state = RelayState::Active;
            true
        } else {
            false
        }
    }

    pub fn increment_message_count(&mut self, session_id: &str) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.message_count += 1;
        }
    }

    pub fn get_session_for_peers(&self, peer_a: &str, peer_b: &str) -> Option<&RelaySession> {
        self.sessions.values().find(|s| {
            (s.peer_a == peer_a && s.peer_b == peer_b)
                || (s.peer_a == peer_b && s.peer_b == peer_a)
        })
    }

    pub fn close_sessions_for_peer(&mut self, peer_id: &str) {
        if let Some(session_ids) = self.peer_sessions.remove(peer_id) {
            for sid in session_ids {
                if let Some(session) = self.sessions.get_mut(&sid) {
                    session.state = RelayState::Closed;
                }
            }
        }
    }

    pub fn get_active_sessions(&self) -> Vec<&RelaySession> {
        self.sessions
            .values()
            .filter(|s| s.state.is_active())
            .collect()
    }

    pub fn get_session_stats(&self, session_id: &str) -> Option<(RelayState, u64, Duration)> {
        self.sessions
            .get(session_id)
            .map(|s| (s.state, s.message_count, s.duration()))
    }

    pub fn active_session_count(&self) -> usize {
        self.sessions.values().filter(|s| s.state.is_active()).count()
    }

    pub fn cleanup_closed(&mut self) {
        let closed_ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.state == RelayState::Closed)
            .map(|(id, _)| id.clone())
            .collect();

        for id in closed_ids {
            self.sessions.remove(&id);
        }
    }
}

impl Default for RelaySessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PeerConnection {
    pub info: PeerInfo,
    pub addr: SocketAddr,
}

impl PeerConnection {
    pub fn new(peer_id: String, addr: SocketAddr) -> Self {
        PeerConnection {
            info: PeerInfo::new(peer_id, addr.to_string()),
            addr,
        }
    }
}

pub struct PeerRegistry {
    pub peers: HashMap<String, PeerConnection>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        PeerRegistry {
            peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, peer_id: String, addr: SocketAddr) {
        let conn = PeerConnection::new(peer_id.clone(), addr);
        self.peers.insert(peer_id, conn);
    }

    pub fn remove_peer(&mut self, peer_id: &str) -> Option<PeerConnection> {
        self.peers.remove(peer_id)
    }

    pub fn get_peer_list(&self) -> Vec<PeerInfo> {
        self.peers.values().map(|c| c.info.clone()).collect()
    }

    #[inline]
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    #[inline]
    pub fn get_peer(&self, peer_id: &str) -> Option<&PeerConnection> {
        self.peers.get(peer_id)
    }

    #[inline]
    pub fn get_peer_mut(&mut self, peer_id: &str) -> Option<&mut PeerConnection> {
        self.peers.get_mut(peer_id)
    }

    #[inline]
    pub fn contains_peer(&self, peer_id: &str) -> bool {
        self.peers.contains_key(peer_id)
    }

    #[inline]
    pub fn get_peer_info(&self, peer_id: &str) -> Option<&PeerInfo> {
        self.peers.get(peer_id).map(|c| &c.info)
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_state() {
        assert!(RelayState::Active.is_active());
        assert!(!RelayState::Pending.is_active());
        assert!(!RelayState::Closed.is_active());
    }

    #[test]
    fn test_relay_session_registry_create() {
        let mut registry = RelaySessionRegistry::new();
        let session_id = registry.create_session("peer-a".to_string(), "peer-b".to_string());

        assert!(!session_id.is_empty());
        assert!(registry.get_session_for_peers("peer-a", "peer-b").is_some());
    }

    #[test]
    fn test_relay_session_registry_activate() {
        let mut registry = RelaySessionRegistry::new();
        let session_id = registry.create_session("peer-a".to_string(), "peer-b".to_string());

        assert!(registry.activate_session(&session_id));

        let stats = registry.get_session_stats(&session_id);
        assert!(stats.is_some());
        let (state, count, _) = stats.unwrap();
        assert_eq!(state, RelayState::Active);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_relay_session_registry_message_count() {
        let mut registry = RelaySessionRegistry::new();
        let session_id = registry.create_session("peer-a".to_string(), "peer-b".to_string());

        registry.increment_message_count(&session_id);
        registry.increment_message_count(&session_id);

        let stats = registry.get_session_stats(&session_id).unwrap();
        assert_eq!(stats.1, 2);
    }

    #[test]
    fn test_relay_session_other_peer() {
        let session = RelaySession {
            session_id: "test".to_string(),
            peer_a: "peer-a".to_string(),
            peer_b: "peer-b".to_string(),
            state: RelayState::Active,
            created_at: Instant::now(),
            message_count: 0,
        };

        assert_eq!(session.other_peer("peer-a"), Some("peer-b"));
        assert_eq!(session.other_peer("peer-b"), Some("peer-a"));
        assert_eq!(session.other_peer("peer-c"), None);
    }

    #[test]
    fn test_relay_session_registry_close_for_peer() {
        let mut registry = RelaySessionRegistry::new();
        registry.create_session("peer-a".to_string(), "peer-b".to_string());

        registry.close_sessions_for_peer("peer-a");

        let active = registry.get_active_sessions();
        assert!(active.is_empty());
    }

    #[test]
    fn test_peer_registry_add_remove() {
        let mut registry = PeerRegistry::new();
        assert_eq!(registry.peer_count(), 0);
    }

    #[test]
    fn test_peer_registry_contains() {
        let mut registry = PeerRegistry::new();
        assert!(!registry.contains_peer("test-peer"));
    }
}
