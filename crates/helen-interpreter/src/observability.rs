//! AI-native observability for Helen runtime.
//!
//! Byte-faithful port of `helen/runtime/observability.py` (v1.44.0).
//! Provides structured execution context for AI debugging:
//! - Call stack tracking
//! - Execution trace logging
//! - Structured error snapshots
//! - LLM call audit logging

use std::collections::HashMap;

use helen_core::source::SourceSpan;

// ---------------------------------------------------------------------------
// Call Frame
// ---------------------------------------------------------------------------

/// A single frame in the call stack.
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_name: String,
    pub location: String,
    pub args: HashMap<String, String>,
    pub entry_time: f64,
}

impl CallFrame {
    pub fn new(
        function_name: &str,
        span: Option<&SourceSpan>,
        args: HashMap<String, String>,
    ) -> Self {
        CallFrame {
            function_name: function_name.to_string(),
            location: Self::format_location(span),
            args,
            entry_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        }
    }

    pub fn format_location(span: Option<&SourceSpan>) -> String {
        match span {
            None => "<unknown>".to_string(),
            Some(s) => {
                if !s.file.is_empty() {
                    format!("{}:{}:{}", s.file, s.start_line, s.start_col)
                } else {
                    format!("{}:{}", s.start_line, s.start_col)
                }
            }
        }
    }

    pub fn to_dict(&self) -> HashMap<String, serde_json::Value> {
        let mut d = HashMap::new();
        d.insert(
            "function".to_string(),
            serde_json::Value::String(self.function_name.clone()),
        );
        d.insert(
            "location".to_string(),
            serde_json::Value::String(self.location.clone()),
        );
        let args_map: serde_json::Map<String, serde_json::Value> = self
            .args
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        d.insert("args".to_string(), serde_json::Value::Object(args_map));
        d
    }
}

// ---------------------------------------------------------------------------
// Call Stack Tracker
// ---------------------------------------------------------------------------

/// Tracks the call stack during execution.
pub struct CallStackTracker {
    stack: Vec<CallFrame>,
    max_depth: usize,
    pub enabled: bool,
}

impl CallStackTracker {
    pub fn new(max_depth: usize) -> Self {
        CallStackTracker {
            stack: Vec::new(),
            max_depth,
            enabled: false,
        }
    }

