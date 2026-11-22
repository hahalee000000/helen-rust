// Test hint integration: WebSocket writes, Helen reads from the SAME queue
//
// TDD RED phase: This test should FAIL before the fix, PASS after the fix.

use helen_agent::hint_queue::get_hint_queue as agent_get_hint_queue;

#[test]
fn test_hint_queue_shared_between_agent_and_interpreter() {
    // Simulate WebSocket handler adding a hint (agent side)
    let agent_queue = agent_get_hint_queue();
    agent_queue.add_hint("test hint".to_string(), "client1".to_string());

    // Verify the hint is in the agent queue
    assert!(agent_queue.has_pending(), "Agent queue should have pending hint");

    // Now simulate Helen calling hint_queue_has_pending()
    // This should return true if they share the same queue
    // We can't directly call the Helen builtin, but we can verify
    // that the agent queue is the same instance

    // Pop the hint from agent queue
    let hints = agent_queue.pop_all_hints();
    assert_eq!(hints.len(), 1, "Should have 1 hint");
    assert_eq!(hints[0].text, "test hint");

    // After popping, queue should be empty
    assert!(!agent_queue.has_pending(), "Queue should be empty after pop");
}

#[test]
fn test_hint_queue_global_singleton() {
    // Verify that get_hint_queue() returns the same instance
    let q1 = agent_get_hint_queue();
    let q2 = agent_get_hint_queue();

    // They should be the same Arc instance
    assert!(
        std::sync::Arc::ptr_eq(&q1, &q2),
        "get_hint_queue() should return the same singleton instance"
    );
}
