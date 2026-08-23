//! HintInjector — queue hints for injection during tool execution
//!
//! Hints are user-provided instructions that should be injected into the
//! Helen runtime at the next tool boundary (on_tool_end callback).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// A queued hint
#[derive(Debug, Clone)]
pub struct Hint {
    pub session_id: String,
    pub text: String,
    pub client_id: String,
}

/// Thread-safe hint queue per session
pub struct HintInjector {
    queues: Mutex<HashMap<String, VecDeque<Hint>>>,
}

impl HintInjector {
    /// Create a new empty injector
    pub fn new() -> Self {
        Self {
            queues: Mutex::new(HashMap::new()),
        }
    }

    /// Enqueue a hint for a session
    pub fn enqueue(&self, session_id: &str, text: &str, client_id: &str) -> Hint {
        let hint = Hint {
            session_id: session_id.to_string(),
            text: text.to_string(),
            client_id: client_id.to_string(),
        };

        let mut queues = self.queues.lock().unwrap();
        let queue = queues.entry(session_id.to_string()).or_default();
        queue.push_back(hint.clone());
        hint
    }

    /// Dequeue the next hint for a session (if any)
    pub fn dequeue(&self, session_id: &str) -> Option<Hint> {
        let mut queues = self.queues.lock().unwrap();
        if let Some(queue) = queues.get_mut(session_id) {
            let hint = queue.pop_front();
            // Clean up empty queues
            if queue.is_empty() {
                queues.remove(session_id);
            }
            hint
        } else {
            None
        }
    }

    /// Check if a session has pending hints
    pub fn has_pending(&self, session_id: &str) -> bool {
        let queues = self.queues.lock().unwrap();
        queues.get(session_id).is_some_and(|q| !q.is_empty())
    }

    /// Clear all pending hints for a session (on WS disconnect)
    pub fn clear_session(&self, session_id: &str) {
        let mut queues = self.queues.lock().unwrap();
        queues.remove(session_id);
    }

    /// Get count of pending hints for a session
    pub fn pending_count(&self, session_id: &str) -> usize {
        let queues = self.queues.lock().unwrap();
        queues.get(session_id).map_or(0, |q| q.len())
    }
}

impl Default for HintInjector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_injector_is_empty() {
        let injector = HintInjector::new();
        assert!(!injector.has_pending("session-1"));
        assert_eq!(injector.pending_count("session-1"), 0);
    }

    #[test]
    fn test_enqueue_adds_hint() {
        let injector = HintInjector::new();
        let hint = injector.enqueue("session-1", "Use Rust", "client-1");

        assert_eq!(hint.session_id, "session-1");
        assert_eq!(hint.text, "Use Rust");
        assert_eq!(hint.client_id, "client-1");
        assert!(injector.has_pending("session-1"));
        assert_eq!(injector.pending_count("session-1"), 1);
    }

    #[test]
    fn test_dequeue_returns_hint() {
        let injector = HintInjector::new();
        injector.enqueue("session-1", "Hint 1", "client-1");
        injector.enqueue("session-1", "Hint 2", "client-1");

        let hint1 = injector.dequeue("session-1").unwrap();
        assert_eq!(hint1.text, "Hint 1");

        let hint2 = injector.dequeue("session-1").unwrap();
        assert_eq!(hint2.text, "Hint 2");

        assert!(injector.dequeue("session-1").is_none());
    }

    #[test]
    fn test_dequeue_empty_returns_none() {
        let injector = HintInjector::new();
        assert!(injector.dequeue("nonexistent").is_none());
    }

    #[test]
    fn test_clear_session_removes_all() {
        let injector = HintInjector::new();
        injector.enqueue("session-1", "Hint 1", "client-1");
        injector.enqueue("session-1", "Hint 2", "client-1");
        assert_eq!(injector.pending_count("session-1"), 2);

        injector.clear_session("session-1");
        assert!(!injector.has_pending("session-1"));
        assert_eq!(injector.pending_count("session-1"), 0);
    }

    #[test]
    fn test_multiple_sessions_independent() {
        let injector = HintInjector::new();
        injector.enqueue("session-1", "Hint A", "client-1");
        injector.enqueue("session-2", "Hint B", "client-2");

        assert_eq!(injector.pending_count("session-1"), 1);
        assert_eq!(injector.pending_count("session-2"), 1);

        injector.clear_session("session-1");
        assert!(!injector.has_pending("session-1"));
        assert!(injector.has_pending("session-2"));
    }

    #[test]
    fn test_fifo_order() {
        let injector = HintInjector::new();
        injector.enqueue("s", "first", "c");
        injector.enqueue("s", "second", "c");
        injector.enqueue("s", "third", "c");

        assert_eq!(injector.dequeue("s").unwrap().text, "first");
        assert_eq!(injector.dequeue("s").unwrap().text, "second");
        assert_eq!(injector.dequeue("s").unwrap().text, "third");
    }
}
