//! Tests for HelenActorBridge creation and basic operations

use helen_agent::actor_bridge::bridge::HelenActorBridge;

#[tokio::test]
async fn test_bridge_creation() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_bridge_send_message() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Should not panic
    bridge.send_message("Hello".to_string(), vec![]).await;
}

#[tokio::test]
async fn test_bridge_subscribe_stream() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Should be able to subscribe
    let _rx = bridge.subscribe_stream();
}

#[tokio::test]
async fn test_bridge_multiple_messages() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Send multiple messages
    bridge.send_message("First".to_string(), vec![]).await;
    bridge.send_message("Second".to_string(), vec![]).await;
    bridge.send_message("Third".to_string(), vec![]).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}
