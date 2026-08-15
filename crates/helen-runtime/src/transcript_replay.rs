//! Transcript post-mortem replay (Task 8.6) —
//! port of `helen/runtime/transcript_replay.py`.
//!
//! Interactive navigation of transcript sessions: step next/prev/jump,
//! search, summaries, and message formatting for post-mortem debugging.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `TranscriptReplay` — interactive transcript replay for debugging.
pub struct TranscriptReplay {
    pub session_id: String,
    pub session_dir: PathBuf,
    pub messages: Vec<Value>,
    pub current_index: usize,
}

impl TranscriptReplay {
    /// Load a session transcript from its directory.
    /// Prefers `transcript.db` (SQLite) then `transcript.jsonl` (JSONL).
    pub fn load(session_id: &str, session_dir: &Path) -> Result<Self, String> {
        let session_path = session_dir.join(session_id);
        if !session_path.exists() {
            return Err(format!(
                "Session directory not found: {}",
                session_path.display()
            ));
        }
        let jsonl_path = session_path.join("transcript.jsonl");
        if !jsonl_path.exists() {
            return Err(format!(
                "No transcript file found in {}",
                session_path.display()
            ));
        }

        let store = crate::transcript::TranscriptStore::load_from_backend(
            crate::transcript::JsonlBackend::new(jsonl_path),
            1000,
        );
        let mut store = store;
        let messages = store
            .read_view()
            .into_iter()
            .map(|m| crate::transcript::message_to_dict(&m))
            .collect();
        Ok(Self {
            session_id: session_id.to_string(),
            session_dir: session_dir.to_path_buf(),
            messages,
            current_index: 0,
        })
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn current_message(&self) -> Option<&Value> {
        self.messages.get(self.current_index)
    }

    /// `next` — move forward (stays at end if already at end).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&Value> {
        if self.current_index + 1 < self.messages.len() {
            self.current_index += 1;
        }
        self.current_message()
    }

    /// `prev` — move backward (stays at 0 if already at 0).
    pub fn prev(&mut self) -> Option<&Value> {
        if self.current_index > 0 {
            self.current_index -= 1;
        }
        self.current_message()
    }

    /// `jump` — move to index (clamped; no-op if out of range).
    pub fn jump(&mut self, index: usize) -> Option<&Value> {
        if index < self.messages.len() {
            self.current_index = index;
        }
        self.current_message()
    }

    pub fn first(&mut self) -> Option<&Value> {
        if !self.messages.is_empty() {
            self.current_index = 0;
        }
        self.current_message()
    }

    pub fn last(&mut self) -> Option<&Value> {
        if !self.messages.is_empty() {
            self.current_index = self.messages.len() - 1;
        }
        self.current_message()
    }

    pub fn get_message_at(&self, index: usize) -> Option<&Value> {
        self.messages.get(index)
    }

    /// `search` — indices of messages whose content contains the query.
    pub fn search(&self, query: &str, case_sensitive: bool) -> Vec<usize> {
        let search_query = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };
        let mut results = Vec::new();
        for (i, msg) in self.messages.iter().enumerate() {
            let content = msg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let haystack = if case_sensitive {
                content
            } else {
                content.to_lowercase()
            };
            if haystack.contains(&search_query) {
                results.push(i);
            }
        }
        results
    }

    /// `get_summary` — session statistics (role/agent counters).
    pub fn get_summary(&self) -> Value {
        let mut roles: HashMap<String, usize> = HashMap::new();
        let mut agents: HashMap<String, usize> = HashMap::new();
        for msg in &self.messages {
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            *roles.entry(role).or_insert(0) += 1;
            if let Some(agent) = msg.get("agent_name").and_then(|v| v.as_str()) {
                if !agent.is_empty() {
                    *agents.entry(agent.to_string()).or_insert(0) += 1;
                }
            }
        }
        json!({
            "session_id": self.session_id,
            "total_messages": self.messages.len(),
            "roles": roles,
            "agents": agents,
            "current_index": self.current_index,
        })
    }

    /// `format_message` — one-line display with role/agent/content.
    pub fn format_message(&self, msg: &Value) -> String {
        let role = msg
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let agent = msg.get("agent_name").and_then(|v| v.as_str()).unwrap_or("");
        let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let truncated: String = content.chars().take(200).collect();
        let content = if content.chars().count() > 200 {
            format!("{truncated}...")
        } else {
            truncated
        };
        if agent.is_empty() {
            format!("{role}: {content}")
        } else {
            format!("[{agent}] {role}: {content}")
        }
    }

    /// `format_current` — "[index/total] message" line (Python print_current).
    pub fn format_current(&self) -> String {
        match self.current_message() {
            Some(msg) => format!(
                "[{}/{}] {}",
                self.current_index,
                self.messages.len(),
                self.format_message(msg)
            ),
            None => "No message at current position".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(dir: &Path, id: &str) {
        let sp = dir.join(id);
        std::fs::create_dir_all(&sp).unwrap();
        let backend = crate::transcript::JsonlBackend::new(sp.join("transcript.jsonl"));
        let mut store = crate::transcript::TranscriptStore::new(Some(backend), 1000);
        for (i, role) in ["user", "assistant", "user"].iter().enumerate() {
            let mut m = crate::transcript::Message::new(
                role,
                serde_json::json!(format!("msg {i} content with hello")),
                vec![],
                None,
                format!("uuid-{i}"),
                None,
                50,
                false,
                false,
                Some(format!("agent{i}")),
                String::new(),
                String::new(),
                vec![],
            );
            store.append(&mut m, true);
        }
    }

    #[test]
    fn navigation_and_search() {
        let dir = std::env::temp_dir().join(format!("helen_replay_nav_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        make_session(&dir, "s1");
        let mut r = TranscriptReplay::load("s1", &dir).unwrap();
        assert_eq!(r.len(), 3);
        assert!(r.current_message().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("msg 0"));
        assert!(r.next().is_some());
        assert_eq!(r.current_index, 1);
        assert!(r.next().is_some());
        r.next(); // stays at end
        assert_eq!(r.current_index, 2);
        assert!(r.prev().is_some());
        assert_eq!(r.current_index, 1);
        r.jump(0);
        assert_eq!(r.current_index, 0);
        let hits = r.search("hello", false);
        assert_eq!(hits, vec![0, 1, 2]);
        let hits = r.search("MSG 1", false);
        assert_eq!(hits, vec![1]);
        let hits = r.search("MSG 1", true);
        assert!(hits.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn summary_and_format() {
        let dir = std::env::temp_dir().join(format!("helen_replay_sum_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        make_session(&dir, "s2");
        let r = TranscriptReplay::load("s2", &dir).unwrap();
        let s = r.get_summary();
        assert_eq!(s["total_messages"], 3);
        assert_eq!(s["roles"]["user"], 2);
        assert_eq!(s["roles"]["assistant"], 1);
        assert_eq!(s["agents"]["agent1"], 1);
        let f = r.format_message(&r.messages[0]);
        assert!(f.contains("user:"), "{f}");
        assert!(f.contains("msg 0"), "{f}");
        let cur = r.format_current();
        assert!(cur.starts_with("[0/3]"), "{cur}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_session_errors() {
        let dir = std::env::temp_dir().join(format!("helen_replay_miss_{}", std::process::id()));
        assert!(TranscriptReplay::load("nope", &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
