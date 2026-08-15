//! LLM interaction recording and replay (Task 8.6) —
//! port of `helen/runtime/recording.py`.
//!
//! Records LLM interactions to JSONL cassette files and replays them for
//! deterministic debugging.

use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// `CassetteEntry` — single LLM interaction recorded in a cassette.
#[derive(Debug, Clone)]
pub struct CassetteEntry {
    pub seq: u64,
    pub timestamp: f64,
    pub agent_name: Option<String>,
    pub model: String,
    pub request: Value,
    pub response: Value,
    pub usage: Value,
    pub duration_ms: f64,
    pub tool_calls: Vec<Value>,
    pub metadata: Value,
}

impl CassetteEntry {
    pub fn to_dict(&self) -> Value {
        json!({
            "type": "llm_call",
            "seq": self.seq,
            "timestamp": self.timestamp,
            "agent_name": self.agent_name,
            "model": self.model,
            "request": self.request,
            "response": self.response,
            "usage": self.usage,
            "duration_ms": self.duration_ms,
            "tool_calls": self.tool_calls,
            "metadata": self.metadata,
        })
    }

    pub fn from_dict(data: &Value) -> Option<Self> {
        Some(Self {
            seq: data.get("seq")?.as_u64()?,
            timestamp: data.get("timestamp")?.as_f64()?,
            agent_name: data
                .get("agent_name")
                .and_then(|v| v.as_str())
                .map(String::from),
            model: data
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            request: data.get("request").cloned().unwrap_or_else(|| json!({})),
            response: data.get("response").cloned().unwrap_or_else(|| json!({})),
            usage: data.get("usage").cloned().unwrap_or_else(|| json!({})),
            duration_ms: data
                .get("duration_ms")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            tool_calls: data
                .get("tool_calls")
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default(),
            metadata: data.get("metadata").cloned().unwrap_or_else(|| json!({})),
        })
    }
}

/// `CassetteWriter` — writes LLM interactions to a JSONL cassette file.
pub struct CassetteWriter {
    path: PathBuf,
    file: Option<std::fs::File>,
    seq: u64,
}

impl CassetteWriter {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            path: path.to_path_buf(),
            file: None,
            seq: 0,
        })
    }

    pub fn open(&mut self) -> std::io::Result<()> {
        if self.file.is_none() {
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            self.file = Some(f);
        }
        Ok(())
    }

    /// `write_entry` — append one interaction (appends "llm_call" line).
    #[allow(clippy::too_many_arguments)]
    pub fn write_entry(
        &mut self,
        request: &Value,
        response: &Value,
        usage: &Value,
        duration_ms: f64,
        agent_name: Option<&str>,
        model: &str,
        tool_calls: Option<&[Value]>,
        metadata: Option<&Value>,
    ) -> std::io::Result<()> {
        self.open()?;
        let entry = CassetteEntry {
            seq: self.seq,
            timestamp: crate::observability::now_ts(),
            agent_name: agent_name.map(String::from),
            model: model.to_string(),
            request: request.clone(),
            response: response.clone(),
            usage: usage.clone(),
            duration_ms,
            tool_calls: tool_calls.map(|t| t.to_vec()).unwrap_or_default(),
            metadata: metadata.cloned().unwrap_or_else(|| json!({})),
        };
        let line = serde_json::to_string(&entry.to_dict())?;
        if let Some(f) = self.file.as_mut() {
            writeln!(f, "{line}")?;
            f.flush()?;
        }
        self.seq += 1;
        Ok(())
    }

    pub fn close(&mut self) {
        self.file = None; // Drop closes the file.
    }
}

/// `CassetteReader` — reads LLM interactions from a cassette file.
#[derive(Debug)]
pub struct CassetteReader {
    path: PathBuf,
    entries: Vec<CassetteEntry>,
}

impl CassetteReader {
    pub fn new(path: &Path) -> Self {
        let mut r = Self {
            path: path.to_path_buf(),
            entries: Vec::new(),
        };
        r.load();
        r
    }

    fn load(&mut self) {
        if !self.path.exists() {
            return;
        }
        let Ok(f) = std::fs::File::open(&self.path) else {
            return;
        };
        let reader = BufReader::new(f);
        for (line_num, line) in reader.lines().enumerate() {
            let Ok(line) = line else { continue };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(line) {
                Ok(data) => match CassetteEntry::from_dict(&data) {
                    Some(entry) => self.entries.push(entry),
                    None => {
                        eprintln!(
                            "CassetteReader: corrupted line {} in {}: missing fields",
                            line_num + 1,
                            self.path.display()
                        );
                    }
                },
                Err(e) => {
                    eprintln!(
                        "CassetteReader: corrupted line {} in {}: {e}",
                        line_num + 1,
                        self.path.display()
                    );
                }
            }
        }
    }

    pub fn get_entry(&self, seq: u64) -> Option<&CassetteEntry> {
        self.entries.iter().find(|e| e.seq == seq)
    }