    pub fn push(
        &mut self,
        function_name: &str,
        span: Option<&SourceSpan>,
        args: HashMap<String, String>,
    ) {
        if !self.enabled {
            return;
        }
        if self.stack.len() >= self.max_depth {
            return;
        }
        self.stack.push(CallFrame::new(function_name, span, args));
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

    pub fn to_list(&self) -> Vec<HashMap<String, serde_json::Value>> {
        self.stack.iter().map(|f| f.to_dict()).collect()
    }

    pub fn format_traceback(&self) -> String {
        if self.stack.is_empty() {
            return String::new();
        }
        let mut lines = vec!["Traceback (most recent call first):".to_string()];
        for (i, frame) in self.stack.iter().rev().enumerate() {
            let prefix = if i < self.stack.len() - 1 {
                "  "
            } else {
                "-> "
            };
            lines.push(format!(
                "{}{} in {}",
                prefix, frame.location, frame.function_name
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

/// A single entry in the execution trace.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    pub timestamp: f64,
    pub event_type: String,
    pub location: String,
    pub data: HashMap<String, String>,
}

impl TraceEntry {
    pub fn new(event_type: &str, span: Option<&SourceSpan>, data: HashMap<String, String>) -> Self {
        TraceEntry {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            event_type: event_type.to_string(),
            location: CallFrame::format_location(span),
            data,
        }
    }

    pub fn to_dict(&self) -> HashMap<String, serde_json::Value> {
        let mut d = HashMap::new();
        d.insert(
            "time".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.timestamp as u64)),
        );
        d.insert(
            "type".to_string(),
            serde_json::Value::String(self.event_type.clone()),
        );
        d.insert(
            "location".to_string(),
            serde_json::Value::String(self.location.clone()),
        );
        let data_map: serde_json::Map<String, serde_json::Value> = self
            .data
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        d.insert("data".to_string(), serde_json::Value::Object(data_map));
        d
    }
}

/// Records execution trace for AI debugging.
pub struct ExecutionTracer {
    entries: Vec<TraceEntry>,
    max_entries: usize,
    pub enabled: bool,
}

impl ExecutionTracer {
    pub fn new(max_entries: usize) -> Self {
        ExecutionTracer {
            entries: Vec::new(),
            max_entries,
            enabled: false,
        }
    }

    pub fn trace(
        &mut self,
        event_type: &str,
        span: Option<&SourceSpan>,
        data: HashMap<String, String>,
    ) {
        if !self.enabled {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(TraceEntry::new(event_type, span, data));
    }

    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    pub fn to_list(&self) -> Vec<HashMap<String, serde_json::Value>> {
        self.entries.iter().map(|e| e.to_dict()).collect()
    }

    pub fn format_trace(&self, last_n: usize) -> String {
        if self.entries.is_empty() {
            return "(no trace entries)".to_string();
        }
        let start = if self.entries.len() > last_n {
            self.entries.len() - last_n
        } else {
            0
        };
        let mut lines = vec![format!(
            "Execution Trace (last {} entries):",
            self.entries.len() - start
        )];
        for entry in &self.entries[start..] {
            let func = entry
                .data
                .get("function")
                .or_else(|| entry.data.get("agent"))
                .cloned()
                .unwrap_or_default();
            match entry.event_type.as_str() {
                "call" => lines.push(format!("  → {} call {}", entry.location, func)),
                "return" => {
                    let ret = entry.data.get("return_value").cloned().unwrap_or_default();
                    lines.push(format!("  ← {} return {} → {}", entry.location, func, ret));
                }
                _ => lines.push(format!(
                    "  [{}] {} {}",
                    entry.event_type, entry.location, func
                )),
            }
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

/// Structured error snapshot for AI debugging.
#[derive(Debug, Clone)]
pub struct ErrorSnapshot {
    pub error_type: String,
    pub message: String,
    pub location: String,
    pub call_stack: Vec<HashMap<String, serde_json::Value>>,
    pub scope: HashMap<String, String>,
    pub trace: Vec<HashMap<String, serde_json::Value>>,
    pub timestamp: f64,
    pub diagnostic_category: String,
    pub suggestion: String,
    pub data_flow: Vec<HashMap<String, serde_json::Value>>,
}

impl ErrorSnapshot {
    pub fn new(
        error_type: &str,
        message: &str,
        span: Option<&SourceSpan>,
        call_stack: Vec<HashMap<String, serde_json::Value>>,
        scope: HashMap<String, String>,
        trace: Vec<HashMap<String, serde_json::Value>>,
    ) -> Self {
        ErrorSnapshot {
            error_type: error_type.to_string(),
            message: message.to_string(),
            location: CallFrame::format_location(span),
            call_stack,
            scope,
            trace,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            diagnostic_category: String::new(),
            suggestion: String::new(),
            data_flow: Vec::new(),
        }
    }

    pub fn format_text(&self, verbose: bool) -> String {
        let ts = chrono::DateTime::from_timestamp(self.timestamp as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let mut lines = vec![
            format!("Error: {}: {}", self.error_type, self.message),
            format!("Location: {}", self.location),
            format!("Time: {}", ts),
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
            for (i, frame) in self.call_stack.iter().rev().enumerate() {
                let prefix = if i < self.call_stack.len() - 1 {
                    "  "
                } else {
                    "-> "
                };
                let func = frame
                    .get("function")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let loc = frame
                    .get("location")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                lines.push(format!("{}{} in {}", prefix, loc, func));
            }
        }

        if !self.scope.is_empty() {
            lines.push(String::new());
            lines.push("Variables in scope:".to_string());
            for (name, value) in &self.scope {
                lines.push(format!("  {} = {}", name, value));
            }
        }

        if verbose && !self.trace.is_empty() {
            lines.push(String::new());
            let trace_count = std::cmp::min(self.trace.len(), 20);
            lines.push(format!("Execution Trace (last {} entries):", trace_count));
            for entry in self.trace.iter().rev().take(20).rev() {
                let trace_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                let data = entry.get("data").and_then(|v| v.as_object());
                let func = data
                    .and_then(|d| d.get("function").or_else(|| d.get("agent")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let loc = entry.get("location").and_then(|v| v.as_str()).unwrap_or("");
                match trace_type {
                    "call" => lines.push(format!("  → {} call {}", loc, func)),
                    "return" => {
                        let ret = data
                            .and_then(|d| d.get("return_value"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        lines.push(format!("  ← {} return {} → {}", loc, func, ret));
                    }
                    _ => lines.push(format!("  [{}] {} {}", trace_type, loc, func)),
                }
            }
        }

        lines.join("\n")
    }

    pub fn to_dict(&self) -> HashMap<String, serde_json::Value> {
        let mut d = HashMap::new();
        let mut error = HashMap::new();
        error.insert(
            "type".to_string(),
            serde_json::Value::String(self.error_type.clone()),
        );
        error.insert(
            "message".to_string(),
            serde_json::Value::String(self.message.clone()),
        );
        error.insert(
            "location".to_string(),
            serde_json::Value::String(self.location.clone()),
        );
        d.insert(
            "error".to_string(),
            serde_json::Value::Object(error.into_iter().collect()),
        );

        let stack_arr: Vec<serde_json::Value> = self
            .call_stack
            .iter()
            .map(|f| serde_json::Value::Object(f.clone().into_iter().collect()))
            .collect();
        d.insert(
            "call_stack".to_string(),
            serde_json::Value::Array(stack_arr),
        );

        let scope_map: serde_json::Map<String, serde_json::Value> = self
            .scope
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        d.insert("scope".to_string(), serde_json::Value::Object(scope_map));

        let trace_arr: Vec<serde_json::Value> = self
            .trace
            .iter()
            .take(20)
            .map(|e| serde_json::Value::Object(e.clone().into_iter().collect()))
            .collect();
        d.insert("trace".to_string(), serde_json::Value::Array(trace_arr));

        d.insert(
            "timestamp".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.timestamp as u64)),
        );
        d.insert(
            "diagnostic_category".to_string(),
            serde_json::Value::String(self.diagnostic_category.clone()),
        );
        d.insert(
            "suggestion".to_string(),
            serde_json::Value::String(self.suggestion.clone()),
        );

        d
    }
}

// ---------------------------------------------------------------------------
// LLM Audit Log
// ---------------------------------------------------------------------------

/// Audit log entry for LLM calls.
#[derive(Debug, Clone)]
pub struct LLMAuditEntry {
    pub timestamp: f64,
    pub call_type: String,
    pub agent_name: Option<String>,
    pub model: Option<String>,
    pub prompt: String,
    pub response: Option<String>,
    pub tokens_in: usize,
    pub tokens_out: usize,
    pub duration_ms: f64,
    pub tool_calls: Vec<HashMap<String, serde_json::Value>>,
    pub error: Option<String>,
}

impl LLMAuditEntry {
    pub fn to_dict(&self) -> HashMap<String, serde_json::Value> {
        let mut d = HashMap::new();
        d.insert(
            "time".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.timestamp as u64)),
        );
        d.insert(
            "type".to_string(),
            serde_json::Value::String(self.call_type.clone()),
        );
        d.insert(
            "agent".to_string(),
            self.agent_name
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone()))
                .unwrap_or(serde_json::Value::Null),
        );
        d.insert(
            "model".to_string(),
            self.model
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone()))
                .unwrap_or(serde_json::Value::Null),
        );
        d.insert(
            "prompt".to_string(),
            serde_json::Value::String(truncate_str(&self.prompt, 500)),
        );
        d.insert(
            "tokens_in".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.tokens_in)),
        );
        d.insert(
            "tokens_out".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.tokens_out)),
        );
        d.insert(
            "duration_ms".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.duration_ms as u64)),
        );
        if let Some(ref resp) = self.response {
            d.insert(
                "response".to_string(),
                serde_json::Value::String(truncate_str(resp, 500)),
            );
        }
        if !self.tool_calls.is_empty() {
            let arr: Vec<serde_json::Value> = self
                .tool_calls
                .iter()
                .map(|tc| serde_json::Value::Object(tc.clone().into_iter().collect()))
                .collect();
            d.insert("tool_calls".to_string(), serde_json::Value::Array(arr));
        }
        if let Some(ref err) = self.error {
            d.insert("error".to_string(), serde_json::Value::String(err.clone()));
        }
        d
    }
}

