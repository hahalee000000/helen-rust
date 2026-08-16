//! Call tracking for cancellable LLM calls.
//!
//! Port of Python's `_CallHandle` and `_active_calls` tracking in `helen/runtime/__init__.py`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Tracks an in-flight LLM call for cancellation.
///
/// Python: `_CallHandle` class with `cancelled` (Event), `done` (Event), `result`, `exception`.
pub struct CallHandle {
    /// Whether this call has been cancelled.
    pub cancelled: AtomicBool,
    /// Whether this call has completed (successfully or with error).
    pub done: AtomicBool,
    /// The result of the call (if completed successfully).
    pub result: Mutex<Option<String>>,
    /// The exception from the call (if failed).
    pub exception: Mutex<Option<String>>,
}

impl CallHandle {
    /// Create a new call handle.
    pub fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            done: AtomicBool::new(false),
            result: Mutex::new(None),
            exception: Mutex::new(None),
        }
    }

    /// Check if this call has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Mark this call as cancelled.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Check if this call has completed.
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }

    /// Mark this call as done.
    pub fn mark_done(&self) {
        self.done.store(true, Ordering::SeqCst);
    }

    /// Set the result of this call.
    pub fn set_result(&self, result: String) {
        if let Ok(mut r) = self.result.lock() {
            *r = Some(result);
        }
    }

    /// Get the result of this call.
    pub fn get_result(&self) -> Option<String> {
        self.result.lock().ok().and_then(|r| r.clone())
    }

    /// Set the exception from this call.
    pub fn set_exception(&self, exception: String) {
        if let Ok(mut e) = self.exception.lock() {
            *e = Some(exception);
        }
    }

    /// Get the exception from this call.
    pub fn get_exception(&self) -> Option<String> {
        self.exception.lock().ok().and_then(|e| e.clone())
    }
}

impl Default for CallHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages active LLM calls for cancellation support.
///
/// Thread-safe tracking of in-flight calls.
pub struct CallTracker {
    /// Map of call_id -> CallHandle.
    active_calls: Mutex<HashMap<String, Arc<CallHandle>>>,
}

impl CallTracker {
    /// Create a new call tracker.
    pub fn new() -> Self {
        Self {
            active_calls: Mutex::new(HashMap::new()),
        }
    }

    /// Register a new active call. Returns the call handle.
    pub fn register_call(&self, call_id: String) -> Arc<CallHandle> {
        let handle = Arc::new(CallHandle::new());
        if let Ok(mut calls) = self.active_calls.lock() {
            calls.insert(call_id, Arc::clone(&handle));
        }
        handle
    }

    /// Unregister a completed call.
    pub fn unregister_call(&self, call_id: &str) {
        if let Ok(mut calls) = self.active_calls.lock() {
            calls.remove(call_id);
        }
    }

    /// Cancel a specific call by ID.
    ///
    /// Returns true if the call was found and cancelled, false otherwise.
    pub fn cancel_call(&self, call_id: &str) -> bool {
        if let Ok(calls) = self.active_calls.lock() {
            if let Some(handle) = calls.get(call_id) {
                handle.cancel();
                return true;
            }
        }
        false
    }

    /// Get the ID of the current active streaming call.
    ///
    /// Returns the first active call ID, or None if no calls are active.
    pub fn get_current_call_id(&self) -> Option<String> {
        if let Ok(calls) = self.active_calls.lock() {
            // Return the first active (not done) call
            calls
                .iter()
                .find(|(_, handle)| !handle.is_done())
                .map(|(id, _)| id.clone())
        } else {
            None
        }
    }

    /// Cancel all active calls.
    ///
    /// Returns the number of calls that were cancelled.
    pub fn cancel_all(&self) -> usize {
        if let Ok(calls) = self.active_calls.lock() {
            let count = calls.len();
            for handle in calls.values() {
                handle.cancel();
            }
            count
        } else {
            0
        }
    }

    /// Get the number of active calls.
    pub fn active_count(&self) -> usize {
        if let Ok(calls) = self.active_calls.lock() {
            calls.len()
        } else {
            0
        }
    }
}

impl Default for CallTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_handle_new() {
        let handle = CallHandle::new();
        assert!(!handle.is_cancelled());
        assert!(!handle.is_done());
        assert!(handle.get_result().is_none());
        assert!(handle.get_exception().is_none());
    }

    #[test]
    fn test_call_handle_cancel() {
        let handle = CallHandle::new();
        assert!(!handle.is_cancelled());
        handle.cancel();
        assert!(handle.is_cancelled());
    }

    #[test]
    fn test_call_handle_done() {
        let handle = CallHandle::new();
        assert!(!handle.is_done());
        handle.mark_done();
        assert!(handle.is_done());
    }

    #[test]
    fn test_call_handle_result() {
        let handle = CallHandle::new();
        assert!(handle.get_result().is_none());
        handle.set_result("test result".to_string());
        assert_eq!(handle.get_result(), Some("test result".to_string()));
    }

    #[test]
    fn test_call_handle_exception() {
        let handle = CallHandle::new();
        assert!(handle.get_exception().is_none());
        handle.set_exception("test error".to_string());
        assert_eq!(handle.get_exception(), Some("test error".to_string()));
    }

    #[test]
    fn test_call_tracker_new() {
        let tracker = CallTracker::new();
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn test_call_tracker_register_unregister() {
        let tracker = CallTracker::new();
        let handle = tracker.register_call("call-1".to_string());
        assert_eq!(tracker.active_count(), 1);
        assert!(!handle.is_cancelled());

        tracker.unregister_call("call-1");
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn test_call_tracker_cancel_call() {
        let tracker = CallTracker::new();
        let handle = tracker.register_call("call-1".to_string());
        assert!(!handle.is_cancelled());

        let cancelled = tracker.cancel_call("call-1");
        assert!(cancelled);
        assert!(handle.is_cancelled());

        // Cancel non-existent call
        let cancelled = tracker.cancel_call("call-2");
        assert!(!cancelled);
    }

    #[test]
    fn test_call_tracker_get_current_call_id() {
        let tracker = CallTracker::new();
        assert!(tracker.get_current_call_id().is_none());

        let _handle = tracker.register_call("call-1".to_string());
        assert_eq!(tracker.get_current_call_id(), Some("call-1".to_string()));
    }

    #[test]
    fn test_call_tracker_cancel_all() {
        let tracker = CallTracker::new();
        let handle1 = tracker.register_call("call-1".to_string());
        let handle2 = tracker.register_call("call-2".to_string());
        let handle3 = tracker.register_call("call-3".to_string());

        assert_eq!(tracker.active_count(), 3);
        let count = tracker.cancel_all();
        assert_eq!(count, 3);

        assert!(handle1.is_cancelled());
        assert!(handle2.is_cancelled());
        assert!(handle3.is_cancelled());
    }
}
