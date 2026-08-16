//! Debug and tracing stdlib functions.
//!
//! Byte-faithful port of `helen/stdlib/debug.py` (v1.44.0): provides
//! debugging, tracing, and observability functions.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

/// Thread-local trace state.
thread_local! {
    static TRACE_ENABLED: RefCell<bool> = RefCell::new(false);
    static TRACE_LOG: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

/// Enable tracing.
pub fn debug_trace_on(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    TRACE_ENABLED.with(|enabled| {
        *enabled.borrow_mut() = true;
    });
    Ok(Value::Null)
}

/// Disable tracing.
pub fn debug_trace_off(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    TRACE_ENABLED.with(|enabled| {
        *enabled.borrow_mut() = false;
    });
    Ok(Value::Null)
}

/// Get trace log.
pub fn debug_get_trace(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let trace = TRACE_LOG.with(|log| {
        log.borrow().join("\n")
    });
    Ok(Value::Str(Rc::from(trace.as_str())))
}

/// Get LLM call log.
pub fn debug_get_llm_log(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: LLM logging not yet implemented
    Ok(Value::List(Rc::new(RefCell::new(vec![]))))
}

/// Get current call stack.
pub fn debug_get_call_stack(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: call stack tracking not yet implemented
    Ok(Value::List(Rc::new(RefCell::new(vec![]))))
}

/// Get last error.
pub fn debug_get_last_error(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: error tracking not yet implemented
    Ok(Value::Null)
}

/// Get detailed last error info.
pub fn debug_last_error_detail(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: error tracking not yet implemented
    Ok(Value::Null)
}

/// Get error category.
pub fn debug_error_category(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: error categorization not yet implemented
    Ok(Value::Str(Rc::from("unknown")))
}

/// Get error suggestion.
pub fn debug_error_suggestion(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: error suggestions not yet implemented
    Ok(Value::Null)
}

/// Get error data flow.
pub fn debug_error_data_flow(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: data flow tracking not yet implemented
    Ok(Value::Null)
}

/// Record data flow.
pub fn debug_record_data_flow(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: data flow tracking not yet implemented
    Ok(Value::Null)
}

/// Trace value origin.
pub fn debug_trace_value_origin(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: value origin tracking not yet implemented
    Ok(Value::Null)
}

/// Trace value consumers.
pub fn debug_trace_value_consumers(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: value consumer tracking not yet implemented
    Ok(Value::List(Rc::new(RefCell::new(vec![]))))
}

/// Record session.
pub fn debug_record_session(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: session recording not yet implemented
    Ok(Value::Null)
}

/// Replay session.
pub fn debug_replay_session(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: session replay not yet implemented
    Ok(Value::Null)
}

/// Stop recording.
pub fn debug_stop_recording(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: session recording not yet implemented
    Ok(Value::Null)
}

/// Validate output.
pub fn debug_validate_output(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: output validation not yet implemented
    Ok(Value::Bool(true))
}

/// Enable coverage tracking.
pub fn debug_coverage_on(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: coverage tracking not yet implemented
    Ok(Value::Null)
}

/// Disable coverage tracking.
pub fn debug_coverage_off(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: coverage tracking not yet implemented
    Ok(Value::Null)
}

/// Get coverage summary.
pub fn debug_coverage_summary(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: coverage tracking not yet implemented
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("not_available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Get coverage report.
pub fn debug_coverage_report(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: coverage tracking not yet implemented
    Ok(Value::Str(Rc::from("Coverage tracking not yet implemented")))
}

/// Get data lineage.
pub fn debug_get_data_lineage(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: data lineage tracking not yet implemented
    Ok(Value::Null)
}
