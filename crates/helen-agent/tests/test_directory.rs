//! Tests for DirectoryManager — CWD-based session management
//!
//! TDD RED phase: these tests should FAIL initially, then pass after implementation.

use helen_agent::directory::DirectoryManager;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_directory_manager_init_with_valid_path() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();

    let dm = DirectoryManager::new(path.to_string());
    assert_eq!(dm.get_cwd(), path);
}

#[test]
fn test_directory_manager_init_with_invalid_path() {
    // Should fall back to current directory
    let dm = DirectoryManager::new("/nonexistent/path/12345".to_string());
    let cwd = dm.get_cwd();
    assert!(std::path::Path::new(&cwd).exists());
}

#[test]
fn test_set_cwd_to_valid_directory() {
    let temp = TempDir::new().unwrap();
    let initial_path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(initial_path.to_string());

    // Create a new directory to switch to
    let new_dir = temp.path().join("new_project");
    fs::create_dir(&new_dir).unwrap();
    let new_path = new_dir.to_str().unwrap();

    let result = dm.set_cwd(new_path);
    assert_eq!(result.status, "ok");
    assert_eq!(result.cwd.unwrap(), new_path);
    assert!(result.display_name.is_some());
    assert_eq!(result.display_name.unwrap(), "new_project");
}

#[test]
fn test_set_cwd_to_nonexistent_directory() {
    let temp = TempDir::new().unwrap();
    let initial_path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(initial_path.to_string());

    let result = dm.set_cwd("/nonexistent/path/12345");
    assert_eq!(result.status, "error");
    assert!(result.message.is_some());
    assert!(result.message.unwrap().contains("不存在"));
}

#[test]
fn test_set_cwd_creates_helen_directory() {
    let temp = TempDir::new().unwrap();
    let initial_path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(initial_path.to_string());

    let new_dir = temp.path().join("project");
    fs::create_dir(&new_dir).unwrap();
    let new_path = new_dir.to_str().unwrap();

    let result = dm.set_cwd(new_path);
    assert_eq!(result.status, "ok");

    // Verify .helen directory was created
    let helen_dir = new_dir.join(".helen");
    assert!(helen_dir.exists());
    assert!(helen_dir.is_dir());
}

#[test]
fn test_get_display_name_for_regular_directory() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(path.to_string());

    let name = dm.get_display_name(None);
    // TempDir creates directories with random names
    assert!(!name.is_empty());
}

#[test]
fn test_get_display_name_for_specific_path() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(path.to_string());

    let name = dm.get_display_name(Some("/home/user/my_project".to_string()));
    assert_eq!(name, "my_project");
}

#[test]
fn test_get_helen_dir_path() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(path.to_string());

    let helen_dir = dm.get_helen_dir();
    assert!(helen_dir.ends_with(".helen"));
    assert!(helen_dir.starts_with(path));
}

#[test]
fn test_get_sessions_dir_path() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(path.to_string());

    let sessions_dir = dm.get_sessions_dir();
    assert!(sessions_dir.ends_with(".helen/sessions"));
    assert!(sessions_dir.starts_with(path));
}

#[test]
fn test_get_memory_path() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(path.to_string());

    let memory_path = dm.get_memory_path();
    assert!(memory_path.ends_with(".helen/MEMORY.md"));
    assert!(memory_path.starts_with(path));
}

#[test]
fn test_get_user_path() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(path.to_string());

    let user_path = dm.get_user_path();
    assert!(user_path.ends_with(".helen/USER.md"));
    assert!(user_path.starts_with(path));
}

#[test]
fn test_cwd_to_session_id_deterministic() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(path.to_string());

    let id1 = dm.cwd_to_session_id(path);
    let id2 = dm.cwd_to_session_id(path);

    // Same path should always produce same session ID
    assert_eq!(id1, id2);
    assert_eq!(id1.len(), 16); // SHA256[:16]
}

#[test]
fn test_cwd_to_session_id_different_paths() {
    let temp = TempDir::new().unwrap();
    let path1 = temp.path().to_str().unwrap();
    let dm = DirectoryManager::new(path1.to_string());

    let path2 = "/home/user/different_project";
    let id1 = dm.cwd_to_session_id(path1);
    let id2 = dm.cwd_to_session_id(path2);

    // Different paths should produce different session IDs
    assert_ne!(id1, id2);
}
