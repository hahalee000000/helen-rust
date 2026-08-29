//! Tests for session persistence

use helen_agent::actor_bridge::session_registry::SessionRegistry;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_registry_creation() {
    let registry = SessionRegistry::new();
    assert_eq!(registry.active_sessions().await, 0);
}

#[tokio::test]
async fn test_registry_register_session() {
    let registry = SessionRegistry::new();
    
    let bridge = registry.get_or_create("session-1".to_string()).await;
    assert!(bridge.is_some());
    assert_eq!(registry.active_sessions().await, 1);
}

#[tokio::test]
async fn test_registry_reuse_session() {
    let registry = SessionRegistry::new();
    
    let bridge1 = registry.get_or_create("session-1".to_string()).await;
    assert!(bridge1.is_some());
    
    let bridge2 = registry.get_or_create("session-1".to_string()).await;
    assert!(bridge2.is_some());
    
    // Should reuse the same bridge
    assert_eq!(registry.active_sessions().await, 1);
}

#[tokio::test]
async fn test_registry_multiple_sessions() {
    let registry = SessionRegistry::new();
    
    let _bridge1 = registry.get_or_create("session-1".to_string()).await;
    let _bridge2 = registry.get_or_create("session-2".to_string()).await;
    let _bridge3 = registry.get_or_create("session-3".to_string()).await;
    
    assert_eq!(registry.active_sessions().await, 3);
}

#[tokio::test]
async fn test_registry_remove_session() {
    let registry = SessionRegistry::new();
    
    let _bridge = registry.get_or_create("session-1".to_string()).await;
    assert_eq!(registry.active_sessions().await, 1);
    
    registry.remove("session-1").await;
    assert_eq!(registry.active_sessions().await, 0);
}

#[tokio::test]
async fn test_registry_cleanup_stale() {
    let registry = SessionRegistry::new();
    
    let _bridge = registry.get_or_create("session-1".to_string()).await;
    
    // Wait a bit
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Cleanup with very short timeout
    registry.cleanup_stale(std::time::Duration::from_millis(50)).await;
    assert_eq!(registry.active_sessions().await, 0);
}
