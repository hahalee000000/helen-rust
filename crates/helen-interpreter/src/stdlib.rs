//! Stdlib module registry for the interpreter (Task 3.7).
//!
//! Ports the observable binding behavior of `import_mixin._import_stdlib_module`
//! (v1.34/v1.38): modules map to export tables; `import std.X.*` binds all
//! exports plus Chinese aliases, `import std.X.{a,b}` binds selectively, and
//! `import std.X as NS` binds a module object (a map of name -> BuiltinFn).
//!
//! M3 scope: the subset of stdlib functions the Tier-A corpus and authored
//! programs exercise. M4 ports the full 378-function stdlib.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::{BuiltinImpl, Interpreter};
use crate::value::Value;

/// A single export: canonical name + implementation.
pub struct StdlibExport {
    pub name: &'static str,
    pub func: BuiltinImpl,
}

/// Module export table for `std.core` (the interpreter's `builtins` map).
pub const CORE_EXPORTS: &[&str] = &[
    "print",
    "len",
    "str",
    "int",
    "float",
    "bool",
    "type",
    "isinstance",
    "range",
    "abs",
    "min",
    "max",
    "list",
    "dict",
];

/// Resolve a stdlib module name to its export table.
///
/// Returns `None` for unknown modules (Python: `Unknown stdlib module`).
pub fn module_exports(module: &str) -> Option<&'static [StdlibExport]> {
    match module {
        "std.str" => Some(STR_EXPORTS),
        "std.list" => Some(LIST_EXPORTS),
        "std.dict" => Some(DICT_EXPORTS),
        "std.math" => Some(MATH_EXPORTS),
        "std.debug" => Some(DEBUG_EXPORTS),
        _ => None,
    }
}

/// `'std.str'` module tag used by `BuiltinFn.module`.
pub fn module_tag(module: &str) -> &'static str {
    match module {
        "std.str" => "str",
        "std.list" => "list",
        "std.dict" => "dict",
        "std.math" => "math",
        "std.debug" => "debug",
        "std.core" => "core",
        _ => "core",
    }
}

// ---------------------------------------------------------------------------
// Argument helpers
// ---------------------------------------------------------------------------

fn arg_str(args: &[Value], i: usize) -> Result<&str, ExceptionValue> {
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

fn arg_int(args: &[Value], i: usize) -> Result<BigInt, ExceptionValue> {
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
fn py_slice(s: &str, start: Option<i64>, end: Option<i64>) -> &str {
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

// ---------------------------------------------------------------------------
// std.str
// ---------------------------------------------------------------------------

fn str_upper(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(s.to_uppercase().as_str())))
}

fn str_lower(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(s.to_lowercase().as_str())))
}

fn str_substring(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let start = arg_int(args, 1)?.to_i64().ok_or_else(|| {
        ExceptionValue::new("RuntimeError", "start index out of range".to_string(), None)
    })?;
    let end = match args.get(2) {
        Some(Value::Int(n)) => Some(n.to_i64().ok_or_else(|| {
            ExceptionValue::new("RuntimeError", "end index out of range".to_string(), None)
        })?),
        Some(Value::Null) | None => None,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected int for end, got {}", v.type_name()),
                None,
            ))
        }
    };
    let out = py_slice(s, Some(start), end);
    Ok(Value::Str(Rc::from(out)))
}

fn str_split(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let sep = match args.get(1) {
        Some(Value::Str(sep)) => Some(sep.as_ref()),
        Some(Value::Null) | None => None,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected string separator, got {}", v.type_name()),
                None,
            ))
        }
    };
    let items: Vec<Value> = match sep {
        None => s
            .split_whitespace()
            .map(|p| Value::Str(Rc::from(p)))
            .collect(),
        Some("") => s
            .chars()
            .map(|c| Value::Str(Rc::from(c.to_string().as_str())))
            .collect(),
        Some(sep) => s.split(sep).map(|p| Value::Str(Rc::from(p))).collect(),
    };
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn str_join(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sep = arg_str(args, 0)?;
    let items = match args.get(1) {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let parts: Vec<String> = items.iter().map(|v| v.python_str()).collect();
    Ok(Value::Str(Rc::from(parts.join(sep).as_str())))
}

fn str_trim(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(s.trim().to_string().as_str())))
}

fn str_contains(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let sub = arg_str(args, 1)?;
    Ok(Value::Bool(s.contains(sub)))
}

fn str_startswith(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let prefix = arg_str(args, 1)?;
    Ok(Value::Bool(s.starts_with(prefix)))
}

fn str_endswith(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let suffix = arg_str(args, 1)?;
    Ok(Value::Bool(s.ends_with(suffix)))
}

