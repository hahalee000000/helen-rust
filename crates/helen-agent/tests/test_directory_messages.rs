//! Tests for get_directory_messages — loading messages for current directory
//!
//! M25.21: Fix message loss on page refresh
//! Root cause: get_directory_messages takes sessions.first() which may not be
//! the current directory's session.
//!
//! TDD RED phase: these tests should FAIL initially, then pass after fix.

use helen_agent::transcript::TranscriptReader;
use std::fs;
use tempfile::TempDir;

/// Test: When multiple sessions exist, we should find the most recent one
/// (since Helen creates one session per directory, and the most recent
/// session is the current one for the current directory).
#[test]
fn test_find_most_recent_session_with_multiple_sessions() {
    let temp = TempDir::new().unwrap();
    let sessions_dir = temp.path().join(".helen/sessions");

    // Create 3 sessions with different timestamps
    // Simulating: user worked on project, left, came back, left again
    let sessions = vec![
        ("session_old", 1000, "Old message"),
        ("session_middle", 2000, "Middle message"),
        ("session_recent", 3000, "Recent message"),
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

    let reader = TranscriptReader::new(sessions_dir);
    let all_sessions = reader.list_sessions();

    // Should return sessions sorted by timestamp (most recent first)
    assert_eq!(all_sessions.len(), 3);
    assert_eq!(all_sessions[0].id, "session_recent");
    assert_eq!(all_sessions[1].id, "session_middle");
    assert_eq!(all_sessions[2].id, "session_old");

    // The fix: get_directory_messages should use the most recent session
    let most_recent_id = all_sessions.first().map(|s| s.id.clone());
    assert_eq!(most_recent_id, Some("session_recent".to_string()));

    // Load messages from the most recent session
    let messages = reader.to_messages("session_recent");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Recent message");
}

/// Test: When no sessions exist, should return empty messages
#[test]
fn test_no_sessions_returns_empty() {
    let temp = TempDir::new().unwrap();
    let sessions_dir = temp.path().join(".helen/sessions");
    fs::create_dir_all(&sessions_dir).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let all_sessions = reader.list_sessions();

    assert_eq!(all_sessions.len(), 0);

    // Should handle empty session list gracefully
    let most_recent_id = all_sessions.first().map(|s| s.id.clone());
    assert_eq!(most_recent_id, None);
}

/// Test: Session with Helen-style ID (session_XXX format)
#[test]
fn test_helen_style_session_id() {
    let temp = TempDir::new().unwrap();
    let sessions_dir = temp.path().join(".helen/sessions");

    // Helen creates sessions with format: session_<timestamp>_<random>
    let session_id = "session_1793780805_4415064f_a0b17c3a";
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"session_meta","session_id":"session_1793780805_4415064f_a0b17c3a","timestamp":1793780805}
{"type":"message","role":"user","content":"Hello from Helen","uuid":"msg1"}
{"type":"message","role":"assistant","content":"Hi there!","uuid":"msg2"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let messages = reader.to_messages(session_id);

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "Hello from Helen");
    assert_eq!(messages[1].content, "Hi there!");
}

/// Test: Multiple sessions, only one has messages for current conversation
#[test]
fn test_find_correct_session_among_multiple() {
    let temp = TempDir::new().unwrap();
    let sessions_dir = temp.path().join(".helen/sessions");

    // Create an old session (from yesterday)
    let old_session = "session_1793700000_old";
    let old_dir = sessions_dir.join(old_session);
    fs::create_dir_all(&old_dir).unwrap();
    let old_transcript = old_dir.join("transcript.jsonl");
    fs::write(
        &old_transcript,
        r#"{"type":"session_meta","session_id":"session_1793700000_old","timestamp":1793700000}
{"type":"message","role":"user","content":"Yesterday's message","uuid":"old1"}
"#,
    )
    .unwrap();

    // Create a recent session (today)
    let recent_session = "session_1793780805_recent";
    let recent_dir = sessions_dir.join(recent_session);
    fs::create_dir_all(&recent_dir).unwrap();
    let recent_transcript = recent_dir.join("transcript.jsonl");
    fs::write(
        &recent_transcript,
        r#"{"type":"session_meta","session_id":"session_1793780805_recent","timestamp":1793780805}
{"type":"message","role":"user","content":"Today's message","uuid":"recent1"}
"#,
    )
    .unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let all_sessions = reader.list_sessions();

    // Should find the most recent session
    assert_eq!(all_sessions.len(), 2);
    let most_recent = all_sessions.first().unwrap();
    assert_eq!(most_recent.id, recent_session);

    // Load messages from the most recent session
    let messages = reader.to_messages(&most_recent.id);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Today's message");
}
