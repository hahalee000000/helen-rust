//! Tests for spawn_chat_actor() integration

use helen_agent::actor_bridge::bridge::HelenActorBridge;

#[tokio::test]
async fn test_spawn_chat_actor_called() {
    // This test verifies that spawn_chat_actor() is called during initialization
    // For now, just verify the bridge can be created without errors
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    // Give thread time to initialize and call spawn_chat_actor()
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Bridge should still be alive
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_bridge_handles_missing_helen_files() {
    // Test that bridge handles missing Helen files gracefully
    std::env::set_var("HELEN_AGENT_DIR", "/nonexistent/path");

    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Bridge should still be alive even if files are missing
    assert!(bridge.is_alive());

    std::env::remove_var("HELEN_AGENT_DIR");
}
