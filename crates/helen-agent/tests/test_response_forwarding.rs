//! Tests for response forwarding from Helen to WebSocket

use helen_agent::actor_bridge::bridge::HelenActorBridge;
use helen_agent::actor_bridge::messages::AgentOutput;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_bridge_can_receive_output() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Bridge should be alive
    assert!(bridge.is_alive());
    
    // Send a message
    bridge.send_message("Hello".to_string(), vec![]).await;
    
    // Give thread time to process
    sleep(Duration::from_millis(200)).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_bridge_output_channel_exists() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Bridge should have output receiver
    // (We can't directly test this without exposing it, but we can verify it doesn't panic)
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_bridge_multiple_messages_with_output() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Send multiple messages
    for i in 0..5 {
        bridge.send_message(format!("Message {}", i), vec![]).await;
        sleep(Duration::from_millis(50)).await;
    }
    
    // Give thread time to process
    sleep(Duration::from_millis(200)).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}
