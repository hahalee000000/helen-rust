//! Tests for Helen interpreter thread spawning

use helen_agent::actor_bridge::bridge::HelenActorBridge;

#[tokio::test]
async fn test_interpreter_thread_spawns() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Give thread time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_interpreter_thread_runs_in_background() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Send multiple messages rapidly
    for i in 0..10 {
        bridge.send_message(format!("Message {}", i), vec![]).await;
    }
    
    // Give thread time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_interpreter_thread_handles_large_context() {
    // Create a large context string
    let large_context = "x".repeat(10000);
    
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        large_context,
    );
    
    // Give thread time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_interpreter_executes_helen_code() {
    // This test will be fully implemented after Task 1.4
    // For now, just verify the bridge can be created with interpreter
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Give thread time to initialize interpreter
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    assert!(bridge.is_alive());
}
