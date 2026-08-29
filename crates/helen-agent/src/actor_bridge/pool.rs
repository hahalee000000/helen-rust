//! Interpreter pooling for efficient resource reuse
//!
//! This module provides a pool of Helen interpreters to avoid creating
//! new ones per WebSocket connection.

use helen_interpreter::interpreter::Interpreter;
use std::collections::VecDeque;
use std::sync::Mutex;

/// Pool of Helen interpreters
///
/// Manages a fixed-size pool of interpreters that can be checked out
/// and checked back in for reuse.
pub struct InterpreterPool {
    /// Available interpreters
    available: Mutex<VecDeque<Interpreter>>,
    /// Maximum pool size
    max_size: usize,
}

impl InterpreterPool {
    /// Create a new interpreter pool
    ///
    /// # Arguments
    /// * `max_size` - Maximum number of interpreters in the pool
    pub fn new(max_size: usize) -> Self {
        let mut available = VecDeque::new();
        
        // Pre-warm the pool
        for _ in 0..max_size {
            available.push_back(Interpreter::new());
        }
        
        Self {
            available: Mutex::new(available),
            max_size,
        }
    }
    
    /// Get the maximum pool size
    pub fn size(&self) -> usize {
        self.max_size
    }
    
    /// Get the number of available interpreters
    pub fn available(&self) -> usize {
        self.available.lock().unwrap().len()
    }
    
    /// Checkout an interpreter from the pool
    ///
    /// Returns None if the pool is exhausted.
    pub fn checkout(&self) -> Option<Interpreter> {
        let mut avail = self.available.lock().unwrap();
        avail.pop_front()
    }
    
    /// Check an interpreter back into the pool
    pub fn checkin(&self, interp: Interpreter) {
        let mut avail = self.available.lock().unwrap();
        avail.push_back(interp);
    }
}