/// Audit log for LLM calls.
pub struct LLMAuditLog {
    entries: Vec<LLMAuditEntry>,
    max_entries: usize,
    pub enabled: bool,
}

impl LLMAuditLog {
    pub fn new(max_entries: usize) -> Self {
        LLMAuditLog {
            entries: Vec::new(),
            max_entries,
            enabled: true,
        }
    }

    pub fn log(&mut self, entry: LLMAuditEntry) {
        if !self.enabled {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[LLMAuditEntry] {
        &self.entries
    }

    pub fn to_list(&self) -> Vec<HashMap<String, serde_json::Value>> {
        self.entries.iter().map(|e| e.to_dict()).collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Observability Manager
// ---------------------------------------------------------------------------

/// Central manager for all observability features.
pub struct ObservabilityManager {
    pub call_stack: CallStackTracker,
    pub tracer: ExecutionTracer,
    pub llm_audit: LLMAuditLog,
    pub last_error: Option<ErrorSnapshot>,
    /// Test coverage tracker (v1.44 — port of helen/runtime/coverage.py).
    pub coverage: helen_runtime::coverage::CoverageTracker,
}

impl ObservabilityManager {
    pub fn new() -> Self {
        ObservabilityManager {
            call_stack: CallStackTracker::new(100),
            tracer: ExecutionTracer::new(10000),
            llm_audit: LLMAuditLog::new(1000),
            last_error: None,
            coverage: helen_runtime::coverage::CoverageTracker::default(),
        }
    }

    pub fn capture_error(
        &mut self,
        error_type: &str,
        message: &str,
        span: Option<&SourceSpan>,
        scope: HashMap<String, String>,
    ) -> ErrorSnapshot {
        let call_stack = self.call_stack.to_list();
        let trace = self.tracer.to_list();
        let snapshot = ErrorSnapshot::new(error_type, message, span, call_stack, scope, trace);
        self.last_error = Some(snapshot.clone());
        snapshot
    }

    pub fn clear(&mut self) {
        self.call_stack.clear();
        self.tracer.clear();
        self.llm_audit.clear();
        self.last_error = None;
    }
}

impl Default for ObservabilityManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
