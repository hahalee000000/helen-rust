//! Tests for streaming chunk forwarding from Helen to WebSocket

use helen_agent::actor_bridge::bridge::HelenActorBridge;
use helen_agent::actor_bridge::messages::StreamChunk;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_bridge_can_subscribe_to_stream() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Should be able to subscribe
    let mut rx = bridge.subscribe_stream();
    
    // Give thread time to initialize
    sleep(Duration::from_millis(100)).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_stream_chunk_structure() {
    let chunk = StreamChunk {
        sequence: 1,
        content: "Hello ".to_string(),
    };
    
    assert_eq!(chunk.sequence, 1);
    assert_eq!(chunk.content, "Hello ");
}

#[tokio::test]
async fn test_multiple_subscribers() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Multiple subscribers should work
    let mut rx1 = bridge.subscribe_stream();
    let mut rx2 = bridge.subscribe_stream();
    
    sleep(Duration::from_millis(100)).await;
    
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_stream_receiver_can_receive() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    let mut rx = bridge.subscribe_stream();
    
    sleep(Duration::from_millis(100)).await;
    
    // Try to receive with timeout (should timeout since no chunks sent yet)
    let result = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;
    
    // Should timeout (no chunks sent)
    assert!(result.is_err());
}
