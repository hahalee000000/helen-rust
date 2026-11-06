//! Integration test for get_directory_messages API
//!
//! M25.21: Fix message loss on page refresh
//! This test verifies that the API correctly loads messages from the most recent session.

use helen_agent::directory::DirectoryManager;
use helen_agent::transcript::TranscriptReader;
use std::fs;
use tempfile::TempDir;

/// Test: get_directory_messages should find the most recent session
#[test]
fn test_get_directory_messages_finds_most_recent_session() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path();
    let sessions_dir = project_dir.join(".helen/sessions");

    // Create 2 sessions (simulating Helen creating multiple sessions)
    let sessions = vec![
        ("session_old", 1000, "Old message"),
        ("session_recent", 2000, "Recent message"),
    ];

    for (id, ts, msg) in &sessions {
        let session_dir = sessions_dir.join(id);
        fs::create_dir_all(&session_dir).unwrap();

        let transcript_path = session_dir.join("transcript.jsonl");
        let content = format!(
            r#"{{"type":"session_meta","session_id":"{}","timestamp":{}}}
{{"type":"message","role":"user","content":"{}","uuid":"msg1"}}
"#,
            id, ts, msg
        );
        fs::write(&transcript_path, content).unwrap();
    }

    // Simulate what get_directory_messages does
    let reader = TranscriptReader::from_path(sessions_dir.to_str().unwrap());
    let sessions_list = reader.list_sessions();

    // Should find both sessions
    assert_eq!(sessions_list.len(), 2, "Should find 2 sessions");

    // Most recent should be first
    let most_recent = sessions_list.first().unwrap();
    assert_eq!(most_recent.id, "session_recent");

    // Load messages from the most recent session
    let messages = reader.to_messages(&most_recent.id);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Recent message");
}

/// Test: get_directory_messages should handle empty sessions directory
#[test]
fn test_get_directory_messages_handles_empty_directory() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path();
    let sessions_dir = project_dir.join(".helen/sessions");
    fs::create_dir_all(&sessions_dir).unwrap();

    let reader = TranscriptReader::from_path(sessions_dir.to_str().unwrap());
    let sessions_list = reader.list_sessions();

    // Should return empty list
    assert_eq!(sessions_list.len(), 0);

    // Should handle gracefully (no panic)
    let most_recent_id = sessions_list.first().map(|s| s.id.clone());
    assert_eq!(most_recent_id, None);
}

/// Test: get_directory_messages should handle non-existent sessions directory
#[test]
fn test_get_directory_messages_handles_nonexistent_directory() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path();
    let sessions_dir = project_dir.join(".helen/sessions");
    // Don't create the directory

    let reader = TranscriptReader::from_path(sessions_dir.to_str().unwrap());
    let sessions_list = reader.list_sessions();

    // Should return empty list (not panic)
    assert_eq!(sessions_list.len(), 0);
}

/// Test: DirectoryManager should return correct sessions_dir
#[test]
fn test_directory_manager_sessions_dir() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path();

    let dm = DirectoryManager::new(project_dir.to_str().unwrap().to_string());

    let sessions_dir = dm.get_sessions_dir();
    let expected = format!("{}/.helen/sessions", project_dir.to_str().unwrap());

    assert_eq!(sessions_dir, expected);
}
