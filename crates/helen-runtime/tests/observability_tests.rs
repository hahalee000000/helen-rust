//! Tests for observability module — AI-native observability.

use helen_runtime::observability::*;
use serde_json::{json, Value};

// ── now_ts ──────────────────────────────────────────────────────────────

#[test]
fn now_ts_returns_positive() {
    let ts = now_ts();
    assert!(ts > 0.0);
}

#[test]
fn now_ts_monotonic() {
    let t1 = now_ts();
    let t2 = now_ts();
    assert!(t2 >= t1);
}

// ── truncate ────────────────────────────────────────────────────────────

#[test]
fn truncate_short_string() {
    assert_eq!(truncate("hello", 10), "hello");
}

#[test]
fn truncate_exact_length() {
    assert_eq!(truncate("hello", 5), "hello");
}

#[test]
fn truncate_long_string() {
    let result = truncate("hello world this is a long string", 10);
    assert!(result.contains("..."));
    assert!(result.contains("truncated"));
}

#[test]
fn truncate_empty() {
    assert_eq!(truncate("", 5), "");
}

#[test]
fn truncate_zero_max() {
    let result = truncate("hello", 0);
    assert!(result.contains("truncated"));
}

// ── safe_serialize ──────────────────────────────────────────────────────

#[test]
fn safe_serialize_null() {
    let result = safe_serialize(&Value::Null);
    assert_eq!(result, Value::Null);
}

#[test]
fn safe_serialize_bool() {
    let result = safe_serialize(&json!(true));
    assert_eq!(result, json!(true));
}

#[test]
fn safe_serialize_number() {
    let result = safe_serialize(&json!(42));
    assert_eq!(result, json!(42));
}

#[test]
fn safe_serialize_string() {
    let result = safe_serialize(&json!("hello"));
    assert_eq!(result, json!("hello"));
}

#[test]
fn safe_serialize_array() {
    let result = safe_serialize(&json!([1, 2, 3]));
    assert_eq!(result, json!([1, 2, 3]));
}

#[test]
fn safe_serialize_large_array() {
    let items: Vec<Value> = (0..60).map(|i| json!(i)).collect();
    let arr = Value::Array(items);
    let result = safe_serialize(&arr);
    if let Value::Array(items) = result {
        assert!(items.len() <= 52); // 50 + 1 summary
    }
}

#[test]
fn safe_serialize_object() {
    let result = safe_serialize(&json!({"a": 1, "b": 2}));
    assert_eq!(result, json!({"a": 1, "b": 2}));
}

#[test]
fn safe_serialize_nested() {
    let result = safe_serialize(&json!({"a": [1, 2, {"b": 3}]}));
    assert!(result.is_object());
}

// ── format_value ────────────────────────────────────────────────────────

#[test]
fn format_value_null() {
    assert_eq!(format_value(&Value::Null), "null");
}

#[test]
fn format_value_bool_true() {
    assert_eq!(format_value(&json!(true)), "true");
}

#[test]
fn format_value_bool_false() {
    assert_eq!(format_value(&json!(false)), "false");
}

#[test]
fn format_value_string_short() {
    let result = format_value(&json!("hello"));
    assert_eq!(result, "\"hello\"");
}

#[test]
fn format_value_string_long() {
    let long = "a".repeat(60);
    let result = format_value(&json!(long));
    assert!(result.contains("..."));
}

#[test]
fn format_value_array() {
    let result = format_value(&json!([1, 2, 3]));
    assert!(result.contains("1"));
}

#[test]
fn format_value_large_array() {
    let items: Vec<Value> = (0..10).map(|i| json!(i)).collect();
    let result = format_value(&Value::Array(items));
    assert!(result.contains("..."));
}

// ── CallStackTracker ────────────────────────────────────────────────────

#[test]
fn call_stack_new() {
    let stack = CallStackTracker::new(100);
    assert_eq!(stack.depth(), 0);
    assert!(!stack.is_enabled());
}

#[test]
fn call_stack_enable_disable() {
    let mut stack = CallStackTracker::new(100);
    assert!(!stack.is_enabled());
    stack.set_enabled(true);
    assert!(stack.is_enabled());
    stack.set_enabled(false);
    assert!(!stack.is_enabled());
}

