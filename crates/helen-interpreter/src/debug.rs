//! Debug and tracing stdlib functions.
//!
//! Byte-faithful port of `helen/stdlib/debug.py` (v1.44.0): provides
//! debugging, tracing, and observability functions.
//!
//! Functions that require infrastructure not yet ported (data lineage,
//! recording/replay, coverage, output validation) return safe defaults
//! matching the Python fallback behavior.

use std::cell::RefCell;
use std::rc::Rc;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::stdlib::json_to_value;
use crate::value::Value;

// ---------------------------------------------------------------------------
// Thread-local trace state (used by trace_on/trace_off/get_trace)
// ---------------------------------------------------------------------------

thread_local! {
    static TRACE_ENABLED: RefCell<bool> = RefCell::new(false);
    static TRACE_LOG: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

/// Enable tracing.
pub fn debug_trace_on(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    TRACE_ENABLED.with(|e| *e.borrow_mut() = true);
    // Also enable the observability tracer
    _i.observability.tracer.enabled = true;
    Ok(Value::Null)
}

/// Disable tracing.
pub fn debug_trace_off(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    TRACE_ENABLED.with(|e| *e.borrow_mut() = false);
    _i.observability.tracer.enabled = false;
    Ok(Value::Null)
}

/// Get trace log (thread-local string log).
pub fn debug_get_trace(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let trace = TRACE_LOG.with(|log| log.borrow().join("\n"));
    Ok(Value::Str(Rc::from(trace.as_str())))
}

// ---------------------------------------------------------------------------
// LLM audit log — wired to observability.llm_audit
// ---------------------------------------------------------------------------

/// Get LLM call log.
/// Python: returns list of dicts from `interp.observability.llm_audit`.
pub fn debug_get_llm_log(i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let n = match args.first() {
        Some(Value::Int(n)) => {
            // Convert BigInt to usize safely
            let s = n.to_string();
            s.parse::<usize>().unwrap_or(10)
        }
        _ => 10,
    };
    let entries = i.observability.llm_audit.entries();
    let start = if entries.len() > n { entries.len() - n } else { 0 };
    let list: Vec<Value> = entries[start..]
        .iter()
        .map(|e| {
            let d = e.to_dict();
            let json = serde_json::Value::Object(d.into_iter().collect());
            json_to_value(&json)
        })
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(list))))
}

// ---------------------------------------------------------------------------
// Call stack — wired to observability.call_stack
// ---------------------------------------------------------------------------

/// Get current call stack.
/// Python: returns list of dicts from `interp.observability.call_stack`.
pub fn debug_get_call_stack(i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let list = i.observability.call_stack.to_list();
    let values: Vec<Value> = list
        .iter()
        .map(|frame| {
            let json = serde_json::Value::Object(frame.clone().into_iter().collect());
            json_to_value(&json)
        })
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(values))))
}

// ---------------------------------------------------------------------------
// Last error — wired to observability.last_error
// ---------------------------------------------------------------------------

/// Get last error (basic).
/// Python: returns `interp.observability.last_error.to_dict()` or None.
pub fn debug_get_last_error(i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    match &i.observability.last_error {
        None => Ok(Value::Null),
        Some(snapshot) => {
            let d = snapshot.to_dict();
            let json = serde_json::Value::Object(d.into_iter().collect());
            Ok(json_to_value(&json))
        }
    }
}

/// Get detailed last error info (verbose format).
/// Python: same as get_last_error but with full trace.
pub fn debug_last_error_detail(i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    match &i.observability.last_error {
        None => Ok(Value::Null),
        Some(snapshot) => {
            let d = snapshot.to_dict();
            let json = serde_json::Value::Object(d.into_iter().collect());
            Ok(json_to_value(&json))
        }
    }
}

// ---------------------------------------------------------------------------
// Error helpers — extract fields from error dict
// ---------------------------------------------------------------------------