fn str_replace(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let old = arg_str(args, 1)?;
    let new = arg_str(args, 2)?;
    Ok(Value::Str(Rc::from(s.replace(old, new).as_str())))
}

fn str_strip(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(
        s.trim_matches(char::is_whitespace).to_string().as_str(),
    )))
}

fn str_repeat(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let n = arg_int(args, 1)?.to_i64().ok_or_else(|| {
        ExceptionValue::new("RuntimeError", "count out of range".to_string(), None)
    })?;
    if n <= 0 {
        return Ok(Value::Str(Rc::from("")));
    }
    Ok(Value::Str(Rc::from(s.repeat(n as usize).as_str())))
}

fn str_reverse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(
        s.chars().rev().collect::<String>().as_str(),
    )))
}

fn str_count(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let sub = arg_str(args, 1)?;
    if sub.is_empty() {
        return Ok(Value::Int(BigInt::from(s.chars().count() + 1)));
    }
    let n = s.matches(sub).count();
    Ok(Value::Int(BigInt::from(n)))
}

fn str_index(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let sub = arg_str(args, 1)?;
    match s.find(sub) {
        Some(pos) => Ok(Value::Int(BigInt::from(pos))),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            format!("substring not found: '{sub}'"),
            None,
        )),
    }
}

fn str_chr(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let code = arg_int(args, 0)?.to_u32().ok_or_else(|| {
        ExceptionValue::new(
            "RuntimeError",
            "chr() arg not in range(0x110000)".to_string(),
            None,
        )
    })?;
    match char::from_u32(code) {
        Some(c) => Ok(Value::Str(Rc::from(c.to_string().as_str()))),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: chr() arg not in range(0x110000)".to_string(),
            None,
        )),
    }
}

fn str_ord(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if chars.next().is_none() => Ok(Value::Int(BigInt::from(c as u32))),
        Some(_) => Err(ExceptionValue::new(
            "RuntimeError",
            "Python TypeError: ord() expected a character, but string of length 2 found"
                .to_string(),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "Python TypeError: ord() expected a character, but string of length 0 found"
                .to_string(),
            None,
        )),
    }
}

fn str_find(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let sub = arg_str(args, 1)?;
    Ok(Value::Int(BigInt::from(
        s.find(sub).map(|p| p as i64).unwrap_or(-1),
    )))
}

// ---------------------------------------------------------------------------
// std.list
// ---------------------------------------------------------------------------

fn list_sort(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let mut items = items;
    // Optional compare closure: Python `sorted(lst, key=cmp_to_key(compare))`.
    if let Some(cmp_fn) = args.get(1) {
        items.sort_by(|a, b| {
            let r = interp.call_value(cmp_fn.clone(), vec![a.clone(), b.clone()]);
            match r {
                Ok(Value::Int(n)) => n.to_i64().unwrap_or(0).cmp(&0),
                Ok(v) => v.truthy().cmp(&true),
                Err(_) => std::cmp::Ordering::Equal,
            }
        });
    } else {
        items.sort_by(|a, b| {
            crate::interpreter::cmp_values(a, b).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn list_map(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => out.push(v),
            Err(e) => {
                let item_repr = item.python_repr();
                let truncated = if item_repr.len() > 100 {
                    format!("{}...(truncated)", &item_repr[..100])
                } else {
                    item_repr
                };
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!(
                        "map operation failed at index {i}: {} (element: {truncated})",
                        e.message
                    ),
                    e.span,
                ));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_filter(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => {
                if v.truthy() {
                    out.push(item.clone());
                }
            }
            Err(e) => {
                let item_repr = item.python_repr();
                let truncated = if item_repr.len() > 100 {
                    format!("{}...(truncated)", &item_repr[..100])
                } else {
                    item_repr
                };
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!(
                        "filter operation failed at index {i}: {} (element: {truncated})",
                        e.message
                    ),
                    e.span,
                ));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_reduce(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    let initial = args.get(2).cloned().unwrap_or(Value::Null);
    let initial_is_null = matches!(initial, Value::Null);
    let mut acc = if initial_is_null {
        match items.first() {
            Some(first) => first.clone(),
            None => {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    "reduce() of empty sequence with no initial value".to_string(),
                    None,
                ))
            }
        }
    } else {
        initial
    };
    let start = if initial_is_null { 1 } else { 0 };
    for item in items.iter().skip(start) {
        match interp.call_value(f.clone(), vec![acc, item.clone()]) {
            Ok(v) => acc = v,
            Err(e) => {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!("reduce operation failed: {}", e.message),
                    e.span,
                ))
            }
        }
    }
    Ok(acc)
}