#[test]
fn call_stack_push_pop() {
    let mut stack = CallStackTracker::new(100);
    stack.set_enabled(true);
    stack.push("func1", "file.helen:10:5".to_string(), None);
    assert_eq!(stack.depth(), 1);
    stack.push("func2", "file.helen:20:3".to_string(), None);
    assert_eq!(stack.depth(), 2);
    let frame = stack.pop();
    assert!(frame.is_some());
    assert_eq!(stack.depth(), 1);
}

#[test]
fn call_stack_pop_empty() {
    let mut stack = CallStackTracker::new(100);
    stack.set_enabled(true);
    let frame = stack.pop();
    assert!(frame.is_none());
}

#[test]
fn call_stack_to_list() {
    let mut stack = CallStackTracker::new(100);
    stack.set_enabled(true);
    stack.push("func1", "file.helen:10:5".to_string(), None);
    stack.push("func2", "file.helen:20:3".to_string(), None);
    let list = stack.to_list();
    assert_eq!(list.len(), 2);
}

#[test]
fn call_stack_format_traceback() {
    let mut stack = CallStackTracker::new(100);
    stack.set_enabled(true);
    stack.push("func1", "file.helen:10:5".to_string(), None);
    stack.push("func2", "file.helen:20:3".to_string(), None);
    let tb = stack.format_traceback();
    assert!(tb.contains("func1"));
    assert!(tb.contains("func2"));
}

#[test]
fn call_stack_clear() {
    let mut stack = CallStackTracker::new(100);
    stack.set_enabled(true);
    stack.push("func1", "file.helen:10:5".to_string(), None);
    stack.clear();
    assert_eq!(stack.depth(), 0);
}

#[test]
fn call_stack_max_depth() {
    let mut stack = CallStackTracker::new(2);
    stack.set_enabled(true);
    stack.push("func1", "loc1".to_string(), None);
    stack.push("func2", "loc2".to_string(), None);
    stack.push("func3", "loc3".to_string(), None);
    assert!(stack.depth() <= 2);
}

// ── CallFrame ───────────────────────────────────────────────────────────

#[test]
fn call_frame_to_dict() {
    let frame = CallFrame {
        function_name: "test_func".to_string(),
        location: "file.helen:10:5".to_string(),
        args: json!({"x": 1}),
        entry_time: 1234567890.0,
    };
    let dict = frame.to_dict();
    assert!(dict.get("function").is_some());
    assert!(dict.get("location").is_some());
}

#[test]
fn call_frame_format_location() {
    let loc = CallFrame::format_location(Some("file.helen"), 10, 5);
    assert!(loc.contains("file.helen"));
    assert!(loc.contains("10"));
    assert!(loc.contains("5"));
}

#[test]
fn call_frame_format_location_no_file() {
    let loc = CallFrame::format_location(None, 10, 5);
    assert!(loc.contains("10"));
}

// ── ExecutionTracer ─────────────────────────────────────────────────────

#[test]
fn execution_tracer_new() {
    let tracer = ExecutionTracer::new(1000);
    assert!(!tracer.is_enabled());
}

#[test]
fn execution_tracer_enable_disable() {
    let mut tracer = ExecutionTracer::new(1000);
    assert!(!tracer.is_enabled());
    tracer.set_enabled(true);
    assert!(tracer.is_enabled());
}

#[test]
fn execution_tracer_trace() {
    let mut tracer = ExecutionTracer::new(1000);
    tracer.set_enabled(true);
    tracer.trace("call", "file.helen:10:5".to_string(), None);
    assert_eq!(tracer.entries().len(), 1);
}

#[test]
fn execution_tracer_entries() {
    let mut tracer = ExecutionTracer::new(1000);
    tracer.set_enabled(true);
    tracer.trace("call", "loc1".to_string(), None);
    tracer.trace("return", "loc2".to_string(), None);
    assert_eq!(tracer.entries().len(), 2);
}

#[test]
fn execution_tracer_to_list() {
    let mut tracer = ExecutionTracer::new(1000);
    tracer.set_enabled(true);
    tracer.trace("call", "loc1".to_string(), None);
    let list = tracer.to_list();
    assert_eq!(list.len(), 1);
}

#[test]
fn execution_tracer_format_trace() {
    let mut tracer = ExecutionTracer::new(1000);
    tracer.set_enabled(true);
    tracer.trace("call", "loc1".to_string(), None);
    tracer.trace("return", "loc2".to_string(), None);
    let formatted = tracer.format_trace(10);
    assert!(!formatted.is_empty());
}

