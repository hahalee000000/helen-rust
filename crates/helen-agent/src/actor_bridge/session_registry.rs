//! Session registry for persistent bridge management
//!
//! This module provides a registry of active HelenActorBridge instances,
//! allowing reuse across WebSocket reconnections.

use crate::actor_bridge::bridge::HelenActorBridge;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

/// Session metadata
struct SessionInfo {
    bridge: Arc<HelenActorBridge>,
    last_accessed: u64,
    connection_count: usize,
}

/// Registry of active sessions
///
/// Manages HelenActorBridge instances by session ID, allowing reuse
/// across WebSocket reconnections.
pub struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<String, SessionInfo>>>,
}

impl SessionRegistry {
    /// Create a new session registry
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the number of active sessions
    pub async fn active_sessions(&self) -> usize {
        self.sessions.lock().await.len()
    }

    /// Get or create a bridge for the given session ID
    ///
    /// If a bridge already exists for this session, it is reused.
    /// Otherwise, a new bridge is created.
    pub async fn get_or_create(&self, session_id: String) -> Option<Arc<HelenActorBridge>> {
        let mut sessions = self.sessions.lock().await;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if let Some(info) = sessions.get_mut(&session_id) {
            // Reuse existing bridge
            info.last_accessed = now;
            info.connection_count += 1;
            Some(info.bridge.clone())
        } else {
            // Create new bridge
            let bridge = Arc::new(HelenActorBridge::new(
                "/tmp".to_string(),
                session_id.clone(),
                "<context></context>".to_string(),
            ));

            let info = SessionInfo {
                bridge: bridge.clone(),
                last_accessed: now,
                connection_count: 1,
            };

            sessions.insert(session_id, info);
            Some(bridge)
        }
    }

    /// Remove a session from the registry
    pub async fn remove(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().await;
        sessions.remove(session_id);
    }

    /// Get the connection count for a session
    pub async fn connection_count(&self, session_id: &str) -> usize {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id)
            .map(|info| info.connection_count)
            .unwrap_or(0)
    }

    /// Decrement connection count for a session
    ///
    /// If the count reaches 0, the session is removed.
    pub async fn disconnect(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().await;
        if let Some(info) = sessions.get_mut(session_id) {
            info.connection_count = info.connection_count.saturating_sub(1);
            if info.connection_count == 0 {
                sessions.remove(session_id);
            }
        }
    }

    /// Cleanup stale sessions (no activity for longer than timeout)
    pub async fn cleanup_stale(&self, timeout: Duration) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut sessions = self.sessions.lock().await;
        sessions.retain(|_, info| {
            let elapsed = now.saturating_sub(info.last_accessed);
            elapsed < timeout.as_secs()
        });
    }

    /// Get all active session IDs
    pub async fn session_ids(&self) -> Vec<String> {
        let sessions = self.sessions.lock().await;
        sessions.keys().cloned().collect()
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
