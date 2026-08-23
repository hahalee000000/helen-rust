//! Transcript reader — reads and parses `.helen/sessions/<sid>/transcript.jsonl` files
//!
//! The transcript is the single source of truth for chat history.
//! Each line is a JSON object with a `type` field:
//! - `session_meta`: session metadata (timestamp, session_id)
//! - `message`: a chat message (role, content, uuid)
//! - `boundary_marker`: session boundary (ignored)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A raw transcript entry (one line from transcript.jsonl)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<serde_json::Value>,
}

/// A message formatted for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub attachments: Vec<serde_json::Value>,
}

/// Summary info for a session (used in session listing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub message_count: usize,
    pub preview: String,
    pub timestamp: Option<String>,
}

/// Reads transcript files from a sessions directory
pub struct TranscriptReader {
    sessions_dir: PathBuf,
}

/// Test message prefix — messages starting with this are filtered out
const TEST_MESSAGE_PREFIX: &str = "[TEST]";

impl TranscriptReader {
    /// Create a new TranscriptReader with the given sessions directory
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    /// Create a TranscriptReader from a sessions directory path string
    pub fn from_path(sessions_dir: &str) -> Self {
        Self {
            sessions_dir: PathBuf::from(sessions_dir),
        }
    }

    /// Get the transcript.jsonl path for a session
    pub fn get_transcript_path(&self, session_id: &str) -> Option<PathBuf> {
        let path = self.sessions_dir.join(session_id).join("transcript.jsonl");
        path.exists().then_some(path)
    }