fn list_unique(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let mut out: Vec<Value> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

// ---------------------------------------------------------------------------
// std.dict
// ---------------------------------------------------------------------------

fn dict_keys(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = match args.first() {
        Some(Value::Map(m)) => m.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected dict, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let keys: Vec<Value> = m.keys().cloned().collect();
    Ok(Value::List(Rc::new(RefCell::new(keys))))
}

fn dict_values(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = match args.first() {
        Some(Value::Map(m)) => m.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected dict, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let vals: Vec<Value> = m.values().cloned().collect();
    Ok(Value::List(Rc::new(RefCell::new(vals))))
}

fn dict_entries(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = match args.first() {
        Some(Value::Map(m)) => m.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected dict, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let entries: Vec<Value> = m
        .iter()
        .map(|(k, v)| Value::List(Rc::new(RefCell::new(vec![k.clone(), v.clone()]))))
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(entries))))
}

fn dict_get(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = match args.first() {
        Some(Value::Map(m)) => m.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected dict, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let key = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing key".to_string(), None))?;
    let default = args.get(2).cloned().unwrap_or(Value::Null);
    Ok(m.get(&key).cloned().unwrap_or(default))
}

fn dict_has_key(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = match args.first() {
        Some(Value::Map(m)) => m.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected dict, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let key = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing key".to_string(), None))?;
    Ok(Value::Bool(m.contains_key(&key)))
}

fn dict_merge(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut out = indexmap::IndexMap::new();
    for arg in args {
        match arg {
            Value::Map(m) => {
                for (k, v) in m.borrow().iter() {
                    out.insert(k.clone(), v.clone());
                }
            }
            v => {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!("expected dict, got {}", v.type_name()),
                    None,
                ))
            }
        }
    }
    Ok(Value::Map(Rc::new(RefCell::new(out))))
}

fn dict_set_key(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = match args.first() {
        Some(Value::Map(m)) => m.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected dict, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let key = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing key".to_string(), None))?;
    let value = args
        .get(2)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing value".to_string(), None))?;
    let mut out = m;
    out.insert(key, value);
    Ok(Value::Map(Rc::new(RefCell::new(out))))
}

fn dict_remove_key(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = match args.first() {
        Some(Value::Map(m)) => m.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected dict, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let key = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing key".to_string(), None))?;
    let mut out = m;
    out.shift_remove(&key);
    Ok(Value::Map(Rc::new(RefCell::new(out))))
}

// ---------------------------------------------------------------------------
// std.math
// ---------------------------------------------------------------------------

fn math_pow(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let base = match args.first() {
        Some(Value::Int(n)) => n.to_f64().unwrap_or(0.0),
        Some(Value::Float(f)) => *f,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected number, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let exp = match args.get(1) {
        Some(Value::Int(n)) => n.to_f64().unwrap_or(0.0),
        Some(Value::Float(f)) => *f,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected number, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    Ok(Value::Float(base.powf(exp)))
}

/// Python `round(value, ndigits)` — banker's rounding (round-half-even),
/// int input stays int, float input stays float.
fn math_round(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument".to_string(), None))?;
    let ndigits = match args.get(1) {
        Some(Value::Int(n)) => Some(n.clone()),
        Some(Value::Null) | None => None,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected int for ndigits, got {}", v.type_name()),
                None,
            ))
        }
    };
    match value {
        Value::Int(n) => {
            // Python: int round with ndigits=0 returns int unchanged;
            // negative ndigits rounds to the nearest power of 10.
            let is_zero = ndigits
                .as_ref()
                .map(|d| d.sign() == num_bigint::Sign::NoSign)
                .unwrap_or(true);
            if is_zero {
                Ok(Value::Int(n))
            } else if let Some(d) = &ndigits {
                if d.sign() == num_bigint::Sign::Minus {
                    // round-half-even to nearest 10^|d|
                    let p = d.to_i64().unwrap_or(0).unsigned_abs() as u32;
                    let pow = BigInt::from(10).pow(p);
                    let q = &n / &pow;
                    let r = &n % &pow;
                    let half = &pow / BigInt::from(2);
                    let rounded = if r > half {
                        q + BigInt::from(1)
                    } else if r == half {
                        // half-even: round to even quotient
                        if (&q % BigInt::from(2)).sign() == num_bigint::Sign::NoSign {
                            q
                        } else {
                            q + BigInt::from(1)
                        }
                    } else {
                        q
                    };
                    Ok(Value::Int(rounded * pow))
                } else {
                    Ok(Value::Int(n))
                }
            } else {
                Ok(Value::Int(n))
            }
        }
        Value::Float(f) => {
            let digits = match ndigits {
                None => 0i32,
                Some(d) => d.to_i32().unwrap_or(0),
            };
            if digits == 0 {
                Ok(Value::Float(f.round_ties_even()))
            } else {
                let factor = 10f64.powi(digits);
                Ok(Value::Float((f * factor).round_ties_even() / factor))
            }
        }
        v => Err(ExceptionValue::new(
            "RuntimeError",
            format!("round() argument must be a number, got {}", v.type_name()),
            None,
        )),
    }
}

fn math_sqrt(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let x = match args.first() {
        Some(Value::Int(n)) => n.to_f64().unwrap_or(0.0),
        Some(Value::Float(f)) => *f,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected number, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    if x < 0.0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: math domain error".to_string(),
            None,
        ));
    }
    Ok(Value::Float(x.sqrt()))
}

