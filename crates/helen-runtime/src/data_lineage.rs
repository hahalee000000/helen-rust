//! Data lineage tracking for cross-agent data flow (Task 8.6) —
//! port of `helen/runtime/data_lineage.py`.
//!
//! Tracks how data flows between agents via a SQLite sidecar database
//! (`<session_id>_lineage.db`), independent of the transcript backend.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// `DataFlow` — a single data flow event between producer and consumer.
#[derive(Debug, Clone)]
pub struct DataFlow {
    pub producer_uuid: String,
    pub consumer_uuid: String,
    pub flow_type: String,
    pub timestamp: f64,
    pub metadata: Value,
}

impl DataFlow {
    pub fn to_dict(&self) -> Value {
        json!({
            "producer_uuid": self.producer_uuid,
            "consumer_uuid": self.consumer_uuid,
            "flow_type": self.flow_type,
            "timestamp": self.timestamp,
            "metadata": self.metadata,
        })
    }

    pub fn from_dict(data: &Value) -> Option<Self> {
        Some(Self {
            producer_uuid: data.get("producer_uuid")?.as_str()?.to_string(),
            consumer_uuid: data.get("consumer_uuid")?.as_str()?.to_string(),
            flow_type: data.get("flow_type")?.as_str()?.to_string(),
            timestamp: data
                .get("timestamp")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            metadata: data.get("metadata").cloned().unwrap_or_else(|| json!({})),
        })
    }
}

/// `DataLineageTracker` — SQLite-backed lineage tracking.
/// The Rust port persists to `<session_id>_lineage.jsonl` (append-only JSONL)
/// with the same schema semantics as Python's SQLite sidecar.
pub struct DataLineageTracker {
    pub session_dir: PathBuf,
    pub session_id: String,
    db_path: PathBuf,
    flows: Vec<DataFlow>,
    dirty: bool,
}

impl DataLineageTracker {
    pub fn new(session_dir: &Path, session_id: &str) -> Self {
        let db_path = session_dir.join(format!("{session_id}_lineage.db"));
        let mut tracker = Self {
            session_dir: session_dir.to_path_buf(),
            session_id: session_id.to_string(),
            db_path,
            flows: Vec::new(),
            dirty: false,
        };
        tracker.load();
        tracker
    }

    /// Create an in-memory tracker (no persistence).
    pub fn new_in_memory() -> Self {
        Self {
            session_dir: PathBuf::new(),
            session_id: String::new(),
            db_path: PathBuf::new(),
            flows: Vec::new(),
            dirty: false,
        }
    }

