//! AI-native observability (Task 8.6) — port of `helen/runtime/observability.py`.
//!
//! Structured execution context for AI debugging: call-stack tracking,
//! execution trace logging, structured error snapshots, LLM audit log,
//! and a central `ObservabilityManager`.

use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

/// Unix timestamp (float seconds, Python `time.time()` parity).
pub fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// `_truncate` — truncate a string to max length with Python's suffix.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}... [truncated]")
    }
}

/// `_safe_serialize` — serialize an arbitrary value for JSON output,
/// handling circular refs and non-serializable types (max depth 3).
pub fn safe_serialize(v: &Value) -> Value {
    fn ser(o: &Value, depth: usize, max_depth: usize) -> Value {
        if depth > max_depth {
            return json!("<max depth>");
        }
        match o {
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => o.clone(),
            Value::Array(items) => {
                if items.len() > 50 {
                    let mut out: Vec<Value> = items
                        .iter()
                        .take(50)
                        .map(|i| ser(i, depth + 1, max_depth))
                        .collect();
                    out.push(json!(format!("... ({} items)", items.len())));
                    Value::Array(out)
                } else {
                    Value::Array(items.iter().map(|i| ser(i, depth + 1, max_depth)).collect())
                }
            }
            Value::Object(map) => {
                let mut out = Map::new();
                for (k, v) in map.iter().take(50) {
                    out.insert(k.clone(), ser(v, depth + 1, max_depth));
                }
                if map.len() > 50 {
                    out.insert("...".into(), json!(format!("({} keys total)", map.len())));
                }
                Value::Object(out)
            }
        }
    }
    ser(v, 0, 3)
}

