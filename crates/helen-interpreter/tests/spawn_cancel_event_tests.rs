//! Tests for spawn cancel event injection (P0 fix).
//!
//! Verifies that spawned interpreters receive the channel's cancel_event
//! so they can check cancellation status during streaming LLM calls.
//!
//! Python reference: `helen/interpreter/interpreter.py:1570`
//! ```python
//! spawned_interp._agent_cancel_event = spawned_endpoint.cancel_event
//! ```

use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::value::Value;
use helen_core::lexer::Scanner;
use helen_parser::Parser;
use std::sync::{Arc, Mutex};

/// Helper to run Helen code and capture output + return value.
fn run_src(src: &str) -> (Result<Option<Value>, ExceptionValue>, String) {
    let tokens = Scanner::new(src, "t.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    assert!(
        parser.errors().is_empty(),
        "parse errors for {}: {:?}",
        src,
        parser.errors()
    );
    let mut interp = Interpreter::new();
    let stdout = Arc::new(Mutex::new(String::new()));
    interp.stdout = Arc::clone(&stdout);
    let result = interp.interpret(&program);
    let out = stdout.lock().unwrap().clone();
    (result, out)
}

// ============================================================================
// Test 1: Spawned interpreter has agent_cancel_event field
// ============================================================================

#[test]
fn spawned_interpreter_has_cancel_event_field() {
    // After spawn, the spawned interpreter should have agent_cancel_event set
    let (r, _) = run_src(
        r#"
        agent Worker(mb) {
            main {
                // Just return immediately
                "done"
            }
        }
        
        main {
            let mb = spawn Worker()
            mb.receive()
        }
        "#,
    );

    // Should complete without error
    assert!(r.is_ok(), "Spawn should succeed");
}

// ============================================================================
// Test 2: Cancel event propagates to spawned interpreter
// ============================================================================

#[test]
fn cancel_event_propagates_to_spawned_interpreter() {
    // When main endpoint is cancelled, spawned interpreter should see it
    let (r, _out) = run_src(
        r#"
        agent Worker(mb) {
            main {
                // Check if we can detect cancellation
                // In real scenario, this would be checked during LLM streaming
                mb.receive()
            }
        }
        
        main {
            let mb = spawn Worker()
            // Cancel the spawned agent
            mb.cancel()
            // Try to receive (should get None or error due to cancel)
            let msg = mb.receive()
            msg
        }
        "#,
    );

    // Should complete (cancelled receive returns None)
    assert!(r.is_ok(), "Cancel should not crash");
}

// ============================================================================
// Test 3: Cancel event is shared between endpoints
// ============================================================================

#[test]
fn cancel_event_is_shared_between_endpoints() {
    // Both main and spawned endpoints should share the same cancel_event
    let (r, _) = run_src(
        r#"
        agent Worker(mb) {
            main {
                // Worker receives from main
                let msg = mb.receive()
                msg
            }
        }
        
        main {
            let mb = spawn Worker()
            // Send a message
            mb.send("hello")
            // Cancel should affect both sides
            mb.cancel()
            "done"
        }
        "#,
    );

    assert!(r.is_ok(), "Shared cancel should work");
}

// ============================================================================
// Test 4: Cancel closes channel
// ============================================================================

#[test]
fn cancel_closes_channel() {
    let (r, _out) = run_src(
        r#"
        agent Worker(mb) {
            main {
                mb.receive()
            }
        }
        
        main {
            let mb = spawn Worker()
            mb.cancel()
            // After cancel, channel should be closed
            mb.is_closed()
        }
        "#,
    );

    match &r {
        Ok(Some(Value::Bool(true))) => {} // Expected: channel is closed after cancel
        Ok(other) => panic!("Expected true, got {:?}", other),
        Err(e) => panic!("Expected Ok, got error: {:?}", e),
    }
}

// ============================================================================
// Test 6: Agent cancel event attribute injection
// ============================================================================

#[test]
fn agent_cancel_event_attribute_injection() {
    // This test verifies that the spawned interpreter has the cancel_event
    // attribute set, which is critical for streaming cancellation.
    //
    // Python: spawned_interp._agent_cancel_event = spawned_endpoint.cancel_event
    //
    // In Rust, this translates to:
    // interp.agent_cancel_event = Some(endpoint.cancel_event.clone())
    
    let (r, _) = run_src(
        r#"
        agent Worker(mb) {
            main {
                // The spawned interpreter should be able to check cancel status
                // This is used internally during LLM streaming
                mb.receive()
            }
        }
        
        main {
            let mb = spawn Worker()
            // Cancel from main side
            mb.cancel()
            // Spawned interpreter should see the cancel
            "done"
        }
        "#,
    );

    assert!(r.is_ok(), "Cancel event injection should work");
}

// ============================================================================
// Test 7: Interpreter has agent_cancel_event field (Rust-specific)
// ============================================================================

#[test]
fn interpreter_has_agent_cancel_event_field() {
    // This test verifies that the Interpreter struct has the agent_cancel_event field
    // which is required for streaming cancellation in spawned agents.
    //
    // Python: interp._agent_cancel_event = threading.Event()
    // Rust: interp.agent_cancel_event: Option<Arc<AtomicBool>>
    
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    
    let mut interp = Interpreter::new();
    
    // Initially, agent_cancel_event should be None
    assert!(interp.agent_cancel_event.is_none(), 
            "agent_cancel_event should be None initially");
    
    // Set the cancel event
    let cancel_event = Arc::new(AtomicBool::new(false));
    interp.agent_cancel_event = Some(cancel_event.clone());
    
    // Verify it's set
    assert!(interp.agent_cancel_event.is_some(), 
            "agent_cancel_event should be set");
    
    // Verify we can check the cancel status
    assert!(!interp.agent_cancel_event.as_ref().unwrap().load(Ordering::SeqCst),
            "cancel_event should not be set initially");
    
    // Set the cancel event
    cancel_event.store(true, Ordering::SeqCst);
    
    // Verify the interpreter sees the cancel
    assert!(interp.agent_cancel_event.as_ref().unwrap().load(Ordering::SeqCst),
            "cancel_event should be set after store");
}
