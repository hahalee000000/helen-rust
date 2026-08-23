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
fn test_read_session_preview() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_preview";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();
    
    let transcript_path = session_dir.join("transcript.jsonl");
    let content = r#"{"type":"message","role":"user","content":"This is a long message that should be truncated","uuid":"msg1"}
{"type":"message","role":"assistant","content":"Response","uuid":"msg2"}
"#;
    fs::write(&transcript_path, content).unwrap();
    
    let reader = TranscriptReader::new(sessions_dir);
    let preview = reader.read_preview(session_id, 20);
    
    assert_eq!(preview, "This is a long messa");
}

#[test]
fn test_read_session_preview_empty() {
    let temp = TempDir::new().unwrap();
    let session_id = "test_session_empty";
    let sessions_dir = temp.path().join(".helen/sessions");
    let session_dir = sessions_dir.join(session_id);
    fs::create_dir_all(&session_dir).unwrap();
    
    let reader = TranscriptReader::new(sessions_dir);
    let preview = reader.read_preview(session_id, 50);
    
    assert_eq!(preview, "");
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
        let content = format!(r#"{{"type":"session_meta","session_id":"session_{}","timestamp":{}}}
{{"type":"message","role":"user","content":"Message {}","uuid":"msg{}"}}
"#, i, 1000 + i, i, i);
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