/// `_format_value` — format a value for display in error snapshots.
pub fn format_value(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => if *b { "true" } else { "false" }.into(),
        Value::String(s) => {
            if s.chars().count() > 50 {
                let head: String = s.chars().take(50).collect();
                format!("\"{head}...\"")
            } else {
                format!("\"{s}\"")
            }
        }
        Value::Array(items) => {
            if items.len() > 5 {
                let head: Vec<String> = items.iter().take(5).map(format_value).collect();
                format!("[{}, ... ({} items)]", head.join(", "), items.len())
            } else {
                let parts: Vec<String> = items.iter().map(format_value).collect();
                format!("[{}]", parts.join(", "))
            }
        }
        Value::Object(map) => {
            if map.len() > 5 {
                let head: Vec<String> = map
                    .iter()
                    .take(5)
                    .map(|(k, v)| format!("{k}: {}", format_value(v)))
                    .collect();
                format!("{{{}, ...}}", head.join(", "))
            } else {
                let parts: Vec<String> = map
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", format_value(v)))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
        }
        Value::Number(n) => n.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Call Stack
// ---------------------------------------------------------------------------

/// `CallFrame` — a single frame in the call stack.
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_name: String,
    pub location: String, // "file:line:col" or "line:col"
    pub args: Value,
    pub entry_time: f64,
}

impl CallFrame {
    /// Format a source location string: `file:start_line:start_col` or
    /// `start_line:start_col` when no file (Python `format_location`).
    pub fn format_location(file: Option<&str>, start_line: u32, start_col: u32) -> String {
        match file {
            Some(f) if !f.is_empty() => format!("{f}:{start_line}:{start_col}"),
            _ => format!("{start_line}:{start_col}"),
        }
    }

    pub fn to_dict(&self) -> Value {
        json!({
            "function": self.function_name,
            "location": self.location,
            "args": safe_serialize(&self.args),
        })
    }
}

/// `CallStackTracker` — tracks the call stack during execution.
#[derive(Debug)]
pub struct CallStackTracker {
    stack: Vec<CallFrame>,
    max_depth: usize,
    enabled: bool,
}

impl Default for CallStackTracker {
    fn default() -> Self {
        Self::new(100)
    }
}

impl CallStackTracker {
    pub fn new(max_depth: usize) -> Self {
        Self {
            stack: Vec::new(),
            max_depth,
            enabled: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
        if !value {
            self.stack.clear();
        }
    }

    pub fn push(&mut self, function_name: &str, location: String, args: Option<Value>) {
        if !self.enabled || self.stack.len() >= self.max_depth {
            return;
        }
        self.stack.push(CallFrame {
            function_name: function_name.to_string(),
            location,
            args: args.unwrap_or_else(|| json!({})),
            entry_time: now_ts(),
        });
    }

    pub fn pop(&mut self) -> Option<CallFrame> {
        if !self.enabled || self.stack.is_empty() {
            return None;
        }
        self.stack.pop()
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn to_list(&self) -> Vec<Value> {
        self.stack.iter().map(CallFrame::to_dict).collect()
    }

    /// `format_traceback` — most recent call first.
    pub fn format_traceback(&self) -> String {
        if self.stack.is_empty() {
            return String::new();
        }
        let mut lines = vec!["Traceback (most recent call first):".to_string()];
        let n = self.stack.len();
        for (i, frame) in self.stack.iter().rev().enumerate() {
            let prefix = if i < n - 1 { "  " } else { "-> " };
            lines.push(format!(
                "{prefix}{} in {}",
                frame.location, frame.function_name
            ));
        }
        lines.join("\n")
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }
}

// ---------------------------------------------------------------------------
// Execution Trace
// ---------------------------------------------------------------------------

/// `TraceEntry` — a single execution trace event.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    pub timestamp: f64,
    pub event_type: String,
    pub location: String,
    pub data: Value,
}

impl TraceEntry {
    pub fn to_dict(&self) -> Value {
        json!({
            "time": self.timestamp,
            "type": self.event_type,
            "location": self.location,
            "data": safe_serialize(&self.data),
        })
    }
}

/// `ExecutionTracer` — records execution trace (drop-oldest ring).
#[derive(Debug)]
pub struct ExecutionTracer {
    entries: Vec<TraceEntry>,
    max_entries: usize,
    enabled: bool,
}

impl Default for ExecutionTracer {
    fn default() -> Self {
        Self::new(10000)
    }
}

impl ExecutionTracer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            enabled: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
        if !value {
            self.entries.clear();
        }
    }

    pub fn trace(&mut self, event_type: &str, location: String, data: Option<Value>) {
        if !self.enabled {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(TraceEntry {
            timestamp: now_ts(),
            event_type: event_type.to_string(),
            location,
            data: data.unwrap_or_else(|| json!({})),
        });
    }

    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    pub fn to_list(&self) -> Vec<Value> {
        self.entries.iter().map(TraceEntry::to_dict).collect()
    }

    /// `format_trace` — last N entries as text (Python json.dumps data).
    pub fn format_trace(&self, last_n: usize) -> String {
        if self.entries.is_empty() {
            return "(no trace entries)".to_string();
        }
        let start = self.entries.len().saturating_sub(last_n);
        let mut lines = Vec::new();
        for entry in &self.entries[start..] {
            let mut line = format!("[{}] {}", entry.event_type, entry.location);
            if !entry.data.is_null() && entry.data != json!({}) {
                line.push_str(&format!(" {}", entry.data));
            }
            lines.push(line);
        }
        lines.join("\n")
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Error Snapshot
// ---------------------------------------------------------------------------

/// `ErrorSnapshot` — structured error context for AI debugging.
#[derive(Debug, Clone)]
pub struct ErrorSnapshot {
    pub error_type: String,
    pub message: String,
    pub location: String,
    pub call_stack: Vec<Value>,
    pub scope: Value,
    pub trace: Vec<Value>,
    pub timestamp: f64,
    pub diagnostic_category: String,
    pub suggestion: String,
    pub data_flow: Vec<Value>,
}

impl ErrorSnapshot {
    pub fn to_dict(&self) -> Value {
        json!({
            "error": {
                "type": self.error_type,
                "message": self.message,
                "location": self.location,
            },
            "call_stack": self.call_stack,
            "scope": safe_serialize(&self.scope),
            "trace": self.trace.iter().rev().take(20).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
            "timestamp": self.timestamp,
            "diagnostic_category": self.diagnostic_category,
            "suggestion": self.suggestion,
            "data_flow": self.data_flow,
        })
    }

    /// `format_text` — human-readable error context (verbose includes trace).
    pub fn format_text(&self, verbose: bool) -> String {
        let ts = format_ts_local(self.timestamp);
        let mut lines = vec![
            format!("Error: {}: {}", self.error_type, self.message),
            format!("Location: {}", self.location),
            format!("Time: {ts}"),
        ];
        if !self.diagnostic_category.is_empty() {
            lines.push(format!("Category: {}", self.diagnostic_category));
        }
        if !self.suggestion.is_empty() {
            lines.push(format!("Suggestion: {}", self.suggestion));
        }
        lines.push(String::new());
        lines.push("Call Stack:".to_string());
        if self.call_stack.is_empty() {
            lines.push("  (empty)".to_string());
        } else {
            let n = self.call_stack.len();
            for (i, frame) in self.call_stack.iter().rev().enumerate() {
                let prefix = if i < n - 1 { "  " } else { "-> " };
                let loc = frame.get("location").and_then(|v| v.as_str()).unwrap_or("");
                let func = frame.get("function").and_then(|v| v.as_str()).unwrap_or("");
                lines.push(format!("{prefix}{loc} in {func}"));
            }
        }
        if self.scope.is_object() && !self.scope.as_object().expect("object exists").is_empty() {
            lines.push(String::new());
            lines.push("Variables in scope:".to_string());
            if let Some(map) = self.scope.as_object() {
                for (name, value) in map {
                    lines.push(format!("  {name} = {}", format_value(value)));
                }
            }
        }
        if !self.data_flow.is_empty() {
            lines.push(String::new());
            lines.push("Data Flow:".to_string());
            for entry in &self.data_flow {
                let var = entry.get("variable").and_then(|v| v.as_str()).unwrap_or("");
                let source = entry.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let via = entry.get("via").and_then(|v| v.as_str()).unwrap_or("");
                if !var.is_empty() {
                    lines.push(format!("  {var} ← {source} (via {via})"));
                } else {
                    lines.push(format!("  {source} → {via}"));
                }
            }
        }
        if verbose && !self.trace.is_empty() {
            lines.push(String::new());
            let n = self.trace.len().min(20);
            lines.push(format!("Execution Trace (last {n} entries):"));
            let start = self.trace.len().saturating_sub(20);
            for entry in &self.trace[start..] {
                let t = entry.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                let loc = entry.get("location").and_then(|v| v.as_str()).unwrap_or("");
                let data = entry.get("data").cloned().unwrap_or_else(|| json!({}));
                let func = data
                    .get("function")
                    .and_then(|v| v.as_str())
                    .or_else(|| data.get("agent").and_then(|v| v.as_str()))
                    .unwrap_or("");
                match t {
                    "call" => lines.push(format!("  → {loc} call {func}")),
                    "return" => {
                        let ret = data
                            .get("return_value")
                            .cloned()
                            .unwrap_or_else(|| json!(""));
                        lines.push(format!("  ← {loc} return {func} → {ret}"));
                    }
                    _ => lines.push(format!("  [{t}] {loc} {func}")),
                }
            }
        }
        lines.join("\n")
    }
}

/// Format a unix timestamp as local `%Y-%m-%d %H:%M:%S` (Python strftime).
fn format_ts_local(ts: f64) -> String {
    let secs = ts as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Civil-from-days (Howard Hinnant algorithm)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mth <= 2 { y + 1 } else { y };
    format!("{y:04}-{mth:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

// ---------------------------------------------------------------------------
// LLM Audit Log
// ---------------------------------------------------------------------------

/// `LLMAuditEntry` — audit log entry for LLM calls.
#[derive(Debug, Clone)]
pub struct LlmAuditEntry {
    pub timestamp: f64,
    pub call_type: String, // act | stream | route
    pub agent_name: Option<String>,
    pub model: Option<String>,
    pub prompt: String,
    pub response: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub duration_ms: f64,
    pub tool_calls: Vec<Value>,
    pub error: Option<String>,
}

impl LlmAuditEntry {
    pub fn to_dict(&self) -> Value {
        let mut result = Map::new();
        result.insert("time".into(), json!(self.timestamp));
        result.insert("type".into(), json!(self.call_type));
        result.insert("agent".into(), json!(self.agent_name));
        result.insert("model".into(), json!(self.model));
        result.insert("prompt".into(), json!(truncate(&self.prompt, 500)));
        result.insert("tokens_in".into(), json!(self.tokens_in));
        result.insert("tokens_out".into(), json!(self.tokens_out));
        result.insert(
            "duration_ms".into(),
            json!((self.duration_ms * 100.0).round() / 100.0),
        );
        if let Some(resp) = &self.response {
            result.insert("response".into(), json!(truncate(resp, 500)));
        }
        if !self.tool_calls.is_empty() {
            result.insert("tool_calls".into(), Value::Array(self.tool_calls.clone()));
        }
        if let Some(err) = &self.error {
            result.insert("error".into(), json!(err));
        }
        Value::Object(result)
    }
}

/// `LLMAuditLog` — bounded audit log for LLM calls.
#[derive(Debug)]
pub struct LlmAuditLog {
    entries: Vec<LlmAuditEntry>,
    max_entries: usize,
    enabled: bool,
}

impl Default for LlmAuditLog {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 1000,
            enabled: true,
        }
    }
}

impl LlmAuditLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            enabled: true,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
    }

    pub fn log(&mut self, entry: LlmAuditEntry) {
        if !self.enabled {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[LlmAuditEntry] {
        &self.entries
    }

    pub fn to_list(&self) -> Vec<Value> {
        self.entries.iter().map(LlmAuditEntry::to_dict).collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Observability Manager
// ---------------------------------------------------------------------------

/// `ObservabilityManager` — central manager for all observability features.
#[derive(Debug, Default)]
pub struct ObservabilityManager {
    pub call_stack: CallStackTracker,
    pub tracer: ExecutionTracer,
    pub llm_audit: LlmAuditLog,
    last_error: Option<ErrorSnapshot>,
}

impl ObservabilityManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// `capture_error` — build an error snapshot with diagnostics and store it.
    pub fn capture_error(
        &mut self,
        error_type: &str,
        message: &str,
        location: String,
        scope: Option<Value>,
        exception_context: Option<&Map<String, Value>>,
    ) -> ErrorSnapshot {
        let call_stack_list = self.call_stack.to_list();
        let scope_dict = scope.unwrap_or_else(|| json!({}));

        // Extract exception context for diagnostics (known attributes).
        let mut exc_ctx = Map::new();
        if let Some(ctx) = exception_context {
            for attr in [
                "tokens_used",
                "tokens_limit",
                "agent_name",
                "agent_args",
                "cause",
                "errors",
            ] {
                if let Some(v) = ctx.get(attr) {
                    exc_ctx.insert(attr.to_string(), v.clone());
                }
            }
        }

        let diag = crate::diagnostics::generate_diagnostics(
            error_type,
            message,
            Some(&scope_dict),
            Some(&call_stack_list),
            Some(&Value::Object(exc_ctx)),
        );
        let category = diag
            .get("diagnostic_category")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let suggestion = diag
            .get("suggestion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let data_flow = diag
            .get("data_flow")
            .cloned()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        let snapshot = ErrorSnapshot {
            error_type: error_type.to_string(),
            message: message.to_string(),
            location,
            call_stack: call_stack_list,
            scope: scope_dict,
            trace: self.tracer.to_list(),
            timestamp: now_ts(),
            diagnostic_category: category,
            suggestion,
            data_flow,
        };
        self.last_error = Some(snapshot.clone());
        snapshot
    }

    pub fn last_error(&self) -> Option<&ErrorSnapshot> {
        self.last_error.as_ref()
    }

    /// `reset` — clear all observability state.
    pub fn reset(&mut self) {
        self.call_stack.clear();
        self.tracer.clear();
        self.llm_audit.clear();
        self.last_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_stack_traceback_ordering() {
        let mut cs = CallStackTracker::new(100);
        cs.set_enabled(true);
        cs.push("inner", "file.helen:3:1".into(), None);
        cs.push("outer", "file.helen:10:1".into(), None);
        let tb = cs.format_traceback();
        // Reversed stack: outer listed first with "  ", inner last with "-> ".
        assert!(tb.contains("  file.helen:10:1 in outer"), "{tb}");
        assert!(tb.contains("-> file.helen:3:1 in inner"), "{tb}");
        assert_eq!(cs.depth(), 2);
        assert!(cs.pop().is_some());
        assert_eq!(cs.depth(), 1);
    }

    #[test]
    fn call_stack_disabled_noop() {
        let mut cs = CallStackTracker::new(10);
        cs.push("f", "a:1:1".into(), None);
        assert_eq!(cs.depth(), 0);
        assert!(cs.pop().is_none());
    }

    #[test]
    fn tracer_ring_bounded() {
        let mut t = ExecutionTracer::new(3);
        t.set_enabled(true);
        for i in 0..5 {
            t.trace("stmt", format!("f:{}:1", i), None);
        }
        assert_eq!(t.entries().len(), 3);
        assert_eq!(t.entries()[0].location, "f:2:1");
    }

    #[test]
    fn audit_log_truncates_and_bounds() {
        let mut log = LlmAuditLog::new(2);
        for i in 0..3 {
            log.log(LlmAuditEntry {
                timestamp: 1.0,
                call_type: "act".into(),
                agent_name: Some(format!("agent{i}")),
                model: None,
                prompt: "p".repeat(600),
                response: None,
                tokens_in: 10,
                tokens_out: 5,
                duration_ms: 1.25,
                tool_calls: vec![],
                error: None,
            });
        }
        assert_eq!(log.entries().len(), 2);
        let d = log.entries()[0].to_dict();
        assert!(d
            .get("prompt")
            .unwrap()
            .as_str()
            .unwrap()
            .ends_with("[truncated]"));
        assert_eq!(
            d.get("duration_ms")
                .expect("key exists")
                .as_f64()
                .expect("f64 value"),
            1.25
        );
    }

    #[test]
    fn snapshot_format_text_verbose() {
        let mut mgr = ObservabilityManager::new();
        mgr.call_stack.set_enabled(true);
        mgr.call_stack.push("f", "a.helen:5:2".into(), None);
        let snap = mgr.capture_error(
            "RuntimeError",
            "division by zero",
            "a.helen:7:3".into(),
            Some(json!({"x": 1})),
            None,
        );
        let text = snap.format_text(true);
        assert!(
            text.contains("Error: RuntimeError: division by zero"),
            "{text}"
        );
        assert!(text.contains("Location: a.helen:7:3"), "{text}");
        assert!(text.contains("Category: RuntimeGenericError"), "{text}");
        assert!(text.contains("Suggestion:"), "{text}");
        assert!(text.contains("Variables in scope:"), "{text}");
        assert!(text.contains("  x = 1"), "{text}");
        assert!(mgr.last_error().is_some());
    }

    #[test]
    fn reset_clears_state() {
        let mut mgr = ObservabilityManager::new();
        mgr.capture_error("E", "m", "l".into(), None, None);
        mgr.tracer.set_enabled(true);
        mgr.tracer.trace("stmt", "l".into(), None);
        mgr.reset();
        assert!(mgr.last_error().is_none());
        assert_eq!(mgr.tracer.entries().len(), 0);
    }

    #[test]
    fn safe_serialize_truncates_long() {
        let big: Vec<Value> = (0..60).map(|i| json!(i)).collect();
        let out = safe_serialize(&Value::Array(big));
        let arr = out.as_array().expect("array exists");
        assert_eq!(arr.len(), 51);
        assert!(arr
            .last()
            .expect("non-empty array")
            .as_str()
            .expect("string value")
            .contains("60 items"));
    }

    #[test]
    fn ts_format_local() {
        // 1970-01-02 00:00:00 UTC (local TZ dependent, just check no panic)
        let s = format_ts_local(86_400.0);
        assert!(!s.is_empty());
    }
}
