//! LLM runtime control stdlib functions.
//!
//! Byte-faithful port of `helen/stdlib/llm_control.py` (v1.44.0): provides
//! runtime overrides for LLM parameters (temperature, max_tokens, etc.).

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

// Thread-local LLM runtime overrides.
thread_local! {
    static LLM_OVERRIDES: RefCell<LlmOverrides> = RefCell::new(LlmOverrides::new());
}

/// LLM parameter overrides.
#[derive(Debug, Default)]
pub struct LlmOverrides {
    pub temperature: Option<f64>,
    pub max_turns: Option<i64>,
    pub max_tokens: Option<i64>,
    pub thinking_mode: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub model: Option<String>,
    pub description: Option<String>,
    pub provider: Option<String>,
}

impl LlmOverrides {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Stdlib function implementations
// ---------------------------------------------------------------------------

pub fn llm_set_temperature(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let t = arg_float(args, 0)?;
    LLM_OVERRIDES.with(|overrides| {
        overrides.borrow_mut().temperature = Some(t);
    });
    Ok(Value::Null)
}

pub fn llm_get_temperature(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let temp = LLM_OVERRIDES.with(|overrides| overrides.borrow().temperature.unwrap_or(1.0));
    Ok(Value::Float(temp))
}

pub fn llm_set_max_turns(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let n = arg_int(args, 0)?;
    LLM_OVERRIDES.with(|overrides| {
        overrides.borrow_mut().max_turns = Some(n);
    });
    Ok(Value::Null)
}

pub fn llm_get_max_turns(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let turns = LLM_OVERRIDES.with(|overrides| overrides.borrow().max_turns.unwrap_or(1));
    Ok(Value::Int(BigInt::from(turns)))
}

pub fn llm_set_max_tokens(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let n = arg_int(args, 0)?;
    LLM_OVERRIDES.with(|overrides| {
        overrides.borrow_mut().max_tokens = Some(n);
    });
    Ok(Value::Null)
}

pub fn llm_get_max_tokens(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let tokens = LLM_OVERRIDES.with(|overrides| overrides.borrow().max_tokens);
    match tokens {
        Some(n) => Ok(Value::Int(BigInt::from(n))),
        None => Ok(Value::Null),
    }
}

pub fn llm_set_thinking_mode(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let enabled = arg_bool(args, 0)?;
    LLM_OVERRIDES.with(|overrides| {
        overrides.borrow_mut().thinking_mode = Some(enabled);
    });
    Ok(Value::Null)
}

pub fn llm_get_thinking_mode(
    _i: &mut Interpreter,
    _args: &[Value],
) -> Result<Value, ExceptionValue> {
    let mode = LLM_OVERRIDES.with(|overrides| overrides.borrow().thinking_mode);
    match mode {
        Some(b) => Ok(Value::Bool(b)),
        None => Ok(Value::Null),
    }
}

pub fn llm_set_reasoning_effort(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let effort = arg_str(args, 0)?;
    LLM_OVERRIDES.with(|overrides| {
        overrides.borrow_mut().reasoning_effort = Some(effort.to_string());
    });
    Ok(Value::Null)
}

pub fn llm_get_reasoning_effort(
    _i: &mut Interpreter,
    _args: &[Value],
) -> Result<Value, ExceptionValue> {
    let effort = LLM_OVERRIDES.with(|overrides| overrides.borrow().reasoning_effort.clone());
    match effort {
        Some(s) => Ok(Value::Str(Rc::from(s.as_str()))),
        None => Ok(Value::Null),
    }
}

pub fn llm_get_model(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let model = LLM_OVERRIDES.with(|overrides| overrides.borrow().model.clone());
    match model {
        Some(s) => Ok(Value::Str(Rc::from(s.as_str()))),
        None => Ok(Value::Null),
    }
}

pub fn llm_get_description(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let desc = LLM_OVERRIDES.with(|overrides| overrides.borrow().description.clone());
    match desc {
        Some(s) => Ok(Value::Str(Rc::from(s.as_str()))),
        None => Ok(Value::Null),
    }
}

pub fn llm_get_provider(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let provider = LLM_OVERRIDES.with(|overrides| overrides.borrow().provider.clone());
    match provider {
        Some(s) => Ok(Value::Str(Rc::from(s.as_str()))),
        None => Ok(Value::Null),
    }
}

pub fn llm_cancel_llm_call(i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let call_id = arg_str(args, 0)?;
    // Cancel a specific in-flight LLM streaming call
    let cancelled = i.call_tracker.cancel_call(&call_id);
    Ok(Value::Bool(cancelled))
}

pub fn llm_current_llm_call_id(
    i: &mut Interpreter,
    _args: &[Value],
) -> Result<Value, ExceptionValue> {
    // Return the ID of the current active streaming LLM call, or None
    match i.call_tracker.get_current_call_id() {
        Some(id) => Ok(Value::Str(std::rc::Rc::from(id))),
        None => Ok(Value::Null),
    }
}

pub fn llm_cancel_all_llm_calls(
    i: &mut Interpreter,
    _args: &[Value],
) -> Result<Value, ExceptionValue> {
    // Cancel all active streaming LLM calls
    let count = i.call_tracker.cancel_all();
    Ok(Value::Int(num_bigint::BigInt::from(count)))
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn arg_str(args: &[Value], i: usize) -> Result<String, ExceptionValue> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s.to_string()),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected string at position {}, got {:?}", i, other),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}

fn arg_float(args: &[Value], i: usize) -> Result<f64, ExceptionValue> {
    match args.get(i) {
        Some(Value::Float(f)) => Ok(*f),
        Some(Value::Int(n)) => Ok(n.to_f64().unwrap_or(0.0)),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected number at position {}, got {:?}", i, other),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}

fn arg_int(args: &[Value], i: usize) -> Result<i64, ExceptionValue> {
    match args.get(i) {
        Some(Value::Int(n)) => Ok(n.to_i64().unwrap_or(0)),
        Some(Value::Float(f)) => Ok(*f as i64),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected integer at position {}, got {:?}", i, other),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}

fn arg_bool(args: &[Value], i: usize) -> Result<bool, ExceptionValue> {
    match args.get(i) {
        Some(Value::Bool(b)) => Ok(*b),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected boolean at position {}, got {:?}", i, other),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}
