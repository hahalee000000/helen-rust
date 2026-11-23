//! Tests for TranscriptReader — reading and parsing transcript.jsonl files
//!
//! TDD RED phase: these tests should FAIL initially, then pass after implementation.

use helen_agent::transcript::TranscriptReader;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_read_empty_transcript() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_123";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let entries = reader.read_entries(session_id);

    assert_eq!(entries.len(), 0);
}

#[test]
fn test_read_transcript_with_entries() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_456";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    // Write test transcript
    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"session_meta","session_id":"test_session_456","timestamp":1234567890}
{"type":"message","role":"user","content":"Hello","uuid":"msg1"}
{"type":"message","role":"assistant","content":"Hi there","uuid":"msg2"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let entries = reader.read_entries(session_id);

    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].entry_type, "session_meta");
    assert_eq!(entries[1].entry_type, "message");
    assert_eq!(entries[1].role, Some("user".to_string()));
    assert_eq!(entries[2].role, Some("assistant".to_string()));
}

#[test]
fn test_read_transcript_skips_invalid_json() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_789";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"message","role":"user","content":"Valid","uuid":"msg1"}
{invalid json}
{"type":"message","role":"assistant","content":"Also valid","uuid":"msg2"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let entries = reader.read_entries(session_id);

    // Should skip the invalid line
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_transcript_to_messages_filters_non_messages() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_filter";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"session_meta","session_id":"test_session_filter","timestamp":1234567890}
{"type":"message","role":"user","content":"Hello","uuid":"msg1"}
{"type":"boundary_marker","marker":"test"}
{"type":"message","role":"assistant","content":"Hi","uuid":"msg2"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let messages = reader.to_messages(session_id);

    // Should only include type="message" entries
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
}

#[test]
fn test_transcript_to_messages_filters_test_messages() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_test_filter";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"message","role":"user","content":"[TEST] This is a test","uuid":"msg1"}
{"type":"message","role":"user","content":"Real message","uuid":"msg2"}
{"type":"message","role":"assistant","content":"[TEST] Test response","uuid":"msg3"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let messages = reader.to_messages(session_id);

    // Should filter out [TEST] messages
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Real message");
}

#[test]
fn test_transcript_to_messages_extracts_timestamp() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_timestamp";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"session_meta","session_id":"test_session_timestamp","timestamp":1234567890}
{"type":"message","role":"user","content":"Hello","uuid":"msg1"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let messages = reader.to_messages(session_id);

    assert_eq!(messages.len(), 1);
    assert!(messages[0].timestamp.is_some());
}

#[test]
fn test_list_sessions() {
    let temp = TempDir::new().unwrap();
    let sessions_dir = temp.path().join(".helen/sessions");

    // Create multiple sessions
    for i in 1..=3 {
        let session_dir = sessions_dir.join(format!("session_{}", i));
        fs::create_dir_all(&session_dir).unwrap();

        let transcript_path = session_dir.join("transcript.jsonl");
        let content = format!(
            r#"{{"type":"session_meta","session_id":"session_{}","timestamp":{}}}
{{"type":"message","role":"user","content":"Message {}","uuid":"msg{}"}}
"#,
            i,
            1000 + i,
            i,
            i
        );
        fs::write(&transcript_path, content).unwrap();
    }

    let reader = TranscriptReader::new(sessions_dir);
    let sessions = reader.list_sessions();

    assert_eq!(sessions.len(), 3);
    // Should be sorted by timestamp (most recent first)
    assert!(sessions[0].id.contains("session_3"));
}

#[test]
fn test_session_info_structure() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_info";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"session_meta","session_id":"test_session_info","timestamp":1234567890}
{"type":"message","role":"user","content":"Hello world","uuid":"msg1"}
{"type":"message","role":"assistant","content":"Hi","uuid":"msg2"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let sessions = reader.list_sessions();

    assert_eq!(sessions.len(), 1);
    let info = &sessions[0];
    assert_eq!(info.id, session_id);
    assert_eq!(info.message_count, 2);
    assert_eq!(info.preview, "Hello world");
    assert!(info.timestamp.is_some());
}

/// Regression test: /clear command should hide messages after page refresh
///
/// This test verifies the fix for the issue where executing /clear command
/// would clear the frontend display, but refreshing the page would cause
/// all messages to reappear. The root cause was that TranscriptReader
/// ignored boundary_marker entries, which are inserted by clear_context()
/// to mark messages as hidden.
///
/// Scenario:
/// 1. User has a conversation (multiple messages)
/// 2. User executes /clear command
/// 3. Helen runtime inserts a boundary_marker covering all messages
/// 4. Frontend receives clear_messages event and clears display ✓
/// 5. User refreshes page
/// 6. Frontend calls GET /api/chat/dir/messages
/// 7. Backend reads transcript and should return EMPTY list (not all messages)
#[test]
fn test_clear_command_hides_messages_after_refresh() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_clear_refresh";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    // Simulate transcript after /clear command:
    // - Multiple messages from conversation
    // - boundary_marker inserted by clear_context() covering all messages
    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"session_meta","session_id":"test_clear_refresh","timestamp":1234567890}
{"type":"message","role":"user","content":"Hello","uuid":"msg_001"}
{"type":"message","role":"assistant","content":"Hi there","uuid":"msg_002"}
{"type":"message","role":"user","content":"How are you?","uuid":"msg_003"}
{"type":"message","role":"assistant","content":"I'm good, thanks!","uuid":"msg_004"}
{"type":"boundary_marker","uuid":"boundary_001","head_uuid":"msg_001","tail_uuid":"msg_004","anchor_uuid":"","summary":"[Context Cleared: 4 messages]","layer":"clear_context"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let messages = reader.to_messages(session_id);

    // After /clear, all messages should be hidden by the boundary_marker
    assert_eq!(
        messages.len(),
        0,
        "All messages should be hidden after /clear command (boundary_marker should hide them)"
    );
}

