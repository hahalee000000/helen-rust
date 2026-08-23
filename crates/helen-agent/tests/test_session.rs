//! Tests for session management

use helen_agent::session::{Session, SessionManager};
use tempfile::tempdir;

#[test]
fn test_create_session() {
    let dir = tempdir().unwrap();
    let manager = SessionManager::new(dir.path().to_path_buf());
    
    let session = manager.create_session();
    assert!(!session.id.is_empty());
    assert!(session.messages.is_empty());
}

#[test]
fn test_add_message_to_session() {
    let dir = tempdir().unwrap();
    let manager = SessionManager::new(dir.path().to_path_buf());
    
    let mut session = manager.create_session();
    session.add_message("user", "Hello");
    session.add_message("assistant", "Hi there!");
    
    assert_eq!(session.messages.len(), 2);
    assert_eq!(session.messages[0].role, "user");
    assert_eq!(session.messages[0].content, "Hello");
}

#[test]
fn test_save_and_load_session() {
    let dir = tempdir().unwrap();
    let manager = SessionManager::new(dir.path().to_path_buf());
    
    let mut session = manager.create_session();
    session.add_message("user", "Test message");
    
    manager.save_session(&session).unwrap();
    
    let loaded = manager.load_session(&session.id).unwrap();
    assert_eq!(loaded.id, session.id);
    assert_eq!(loaded.messages.len(), 1);
}

#[test]
fn test_list_sessions() {
    let dir = tempdir().unwrap();
    let manager = SessionManager::new(dir.path().to_path_buf());
    
    let session1 = manager.create_session();
    manager.save_session(&session1).unwrap();
    
    let session2 = manager.create_session();
    manager.save_session(&session2).unwrap();
    
    let sessions = manager.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_delete_session() {
    let dir = tempdir().unwrap();
    let manager = SessionManager::new(dir.path().to_path_buf());
    
    let session = manager.create_session();
    manager.save_session(&session).unwrap();
    
    manager.delete_session(&session.id).unwrap();
    
    let sessions = manager.list_sessions().unwrap();
    assert_eq!(sessions.len(), 0);
}