/// Get error category from error dict.
/// Python: `error.get("diagnostic_category", "Unknown")`
pub fn debug_error_category(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    match args.first() {
        Some(Value::Map(m)) => {
            let map = m.borrow();
            match map.get(&Value::Str(Rc::from("diagnostic_category"))) {
                Some(Value::Str(s)) => Ok(Value::Str(s.clone())),
                _ => Ok(Value::Str(Rc::from("Unknown"))),
            }
        }
        _ => Ok(Value::Str(Rc::from("Unknown"))),
    }
}

/// Get error suggestion from error dict.
/// Python: `error.get("suggestion", "")`
pub fn debug_error_suggestion(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    match args.first() {
        Some(Value::Map(m)) => {
            let map = m.borrow();
            match map.get(&Value::Str(Rc::from("suggestion"))) {
                Some(Value::Str(s)) => Ok(Value::Str(s.clone())),
                _ => Ok(Value::Str(Rc::from(""))),
            }
        }
        _ => Ok(Value::Str(Rc::from(""))),
    }
}

/// Get error data flow from error dict.
/// Python: `error.get("data_flow", [])`
pub fn debug_error_data_flow(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    match args.first() {
        Some(Value::Map(m)) => {
            let map = m.borrow();
            match map.get(&Value::Str(Rc::from("data_flow"))) {
                Some(v) => Ok(v.clone()),
                _ => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
            }
        }
        _ => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
    }
}

// ---------------------------------------------------------------------------
// Data lineage — requires _data_lineage_tracker (not yet ported)
// Python returns [] / {} when tracker is None.
// ---------------------------------------------------------------------------

/// Record data flow (manual).
/// Python: delegates to `interp._data_lineage_tracker.record_flow(...)`.
pub fn debug_record_data_flow(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // No data lineage tracker in Rust yet — return Python's fallback.
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(
        Value::Str(Rc::from("message")),
        Value::Str(Rc::from("Data lineage tracker not initialized")),
    );
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Trace value origin.
/// Python: returns `tracker.get_origin(uuid)` or [] if no tracker.
pub fn debug_trace_value_origin(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::List(Rc::new(RefCell::new(vec![]))))
}

/// Trace value consumers.
/// Python: returns `tracker.get_consumers(uuid)` or [] if no tracker.
pub fn debug_trace_value_consumers(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::List(Rc::new(RefCell::new(vec![]))))
}

/// Get data lineage graph.
/// Python: returns `tracker.get_full_lineage()` or {"nodes":[], "edges":[]} if no tracker.
pub fn debug_get_data_lineage(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("nodes")), Value::List(Rc::new(RefCell::new(vec![]))));
    result.insert(Value::Str(Rc::from("edges")), Value::List(Rc::new(RefCell::new(vec![]))));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

// ---------------------------------------------------------------------------
// Session recording/replay — requires LLM runtime recording support
// Python returns error dict when llm_runtime doesn't support recording.
// ---------------------------------------------------------------------------

/// Record session (start LLM recording).
/// Python: `interp.llm_runtime.enable_recording(cassette_path)`.
pub fn debug_record_session(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let cassette_path = match args.first() {
        Some(Value::Str(s)) => s.to_string(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "record_session requires a cassette_path string".to_string(),
                None,
            ));
        }
    };
    // LLM runtime is Arc<dyn LlmRuntime> — recording requires mutable access
    // which isn't available through the Arc. Return Python's fallback.
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("cassette_path")), Value::Str(Rc::from(cassette_path.as_str())));
    result.insert(
        Value::Str(Rc::from("message")),
        Value::Str(Rc::from("LLM runtime does not support recording")),
    );
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Stop recording.
/// Python: `interp.llm_runtime.disable_recording()`.
pub fn debug_stop_recording(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(
        Value::Str(Rc::from("message")),
        Value::Str(Rc::from("LLM runtime does not support recording")),
    );
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Replay session from cassette.
/// Python: replaces llm_runtime with ReplayLLMRuntime.
pub fn debug_replay_session(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let cassette_path = match args.first() {
        Some(Value::Str(s)) => s.to_string(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "replay_session requires a cassette_path string".to_string(),
                None,
            ));
        }
    };
    // ReplayLLMRuntime not yet ported — return Python's error fallback.
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("cassette_path")), Value::Str(Rc::from(cassette_path.as_str())));
    result.insert(Value::Str(Rc::from("entry_count")), Value::Int(num_bigint::BigInt::from(0)));
    result.insert(
        Value::Str(Rc::from("message")),
        Value::Str(Rc::from("ReplayLLMRuntime not yet implemented in Rust port")),
    );
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

