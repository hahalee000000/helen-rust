//! Tests for message protocol (calling Helen functions from Rust)

use helen_agent::actor_bridge::bridge::HelenActorBridge;

#[tokio::test]
async fn test_call_spawn_chat_actor() {
    // This test verifies that we can call spawn_chat_actor() from Rust
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Give thread time to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    
    // Bridge should be alive
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_call_tui_chat_handler() {
    // This test verifies that we can call tui_chat_handler_actor() from Rust
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Give thread time to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    
    // Send a message (this will call tui_chat_handler_actor internally)
    bridge.send_message("Hello".to_string(), vec![]).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}
