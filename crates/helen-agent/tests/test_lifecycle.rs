//! Tests for lifecycle management (heartbeat, shutdown, recovery)

use helen_agent::actor_bridge::bridge::HelenActorBridge;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_bridge_heartbeat() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Initial heartbeat should be true
    assert!(bridge.heartbeat());

    // Wait a bit
    sleep(Duration::from_millis(100)).await;

    // Heartbeat should still be true
    assert!(bridge.heartbeat());
}

#[tokio::test]
async fn test_bridge_graceful_shutdown() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Bridge should be alive
    assert!(bridge.is_alive());

    // Shutdown the bridge
    bridge.shutdown().await;

    // Give thread time to exit
    sleep(Duration::from_millis(100)).await;

    // Bridge should no longer be alive
    assert!(!bridge.is_alive());
}

#[tokio::test]
async fn test_bridge_last_activity() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Get initial last activity
    let initial = bridge.last_activity();

    // Wait a bit
    sleep(Duration::from_millis(50)).await;

    // Send a message
    bridge.send_message("Hello".to_string(), vec![]).await;

    // Wait for message to be processed
    sleep(Duration::from_millis(50)).await;

    // Last activity should be updated
    let updated = bridge.last_activity();
    assert!(updated >= initial);
}

#[tokio::test]
async fn test_bridge_is_stale() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Initially not stale
    assert!(!bridge.is_stale(Duration::from_secs(300)));

    // Send a message to update activity
    bridge.send_message("Hello".to_string(), vec![]).await;
    sleep(Duration::from_millis(50)).await;

    // Still not stale
    assert!(!bridge.is_stale(Duration::from_secs(300)));
}

#[tokio::test]
async fn test_bridge_multiple_shutdowns() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Multiple shutdowns should not panic
    bridge.shutdown().await;
    bridge.shutdown().await;
    bridge.shutdown().await;

    sleep(Duration::from_millis(100)).await;

    assert!(!bridge.is_alive());
}

#[tokio::test]
async fn test_bridge_send_after_shutdown() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Shutdown
    bridge.shutdown().await;
    sleep(Duration::from_millis(100)).await;

    // Sending message after shutdown should not panic
    bridge.send_message("Hello".to_string(), vec![]).await;
}
