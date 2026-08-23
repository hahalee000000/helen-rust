//! Integration tests for upload functionality
//!
//! These tests verify the upload functionality by testing the UploadManager
//! directly rather than through HTTP handlers.

use helen_agent::upload::UploadManager;
use tempfile::TempDir;

#[test]
fn test_upload_manager_save_and_read() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();
    let mgr = UploadManager::new(cwd);

    // Save a file
    let result = mgr
        .save_upload("test.txt", "text/plain", b"hello world")
        .unwrap();

    assert_eq!(result.filename, "test.txt");
    assert_eq!(result.mime_type, "text/plain");
    assert_eq!(result.size, 11);
    assert!(!result.upload_id.is_empty());
    assert!(result.url.contains(&result.upload_id));

    // Read it back
    let (content, mime_type) = mgr.read_upload_file(&result.upload_id).unwrap();
    assert_eq!(content, b"hello world");
    assert_eq!(mime_type, "text/plain");
}

#[test]
fn test_upload_manager_rejects_bad_mime_type() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();
    let mgr = UploadManager::new(cwd);

    let result = mgr.save_upload("test.exe", "application/exe", b"data");
    assert!(result.is_err());
}

#[test]
fn test_upload_manager_rejects_large_file() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();
    let mgr = UploadManager::new(cwd);

    // Try to upload a file larger than 50MB
    let large_data = vec![0u8; 51 * 1024 * 1024];
    let result = mgr.save_upload("large.bin", "application/octet-stream", &large_data);
    assert!(result.is_err());
}

#[test]
fn test_upload_manager_validates_upload_id() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();
    let mgr = UploadManager::new(cwd);

    // Valid UUID should work
    let result = mgr.save_upload("test.txt", "text/plain", b"data").unwrap();
    assert!(UploadManager::validate_upload_id(&result.upload_id).is_ok());

    // Invalid IDs should fail
    assert!(UploadManager::validate_upload_id("").is_err());
    assert!(UploadManager::validate_upload_id("../etc/passwd").is_err());
    assert!(UploadManager::validate_upload_id("foo/bar").is_err());
    assert!(UploadManager::validate_upload_id("foo\\bar").is_err());
}

#[test]
fn test_upload_manager_file_not_found() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();
    let mgr = UploadManager::new(cwd);

    let result = mgr.read_upload_file("nonexistent-uuid");
    assert!(result.is_err());
}

#[test]
fn test_upload_manager_multiple_files() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();
    let mgr = UploadManager::new(cwd);

    // Upload multiple files
    let result1 = mgr
        .save_upload("file1.txt", "text/plain", b"content1")
        .unwrap();
    let result2 = mgr
        .save_upload("file2.png", "image/png", b"content2")
        .unwrap();
    let result3 = mgr
        .save_upload("file3.pdf", "application/pdf", b"content3")
        .unwrap();

    // All should have different IDs
    assert_ne!(result1.upload_id, result2.upload_id);
    assert_ne!(result2.upload_id, result3.upload_id);

    // All should be readable
    let (c1, m1) = mgr.read_upload_file(&result1.upload_id).unwrap();
    let (c2, m2) = mgr.read_upload_file(&result2.upload_id).unwrap();
    let (c3, m3) = mgr.read_upload_file(&result3.upload_id).unwrap();

    assert_eq!(c1, b"content1");
    assert_eq!(m1, "text/plain");
    assert_eq!(c2, b"content2");
    assert_eq!(m2, "image/png");
    assert_eq!(c3, b"content3");
    assert_eq!(m3, "application/pdf");
}

#[test]
fn test_upload_manager_allowed_mime_types() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();
    let mgr = UploadManager::new(cwd);

    // Test various allowed MIME types
    let allowed_types = vec![
        ("image.jpeg", "image/jpeg"),
        ("image.png", "image/png"),
        ("image.gif", "image/gif"),
        ("image.webp", "image/webp"),
        ("audio.mp3", "audio/mpeg"),
        ("audio.wav", "audio/wav"),
        ("video.mp4", "video/mp4"),
        ("video.webm", "video/webm"),
        ("doc.pdf", "application/pdf"),
        ("data.json", "application/json"),
        ("text.txt", "text/plain"),
        ("readme.md", "text/markdown"),
    ];

    for (filename, mime_type) in allowed_types {
        let result = mgr.save_upload(filename, mime_type, b"data");
        assert!(result.is_ok(), "Failed for {}: {}", filename, mime_type);
    }
}
