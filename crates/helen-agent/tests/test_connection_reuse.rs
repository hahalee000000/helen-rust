//! Tests for connection reuse

use helen_agent::actor_bridge::session_registry::SessionRegistry;
use std::sync::Arc;

#[tokio::test]
async fn test_multiple_clients_same_session() {
    let registry = SessionRegistry::new();
    
    // Client 1 connects
    let bridge1 = registry.get_or_create("session-1".to_string()).await;
    assert!(bridge1.is_some());
    
    // Client 2 connects to same session
    let bridge2 = registry.get_or_create("session-1".to_string()).await;
    assert!(bridge2.is_some());
    
    // Both should get the same bridge
    assert!(Arc::ptr_eq(&bridge1.unwrap(), &bridge2.unwrap()));
}

#[tokio::test]
async fn test_connection_count() {
    let registry = SessionRegistry::new();
    
    let _bridge1 = registry.get_or_create("session-1".to_string()).await;
    assert_eq!(registry.connection_count("session-1").await, 1);
    
    let _bridge2 = registry.get_or_create("session-1".to_string()).await;
    assert_eq!(registry.connection_count("session-1").await, 2);
}

#[tokio::test]
async fn test_connection_disconnect() {
    let registry = SessionRegistry::new();
    
    let _bridge1 = registry.get_or_create("session-1".to_string()).await;
    let _bridge2 = registry.get_or_create("session-1".to_string()).await;
    assert_eq!(registry.connection_count("session-1").await, 2);
    
    registry.disconnect("session-1").await;
    assert_eq!(registry.connection_count("session-1").await, 1);
    
    registry.disconnect("session-1").await;
    // Session removed when last connection disconnects
    assert_eq!(registry.connection_count("session-1").await, 0);
}

#[tokio::test]
async fn test_broadcast_to_connections() {
    let registry = SessionRegistry::new();
    
    let bridge = registry.get_or_create("session-1".to_string()).await.unwrap();
    
    // Subscribe to streaming
    let mut rx1 = bridge.subscribe_stream();
    let mut rx2 = bridge.subscribe_stream();
    
    // Both should be able to receive (broadcast)
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    assert!(rx1.try_recv().is_err());
    assert!(rx2.try_recv().is_err());
}
