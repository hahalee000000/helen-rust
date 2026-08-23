//! Tests for file storage

use helen_agent::storage::FileStorage;
use tempfile::tempdir;

#[test]
fn test_store_file() {
    let dir = tempdir().unwrap();
    let storage = FileStorage::new(dir.path().to_path_buf());

    let content = b"Hello, World!";
    let file_id = storage.store_file("test.txt", content).unwrap();

    assert!(!file_id.is_empty());
}

#[test]
fn test_retrieve_file() {
    let dir = tempdir().unwrap();
    let storage = FileStorage::new(dir.path().to_path_buf());

    let content = b"Test content";
    let file_id = storage.store_file("test.txt", content).unwrap();

    let retrieved = storage.retrieve_file(&file_id).unwrap();
    assert_eq!(retrieved, content);
}

#[test]
fn test_list_files() {
    let dir = tempdir().unwrap();
    let storage = FileStorage::new(dir.path().to_path_buf());

    storage.store_file("file1.txt", b"content1").unwrap();
    storage.store_file("file2.txt", b"content2").unwrap();

    let files = storage.list_files().unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn test_delete_file() {
    let dir = tempdir().unwrap();
    let storage = FileStorage::new(dir.path().to_path_buf());

    let file_id = storage.store_file("test.txt", b"content").unwrap();
    storage.delete_file(&file_id).unwrap();

    let files = storage.list_files().unwrap();
    assert_eq!(files.len(), 0);
}

#[test]
fn test_file_metadata() {
    let dir = tempdir().unwrap();
    let storage = FileStorage::new(dir.path().to_path_buf());

    let content = b"Test content";
    let file_id = storage.store_file("test.txt", content).unwrap();

    let metadata = storage.get_metadata(&file_id).unwrap();
    assert_eq!(metadata.filename, "test.txt");
    assert_eq!(metadata.size, content.len() as u64);
}
