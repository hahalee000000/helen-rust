//! Tests for loading Helen agent files

use helen_agent::actor_bridge::bridge::HelenActorBridge;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_load_simple_helen_file() {
    // Create a temporary directory with a simple Helen file
    let temp_dir = TempDir::new().unwrap();
    let helen_file = temp_dir.path().join("test.helen");
    fs::write(&helen_file, "fn test_fn(): int { return 42 }").unwrap();
    
    // Set environment variable to point to temp directory
    std::env::set_var("HELEN_AGENT_DIR", temp_dir.path());
    
    let bridge = HelenActorBridge::new(
        temp_dir.path().to_str().unwrap().to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Give thread time to load files
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
    
    // Clean up
    std::env::remove_var("HELEN_AGENT_DIR");
}

#[tokio::test]
async fn test_load_multiple_helen_files() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create multiple Helen files
    fs::write(
        temp_dir.path().join("utils.helen"),
        "fn helper(): str { return \"help\" }",
    )
    .unwrap();
    
    fs::write(
        temp_dir.path().join("main.helen"),
        "fn main_fn(): int { return 100 }",
    )
    .unwrap();
    
    std::env::set_var("HELEN_AGENT_DIR", temp_dir.path());
    
    let bridge = HelenActorBridge::new(
        temp_dir.path().to_str().unwrap().to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    assert!(bridge.is_alive());
    
    std::env::remove_var("HELEN_AGENT_DIR");
}

#[tokio::test]
async fn test_load_helen_file_with_syntax_error() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create a Helen file with syntax error
    fs::write(
        temp_dir.path().join("bad.helen"),
        "fn bad_fn( { return 42 }", // Missing closing paren
    )
    .unwrap();
    
    std::env::set_var("HELEN_AGENT_DIR", temp_dir.path());
    
    let bridge = HelenActorBridge::new(
        temp_dir.path().to_str().unwrap().to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Bridge should still be alive even if file has errors
    assert!(bridge.is_alive());
    
    std::env::remove_var("HELEN_AGENT_DIR");
}
