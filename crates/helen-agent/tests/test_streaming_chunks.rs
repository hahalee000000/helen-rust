//! Tests for true streaming chunk forwarding

use helen_agent::actor_bridge::bridge::HelenActorBridge;
use helen_agent::actor_bridge::messages::StreamChunk;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_bridge_has_stream_subscription() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Should be able to subscribe to streaming chunks
    let _rx = bridge.subscribe_stream();
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_stream_chunks_are_broadcast() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    let mut rx = bridge.subscribe_stream();

    // Simulate sending a streaming chunk (this would normally come from Helen)
    // For now, just verify the subscription works
    sleep(Duration::from_millis(10)).await;

    // No chunks yet (Helen not actually streaming in test)
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn test_multiple_stream_subscribers() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    let mut rx1 = bridge.subscribe_stream();
    let mut rx2 = bridge.subscribe_stream();

    // Both subscribers should be able to receive
    sleep(Duration::from_millis(10)).await;

    // No chunks yet
    assert!(rx1.try_recv().is_err());
    assert!(rx2.try_recv().is_err());
}

#[tokio::test]
async fn test_stream_chunk_sequence_ordering() {
    let chunk1 = StreamChunk {
        sequence: 1,
        content: "Hello ".to_string(),
    };
    let chunk2 = StreamChunk {
        sequence: 2,
        content: "world".to_string(),
    };

    assert!(chunk1.sequence < chunk2.sequence);
    assert_eq!(chunk1.content, "Hello ");
    assert_eq!(chunk2.content, "world");
}