fn math_floor(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    match args.first() {
        Some(Value::Float(f)) => Ok(Value::Int(BigInt::from(f.floor() as i64))),
        Some(Value::Int(n)) => Ok(Value::Int(n.clone())),
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

fn math_ceil(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    match args.first() {
        Some(Value::Float(f)) => Ok(Value::Int(BigInt::from(f.ceil() as i64))),
        Some(Value::Int(n)) => Ok(Value::Int(n.clone())),
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

// ---------------------------------------------------------------------------
// std.debug
// ---------------------------------------------------------------------------

fn debug_debug(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let msg = arg_str(args, 0).unwrap_or("");
    let data = args.get(1).map(|v| v.to_display(true)).unwrap_or_default();
    let out = if data.is_empty() {
        format!("[debug] {msg}")
    } else {
        format!("[debug] {msg} {data}")
    };
    interp.stdout.borrow_mut().push_str(&out);
    interp.stdout.borrow_mut().push('\n');
    Ok(Value::Str(Rc::from(out.as_str())))
}

// ---------------------------------------------------------------------------
// Export tables
// ---------------------------------------------------------------------------

pub static STR_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "upper",
        func: str_upper,
    },
    StdlibExport {
        name: "lower",
        func: str_lower,
    },
    StdlibExport {
        name: "substring",
        func: str_substring,
    },
    StdlibExport {
        name: "split",
        func: str_split,
    },
    StdlibExport {
        name: "join",
        func: str_join,
    },
    StdlibExport {
        name: "trim",
        func: str_trim,
    },
    StdlibExport {
        name: "contains",
        func: str_contains,
    },
    StdlibExport {
        name: "startswith",
        func: str_startswith,
    },
    StdlibExport {
        name: "endswith",
        func: str_endswith,
    },
    StdlibExport {
        name: "replace",
        func: str_replace,
    },
    StdlibExport {
        name: "strip",
        func: str_strip,
    },
    StdlibExport {
        name: "repeat",
        func: str_repeat,
    },
    StdlibExport {
        name: "reverse",
        func: str_reverse,
    },
    StdlibExport {
        name: "count",
        func: str_count,
    },
    StdlibExport {
        name: "index",
        func: str_index,
    },
    StdlibExport {
        name: "find",
        func: str_find,
    },
    StdlibExport {
        name: "chr",
        func: str_chr,
    },
    StdlibExport {
        name: "ord",
        func: str_ord,
    },
];

pub static LIST_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "sort",
        func: list_sort,
    },
    StdlibExport {
        name: "map",
        func: list_map,
    },
    StdlibExport {
        name: "filter",
        func: list_filter,
    },
    StdlibExport {
        name: "reduce",
        func: list_reduce,
    },
    StdlibExport {
        name: "unique",
        func: list_unique,
    },
];

pub static DICT_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "keys",
        func: dict_keys,
    },
    StdlibExport {
        name: "values",
        func: dict_values,
    },
    StdlibExport {
        name: "entries",
        func: dict_entries,
    },
    StdlibExport {
        name: "get",
        func: dict_get,
    },
    StdlibExport {
        name: "has_key",
        func: dict_has_key,
    },
    StdlibExport {
        name: "merge",
        func: dict_merge,
    },
    StdlibExport {
        name: "set_key",
        func: dict_set_key,
    },
    StdlibExport {
        name: "remove_key",
        func: dict_remove_key,
    },
];

pub static MATH_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "pow",
        func: math_pow,
    },
    StdlibExport {
        name: "round",
        func: math_round,
    },
    StdlibExport {
        name: "sqrt",
        func: math_sqrt,
    },
    StdlibExport {
        name: "floor",
        func: math_floor,
    },
    StdlibExport {
        name: "ceil",
        func: math_ceil,
    },
];

pub static DEBUG_EXPORTS: &[StdlibExport] = &[StdlibExport {
    name: "debug",
    func: debug_debug,
}];