// ---------------------------------------------------------------------------
// Output validation — requires output_validator (not yet ported)
// Python: validate_output(output, contract) → {valid, violation, parsed}
// ---------------------------------------------------------------------------

/// Validate LLM output against a contract.
/// Python: delegates to `helen.runtime.output_validator.validate_output`.
pub fn debug_validate_output(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let output = match args.first() {
        Some(Value::Str(s)) => s.to_string(),
        _ => String::new(),
    };
    let contract = args.get(1);

    // Simple contract validation (Python parity for "json" and "text")
    let (valid, violation, parsed) = match contract {
        None => (true, String::new(), Value::Str(Rc::from(output.as_str()))),
        Some(Value::Str(s)) if s.as_ref() == "json" => {
            match serde_json::from_str::<serde_json::Value>(&output) {
                Ok(json) => (true, String::new(), json_to_value(&json)),
                Err(e) => (false, format!("Invalid JSON: {}", e), Value::Null),
            }
        }
        Some(Value::Str(s)) if s.as_ref() == "text" => {
            (true, String::new(), Value::Str(Rc::from(output.as_str())))
        }
        Some(Value::Map(schema)) => {
            // Schema contract validation (basic)
            let schema_map = schema.borrow();
            let required = schema_map.get(&Value::Str(Rc::from("required")));
            // Try to parse as JSON first
            match serde_json::from_str::<serde_json::Value>(&output) {
                Ok(json) => {
                    let parsed_val = json_to_value(&json);
                    // Check required fields if present
                    let mut missing_field = None;
                    if let Some(Value::List(req_list)) = required {
                        let req = req_list.borrow();
                        if let Value::Map(ref obj) = parsed_val {
                            let obj_map = obj.borrow();
                            for field in req.iter() {
                                if let Value::Str(field_name) = field {
                                    let key = Value::Str(field_name.clone());
                                    if !obj_map.contains_key(&key) {
                                        missing_field = Some(field_name.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if let Some(field_name) = missing_field {
                        (false, format!("Missing required field: {}", field_name), parsed_val)
                    } else {
                        (true, String::new(), parsed_val)
                    }
                }
                Err(e) => (false, format!("Invalid JSON: {}", e), Value::Null),
            }
        }
        _ => (true, String::new(), Value::Str(Rc::from(output.as_str()))),
    };

    Ok(make_validation_result(valid, violation, parsed))
}

fn make_validation_result(valid: bool, violation: String, parsed: Value) -> Value {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("valid")), Value::Bool(valid));
    result.insert(Value::Str(Rc::from("violation")), Value::Str(Rc::from(violation.as_str())));
    result.insert(Value::Str(Rc::from("parsed")), parsed);
    Value::Map(Rc::new(RefCell::new(result)))
}

// ---------------------------------------------------------------------------
// Coverage tracking — not yet ported (Python uses coverage.py integration)
// ---------------------------------------------------------------------------

/// Enable coverage tracking.
pub fn debug_coverage_on(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Coverage tracking requires coverage.py integration — not yet ported.
    Ok(Value::Null)
}

/// Disable coverage tracking.
pub fn debug_coverage_off(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

/// Get coverage summary.
pub fn debug_coverage_summary(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("not_available")));
    result.insert(
        Value::Str(Rc::from("message")),
        Value::Str(Rc::from("Coverage tracking requires coverage.py integration (not yet ported)")),
    );
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Get coverage report.
pub fn debug_coverage_report(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Str(Rc::from("Coverage tracking not yet implemented in Rust port")))
}