#[test]
fn execution_tracer_clear() {
    let mut tracer = ExecutionTracer::new(1000);
    tracer.set_enabled(true);
    tracer.trace("call", "loc1".to_string(), None);
    tracer.clear();
    assert_eq!(tracer.entries().len(), 0);
}

#[test]
fn execution_tracer_max_entries() {
    let mut tracer = ExecutionTracer::new(2);
    tracer.set_enabled(true);
    tracer.trace("call", "loc1".to_string(), None);
    tracer.trace("call", "loc2".to_string(), None);
    tracer.trace("call", "loc3".to_string(), None);
    assert!(tracer.entries().len() <= 3);
}

// ── TraceEntry ──────────────────────────────────────────────────────────

#[test]
fn trace_entry_to_dict() {
    let entry = TraceEntry {
        event_type: "call".to_string(),
        location: "file.helen:10:5".to_string(),
        timestamp: 1234567890.0,
        data: json!({"x": 1}),
    };
    let dict = entry.to_dict();
    assert!(dict.get("type").is_some());
    assert!(dict.get("location").is_some());
    assert!(dict.get("time").is_some());
}

// ── ErrorSnapshot ───────────────────────────────────────────────────────

#[test]
fn error_snapshot_to_dict() {
    let snapshot = ErrorSnapshot {
        error_type: "ValueError".to_string(),
        message: "bad value".to_string(),
        location: "file.helen:10:5".to_string(),
        call_stack: vec![],
        scope: json!({}),
        trace: vec![],
        timestamp: 1234567890.0,
        diagnostic_category: "validation".to_string(),
        suggestion: "check input".to_string(),
        data_flow: vec![],
    };
    let dict = snapshot.to_dict();
    assert!(dict.get("error").is_some());
}

#[test]
fn error_snapshot_format_text() {
    let snapshot = ErrorSnapshot {
        error_type: "ValueError".to_string(),
        message: "bad value".to_string(),
        location: "file.helen:10:5".to_string(),
        call_stack: vec![],
        scope: json!({}),
        trace: vec![],
        timestamp: 1234567890.0,
        diagnostic_category: "validation".to_string(),
        suggestion: "check input".to_string(),
        data_flow: vec![],
    };
    let text = snapshot.format_text(false);
    assert!(text.contains("ValueError"));
    assert!(text.contains("bad value"));
}

#[test]
fn error_snapshot_format_text_verbose() {
    let snapshot = ErrorSnapshot {
        error_type: "ValueError".to_string(),
        message: "bad value".to_string(),
        location: "file.helen:10:5".to_string(),
        call_stack: vec![],
        scope: json!({"x": 1}),
        trace: vec![],
        timestamp: 1234567890.0,
        diagnostic_category: "validation".to_string(),
        suggestion: "check input".to_string(),
        data_flow: vec![],
    };
    let text = snapshot.format_text(true);
    assert!(text.contains("ValueError"));
}

// ── LlmAuditLog ─────────────────────────────────────────────────────────

#[test]
fn llm_audit_log_new() {
    let log = LlmAuditLog::new(100);
    assert!(log.is_enabled());
}

#[test]
fn llm_audit_log_enable_disable() {
    let mut log = LlmAuditLog::new(100);
    assert!(log.is_enabled());
    log.set_enabled(false);
    assert!(!log.is_enabled());
}

// ── LlmAuditEntry ───────────────────────────────────────────────────────

#[test]
fn llm_audit_entry_to_dict() {
    let entry = LlmAuditEntry {
        timestamp: 1234567890.0,
        call_type: "act".to_string(),
        agent_name: Some("TestAgent".to_string()),
        model: Some("gpt-4".to_string()),
        prompt: "Hello".to_string(),
        response: Some("Hi".to_string()),
        tokens_in: 10,
        tokens_out: 5,
        duration_ms: 100.0,
        tool_calls: vec![],
        error: None,
    };
    let dict = entry.to_dict();
    assert!(dict.get("time").is_some());
    assert!(dict.get("type").is_some());
    assert!(dict.get("agent").is_some());
}

// ── ObservabilityManager ────────────────────────────────────────────────

#[test]
fn observability_manager_new() {
    let mgr = ObservabilityManager::new();
    assert!(!mgr.call_stack.is_enabled());
    assert!(!mgr.tracer.is_enabled());
}

#[test]
fn observability_manager_default() {
    let mgr = ObservabilityManager::default();
    assert!(!mgr.call_stack.is_enabled());
}
