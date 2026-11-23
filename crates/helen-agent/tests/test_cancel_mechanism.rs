// Test cancel mechanism: bridge.cancel() should cancel active LLM calls via call_tracker
//
// TDD RED phase: This test should FAIL before the fix, PASS after the fix.

use helen_agent::actor_bridge::bridge::HelenActorBridge;
use std::thread;
use std::time::Duration;

#[test]
fn test_bridge_cancel_cancels_call_tracker() {
    // Create a bridge with minimal setup
    let bridge = HelenActorBridge::new(
        "/tmp/test_cancel".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Get the call_tracker reference
    let call_tracker = bridge.get_call_tracker();

    // Register a mock LLM call
    let call_handle = call_tracker.register_call("test-call-1".to_string());

    // Verify the call is active
    assert!(
        !call_handle.is_cancelled(),
        "Call should not be cancelled initially"
    );
    assert_eq!(call_tracker.active_count(), 1, "Should have 1 active call");

    // Call cancel on the bridge
    bridge.cancel();

    // Give a moment for the cancel to propagate
    thread::sleep(Duration::from_millis(10));

    // Verify the call was cancelled
    assert!(
        call_handle.is_cancelled(),
        "Call should be cancelled after bridge.cancel()"
    );
    assert_eq!(
        call_tracker.active_count(),
        1,
        "Call should still be registered (not unregistered)"
    );

    // Clean up
    call_tracker.unregister_call("test-call-1");
}

#[test]
fn test_bridge_cancel_multiple_calls() {
    let bridge = HelenActorBridge::new(
        "/tmp/test_cancel_multi".to_string(),
        "test-session-2".to_string(),
        "<context></context>".to_string(),
    );

    let call_tracker = bridge.get_call_tracker();

    // Register multiple mock LLM calls
    let handle1 = call_tracker.register_call("call-1".to_string());
    let handle2 = call_tracker.register_call("call-2".to_string());
    let handle3 = call_tracker.register_call("call-3".to_string());

    assert_eq!(call_tracker.active_count(), 3, "Should have 3 active calls");

    // Cancel all via bridge
    bridge.cancel();

    thread::sleep(Duration::from_millis(10));

    // All calls should be cancelled
    assert!(handle1.is_cancelled(), "Call 1 should be cancelled");
    assert!(handle2.is_cancelled(), "Call 2 should be cancelled");
    assert!(handle3.is_cancelled(), "Call 3 should be cancelled");

    // Clean up
    call_tracker.unregister_call("call-1");
    call_tracker.unregister_call("call-2");
    call_tracker.unregister_call("call-3");
}

#[test]
fn test_bridge_clear_cancel_resets_flag() {
    let bridge = HelenActorBridge::new(
        "/tmp/test_clear_cancel".to_string(),
        "test-session-3".to_string(),
        "<context></context>".to_string(),
    );

    // Initially not cancelled
    assert!(
        !bridge.is_cancel_requested(),
        "Should not be cancelled initially"
    );

    // Cancel
    bridge.cancel();
    assert!(
        bridge.is_cancel_requested(),
        "Should be cancelled after cancel()"
    );

    // Clear cancel
    bridge.clear_cancel();
    assert!(
        !bridge.is_cancel_requested(),
        "Should not be cancelled after clear_cancel()"
    );
}