    /// Read all entries from a transcript file
    pub fn read_entries(&self, session_id: &str) -> Vec<TranscriptEntry> {
        let path = match self.get_transcript_path(session_id) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut entries = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<TranscriptEntry>(line) {
                Ok(entry) => entries.push(entry),
                Err(_) => continue, // Skip invalid JSON lines
            }
        }
        entries
    }

    /// Check if an entry is a test message (starts with [TEST])
    pub fn is_test_message(entry: &TranscriptEntry) -> bool {
        if let Some(ref content) = entry.content {
            if let Some(s) = content.as_str() {
                return s.trim().starts_with(TEST_MESSAGE_PREFIX);
            }
        }
        false
    }

    /// Convert transcript entries to frontend Message format.
    ///
    /// Filters:
    /// - Only `type == "message"` entries
    /// - Skips `[TEST]` messages
    /// - Skips internal protocol commands (`__helen_*__`)
    /// - Skips slash commands
    /// - Extracts timestamp from `session_meta`
    pub fn to_messages(&self, session_id: &str) -> Vec<Message> {
        let entries = self.read_entries(session_id);
        Self::entries_to_messages(&entries)
    }

    /// Convert a slice of entries to messages (static helper for testing)
    pub fn entries_to_messages(entries: &[TranscriptEntry]) -> Vec<Message> {
        // Extract session timestamp from session_meta
        let session_timestamp = entries
            .iter()
            .find(|e| e.entry_type == "session_meta")
            .and_then(|e| e.timestamp.as_ref())
            .and_then(|ts| ts.as_i64().or_else(|| ts.as_f64().map(|f| f as i64)))
            .map(timestamp_to_iso);

        let mut messages = Vec::new();

        for entry in entries {
            if entry.entry_type != "message" {
                continue;
            }
            if Self::is_test_message(entry) {
                continue;
            }

            let role = entry.role.as_deref().unwrap_or("user");
            let uuid = entry.uuid.as_deref().unwrap_or("");
            let content_value = match &entry.content {
                Some(v) => v,
                None => continue,
            };

            if role == "user" {
                let (text, _hints, _attachments, has_internal_cmd) =
                    parse_user_content(content_value);
                if has_internal_cmd {
                    continue;
                }
                // Skip slash commands
                if text.trim().starts_with('/') {
                    continue;
                }
                // Skip pure boilerplate (no user input, no attachments)
                if text.is_empty() {
                    continue;
                }
                messages.push(Message {
                    id: uuid.to_string(),
                    role: role.to_string(),
                    content: text,
                    timestamp: session_timestamp.clone(),
                    attachments: Vec::new(),
                });
            } else {
                // assistant or other roles: content as-is
                let text = extract_text_content(content_value);
                messages.push(Message {
                    id: uuid.to_string(),
                    role: role.to_string(),
                    content: text,
                    timestamp: session_timestamp.clone(),
                    attachments: Vec::new(),
                });
            }
        }

        messages
    }

    /// List all sessions with their summary info.
    ///
    /// Scans the sessions directory for subdirectories containing transcript.jsonl.
    /// Returns sessions sorted by timestamp (most recent first).
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        let mut sessions = Vec::new();

        let entries = match fs::read_dir(&self.sessions_dir) {
            Ok(e) => e,
            Err(_) => return sessions,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let transcript_path = path.join("transcript.jsonl");
            if !transcript_path.exists() {
                continue;
            }

            let session_id = match path.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => continue,
            };

            let all_entries = self.read_entries(&session_id);
            let messages = Self::entries_to_messages(&all_entries);

            // Extract timestamp from session_meta
            let timestamp = all_entries
                .iter()
                .find(|e| e.entry_type == "session_meta")
                .and_then(|e| e.timestamp.as_ref())
                .and_then(|ts| ts.as_i64().or_else(|| ts.as_f64().map(|f| f as i64)))
                .map(timestamp_to_iso);

            // Preview from first user message
            let preview = Self::extract_preview(&all_entries);

            sessions.push(SessionInfo {
                id: session_id,
                message_count: messages.len(),
                preview,
                timestamp,
            });
        }

        // Sort by timestamp descending (most recent first)
        sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        sessions
    }

    /// Delete a session directory
    pub fn delete_session(&self, session_id: &str) -> Result<(), std::io::Error> {
        let session_dir = self.sessions_dir.join(session_id);
        if session_dir.exists() {
            fs::remove_dir_all(session_dir)?;
        }
        Ok(())
    }

    // ── Private helpers ──────────────────────────────────────────

    fn extract_preview(entries: &[TranscriptEntry]) -> String {
        for entry in entries {
            if entry.entry_type != "message" {
                continue;
            }
            if entry.role.as_deref() != Some("user") {
                continue;
            }
            if Self::is_test_message(entry) {
                continue;
            }

            let content_value = match &entry.content {
                Some(v) => v,
                None => continue,
            };

            let (text, _hints, _attachments, has_internal_cmd) = parse_user_content(content_value);
            if has_internal_cmd || text.trim().starts_with('/') || text.is_empty() {
                continue;
            }

            return if text.len() > 50 {
                text[..50].to_string()
            } else {
                text
            };
        }
        String::new()
    }
}

// ── Helper functions ──────────────────────────────────────────────

/// Convert a Unix timestamp (seconds) to ISO 8601 string
fn timestamp_to_iso(ts: i64) -> String {
    use chrono::DateTime;
    match DateTime::from_timestamp(ts, 0) {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        None => String::new(),
    }
}

/// Extract text content from a content value (string or array of parts)
fn extract_text_content(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(parts) => {
            let texts: Vec<&str> = parts
                .iter()
                .filter_map(|part| {
                    let obj = part.as_object()?;
                    if obj.get("type")?.as_str() != Some("text") {
                        return None;
                    }
                    obj.get("text")?.as_str()
                })
                .collect();
            texts.join("\n")
        }
        _ => content.to_string(),
    }
}

