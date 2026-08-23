//! StreamRegistry — track active streaming sessions
//!
//! Solves: After page refresh/WS reconnect, frontend can't know if backend is still processing.
//! Frontend queries GET /api/chat/status to get is_processing flag.

use std::collections::HashSet;
use std::sync::Mutex;

/// Thread-safe registry of active streaming sessions
pub struct StreamRegistry {
    active_sessions: Mutex<HashSet<String>>,
}

impl StreamRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            active_sessions: Mutex::new(HashSet::new()),
        }
    }

    /// Register a session as actively streaming
    pub fn register(&self, session_id: &str) {
        let mut sessions = self.active_sessions.lock().unwrap();
        sessions.insert(session_id.to_string());
    }

    /// Unregister a session (called on complete/cancel/error)
    pub fn unregister(&self, session_id: &str) {
        let mut sessions = self.active_sessions.lock().unwrap();
        sessions.remove(session_id);
    }

    /// Check if any session is currently streaming
    pub fn is_processing(&self) -> bool {
        let sessions = self.active_sessions.lock().unwrap();
        !sessions.is_empty()
    }

    /// Get list of active session IDs
    pub fn active_sessions(&self) -> Vec<String> {
        let sessions = self.active_sessions.lock().unwrap();
        sessions.iter().cloned().collect()
    }

    /// Check if a specific session is streaming
    pub fn is_session_active(&self, session_id: &str) -> bool {
        let sessions = self.active_sessions.lock().unwrap();
        sessions.contains(session_id)
    }
}

impl Default for StreamRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_is_empty() {
        let registry = StreamRegistry::new();
        assert!(!registry.is_processing());
        assert!(registry.active_sessions().is_empty());
    }

    #[test]
    fn test_register_makes_processing() {
        let registry = StreamRegistry::new();
        registry.register("session-1");
        assert!(registry.is_processing());
        assert!(registry.is_session_active("session-1"));
    }

    #[test]
    fn test_unregister_removes_session() {
        let registry = StreamRegistry::new();
        registry.register("session-1");
        registry.register("session-2");
        assert!(registry.is_processing());

        registry.unregister("session-1");
        assert!(registry.is_processing()); // session-2 still active
        assert!(!registry.is_session_active("session-1"));
        assert!(registry.is_session_active("session-2"));

        registry.unregister("session-2");
        assert!(!registry.is_processing());
    }

    #[test]
    fn test_unregister_nonexistent_is_noop() {
        let registry = StreamRegistry::new();
        registry.unregister("nonexistent"); // Should not panic
        assert!(!registry.is_processing());
    }

    #[test]
    fn test_active_sessions_returns_all() {
        let registry = StreamRegistry::new();
        registry.register("a");
        registry.register("b");
        registry.register("c");

        let mut sessions = registry.active_sessions();
        sessions.sort();
        assert_eq!(sessions, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_register_same_session_twice() {
        let registry = StreamRegistry::new();
        registry.register("session-1");
        registry.register("session-1"); // Duplicate
        assert!(registry.is_processing());
        assert_eq!(registry.active_sessions().len(), 1);
    }
}
