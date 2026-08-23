//! Integration tests for chat API endpoints
//!
//! These tests verify the chat API functionality by testing the underlying
//! logic directly rather than through HTTP handlers.

use helen_agent::directory::DirectoryManager;
use helen_agent::transcript::TranscriptReader;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn test_directory_manager_get_cwd() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = Arc::new(DirectoryManager::new(path.to_string()));

    let cwd = dm.get_cwd();
    assert_eq!(cwd, path);
}

#[test]
fn test_directory_manager_set_cwd_valid() {
    let temp = TempDir::new().unwrap();
    let initial_path = temp.path().to_str().unwrap();
    let dm = Arc::new(DirectoryManager::new(initial_path.to_string()));

    // Create a new directory to switch to
    let new_dir = temp.path().join("new_project");
    std::fs::create_dir(&new_dir).unwrap();
    let new_path = new_dir.to_str().unwrap();

    let result = dm.set_cwd(new_path);
    assert_eq!(result.status, "ok");
    assert_eq!(result.cwd.unwrap(), new_path);
}

#[test]
fn test_directory_manager_set_cwd_nonexistent() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = Arc::new(DirectoryManager::new(path.to_string()));

    let result = dm.set_cwd("/nonexistent/path/12345");
    assert_eq!(result.status, "error");
    assert!(result.message.unwrap().contains("不存在"));
}

#[test]
fn test_list_sessions_empty() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = Arc::new(DirectoryManager::new(path.to_string()));

    let sessions_dir = dm.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);
    let sessions = reader.list_sessions();

    assert!(sessions.is_empty());
}

#[test]
fn test_list_sessions_with_data() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = Arc::new(DirectoryManager::new(path.to_string()));

    // Create a session with transcript
    let sessions_dir = temp.path().join(".helen/sessions/test_session");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let transcript = r#"{"type":"session_meta","session_id":"test_session","timestamp":1234567890}
{"type":"message","role":"user","content":"Hello","uuid":"msg1"}
{"type":"message","role":"assistant","content":"Hi","uuid":"msg2"}
"#;
    std::fs::write(sessions_dir.join("transcript.jsonl"), transcript).unwrap();

    let sessions_dir = dm.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);
    let sessions = reader.list_sessions();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "test_session");
    assert_eq!(sessions[0].message_count, 2);
}

#[test]
fn test_get_messages_empty_session() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = Arc::new(DirectoryManager::new(path.to_string()));

    let sessions_dir = dm.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);
    let messages = reader.to_messages("nonexistent_session");

    assert!(messages.is_empty());
}

#[test]
fn test_get_messages_with_data() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = Arc::new(DirectoryManager::new(path.to_string()));

    // Create a session with transcript
    let sessions_dir = temp.path().join(".helen/sessions/test_session");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let transcript = r#"{"type":"session_meta","session_id":"test_session","timestamp":1234567890}
{"type":"message","role":"user","content":"Hello world","uuid":"msg1"}
{"type":"message","role":"assistant","content":"Hi there","uuid":"msg2"}
"#;
    std::fs::write(sessions_dir.join("transcript.jsonl"), transcript).unwrap();

    let sessions_dir = dm.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);
    let messages = reader.to_messages("test_session");

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "Hello world");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Hi there");
}