/// Regression test: /clear followed by new messages should only show new messages
///
/// Scenario:
/// 1. User has a conversation
/// 2. User executes /clear
/// 3. User continues conversation
/// 4. Only new messages (after boundary) should be visible
#[test]
fn test_clear_then_new_messages_only_shows_new() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_clear_new_messages";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    // Transcript: old messages + boundary + new messages
    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"session_meta","session_id":"test_clear_new_messages","timestamp":1234567890}
{"type":"message","role":"user","content":"Old message 1","uuid":"msg_old_1"}
{"type":"message","role":"assistant","content":"Old response 1","uuid":"msg_old_2"}
{"type":"boundary_marker","uuid":"boundary_001","head_uuid":"msg_old_1","tail_uuid":"msg_old_2","anchor_uuid":"","summary":"[Context Cleared: 2 messages]","layer":"clear_context"}
{"type":"message","role":"user","content":"New message after clear","uuid":"msg_new_1"}
{"type":"message","role":"assistant","content":"New response after clear","uuid":"msg_new_2"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let messages = reader.to_messages(session_id);

    // Only new messages should be visible
    assert_eq!(
        messages.len(),
        2,
        "Only messages after boundary_marker should be visible"
    );
    assert_eq!(messages[0].content, "New message after clear");
    assert_eq!(messages[1].content, "New response after clear");
}

/// Regression test: multiple /clear commands should hide all covered messages
#[test]
fn test_multiple_clear_commands() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_multiple_clears";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();

    // Transcript with multiple clear operations
    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"session_meta","session_id":"test_multiple_clears","timestamp":1234567890}
{"type":"message","role":"user","content":"First conversation","uuid":"msg_1"}
{"type":"message","role":"assistant","content":"First response","uuid":"msg_2"}
{"type":"boundary_marker","uuid":"boundary_1","head_uuid":"msg_1","tail_uuid":"msg_2","anchor_uuid":"","summary":"[Context Cleared]","layer":"clear_context"}
{"type":"message","role":"user","content":"Second conversation","uuid":"msg_3"}
{"type":"message","role":"assistant","content":"Second response","uuid":"msg_4"}
{"type":"boundary_marker","uuid":"boundary_2","head_uuid":"msg_3","tail_uuid":"msg_4","anchor_uuid":"","summary":"[Context Cleared]","layer":"clear_context"}
{"type":"message","role":"user","content":"Third conversation","uuid":"msg_5"}
"#;
    fs::write(&transcript_path, content).unwrap();

    let reader = TranscriptReader::new(sessions_dir);
    let messages = reader.to_messages(session_id);

    // Only the last message should be visible
    assert_eq!(
        messages.len(),
        1,
        "Only messages after all boundaries should be visible"
    );
    assert_eq!(messages[0].content, "Third conversation");
}
