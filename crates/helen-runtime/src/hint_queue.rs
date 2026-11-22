//! Thread-safe hint queue for mid-processing user hints.
//!
//! Cross-thread communication: WebSocket handler writes, Helen runtime
//! (on_tool_end callback) reads.
//!
//! v1.46.27: Moved from helen-agent to helen-runtime to enable sharing
//! between WebSocket handler (helen-agent) and Helen builtins (helen-interpreter).

use std::sync::{Arc, Mutex, OnceLock};

/// A single hint message from the user.
#[derive(Debug, Clone)]
pub struct Hint {
    pub text: String,
    pub timestamp: f64,
    pub client_id: String,
}

/// Thread-safe FIFO queue of pending user hints (singleton).
#[derive(Debug, Default)]
pub struct HintQueue {
    queue: Mutex<Vec<Hint>>,
}

impl HintQueue {
    /// Create a new empty hint queue.
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }

    /// Add a hint to the queue. Thread-safe.
    pub fn add_hint(&self, text: String, client_id: String) -> Hint {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let hint = Hint {
            text: text.clone(),
            timestamp,
            client_id: client_id.clone(),
        };
        let mut queue = self.queue.lock().expect("mutex poisoned");
        queue.push(hint.clone());
        hint
    }

    /// Atomically pop all pending hints. Returns [] if empty.
    /// Called from Helen's on_tool_end callback thread.
    pub fn pop_all_hints(&self) -> Vec<Hint> {
        let mut queue = self.queue.lock().expect("mutex poisoned");
        let hints = queue.drain(..).collect();
        hints
    }

    /// Check if there are pending hints.
    pub fn has_pending(&self) -> bool {
        let queue = self.queue.lock().expect("mutex poisoned");
        !queue.is_empty()
    }

    /// Drop all pending hints (called on WS disconnect / session cleanup).
    pub fn clear(&self) {
        let mut queue = self.queue.lock().expect("mutex poisoned");
        queue.clear();
    }
}

/// Global singleton hint queue instance.
static GLOBAL_HINT_QUEUE: OnceLock<Arc<HintQueue>> = OnceLock::new();

/// Get the global hint queue instance.
pub fn get_hint_queue() -> Arc<HintQueue> {
    GLOBAL_HINT_QUEUE
        .get_or_init(|| Arc::new(HintQueue::new()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hint_queue_basic() {
        let queue = HintQueue::new();
        assert!(!queue.has_pending());

        queue.add_hint("hint1".to_string(), "client1".to_string());
        assert!(queue.has_pending());

        queue.add_hint("hint2".to_string(), "client2".to_string());
        assert!(queue.has_pending());

        let hints = queue.pop_all_hints();
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].text, "hint1");
        assert_eq!(hints[1].text, "hint2");

        assert!(!queue.has_pending());
    }

    #[test]
    fn test_hint_queue_clear() {
        let queue = HintQueue::new();
        queue.add_hint("hint1".to_string(), "client1".to_string());
        queue.add_hint("hint2".to_string(), "client2".to_string());
        assert!(queue.has_pending());

        queue.clear();
        assert!(!queue.has_pending());
    }

    #[test]
    fn test_hint_queue_empty_pop() {
        let queue = HintQueue::new();
        let hints = queue.pop_all_hints();
        assert_eq!(hints.len(), 0);
    }

    #[test]
    fn test_global_singleton() {
        let q1 = get_hint_queue();
        let q2 = get_hint_queue();
        assert!(Arc::ptr_eq(&q1, &q2));

        q1.add_hint("test".to_string(), "client".to_string());
        assert!(q2.has_pending());

        q2.clear();
    }
}