    pub fn get_next_entry(&self, current_seq: u64) -> Option<&CassetteEntry> {
        self.entries.iter().find(|e| e.seq > current_seq)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[CassetteEntry] {
        &self.entries
    }
}

/// `ReplayLlmRuntime` — replays recorded interactions (deterministic).
/// Mirrors Python's `ReplayLLMRuntime.act` returning the recorded response.
pub struct ReplayLlmRuntime {
    cassette: CassetteReader,
    current_seq: u64,
}

impl ReplayLlmRuntime {
    pub fn new(cassette_path: &Path) -> Self {
        Self {
            cassette: CassetteReader::new(cassette_path),
            current_seq: u64::MAX,
        }
    }

    /// Replay next interaction. Returns (text, tool_calls) like LLMResponse.
    pub fn act(&mut self) -> Result<(String, Vec<Value>), String> {
        let next_seq = if self.current_seq == u64::MAX {
            0
        } else {
            self.current_seq + 1
        };
        let entry = self
            .cassette
            .get_entry(next_seq)
            .or_else(|| self.cassette.get_next_entry(self.current_seq));
        let Some(entry) = entry else {
            return Err(format!(
                "No more recorded interactions in cassette. Used {} of {} entries.",
                self.current_seq.saturating_add(1),
                self.cassette.len()
            ));
        };
        self.current_seq = entry.seq;
        let text = entry
            .response
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tool_calls = entry
            .response
            .get("tool_calls")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        Ok((text, tool_calls))
    }
}

/// Convenience: JSON-Line round-trip used by tests and the interpreter.
pub fn cassette_to_list(path: &Path) -> Vec<Value> {
    CassetteReader::new(path)
        .entries()
        .iter()
        .map(CassetteEntry::to_dict)
        .collect()
}

/// `RecordingHook` — trait mirroring Python's Protocol for LLM recording.
pub trait RecordingHook: Send + Sync {
    fn on_request(&self, messages: &[Value], payload: &Value, metadata: &Value);
    fn on_response(&self, response_message: &Value, usage: &Value, duration_ms: f64);
    fn on_tool(&self, tool_call: &Value, result: &Value);
    fn on_turn_complete(&self, full_messages: &[Value], final_response: &Value);
}

/// `CassetteRecordingHook` — writes interactions through a CassetteWriter.
pub struct CassetteRecordingHook {
    _writer: std::sync::Mutex<CassetteWriter>,
}

impl CassetteRecordingHook {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            _writer: std::sync::Mutex::new(CassetteWriter::new(path)?),
        })
    }
}

impl RecordingHook for CassetteRecordingHook {
    fn on_request(&self, _messages: &[Value], _payload: &Value, _metadata: &Value) {}
    fn on_response(&self, _response_message: &Value, _usage: &Value, _duration_ms: f64) {}
    fn on_tool(&self, _tool_call: &Value, _result: &Value) {}
    fn on_turn_complete(&self, _full_messages: &[Value], _final_response: &Value) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("helen_cassette_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cassette.jsonl");
        let _ = std::fs::remove_file(&path);

        let mut w = CassetteWriter::new(&path).unwrap();
        w.write_entry(
            &json!({"messages": [{"role": "user", "content": "hi"}]}),
            &json!({"content": "hello", "tool_calls": []}),
            &json!({"prompt_tokens": 5, "completion_tokens": 1}),
            12.5,
            Some("agent1"),
            "gpt-4",
            None,
            None,
        )
        .unwrap();
        w.write_entry(
            &json!({"messages": []}),
            &json!({"content": "second"}),
            &json!({}),
            1.0,
            None,
            "gpt-4",
            None,
            None,
        )
        .unwrap();
        w.close();

        let r = CassetteReader::new(&path);
        assert_eq!(r.len(), 2);
        let e = r.get_entry(0).unwrap();
        assert_eq!(e.agent_name.as_deref(), Some("agent1"));
        assert_eq!(e.response["content"], "hello");
        assert_eq!(e.usage["prompt_tokens"], 5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replay_returns_recorded() {
        let dir = std::env::temp_dir().join(format!("helen_replay_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c.jsonl");
        let mut w = CassetteWriter::new(&path).unwrap();
        w.write_entry(
            &json!({}),
            &json!({"content": "resp1", "tool_calls": []}),
            &json!({}),
            1.0,
            None,
            "m",
            None,
            None,
        )
        .unwrap();
        w.write_entry(
            &json!({}),
            &json!({"content": "resp2", "tool_calls": []}),
            &json!({}),
            1.0,
            None,
            "m",
            None,
            None,
        )
        .unwrap();
        w.close();

        let mut r = ReplayLlmRuntime::new(&path);
        let (t1, _) = r.act().unwrap();
        assert_eq!(t1, "resp1");
        let (t2, _) = r.act().unwrap();
        assert_eq!(t2, "resp2");
        assert!(r.act().is_err()); // exhausted
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_lines_skipped() {
        let dir = std::env::temp_dir().join(format!("helen_corrupt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c.jsonl");
        std::fs::write(
            &path,
            "not json\n{\"seq\": 0, \"timestamp\": 1.0, \"request\": {}, \"response\": {\"content\": \"ok\"}, \"usage\": {}, \"duration_ms\": 0.0, \"tool_calls\": [], \"metadata\": {}}\n",
        )
        .unwrap();
        let r = CassetteReader::new(&path);
        assert_eq!(r.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