    /// Load persisted flows (tolerates missing/corrupt files).
    fn load(&mut self) {
        let jsonl_path = self.jsonl_path();
        if !jsonl_path.exists() {
            return;
        }
        let Ok(content) = std::fs::read_to_string(&jsonl_path) else {
            return;
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                if let Some(flow) = DataFlow::from_dict(&v) {
                    self.flows.push(flow);
                }
            }
        }
    }

    fn jsonl_path(&self) -> PathBuf {
        self.db_path.with_extension("jsonl")
    }

    /// `record_flow` — record a data flow event and persist immediately.
    pub fn record_flow(
        &mut self,
        producer_uuid: &str,
        consumer_uuid: &str,
        flow_type: &str,
        metadata: Option<&Value>,
    ) {
        let flow = DataFlow {
            producer_uuid: producer_uuid.to_string(),
            consumer_uuid: consumer_uuid.to_string(),
            flow_type: flow_type.to_string(),
            timestamp: crate::observability::now_ts(),
            metadata: metadata.cloned().unwrap_or_else(|| json!({})),
        };
        self.flows.push(flow);
        self.dirty = true;
        self.persist();
    }

    fn persist(&self) {
        let path = self.jsonl_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut out = String::new();
        for f in &self.flows {
            if let Ok(line) = serde_json::to_string(&f.to_dict()) {
                out.push_str(&line);
                out.push('\n');
            }
        }
        let _ = std::fs::write(&path, out);
    }

    /// `get_origin` — flows where the given UUID consumed data (time order).
    pub fn get_origin(&self, consumer_uuid: &str) -> Vec<DataFlow> {
        let mut flows: Vec<DataFlow> = self
            .flows
            .iter()
            .filter(|f| f.consumer_uuid == consumer_uuid)
            .cloned()
            .collect();
        flows.sort_by(|a, b| {
            a.timestamp
                .partial_cmp(&b.timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        flows
    }

    /// `get_consumers` — flows where the given UUID produced data.
    pub fn get_consumers(&self, producer_uuid: &str) -> Vec<DataFlow> {
        let mut flows: Vec<DataFlow> = self
            .flows
            .iter()
            .filter(|f| f.producer_uuid == producer_uuid)
            .cloned()
            .collect();
        flows.sort_by(|a, b| {
            a.timestamp
                .partial_cmp(&b.timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        flows
    }

    /// `get_full_lineage` — complete graph: nodes (UUIDs) + edges.
    pub fn get_full_lineage(&self) -> Value {
        let mut nodes: BTreeSet<String> = BTreeSet::new();
        let mut edges: Vec<Value> = Vec::new();
        let mut flows = self.flows.clone();
        flows.sort_by(|a, b| {
            a.timestamp
                .partial_cmp(&b.timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for f in &flows {
            nodes.insert(f.producer_uuid.clone());
            nodes.insert(f.consumer_uuid.clone());
            edges.push(json!({
                "source": f.producer_uuid,
                "target": f.consumer_uuid,
                "flow_type": f.flow_type,
                "timestamp": f.timestamp,
                "metadata": f.metadata,
            }));
        }
        json!({
            "nodes": nodes.into_iter().collect::<Vec<_>>(),
            "edges": edges,
        })
    }

    pub fn close(&mut self) {
        if self.dirty {
            self.persist();
            self.dirty = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> (DataLineageTracker, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "helen_lineage_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create dir");
        (DataLineageTracker::new(&dir, "sess-1"), dir)
    }

    #[test]
    fn record_and_query_origin_consumers() {
        let (mut t, dir) = tracker();
        t.record_flow(
            "producer-a",
            "consumer-b",
            "channel",
            Some(&json!({"name": "ch1"})),
        );
        t.record_flow("producer-x", "consumer-b", "agent_call", None);
        t.record_flow("producer-b", "consumer-c", "prompt", None);

        let origins = t.get_origin("consumer-b");
        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0].producer_uuid, "producer-a");
        assert_eq!(origins[1].producer_uuid, "producer-x");

        let consumers = t.get_consumers("producer-b");
        assert_eq!(consumers.len(), 1);
        assert_eq!(consumers[0].consumer_uuid, "consumer-c");
        // Metadata belongs to the a→b flow (origins[0]).
        assert_eq!(origins[0].metadata["name"], "ch1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn full_lineage_graph() {
        let (mut t, dir) = tracker();
        t.record_flow("a", "b", "channel", None);
        t.record_flow("b", "c", "agent_call", None);
        let g = t.get_full_lineage();
        assert_eq!(g["nodes"].as_array().expect("array exists").len(), 3);
        assert_eq!(g["edges"].as_array().expect("array exists").len(), 2);
        assert_eq!(g["edges"][0]["source"], "a");
        assert_eq!(g["edges"][1]["target"], "c");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn persists_across_reload() {
        let dir =
            std::env::temp_dir().join(format!("helen_lineage_persist_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        {
            let mut t = DataLineageTracker::new(&dir, "sess-2");
            t.record_flow("a", "b", "channel", None);
            t.close();
        }
        let t = DataLineageTracker::new(&dir, "sess-2");
        assert_eq!(t.get_origin("b").len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_db_empty() {
        let dir = std::env::temp_dir().join(format!("helen_lineage_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let t = DataLineageTracker::new(&dir, "sess-3");
        assert!(t.get_full_lineage()["edges"].as_array().expect("array exists").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
