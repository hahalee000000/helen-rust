//! Tests for llm_control module — LLM runtime parameter overrides.

use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::llm_control::*;
use helen_interpreter::value::Value;
use num_bigint::BigInt;

fn make_interp() -> Interpreter {
    Interpreter::new()
}

// ── Temperature ─────────────────────────────────────────────────────────

#[test]
fn test_set_get_temperature_float() {
    let mut interp = make_interp();
    let result = llm_set_temperature(&mut interp, &[Value::Float(0.7)]);
    assert!(result.is_ok());
    
    let temp = llm_get_temperature(&mut interp, &[]).unwrap();
    match temp {
        Value::Float(f) => assert!((f - 0.7).abs() < 1e-10),
        _ => panic!("Expected Float, got {:?}", temp),
    }
}

#[test]
fn test_set_get_temperature_int() {
    let mut interp = make_interp();
    let result = llm_set_temperature(&mut interp, &[Value::Int(BigInt::from(1))]);
    assert!(result.is_ok());
    
    let temp = llm_get_temperature(&mut interp, &[]).unwrap();
    match temp {
        Value::Float(f) => assert!((f - 1.0).abs() < 1e-10),
        _ => panic!("Expected Float, got {:?}", temp),
    }
}

#[test]
fn test_get_temperature_default() {
    let mut interp = make_interp();
    let temp = llm_get_temperature(&mut interp, &[]).unwrap();
    match temp {
        Value::Float(f) => assert!((f - 1.0).abs() < 1e-10),
        _ => panic!("Expected default Float(1.0), got {:?}", temp),
    }
}

#[test]
fn test_set_temperature_type_error() {
    let mut interp = make_interp();
    let result = llm_set_temperature(&mut interp, &[Value::Str("hot".into())]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_set_temperature_missing_arg() {
    let mut interp = make_interp();
    let result = llm_set_temperature(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

// ── Max Turns ───────────────────────────────────────────────────────────

#[test]
fn test_set_get_max_turns_int() {
    let mut interp = make_interp();
    let result = llm_set_max_turns(&mut interp, &[Value::Int(BigInt::from(5))]);
    assert!(result.is_ok());
    
    let turns = llm_get_max_turns(&mut interp, &[]).unwrap();
    assert_eq!(turns, Value::Int(BigInt::from(5)));
}

#[test]
fn test_set_get_max_turns_float() {
    let mut interp = make_interp();
    let result = llm_set_max_turns(&mut interp, &[Value::Float(3.7)]);
    assert!(result.is_ok());
    
    let turns = llm_get_max_turns(&mut interp, &[]).unwrap();
    assert_eq!(turns, Value::Int(BigInt::from(3)));
}

#[test]
fn test_get_max_turns_default() {
    let mut interp = make_interp();
    let turns = llm_get_max_turns(&mut interp, &[]).unwrap();
    assert_eq!(turns, Value::Int(BigInt::from(1)));
}

#[test]
fn test_set_max_turns_type_error() {
    let mut interp = make_interp();
    let result = llm_set_max_turns(&mut interp, &[Value::Bool(true)]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

// ── Max Tokens ──────────────────────────────────────────────────────────

#[test]
fn test_set_get_max_tokens() {
    let mut interp = make_interp();
    let result = llm_set_max_tokens(&mut interp, &[Value::Int(BigInt::from(1000))]);
    assert!(result.is_ok());
    
    let tokens = llm_get_max_tokens(&mut interp, &[]).unwrap();
    assert_eq!(tokens, Value::Int(BigInt::from(1000)));
}

#[test]
fn test_get_max_tokens_default_null() {
    let mut interp = make_interp();
    let tokens = llm_get_max_tokens(&mut interp, &[]).unwrap();
    assert_eq!(tokens, Value::Null);
}

// ── Thinking Mode ───────────────────────────────────────────────────────

#[test]
fn test_set_get_thinking_mode() {
    let mut interp = make_interp();
    let result = llm_set_thinking_mode(&mut interp, &[Value::Bool(true)]);
    assert!(result.is_ok());
    
    let mode = llm_get_thinking_mode(&mut interp, &[]).unwrap();
    assert_eq!(mode, Value::Bool(true));
}

#[test]
fn test_get_thinking_mode_default_null() {
    let mut interp = make_interp();
    let mode = llm_get_thinking_mode(&mut interp, &[]).unwrap();
    assert_eq!(mode, Value::Null);
}

#[test]
fn test_set_thinking_mode_type_error() {
    let mut interp = make_interp();
    let result = llm_set_thinking_mode(&mut interp, &[Value::Int(BigInt::from(1))]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

// ── Reasoning Effort ────────────────────────────────────────────────────

#[test]
fn test_set_get_reasoning_effort() {
    let mut interp = make_interp();
    let result = llm_set_reasoning_effort(&mut interp, &[Value::Str("high".into())]);
    assert!(result.is_ok());
    
    let effort = llm_get_reasoning_effort(&mut interp, &[]).unwrap();
    assert_eq!(effort, Value::Str("high".into()));
}

#[test]
fn test_get_reasoning_effort_default_null() {
    let mut interp = make_interp();
    let effort = llm_get_reasoning_effort(&mut interp, &[]).unwrap();
    assert_eq!(effort, Value::Null);
}

// ── Model/Description/Provider (getters only) ───────────────────────────

#[test]
fn test_get_model_default_null() {
    let mut interp = make_interp();
    let model = llm_get_model(&mut interp, &[]).unwrap();
    assert_eq!(model, Value::Null);
}

#[test]
fn test_get_description_default_null() {
    let mut interp = make_interp();
    let desc = llm_get_description(&mut interp, &[]).unwrap();
    assert_eq!(desc, Value::Null);
}

#[test]
fn test_get_provider_default_null() {
    let mut interp = make_interp();
    let provider = llm_get_provider(&mut interp, &[]).unwrap();
    assert_eq!(provider, Value::Null);
}

// ── Cancel/Control Stubs ────────────────────────────────────────────────

#[test]
fn test_cancel_llm_call_stub() {
    let mut interp = make_interp();
    let result = llm_cancel_llm_call(&mut interp, &[Value::Str("call-123".into())]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Bool(false));
}

#[test]
fn test_cancel_llm_call_type_error() {
    let mut interp = make_interp();
    let result = llm_cancel_llm_call(&mut interp, &[Value::Int(BigInt::from(123))]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_current_llm_call_id_stub() {
    let mut interp = make_interp();
    let result = llm_current_llm_call_id(&mut interp, &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
fn test_cancel_all_llm_calls_stub() {
    let mut interp = make_interp();
    let result = llm_cancel_all_llm_calls(&mut interp, &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Int(BigInt::from(0)));
}
