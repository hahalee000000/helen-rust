//! Shared argument-parsing helpers for stdlib modules.
//!
//! These helpers extract and validate arguments from the `&[Value]` slice
//! passed to every stdlib builtin. They are `pub(crate)` so that domain
//! modules (stdlib_string, stdlib_list, etc.) can reuse them.

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::value::Value;

pub(crate) fn arg_str(args: &[Value], i: usize) -> Result<&str, ExceptionValue> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected string, got {}", v.type_name()),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "missing argument".to_string(),
            None,
        )),
    }
}

pub(crate) fn arg_int(args: &[Value], i: usize) -> Result<BigInt, ExceptionValue> {
    match args.get(i) {
        Some(Value::Int(n)) => Ok(n.clone()),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected int, got {}", v.type_name()),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "missing argument".to_string(),
            None,
        )),
    }
}

/// Python slice semantics on byte indices (Helen is byte-based; D4).
pub(crate) fn py_slice(s: &str, start: Option<i64>, end: Option<i64>) -> &str {
    let len = s.len() as i64;
    let start = start.unwrap_or(0);
    let end = end.unwrap_or(len);
    let start = if start < 0 {
        (len + start).max(0)
    } else {
        start.min(len)
    };
    let end = if end < 0 {
        (len + end).max(0)
    } else {
        end.min(len)
    };
    let start = start.max(0);
    let end = end.max(start);
    &s[start as usize..end as usize]
}

pub(crate) fn arg_f64(args: &[Value], i: usize) -> Result<f64, ExceptionValue> {
    match args.get(i) {
        Some(Value::Int(n)) => n.to_f64().ok_or_else(|| {
            ExceptionValue::new("RuntimeError", "number out of range".to_string(), None)
        }),
        Some(Value::Float(f)) => Ok(*f),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected number, got {}", v.type_name()),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "missing argument".to_string(),
            None,
        )),
    }
}

pub(crate) fn arg_bool(args: &[Value], i: usize) -> Result<bool, ExceptionValue> {
    match args.get(i) {
        Some(Value::Bool(b)) => Ok(*b),
        Some(v) => Ok(v.truthy()),
        None => Ok(false),
    }
}

/// Extract a list argument (Python: requires a list).
pub(crate) fn arg_list(args: &[Value], i: usize) -> Result<Vec<Value>, ExceptionValue> {
    match args.get(i) {
        Some(Value::List(l)) => Ok(l.borrow().clone()),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected list, got {}", v.type_name()),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "missing argument".to_string(),
            None,
        )),
    }
}

/// Extract a map argument (Python: requires a dict).
pub(crate) fn arg_map(args: &[Value], i: usize) -> Result<indexmap::IndexMap<Value, Value>, ExceptionValue> {
    match args.get(i) {
        Some(Value::Map(m)) => Ok(m.borrow().clone()),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected dict, got {}", v.type_name()),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "missing argument".to_string(),
            None,
        )),
    }
}

/// Optional string argument (Python: default `None`).
pub(crate) fn arg_opt_str(args: &[Value], i: usize) -> Result<Option<&str>, ExceptionValue> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(Some(s)),
        Some(Value::Null) | None => Ok(None),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected string, got {}", v.type_name()),
            None,
        )),
    }
}

/// Optional int argument (Python: default `None`).
pub(crate) fn arg_opt_int(args: &[Value], i: usize) -> Result<Option<BigInt>, ExceptionValue> {
    match args.get(i) {
        Some(Value::Int(n)) => Ok(Some(n.clone())),
        Some(Value::Null) | None => Ok(None),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected int, got {}", v.type_name()),
            None,
        )),
    }
}

pub(crate) fn err_expected<T>(what: &str, got: &Value) -> Result<T, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        format!("expected {what}, got {}", got.type_name()),
        None,
    ))
}