/// Parse user content, separating user input from boilerplate/hints/attachments.
///
/// Returns (main_text, system_hints, attachments, has_internal_command)
fn parse_user_content(
    content: &serde_json::Value,
) -> (String, Vec<String>, Vec<serde_json::Value>, bool) {
    match content {
        serde_json::Value::Array(parts) => {
            let mut text_lines = Vec::new();
            let mut attachments = Vec::new();

            for part in parts {
                if let Some(s) = part.as_str() {
                    text_lines.push(s);
                    continue;
                }
                if let Some(obj) = part.as_object() {
                    let ptype = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match ptype {
                        "text" => {
                            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                                text_lines.push(text);
                            }
                        }
                        "image_url" | "input_audio" => {
                            attachments.push(part.clone());
                        }
                        _ => {}
                    }
                }
            }

            let joined = text_lines.join("\n");
            let (user_text, _boilerplate) = extract_from_boilerplate(&joined);
            let (user_text, hints) = filter_system_hints(&user_text);
            (user_text, hints, attachments, false)
        }
        serde_json::Value::String(s) => {
            // Check for internal protocol commands
            let trimmed = s.trim();
            if let Some(rest) = trimmed.strip_prefix("__helen_") {
                if rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .count()
                    > 0
                    && rest.contains("__")
                {
                    return (String::new(), Vec::new(), Vec::new(), true);
                }
            }

            let (user_text, _boilerplate) = extract_from_boilerplate(s);
            let (user_text, hints) = filter_system_hints(&user_text);
            (user_text, hints, Vec::new(), false)
        }
        _ => (String::new(), Vec::new(), Vec::new(), false),
    }
}

/// Separate prompt boilerplate from user input.
///
/// Helen actor prepends agent prompt (## Identity ... ## Reminders ...)
/// to user input. The boilerplate starts with "## " and contains "## Identity".
///
/// Returns (user_text, boilerplate).
fn extract_from_boilerplate(content: &str) -> (String, String) {
    let trimmed = content.trim();
    if !(trimmed.starts_with("## ") && trimmed.contains("## Identity")) {
        return (content.to_string(), String::new());
    }

    let lines: Vec<&str> = content.split('\n').collect();

    // Find the last ## heading
    let last_heading_idx = lines.iter().rposition(|line| line.starts_with("## "));

    let last_heading_idx = match last_heading_idx {
        Some(i) => i,
        None => return (content.to_string(), String::new()),
    };

    // Find first blank line after the last heading
    let blank_idx = lines
        .iter()
        .enumerate()
        .skip(last_heading_idx + 1)
        .find(|(_, line)| line.trim().is_empty())
        .map(|(i, _)| i);

    match blank_idx {
        Some(idx) => {
            let user_text = lines[idx + 1..].join("\n").trim().to_string();
            let boilerplate = lines[..idx].join("\n") + "\n";
            (user_text, boilerplate)
        }
        None => {
            // No blank line: entire content is boilerplate
            (String::new(), content.to_string())
        }
    }
}

/// Filter [System Hint] lines from text, returning (cleaned_text, hints)
fn filter_system_hints(text: &str) -> (String, Vec<String>) {
    let mut hints = Vec::new();
    let mut text_lines = Vec::new();

    for line in text.split('\n') {
        if let Some(rest) = line.strip_prefix("[System Hint]") {
            hints.push(rest.trim().to_string());
            text_lines.push(""); // Preserve line structure with empty line
        } else {
            text_lines.push(line);
        }
    }

    (text_lines.join("\n"), hints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_iso() {
        let iso = timestamp_to_iso(1234567890);
        assert!(!iso.is_empty());
        assert!(iso.contains("2009")); // 2009-02-13
    }

    #[test]
    fn test_extract_text_content_string() {
        let val = serde_json::json!("Hello world");
        assert_eq!(extract_text_content(&val), "Hello world");
    }

    #[test]
    fn test_extract_text_content_array() {
        let val = serde_json::json!([
            {"type": "text", "text": "Hello"},
            {"type": "text", "text": "world"}
        ]);
        assert_eq!(extract_text_content(&val), "Hello\nworld");
    }

    #[test]
    fn test_filter_system_hints() {
        let text = "Hello\n[System Hint] some hint\nWorld";
        let (cleaned, hints) = filter_system_hints(text);
        assert_eq!(cleaned, "Hello\n\nWorld");
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0], "some hint");
    }

    #[test]
    fn test_extract_from_boilerplate_no_boilerplate() {
        let content = "Just a normal message";
        let (user_text, boilerplate) = extract_from_boilerplate(content);
        assert_eq!(user_text, "Just a normal message");
        assert_eq!(boilerplate, "");
    }
}
