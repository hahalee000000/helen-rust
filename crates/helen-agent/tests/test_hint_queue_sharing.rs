// Integration test: Verify agent and interpreter share the SAME hint queue
//
// This test verifies the fix for the bug where WebSocket handler (helen-agent)
// and Helen builtins (helen-interpreter) used separate hint queues.

use helen_agent::hint_queue::get_hint_queue as agent_get_hint_queue;
use helen_runtime::hint_queue::get_hint_queue as runtime_get_hint_queue;

#[test]
fn test_agent_and_runtime_share_same_queue() {
    // Get queue from agent's re-export
    let agent_queue = agent_get_hint_queue();
    
    // Get queue from runtime directly
    let runtime_queue = runtime_get_hint_queue();
    
    // They should be the SAME Arc instance (not just equal)
    assert!(
        std::sync::Arc::ptr_eq(&agent_queue, &runtime_queue),
        "Agent and runtime should share the SAME hint queue instance"
    );
}

#[test]
fn test_cross_crate_hint_flow() {
    // Simulate the complete hint flow:
    // 1. WebSocket handler (agent) adds hint
    // 2. Helen builtin (interpreter) reads hint
    
    let agent_queue = agent_get_hint_queue();
    let runtime_queue = runtime_get_hint_queue();
    
    // Step 1: Agent adds hint (simulating WebSocket handler)
    agent_queue.add_hint("test hint from agent".to_string(), "client1".to_string());
    
    // Step 2: Runtime can see the hint (simulating Helen builtin)
    assert!(
        runtime_queue.has_pending(),
        "Runtime queue should see hint added by agent"
    );
    
    // Step 3: Runtime pops the hint
    let hints = runtime_queue.pop_all_hints();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].text, "test hint from agent");
    
    // Step 4: Queue is now empty
    assert!(!agent_queue.has_pending(), "Queue should be empty after pop");
}
