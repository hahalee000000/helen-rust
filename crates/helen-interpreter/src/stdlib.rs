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

use chrono::{Datelike, Timelike};
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

fn arg_f64(args: &[Value], i: usize) -> Result<f64, ExceptionValue> {
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

fn arg_bool(args: &[Value], i: usize) -> Result<bool, ExceptionValue> {
    match args.get(i) {
        Some(Value::Bool(b)) => Ok(*b),
        Some(v) => Ok(v.truthy()),
        None => Ok(false),
    }
}

/// Extract a list argument (Python: requires a list).
fn arg_list(args: &[Value], i: usize) -> Result<Vec<Value>, ExceptionValue> {
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

/// Build a `Value::Map` from str-keyed pairs (insertion-ordered).
fn make_str_map(pairs: &[(&str, &str)]) -> Value {
    let mut map = indexmap::IndexMap::new();
    for (k, v) in pairs {
        map.insert(Value::Str(Rc::from(*k)), Value::Str(Rc::from(*v)));
    }
    Value::Map(Rc::new(RefCell::new(map)))
}

/// Extract a map argument (Python: requires a dict).
fn arg_map(args: &[Value], i: usize) -> Result<indexmap::IndexMap<Value, Value>, ExceptionValue> {
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
fn arg_opt_str(args: &[Value], i: usize) -> Result<Option<&str>, ExceptionValue> {
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
fn arg_opt_int(args: &[Value], i: usize) -> Result<Option<BigInt>, ExceptionValue> {
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

fn err_expected<T>(what: &str, got: &Value) -> Result<T, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        format!("expected {what}, got {}", got.type_name()),
        None,
    ))
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
    let sep = match args.get(1) {
        Some(Value::Str(s)) => s.to_string(),
        Some(Value::Null) | None => " ".to_string(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected string separator, got {}", v.type_name()),
                None,
            ))
        }
    };
    let parts: Vec<String> = items.iter().map(|v| v.python_str()).collect();
    Ok(Value::Str(Rc::from(parts.join(&sep).as_str())))
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

fn str_rsplit(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let sep = arg_opt_str(args, 1)?;
    let items: Vec<Value> = match sep {
        None => s
            .split_whitespace()
            .rev()
            .map(|p| Value::Str(Rc::from(p)))
            .collect(),
        Some("") => s
            .chars()
            .rev()
            .map(|c| Value::Str(Rc::from(c.to_string().as_str())))
            .collect(),
        Some(sep) => s.rsplit(sep).map(|p| Value::Str(Rc::from(p))).collect(),
    };
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn str_lstrip(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(
        s.trim_start_matches(char::is_whitespace)
            .to_string()
            .as_str(),
    )))
}

fn str_rstrip(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(
        s.trim_end_matches(char::is_whitespace).to_string().as_str(),
    )))
}

fn str_find_from(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let sub = arg_str(args, 1)?;
    let start = arg_int(args, 2)?.to_i64().unwrap_or(0).max(0) as usize;
    Ok(Value::Int(BigInt::from(
        s[start..]
            .find(sub)
            .map(|p| (p + start) as i64)
            .unwrap_or(-1),
    )))
}

fn str_trim_prefix(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let prefix = arg_str(args, 1)?;
    Ok(Value::Str(Rc::from(
        s.strip_prefix(prefix).unwrap_or(s).to_string().as_str(),
    )))
}

fn str_trim_suffix(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let suffix = arg_str(args, 1)?;
    Ok(Value::Str(Rc::from(
        s.strip_suffix(suffix).unwrap_or(s).to_string().as_str(),
    )))
}

fn str_pad_left(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let width = arg_int(args, 1)?.to_i64().unwrap_or(0).max(0) as usize;
    let ch = arg_opt_str(args, 2)?.unwrap_or(" ");
    let pad_char = ch.chars().next().unwrap_or(' ');
    if s.len() >= width {
        return Ok(Value::Str(Rc::from(s.to_string().as_str())));
    }
    let fill = std::iter::repeat_n(pad_char, width - s.len()).collect::<String>();
    Ok(Value::Str(Rc::from(format!("{fill}{s}").as_str())))
}

fn str_pad_right(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let width = arg_int(args, 1)?.to_i64().unwrap_or(0).max(0) as usize;
    let ch = arg_opt_str(args, 2)?.unwrap_or(" ");
    let pad_char = ch.chars().next().unwrap_or(' ');
    if s.len() >= width {
        return Ok(Value::Str(Rc::from(s.to_string().as_str())));
    }
    let fill = std::iter::repeat_n(pad_char, width - s.len()).collect::<String>();
    Ok(Value::Str(Rc::from(format!("{s}{fill}").as_str())))
}

fn str_center(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let width = arg_int(args, 1)?.to_i64().unwrap_or(0).max(0) as usize;
    let ch = arg_opt_str(args, 2)?.unwrap_or(" ");
    let pad_char = ch.chars().next().unwrap_or(' ');
    if s.len() >= width {
        return Ok(Value::Str(Rc::from(s.to_string().as_str())));
    }
    let total = width - s.len();
    let left = total / 2;
    let right = total - left;
    let lfill = std::iter::repeat_n(pad_char, left).collect::<String>();
    let rfill = std::iter::repeat_n(pad_char, right).collect::<String>();
    Ok(Value::Str(Rc::from(format!("{lfill}{s}{rfill}").as_str())))
}

fn str_format_float(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = arg_f64(args, 0)?;
    let decimals = arg_int(args, 1)?.to_i32().unwrap_or(0);
    if decimals < 0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: decimals must be non-negative, got {decimals}"),
            None,
        ));
    }
    Ok(Value::Str(Rc::from(
        format!("{:.d$}", value, d = decimals as usize).as_str(),
    )))
}

fn str_levenshtein(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s1 = arg_str(args, 0)?.to_string();
    let s2 = arg_str(args, 1)?.to_string();
    let d = levenshtein(&s1, &s2);
    Ok(Value::Int(BigInt::from(d)))
}

fn levenshtein(s1: &str, s2: &str) -> usize {
    let a: Vec<char> = s1.chars().collect();
    let b: Vec<char> = s2.chars().collect();
    if a.len() < b.len() {
        return levenshtein(s2, s1);
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, c1) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, c2) in b.iter().enumerate() {
            let insert = prev[j + 1] + 1;
            let delete = cur[j] + 1;
            let sub = prev[j] + usize::from(c1 != c2);
            cur.push(insert.min(delete).min(sub));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn str_similarity(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s1 = arg_str(args, 0)?;
    let s2 = arg_str(args, 1)?;
    if s1 == s2 {
        return Ok(Value::Float(1.0));
    }
    let max_len = s1.len().max(s2.len());
    if max_len == 0 {
        return Ok(Value::Float(1.0));
    }
    let dist = levenshtein(s1, s2);
    Ok(Value::Float(1.0 - (dist as f64 / max_len as f64)))
}

fn str_tokenize(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let re = regex::Regex::new(r"\w+").map_err(|e| {
        ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None)
    })?;
    let items: Vec<Value> = re
        .find_iter(s)
        .map(|m| Value::Str(Rc::from(m.as_str())))
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn str_word_count(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let lower = s.to_lowercase();
    let re = regex::Regex::new(r"\w+").map_err(|e| {
        ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None)
    })?;
    let mut counts = indexmap::IndexMap::new();
    for m in re.find_iter(&lower) {
        let word = m.as_str().to_string();
        *counts
            .entry(Value::Str(Rc::from(word.as_str())))
            .or_insert(Value::Int(BigInt::from(0))) =
            match counts.get(&Value::Str(Rc::from(word.as_str()))) {
                Some(Value::Int(n)) => Value::Int(n + 1),
                _ => Value::Int(BigInt::from(1)),
            };
    }
    Ok(Value::Map(Rc::new(RefCell::new(counts))))
}

fn str_normalize_whitespace(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let normalized = s.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(Value::Str(Rc::from(normalized.as_str())))
}

fn str_remove_punctuation(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let out: String = s.chars().filter(|c| !c.is_ascii_punctuation()).collect();
    Ok(Value::Str(Rc::from(out.as_str())))
}

fn str_interpolate(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let template = arg_str(args, 0)?;
    let vars = arg_map(args, 1)?;
    let re = regex::Regex::new(r"\{\{(.+?)\}\}").map_err(|e| {
        ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None)
    })?;
    let out = re
        .replace_all(template, |caps: &regex::Captures| {
            let path = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            let mut parts = path.split('.');
            let first = parts.next().unwrap_or("");
            let mut value = vars.get(&Value::Str(Rc::from(first))).cloned();
            for part in parts {
                match value {
                    Some(Value::Map(m)) => {
                        value = m.borrow().get(&Value::Str(Rc::from(part))).cloned();
                    }
                    _ => {
                        value = None;
                        break;
                    }
                }
            }
            match value {
                Some(v) if !matches!(v, Value::Null) => v.python_str(),
                _ => caps
                    .get(0)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default(),
            }
        })
        .to_string();
    Ok(Value::Str(Rc::from(out.as_str())))
}

fn str_base64_encode(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    use base64::Engine;
    let enc = base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
    Ok(Value::Str(Rc::from(enc.as_str())))
}

fn str_base64_decode(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    use base64::Engine;
    match base64::engine::general_purpose::STANDARD.decode(s.as_bytes()) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Ok(Value::Str(Rc::from(text.as_str()))),
            Err(_) => Err(ExceptionValue::new(
                "RuntimeError",
                "Python binascii.Error: Incorrect padding".to_string(),
                None,
            )),
        },
        Err(_) => Err(ExceptionValue::new(
            "RuntimeError",
            "Python binascii.Error: Incorrect padding".to_string(),
            None,
        )),
    }
}

fn str_html_escape(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    Ok(Value::Str(Rc::from(out.as_str())))
}

fn str_html_unescape(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let mut out = s.replace("&amp;", "&");
    out = out.replace("&lt;", "<");
    out = out.replace("&gt;", ">");
    out = out.replace("&quot;", "\"");
    out = out.replace("&#x27;", "'");
    out = out.replace("&#39;", "'");
    out = out.replace("&nbsp;", " ");
    Ok(Value::Str(Rc::from(out.as_str())))
}

fn str_extract_urls(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let re = regex::Regex::new(r#"https?://[^\s<>"']+"#).map_err(|e| {
        ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None)
    })?;
    let items: Vec<Value> = re
        .find_iter(s)
        .map(|m| Value::Str(Rc::from(m.as_str())))
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn str_extract_emails(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let re = regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").map_err(|e| {
        ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None)
    })?;
    let items: Vec<Value> = re
        .find_iter(s)
        .map(|m| Value::Str(Rc::from(m.as_str())))
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn str_regex_match(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let pattern = arg_str(args, 0)?;
    let s = arg_str(args, 1)?;
    let re = regex::Regex::new(pattern).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid regex pattern: {e}"),
            None,
        )
    })?;
    match re.find(s) {
        Some(m) if m.start() == 0 => Ok(regex_match_map(&re, m)),
        _ => Ok(Value::Null),
    }
}

fn str_regex_search(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let pattern = arg_str(args, 0)?;
    let s = arg_str(args, 1)?;
    let re = regex::Regex::new(pattern).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid regex pattern: {e}"),
            None,
        )
    })?;
    match re.find(s) {
        Some(m) => Ok(regex_match_map(&re, m)),
        None => Ok(Value::Null),
    }
}

fn regex_match_map(re: &regex::Regex, m: regex::Match) -> Value {
    let mut map = indexmap::IndexMap::new();
    map.insert(
        Value::Str(Rc::from("match")),
        Value::Str(Rc::from(m.as_str())),
    );
    let groups: Vec<Value> = re
        .captures(m.as_str())
        .map(|c| {
            c.iter()
                .skip(1)
                .map(|g| match g {
                    Some(g) => Value::Str(Rc::from(g.as_str())),
                    None => Value::Null,
                })
                .collect()
        })
        .unwrap_or_default();
    map.insert(
        Value::Str(Rc::from("groups")),
        Value::List(Rc::new(RefCell::new(groups))),
    );
    map.insert(
        Value::Str(Rc::from("start")),
        Value::Int(BigInt::from(m.start() as i64)),
    );
    map.insert(
        Value::Str(Rc::from("end")),
        Value::Int(BigInt::from(m.end() as i64)),
    );
    Value::Map(Rc::new(RefCell::new(map)))
}

fn str_regex_test(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let pattern = arg_str(args, 0)?;
    let s = arg_str(args, 1)?;
    let re = regex::Regex::new(pattern).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid regex pattern: {e}"),
            None,
        )
    })?;
    Ok(Value::Bool(re.is_match(s)))
}

fn str_regex_replace(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let pattern = arg_str(args, 0)?;
    let s = arg_str(args, 1)?;
    let replacement = arg_str(args, 2)?;
    let re = regex::Regex::new(pattern).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid regex pattern: {e}"),
            None,
        )
    })?;
    // Convert Python-style \\1/$1 group refs to Rust $1.
    let repl = convert_group_refs(replacement);
    let out = re.replace_all(s, repl.as_str());
    Ok(Value::Str(Rc::from(out.to_string().as_str())))
}

fn convert_group_refs(replacement: &str) -> String {
    // Python re.sub uses \1 or \g<1>; Rust regex uses $1.
    let mut out = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        out.push('$');
                        out.push(d);
                        chars.next();
                        continue;
                    }
                }
                out.push('\\');
            }
            '$' => {
                if let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        out.push('$');
                        out.push(d);
                        chars.next();
                        continue;
                    }
                }
                out.push('$');
            }
            _ => out.push(c),
        }
    }
    out
}

fn str_regex_split(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let pattern = arg_str(args, 0)?;
    let s = arg_str(args, 1)?;
    let re = regex::Regex::new(pattern).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid regex pattern: {e}"),
            None,
        )
    })?;
    let items: Vec<Value> = re.split(s).map(|p| Value::Str(Rc::from(p))).collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn str_regex_findall(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let pattern = arg_str(args, 0)?;
    let s = arg_str(args, 1)?;
    let re = regex::Regex::new(pattern).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid regex pattern: {e}"),
            None,
        )
    })?;
    let items: Vec<Value> = re
        .find_iter(s)
        .map(|m| Value::Str(Rc::from(m.as_str())))
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
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

fn list_flatten(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let mut out = Vec::new();
    for item in items {
        match item {
            Value::List(l) => {
                for sub in l.borrow().iter() {
                    out.push(sub.clone());
                }
            }
            other => out.push(other),
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_chunk(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let size = arg_int(args, 1)?.to_i64().unwrap_or(0).max(0) as usize;
    if size == 0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: chunk size must be positive".to_string(),
            None,
        ));
    }
    let mut out = Vec::new();
    for chunk in items.chunks(size) {
        out.push(Value::List(Rc::new(RefCell::new(chunk.to_vec()))));
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_zip(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let lists: Vec<Vec<Value>> = args
        .iter()
        .map(|a| match a {
            Value::List(l) => Ok(l.borrow().clone()),
            v => err_expected("list", v),
        })
        .collect::<Result<_, _>>()?;
    if lists.is_empty() {
        return Ok(Value::List(Rc::new(RefCell::new(vec![]))));
    }
    let min_len = lists.iter().map(|l| l.len()).min().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..min_len {
        let row: Vec<Value> = lists.iter().map(|l| l[i].clone()).collect();
        // Python zip() → list of tuples
        out.push(Value::Tuple(Rc::new(RefCell::new(row))));
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_every(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => {
                if !v.truthy() {
                    return Ok(Value::Bool(false));
                }
            }
            Err(e) => {
                return Err(wrap_hof_error("every", i, item, e));
            }
        }
    }
    Ok(Value::Bool(true))
}

fn list_some(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => {
                if v.truthy() {
                    return Ok(Value::Bool(true));
                }
            }
            Err(e) => {
                return Err(wrap_hof_error("some", i, item, e));
            }
        }
    }
    Ok(Value::Bool(false))
}

fn list_find_if(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => {
                if v.truthy() {
                    return Ok(item.clone());
                }
            }
            Err(e) => {
                return Err(wrap_hof_error("find", i, item, e));
            }
        }
    }
    Ok(Value::Null)
}

fn wrap_hof_error(op: &str, index: usize, item: &Value, e: ExceptionValue) -> ExceptionValue {
    let item_repr = item.python_repr();
    let truncated = if item_repr.len() > 100 {
        format!("{}...(truncated)", &item_repr[..100])
    } else {
        item_repr
    };
    ExceptionValue::new(
        "RuntimeError",
        format!(
            "{op} operation failed at index {index}: {} (element: {truncated})",
            e.message
        ),
        e.span,
    )
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
        // Python dict.items() → list of (key, value) tuples
        .map(|(k, v)| Value::Tuple(Rc::new(RefCell::new(vec![k.clone(), v.clone()]))))
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

fn dict_omit(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = arg_map(args, 0)?;
    let keys = arg_list(args, 1)?;
    let mut out = m;
    for k in keys {
        out.shift_remove(&k);
    }
    Ok(Value::Map(Rc::new(RefCell::new(out))))
}

fn dict_pick(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let m = arg_map(args, 0)?;
    let keys = arg_list(args, 1)?;
    let mut out = indexmap::IndexMap::new();
    for k in keys {
        if let Some(v) = m.get(&k) {
            out.insert(k, v.clone());
        }
    }
    Ok(Value::Map(Rc::new(RefCell::new(out))))
}

fn set_make_set(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let mut out: Vec<Value> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn set_union(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut out: Vec<Value> = Vec::new();
    for arg in args {
        let items = arg_list(std::slice::from_ref(arg), 0)?;
        for item in items {
            if !out.contains(&item) {
                out.push(item);
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn set_intersection(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s1 = arg_list(args, 0)?;
    let s2 = arg_list(args, 1)?;
    let mut out: Vec<Value> = Vec::new();
    for item in &s1 {
        if s2.contains(item) && !out.contains(item) {
            out.push(item.clone());
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn set_difference(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s1 = arg_list(args, 0)?;
    let s2 = arg_list(args, 1)?;
    let mut out: Vec<Value> = Vec::new();
    for item in &s1 {
        if !s2.contains(item) && !out.contains(item) {
            out.push(item.clone());
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn set_has(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_list(args, 0)?;
    let item = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing item".to_string(), None))?;
    Ok(Value::Bool(s.contains(&item)))
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
// std.math — statistics (mean, median, mode, variance, stddev, ...)
// ---------------------------------------------------------------------------

fn math_list_f64(args: &[Value], i: usize) -> Result<Vec<f64>, ExceptionValue> {
    let items = arg_list(args, i)?;
    let mut out = Vec::with_capacity(items.len());
    for v in items {
        match v {
            Value::Int(n) => out.push(n.to_f64().ok_or_else(|| {
                ExceptionValue::new("RuntimeError", "number out of range".to_string(), None)
            })?),
            Value::Float(f) => out.push(f),
            other => return err_expected("number", &other),
        }
    }
    Ok(out)
}

fn math_empty_err(op: &str) -> ExceptionValue {
    ExceptionValue::new(
        "RuntimeError",
        format!("Python ValueError: Cannot calculate {op} of empty list"),
        None,
    )
}

fn math_mean(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let nums = math_list_f64(args, 0)?;
    if nums.is_empty() {
        return Err(math_empty_err("mean"));
    }
    Ok(Value::Float(nums.iter().sum::<f64>() / nums.len() as f64))
}

fn math_median(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    // Python _median: odd length returns the middle element unchanged (ints
    // stay ints); even length is float division.
    let mut items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(math_empty_err("median"));
    }
    // Sort by numeric value
    items.sort_by(|a, b| {
        let fa = match a {
            Value::Int(n) => n.to_f64().unwrap_or(0.0),
            Value::Float(f) => *f,
            _ => 0.0,
        };
        let fb = match b {
            Value::Int(n) => n.to_f64().unwrap_or(0.0),
            Value::Float(f) => *f,
            _ => 0.0,
        };
        fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let n = items.len();
    let mid = n / 2;
    if n % 2 == 0 {
        let a = match &items[mid - 1] {
            Value::Int(x) => x.to_f64().unwrap_or(0.0),
            Value::Float(f) => *f,
            _ => 0.0,
        };
        let b = match &items[mid] {
            Value::Int(x) => x.to_f64().unwrap_or(0.0),
            Value::Float(f) => *f,
            _ => 0.0,
        };
        Ok(Value::Float((a + b) / 2.0))
    } else {
        Ok(items[mid].clone())
    }
}

fn math_mode(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(math_empty_err("mode"));
    }
    // Python _mode: Counter on ORIGINAL values — ints stay ints (no float coercion)
    use std::collections::HashMap;
    let mut counts: HashMap<Vec<u8>, usize> = HashMap::new();
    for v in &items {
        *counts.entry(mode_key(v)).or_insert(0) += 1;
    }
    let max_count = counts.values().cloned().max().unwrap_or(0);
    let mut modes: Vec<Value> = Vec::new();
    for v in &items {
        if counts.get(&mode_key(v)) == Some(&max_count) && !modes.contains(v) {
            modes.push(v.clone());
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(modes))))
}

/// Numeric `a < b` across int/float (Python comparison semantics).
fn num_lt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x < y,
        _ => {
            let fa = num_as_f64(a);
            let fb = num_as_f64(b);
            fa < fb
        }
    }
}

/// Numeric `a > b` across int/float (Python comparison semantics).
fn num_gt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x > y,
        _ => {
            let fa = num_as_f64(a);
            let fb = num_as_f64(b);
            fa > fb
        }
    }
}

/// Extract a numeric value as f64 for cross-type comparisons.
fn num_as_f64(v: &Value) -> f64 {
    match v {
        Value::Int(n) => n.to_f64().unwrap_or(0.0),
        Value::Float(f) => *f,
        _ => 0.0,
    }
}

/// Canonical key for mode counting: bytes + type tag so `1` and `1.0` stay distinct.
fn mode_key(v: &Value) -> Vec<u8> {
    match v {
        Value::Int(n) => {
            let mut k = vec![0u8];
            k.extend_from_slice(&n.to_signed_bytes_be());
            k
        }
        Value::Float(f) => {
            let mut k = vec![1u8];
            k.extend_from_slice(&f.to_bits().to_le_bytes());
            k
        }
        Value::Str(s) => {
            let mut k = vec![2u8];
            k.extend_from_slice(s.as_bytes());
            k
        }
        Value::Bool(b) => vec![3u8, *b as u8],
        other => {
            let mut k = vec![4u8];
            k.extend_from_slice(format!("{other:?}").as_bytes());
            k
        }
    }
}

fn math_variance(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let nums = math_list_f64(args, 0)?;
    // Python: population=True is the DEFAULT (only sample when 2nd arg present)
    let population = if args.len() > 1 {
        arg_bool(args, 1)?
    } else {
        true
    };
    if nums.is_empty() {
        return Err(math_empty_err("variance"));
    }
    if !population && nums.len() < 2 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Sample variance requires at least 2 values".to_string(),
            None,
        ));
    }
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let sq: f64 = nums.iter().map(|x| (x - mean).powi(2)).sum();
    let denom = if population {
        nums.len() as f64
    } else {
        (nums.len() - 1) as f64
    };
    Ok(Value::Float(sq / denom))
}

fn math_stddev(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let nums = math_list_f64(args, 0)?;
    // Python: population=True is the DEFAULT (only sample when 2nd arg present)
    let population = if args.len() > 1 {
        arg_bool(args, 1)?
    } else {
        true
    };
    if nums.is_empty() {
        return Err(math_empty_err("standard deviation"));
    }
    if !population && nums.len() < 2 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Sample variance requires at least 2 values".to_string(),
            None,
        ));
    }
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let sq: f64 = nums.iter().map(|x| (x - mean).powi(2)).sum();
    let denom = if population {
        nums.len() as f64
    } else {
        (nums.len() - 1) as f64
    };
    Ok(Value::Float((sq / denom).sqrt()))
}

fn math_correlation(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let x = math_list_f64(args, 0)?;
    let y = math_list_f64(args, 1)?;
    if x.is_empty() || y.is_empty() {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Cannot calculate correlation of empty lists".to_string(),
            None,
        ));
    }
    if x.len() != y.len() {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Lists must have the same length".to_string(),
            None,
        ));
    }
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let cov: f64 = x
        .iter()
        .zip(y.iter())
        .map(|(xi, yi)| (xi - mx) * (yi - my))
        .sum::<f64>()
        / n;
    let sx = (x.iter().map(|v| (v - mx).powi(2)).sum::<f64>() / n).sqrt();
    let sy = (y.iter().map(|v| (v - my).powi(2)).sum::<f64>() / n).sqrt();
    if sx == 0.0 || sy == 0.0 {
        return Ok(Value::Float(0.0));
    }
    Ok(Value::Float(cov / (sx * sy)))
}

fn math_percentile(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut nums = math_list_f64(args, 0)?;
    let p = arg_f64(args, 1)?;
    if nums.is_empty() {
        return Err(math_empty_err("percentile"));
    }
    if !(0.0..=100.0).contains(&p) {
        return Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Percentile must be between 0 and 100, got {p}"),
            None,
        ));
    }
    nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = nums.len();
    if p == 0.0 {
        return Ok(Value::Float(nums[0]));
    }
    if p == 100.0 {
        return Ok(Value::Float(nums[n - 1]));
    }
    let k = (n - 1) as f64 * (p / 100.0);
    let f = k.floor();
    let c = k.ceil();
    if f == c {
        Ok(Value::Float(nums[k as usize]))
    } else {
        Ok(Value::Float(
            nums[f as usize] * (c - k) + nums[c as usize] * (k - f),
        ))
    }
}

fn math_sum(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    // Python sum(): preserves ints when all inputs are ints
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Ok(Value::Int(BigInt::from(0)));
    }
    let mut all_int = true;
    for v in &items {
        if !matches!(v, Value::Int(_)) {
            all_int = false;
            break;
        }
    }
    if all_int {
        let mut total = BigInt::from(0);
        for v in &items {
            if let Value::Int(n) = v {
                total += n;
            }
        }
        Ok(Value::Int(total))
    } else {
        let mut total = 0.0f64;
        for v in &items {
            let f = match v {
                Value::Int(n) => n.to_f64().ok_or_else(|| {
                    ExceptionValue::new("RuntimeError", "number out of range".to_string(), None)
                })?,
                Value::Float(f) => *f,
                other => return err_expected("number", other),
            };
            total += f;
        }
        Ok(Value::Float(total))
    }
}

fn math_product(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let nums = math_list_f64(args, 0)?;
    if nums.is_empty() {
        return Ok(Value::Float(1.0));
    }
    Ok(Value::Float(nums.iter().product()))
}

fn math_stats_min(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(math_empty_err("minimum"));
    }
    // Python _min: preserves ints for int inputs
    let mut best = items[0].clone();
    for v in &items[1..] {
        if num_lt(v, &best) {
            best = v.clone();
        }
    }
    Ok(best)
}

fn math_stats_max(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(math_empty_err("maximum"));
    }
    // Python _max: preserves ints for int inputs
    let mut best = items[0].clone();
    for v in &items[1..] {
        if num_gt(v, &best) {
            best = v.clone();
        }
    }
    Ok(best)
}

// ── Trig ───────────────────────────────────────────────────────

macro_rules! math_unary {
    ($name:ident, $f:expr) => {
        fn $name(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
            let x = arg_f64(args, 0)?;
            Ok(Value::Float($f(x)))
        }
    };
}

math_unary!(math_cos, f64::cos);
math_unary!(math_sin, f64::sin);
math_unary!(math_tan, f64::tan);
math_unary!(math_acos, f64::acos);
math_unary!(math_asin, f64::asin);
math_unary!(math_atan, f64::atan);
math_unary!(math_exp, f64::exp);
math_unary!(math_log10, f64::log10);

fn math_atan2(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let y = arg_f64(args, 0)?;
    let x = arg_f64(args, 1)?;
    Ok(Value::Float(y.atan2(x)))
}

fn math_log(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let x = arg_f64(args, 0)?;
    if x <= 0.0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Logarithm requires positive number".to_string(),
            None,
        ));
    }
    match args.get(1) {
        Some(Value::Null) | None => Ok(Value::Float(x.ln())),
        Some(_) => {
            let base = arg_f64(args, 1)?;
            Ok(Value::Float(x.log(base)))
        }
    }
}

fn math_log2(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let x = arg_f64(args, 0)?;
    if x <= 0.0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Logarithm requires positive number".to_string(),
            None,
        ));
    }
    Ok(Value::Float(x.log2()))
}

// ── Bitwise (v1.39.4) ──────────────────────────────────────────

fn math_bit_and(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let b = arg_int(args, 1)?;
    Ok(Value::Int(a & b))
}

fn math_bit_or(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let b = arg_int(args, 1)?;
    Ok(Value::Int(a | b))
}

fn math_bit_xor(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let b = arg_int(args, 1)?;
    Ok(Value::Int(a ^ b))
}

fn math_bit_not(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    Ok(Value::Int(-a - 1))
}

fn math_bit_shift_left(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let n = arg_int(args, 1)?;
    let n_u = n.to_u32().ok_or_else(|| {
        ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Shift amount must be non-negative".to_string(),
            None,
        )
    })?;
    if n.sign() == num_bigint::Sign::Minus {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Shift amount must be non-negative".to_string(),
            None,
        ));
    }
    Ok(Value::Int(a << n_u))
}

fn math_bit_shift_right(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let n = arg_int(args, 1)?;
    if n.sign() == num_bigint::Sign::Minus {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Shift amount must be non-negative".to_string(),
            None,
        ));
    }
    let n_u = n.to_u32().unwrap_or(0);
    Ok(Value::Int(a >> n_u))
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
    interp.stdout.lock().unwrap().push_str(&out);
    interp.stdout.lock().unwrap().push('\n');
    Ok(Value::Str(Rc::from(out.as_str())))
}

// ---------------------------------------------------------------------------
// std.data — json / csv
// ---------------------------------------------------------------------------

/// Convert a serde_json Value into a Helen Value.
pub fn json_to_value(j: &serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(BigInt::from(i))
            } else if let Some(u) = n.as_u64() {
                Value::Int(BigInt::from(u))
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::Str(Rc::from(s.as_str())),
        serde_json::Value::Array(a) => {
            let items: Vec<Value> = a.iter().map(json_to_value).collect();
            Value::List(Rc::new(RefCell::new(items)))
        }
        serde_json::Value::Object(o) => {
            let mut m = indexmap::IndexMap::new();
            for (k, v) in o {
                m.insert(Value::Str(Rc::from(k.as_str())), json_to_value(v));
            }
            Value::Map(Rc::new(RefCell::new(m)))
        }
    }
}

/// Convert a Helen Value into a serde_json Value (Python json.dumps).
pub fn value_to_json(v: &Value) -> Result<serde_json::Value, String> {
    match v {
        Value::Null => Ok(serde_json::Value::Null),
        Value::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Int(n) => {
            if let Some(i) = n.to_i64() {
                Ok(serde_json::Value::Number(i.into()))
            } else if let Some(u) = n.to_u64() {
                Ok(serde_json::Value::Number(u.into()))
            } else {
                Ok(serde_json::Value::Number(
                    serde_json::Number::from_f64(n.to_f64().unwrap_or(0.0))
                        .unwrap_or(serde_json::Number::from(0)),
                ))
            }
        }
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .ok_or_else(|| "Out of range float values are not JSON compliant".to_string()),
        Value::Str(s) => Ok(serde_json::Value::String(s.to_string())),
        Value::List(l) => {
            let mut arr = Vec::new();
            for item in l.borrow().iter() {
                arr.push(value_to_json(item)?);
            }
            Ok(serde_json::Value::Array(arr))
        }
        Value::Tuple(t) => {
            // Python json.dumps(tuple) serializes tuples as arrays.
            let mut arr = Vec::new();
            for item in t.borrow().iter() {
                arr.push(value_to_json(item)?);
            }
            Ok(serde_json::Value::Array(arr))
        }
        Value::Map(m) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in m.borrow().iter() {
                let key = match k {
                    Value::Str(s) => s.to_string(),
                    other => other.python_str(),
                };
                obj.insert(key, value_to_json(val)?);
            }
            Ok(serde_json::Value::Object(obj))
        }
        other => Err(format!(
            "Object of type {} is not JSON serializable",
            other.type_name()
        )),
    }
}

/// Python `json.dumps(v)` (indent=None): compact with `", "` / `": "` separators.
fn json_dumps_compact(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => json_str_quote(s),
        serde_json::Value::Array(a) => {
            let items: Vec<String> = a.iter().map(json_dumps_compact).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(o) => {
            let items: Vec<String> = o
                .iter()
                .map(|(k, val)| format!("{}: {}", json_str_quote(k), json_dumps_compact(val)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

/// Python `json.dumps(v, indent=N)`: pretty with `N`-space indent (not 2-space
/// like json.tool); matches CPython's `json.dumps` indent semantics closely.
fn json_dumps_pretty(v: &serde_json::Value, indent: usize) -> String {
    fn inner(v: &serde_json::Value, level: usize, indent: usize) -> String {
        match v {
            serde_json::Value::Array(a) => {
                if a.is_empty() {
                    return "[]".into();
                }
                let pad = " ".repeat(indent * (level + 1));
                let close_pad = " ".repeat(indent * level);
                let items: Vec<String> = a.iter().map(|x| inner(x, level + 1, indent)).collect();
                format!(
                    "[\n{}{}\n{}]",
                    pad,
                    items.join(&format!(",\n{pad}")),
                    close_pad
                )
            }
            serde_json::Value::Object(o) => {
                if o.is_empty() {
                    return "{}".into();
                }
                let pad = " ".repeat(indent * (level + 1));
                let close_pad = " ".repeat(indent * level);
                let items: Vec<String> = o
                    .iter()
                    .map(|(k, val)| {
                        format!("{}: {}", json_str_quote(k), inner(val, level + 1, indent))
                    })
                    .collect();
                format!(
                    "{{\n{}{}\n{}}}",
                    pad,
                    items.join(&format!(",\n{pad}")),
                    close_pad
                )
            }
            _ => json_dumps_compact(v),
        }
    }
    inner(v, 0, indent)
}

/// Python `json.dumps` string quoting: double quotes, escaped control chars.
fn json_str_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn data_json_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(j) => Ok(json_to_value(&j)),
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid JSON: {e}"),
            None,
        )),
    }
}

fn data_json_parse_lenient(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let mut cleaned = s.trim().to_string();
    cleaned = cleaned
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim()
        .to_string();
    cleaned = cleaned.trim_end_matches("```").trim().to_string();
    match serde_json::from_str::<serde_json::Value>(&cleaned) {
        Ok(j) => Ok(json_to_value(&j)),
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid JSON: {e}"),
            None,
        )),
    }
}

fn data_json_stringify(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument".to_string(), None))?;
    let indent = arg_opt_int(args, 1)?
        .and_then(|n| n.to_i64())
        .map(|n| n.max(0) as usize);
    let j = value_to_json(&value)
        .map_err(|m| ExceptionValue::new("RuntimeError", format!("Python TypeError: {m}"), None))?;
    // Python json.dumps(value, indent=None): compact with ", " and ": "
    // separators, ensure_ascii=False. serde_json::to_string is denser
    // (no spaces) and escapes non-ASCII — build the string ourselves.
    let out = match indent {
        Some(ind) => json_dumps_pretty(&j, ind),
        None => json_dumps_compact(&j),
    };
    Ok(Value::Str(Rc::from(out.as_str())))
}

fn data_json_load(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let content = std::fs::read_to_string(path).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python FileNotFoundError: File not found: {path} ({e})"),
            None,
        )
    })?;
    match serde_json::from_str::<serde_json::Value>(&content) {
        Ok(j) => Ok(json_to_value(&j)),
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid JSON in file: {e}"),
            None,
        )),
    }
}

fn data_json_save(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let value = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument".to_string(), None))?;
    let indent = arg_opt_int(args, 2)?
        .and_then(|n| n.to_i64())
        .map(|n| n.max(0) as usize);
    let j = value_to_json(&value)
        .map_err(|m| ExceptionValue::new("RuntimeError", format!("Python TypeError: {m}"), None))?;
    let text = if let Some(_ind) = indent {
        serde_json::to_string_pretty(&j).unwrap_or_default()
    } else {
        serde_json::to_string(&j).unwrap_or_default()
    };
    std::fs::write(path, text)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Saved JSON to {path}").as_str(),
    )))
}

fn data_csv_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let delim = arg_opt_str(args, 1)?
        .unwrap_or(",")
        .chars()
        .next()
        .unwrap_or(',');
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim as u8)
        .has_headers(false)
        .from_reader(s.as_bytes());
    let mut rows: Vec<Value> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| {
            ExceptionValue::new("RuntimeError", format!("Python csv.Error: {e}"), None)
        })?;
        let row: Vec<Value> = rec.iter().map(|f| Value::Str(Rc::from(f))).collect();
        rows.push(Value::List(Rc::new(RefCell::new(row))));
    }
    Ok(Value::List(Rc::new(RefCell::new(rows))))
}

fn data_csv_stringify(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let rows = arg_list(args, 0)?;
    let delim = arg_opt_str(args, 1)?
        .unwrap_or(",")
        .chars()
        .next()
        .unwrap_or(',');
    // Python csv.writer: \r\n line terminator (csv module default).
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim as u8)
        .terminator(csv::Terminator::CRLF)
        .from_writer(vec![]);
    for row in rows {
        let items = match row {
            Value::List(l) => l.borrow().clone(),
            v => return err_expected("list", &v),
        };
        let fields: Vec<String> = items.iter().map(|v| v.python_str()).collect();
        wtr.write_record(&fields).map_err(|e| {
            ExceptionValue::new("RuntimeError", format!("Python csv.Error: {e}"), None)
        })?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python csv.Error: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        String::from_utf8_lossy(&bytes).to_string().as_str(),
    )))
}

fn data_csv_load(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let delim = arg_opt_str(args, 1)?
        .unwrap_or(",")
        .chars()
        .next()
        .unwrap_or(',');
    let content = std::fs::read_to_string(path).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python FileNotFoundError: File not found: {path} ({e})"),
            None,
        )
    })?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim as u8)
        .has_headers(false)
        .from_reader(content.as_bytes());
    let mut rows: Vec<Value> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| {
            ExceptionValue::new("RuntimeError", format!("Python csv.Error: {e}"), None)
        })?;
        let row: Vec<Value> = rec.iter().map(|f| Value::Str(Rc::from(f))).collect();
        rows.push(Value::List(Rc::new(RefCell::new(row))));
    }
    Ok(Value::List(Rc::new(RefCell::new(rows))))
}

fn data_csv_save(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let rows = arg_list(args, 1)?;
    let delim = arg_opt_str(args, 2)?
        .unwrap_or(",")
        .chars()
        .next()
        .unwrap_or(',');
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim as u8)
        .from_writer(vec![]);
    for row in rows {
        let items = match row {
            Value::List(l) => l.borrow().clone(),
            v => return err_expected("list", &v),
        };
        let fields: Vec<String> = items.iter().map(|v| v.python_str()).collect();
        wtr.write_record(&fields).map_err(|e| {
            ExceptionValue::new("RuntimeError", format!("Python csv.Error: {e}"), None)
        })?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python csv.Error: {e}"), None))?;
    std::fs::write(path, bytes)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Saved CSV to {path}").as_str(),
    )))
}

// ---------------------------------------------------------------------------
// std.time
// ---------------------------------------------------------------------------

fn time_now(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let now = chrono::Local::now();
    Ok(Value::Str(Rc::from(
        now.format("%Y-%m-%dT%H:%M:%S").to_string().as_str(),
    )))
}

fn time_time_func(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    Ok(Value::Float(t))
}

fn time_sleep(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let secs = arg_f64(args, 0)?;
    std::thread::sleep(std::time::Duration::from_secs_f64(secs.max(0.0)));
    Ok(Value::Str(Rc::from(
        format!("Slept for {secs} seconds").as_str(),
    )))
}

fn time_date(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let y = arg_opt_int(args, 0)?;
    if y.is_none() {
        let now = chrono::Local::now();
        return Ok(Value::Str(Rc::from(
            now.format("%Y-%m-%d").to_string().as_str(),
        )));
    }
    let year = arg_int(args, 0)?.to_i32().unwrap_or(1970);
    let month = arg_int(args, 1)?.to_u32().unwrap_or(1);
    let day = arg_int(args, 2)?.to_u32().unwrap_or(1);
    match chrono::NaiveDate::from_ymd_opt(year, month, day) {
        Some(d) => Ok(Value::Str(Rc::from(
            d.format("%Y-%m-%d").to_string().as_str(),
        ))),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid date {year}-{month}-{day}"),
            None,
        )),
    }
}

fn time_datetime(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    if args.is_empty() {
        return time_now(_i, args);
    }
    let year = arg_int(args, 0)?.to_i32().unwrap_or(1970);
    let month = arg_int(args, 1)?.to_u32().unwrap_or(1);
    let day = arg_int(args, 2)?.to_u32().unwrap_or(1);
    let hour = arg_int(args, 3)?.to_u32().unwrap_or(0);
    let minute = arg_int(args, 4)?.to_u32().unwrap_or(0);
    let second = arg_int(args, 5)?.to_u32().unwrap_or(0);
    match chrono::NaiveDate::from_ymd_opt(year, month, day) {
        Some(d) => {
            let dt = d.and_hms_opt(hour, minute, second);
            match dt {
                Some(dt) => Ok(Value::Str(Rc::from(
                    dt.format("%Y-%m-%dT%H:%M:%S").to_string().as_str(),
                ))),
                None => Err(ExceptionValue::new(
                    "RuntimeError",
                    "Python ValueError: invalid time".to_string(),
                    None,
                )),
            }
        }
        None => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid date {year}-{month}-{day}"),
            None,
        )),
    }
}

fn time_fromtimestamp(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let ts = arg_f64(args, 0)?;
    let secs = ts as i64;
    let nanos = ((ts - secs as f64) * 1e9) as u32;
    match chrono::DateTime::from_timestamp(secs, nanos) {
        Some(dt) => {
            let local: chrono::DateTime<chrono::Local> = dt.into();
            Ok(Value::Str(Rc::from(
                local.format("%Y-%m-%dT%H:%M:%S").to_string().as_str(),
            )))
        }
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Invalid timestamp".to_string(),
            None,
        )),
    }
}

fn parse_date_like(s: &str) -> Result<chrono::NaiveDateTime, ExceptionValue> {
    if s.contains('T') {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
            .map_err(|e| {
                ExceptionValue::new(
                    "RuntimeError",
                    format!("Python ValueError: Invalid date format: {e}"),
                    None,
                )
            })
    } else {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
            .map_err(|e| {
                ExceptionValue::new(
                    "RuntimeError",
                    format!("Python ValueError: Invalid date format: {e}"),
                    None,
                )
            })
    }
}

fn time_date_format(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let date_str = arg_str(args, 0)?;
    let format_str = arg_str(args, 1)?;
    let dt = parse_date_like(date_str)?;
    let out = strftime(&dt, format_str);
    Ok(Value::Str(Rc::from(out.as_str())))
}

/// Minimal `strftime` directive mapping (Python parity for common directives).
fn strftime(dt: &chrono::NaiveDateTime, fmt: &str) -> String {
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let Some(d) = chars.next() else {
            out.push('%');
            break;
        };
        match d {
            'Y' => out.push_str(&format!("{:04}", dt.year())),
            'y' => out.push_str(&format!("{:02}", dt.year() % 100)),
            'm' => out.push_str(&format!("{:02}", dt.month())),
            'd' => out.push_str(&format!("{:02}", dt.day())),
            'H' => out.push_str(&format!("{:02}", dt.hour())),
            'M' => out.push_str(&format!("{:02}", dt.minute())),
            'S' => out.push_str(&format!("{:02}", dt.second())),
            'B' => out.push_str(month_name(dt.month())),
            'b' => out.push_str(&month_name(dt.month())[..3]),
            'A' => out.push_str(weekday_name(dt.weekday())),
            'a' => out.push_str(&weekday_name(dt.weekday())[..3]),
            'j' => out.push_str(&format!("{:03}", dt.ordinal())),
            'w' => out.push_str(&format!("{}", dt.weekday().num_days_from_sunday())),
            'Z' => out.push_str("UTC"),
            'z' => out.push_str("+0000"),
            '%' => out.push('%'),
            _ => {
                out.push('%');
                out.push(d);
            }
        }
    }
    out
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        _ => "December",
    }
}

fn weekday_name(w: chrono::Weekday) -> &'static str {
    match w {
        chrono::Weekday::Mon => "Monday",
        chrono::Weekday::Tue => "Tuesday",
        chrono::Weekday::Wed => "Wednesday",
        chrono::Weekday::Thu => "Thursday",
        chrono::Weekday::Fri => "Friday",
        chrono::Weekday::Sat => "Saturday",
        chrono::Weekday::Sun => "Sunday",
    }
}

fn time_date_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let date_str = arg_str(args, 0)?;
    let format_str = arg_str(args, 1)?;
    let dt = chrono::NaiveDateTime::parse_from_str(date_str, format_str)
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(date_str, format_str)
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
        })
        .map_err(|e| {
            ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: Cannot parse date: {e}"),
                None,
            )
        })?;
    if !format_str.contains("%H") && !format_str.contains("%M") && !format_str.contains("%S") {
        return Ok(Value::Str(Rc::from(
            dt.format("%Y-%m-%d").to_string().as_str(),
        )));
    }
    Ok(Value::Str(Rc::from(
        dt.format("%Y-%m-%dT%H:%M:%S").to_string().as_str(),
    )))
}

fn time_date_add(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let date_str = arg_str(args, 0)?;
    let days = arg_opt_int(args, 1)?
        .map(|n| n.to_i64().unwrap_or(0))
        .unwrap_or(0);
    let hours = arg_opt_int(args, 2)?
        .map(|n| n.to_i64().unwrap_or(0))
        .unwrap_or(0);
    let minutes = arg_opt_int(args, 3)?
        .map(|n| n.to_i64().unwrap_or(0))
        .unwrap_or(0);
    let seconds = arg_opt_int(args, 4)?
        .map(|n| n.to_i64().unwrap_or(0))
        .unwrap_or(0);
    let is_date_only = !date_str.contains('T');
    let dt = parse_date_like(date_str)?;
    let result =
        dt + chrono::Duration::seconds(days * 86400 + hours * 3600 + minutes * 60 + seconds);
    if is_date_only && hours == 0 && minutes == 0 && seconds == 0 {
        Ok(Value::Str(Rc::from(
            result.format("%Y-%m-%d").to_string().as_str(),
        )))
    } else {
        Ok(Value::Str(Rc::from(
            result.format("%Y-%m-%dT%H:%M:%S").to_string().as_str(),
        )))
    }
}

fn time_date_diff(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let d1 = parse_date_like(arg_str(args, 0)?)?;
    let d2 = parse_date_like(arg_str(args, 1)?)?;
    let unit = arg_opt_str(args, 2)?.unwrap_or("days");
    let secs = (d2 - d1).num_seconds() as f64;
    let out = match unit {
        "seconds" => secs,
        "minutes" => secs / 60.0,
        "hours" => secs / 3600.0,
        "days" => secs / 86400.0,
        other => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: Invalid unit: {other}. Must be 'seconds', 'minutes', 'hours', or 'days'"),
                None,
            ))
        }
    };
    Ok(Value::Float(out))
}

fn time_date_year(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let dt = parse_date_like(arg_str(args, 0)?)?;
    Ok(Value::Int(BigInt::from(dt.year())))
}

fn time_date_month(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let dt = parse_date_like(arg_str(args, 0)?)?;
    Ok(Value::Int(BigInt::from(dt.month())))
}

fn time_date_day(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let dt = parse_date_like(arg_str(args, 0)?)?;
    Ok(Value::Int(BigInt::from(dt.day())))
}

fn time_date_weekday(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let dt = parse_date_like(arg_str(args, 0)?)?;
    Ok(Value::Int(BigInt::from(
        dt.weekday().num_days_from_monday(),
    )))
}

fn time_stopwatch_start(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Float(
        std::time::Instant::now().elapsed().as_secs_f64().max(0.0),
    ))
}

fn time_stopwatch_elapsed(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Float(0.0))
}

fn time_stopwatch_lap(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let start = arg_f64(args, 0).unwrap_or(0.0);
    let mut m = indexmap::IndexMap::new();
    m.insert(Value::Str(Rc::from("lap")), Value::Float(0.0));
    m.insert(Value::Str(Rc::from("start")), Value::Float(start));
    m.insert(Value::Str(Rc::from("now")), Value::Float(start));
    Ok(Value::Map(Rc::new(RefCell::new(m))))
}

fn time_time(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    time_time_func(_i, args)
}

// ---------------------------------------------------------------------------
// std.crypto
// ---------------------------------------------------------------------------

fn hex_digest<D: digest::Digest>(s: &str) -> String {
    let mut hasher = D::new();
    hasher.update(s.as_bytes());
    let out = hasher.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

fn crypto_md5(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(hex_digest::<md5::Md5>(s).as_str())))
}

fn crypto_sha1(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(hex_digest::<sha1::Sha1>(s).as_str())))
}

fn crypto_sha256(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(hex_digest::<sha2::Sha256>(s).as_str())))
}

fn crypto_sha512(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Str(Rc::from(hex_digest::<sha2::Sha512>(s).as_str())))
}

fn crypto_hmac_sha256(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use hmac::{Hmac, Mac};
    let key = arg_str(args, 0)?;
    let msg = arg_str(args, 1)?;
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key.as_bytes()).map_err(|e| {
        ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None)
    })?;
    mac.update(msg.as_bytes());
    let out = mac.finalize().into_bytes();
    Ok(Value::Str(Rc::from(
        out.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            .as_str(),
    )))
}

fn crypto_random(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    Ok(Value::Float(rng.gen::<f64>()))
}

fn crypto_randint(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use rand::Rng;
    let min = arg_int(args, 0)?.to_i64().unwrap_or(0);
    let max = arg_int(args, 1)?.to_i64().unwrap_or(0);
    let mut rng = rand::thread_rng();
    Ok(Value::Int(BigInt::from(rng.gen_range(min..=max))))
}

fn crypto_choice(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use rand::Rng;
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python IndexError: Cannot choose from an empty sequence".to_string(),
            None,
        ));
    }
    let mut rng = rand::thread_rng();
    Ok(items[rng.gen_range(0..items.len())].clone())
}

fn crypto_shuffle(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use rand::seq::SliceRandom;
    let mut items = arg_list(args, 0)?;
    let mut rng = rand::thread_rng();
    items.shuffle(&mut rng);
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn crypto_sample(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use rand::seq::SliceRandom;
    let items = arg_list(args, 0)?;
    let k = arg_int(args, 1)?.to_i64().unwrap_or(0).max(0) as usize;
    let mut rng = rand::thread_rng();
    let sampled: Vec<Value> = items.choose_multiple(&mut rng, k).cloned().collect();
    Ok(Value::List(Rc::new(RefCell::new(sampled))))
}

fn crypto_uuid_generate(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let u = uuid::Uuid::new_v4();
    Ok(Value::Str(Rc::from(u.to_string().as_str())))
}

fn crypto_uuid_from_string(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    match uuid::Uuid::parse_str(s) {
        Ok(_) => Ok(Value::Str(Rc::from(s))),
        Err(_) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: invalid UUID: {s}"),
            None,
        )),
    }
}

fn crypto_uuid_nil(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Str(Rc::from(uuid::Uuid::nil().to_string().as_str())))
}

fn crypto_random_bytes(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use rand::Rng;
    let n = arg_int(args, 0)?.to_i64().unwrap_or(0).max(0) as usize;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..n).map(|_| rng.gen()).collect();
    Ok(Value::Str(Rc::from(
        String::from_utf8_lossy(&bytes).to_string().as_str(),
    )))
}

fn crypto_random_hex(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use rand::Rng;
    let n = arg_int(args, 0)?.to_i64().unwrap_or(0).max(0) as usize;
    let mut rng = rand::thread_rng();
    let hex: String = (0..n).map(|_| format!("{:02x}", rng.gen::<u8>())).collect();
    Ok(Value::Str(Rc::from(hex.as_str())))
}

fn crypto_random_base64(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use rand::Rng;
    let n = arg_int(args, 0)?.to_i64().unwrap_or(0).max(0) as usize;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..n).map(|_| rng.gen()).collect();
    use base64::Engine;
    let enc = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Value::Str(Rc::from(enc.as_str())))
}

fn crypto_hash_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    use digest::Digest;
    let path = arg_str(args, 0)?;
    let algo = arg_opt_str(args, 1)?.unwrap_or("sha256");
    let content = std::fs::read(path).map_err(|e| {
        ExceptionValue::new(
            "RuntimeError",
            format!("Python FileNotFoundError: File not found: {path} ({e})"),
            None,
        )
    })?;
    let out: String = match algo {
        "md5" => {
            let mut h = md5::Md5::new();
            h.update(&content);
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        }
        "sha1" => {
            let mut h = sha1::Sha1::new();
            h.update(&content);
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        }
        "sha512" => {
            let mut h = sha2::Sha512::new();
            h.update(&content);
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        }
        _ => {
            let mut h = sha2::Sha256::new();
            h.update(&content);
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        }
    };
    Ok(Value::Str(Rc::from(out.as_str())))
}

// ---------------------------------------------------------------------------
// std.path
// ---------------------------------------------------------------------------

fn path_join(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let parts: Vec<String> = args.iter().map(|v| v.python_str()).collect();
    let mut p = std::path::PathBuf::new();
    for part in &parts {
        p.push(part);
    }
    Ok(Value::Str(Rc::from(
        p.to_string_lossy().to_string().as_str(),
    )))
}

fn path_basename(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let p = std::path::Path::new(s);
    Ok(Value::Str(Rc::from(
        p.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default()
            .as_str(),
    )))
}

fn path_dirname(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let p = std::path::Path::new(s);
    Ok(Value::Str(Rc::from(
        p.parent()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default()
            .as_str(),
    )))
}

fn path_exists(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Bool(std::path::Path::new(s).exists()))
}

fn path_is_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Bool(std::path::Path::new(s).is_file()))
}

fn path_is_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Bool(std::path::Path::new(s).is_dir()))
}

// ---------------------------------------------------------------------------
// std.io
// ---------------------------------------------------------------------------

fn io_write_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let content = arg_str(args, 1)?;
    std::fs::write(path, content)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Wrote {path}").as_str())))
}

fn io_append_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let content = arg_str(args, 1)?;
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    f.write_all(content.as_bytes())
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Appended to {path}").as_str())))
}

fn io_mkdir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    std::fs::create_dir(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Created directory {path}").as_str(),
    )))
}

fn io_mkdir_p(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    std::fs::create_dir_all(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Created directory {path}").as_str(),
    )))
}

fn io_stream_print(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    _i.stdout.lock().unwrap().push_str(text);
    _i.stdout.lock().unwrap().push('\n');
    Ok(Value::Str(Rc::from(text)))
}

fn io_stream_clear(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

fn io_stream_cursor_up(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

fn io_stream_cursor_down(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

fn io_progress_bar(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let current = arg_f64(args, 0)?;
    let total = arg_f64(args, 1)?;
    let width = arg_opt_int(args, 2)?
        .map(|n| n.to_i64().unwrap_or(0))
        .unwrap_or(40)
        .max(1) as usize;
    let pct = if total > 0.0 {
        (current / total * 100.0).min(100.0)
    } else {
        0.0
    };
    let filled = ((pct / 100.0) * width as f64) as usize;
    let bar = format!(
        "[{}>{}] {:.0}%",
        "#".repeat(filled),
        "-".repeat(width.saturating_sub(filled + 1)),
        pct
    );
    Ok(Value::Str(Rc::from(bar.as_str())))
}

// ---------------------------------------------------------------------------
// std.file
// ---------------------------------------------------------------------------

fn file_file_size(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let meta = std::fs::metadata(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Int(BigInt::from(meta.len() as i64)))
}

fn file_file_modified(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let meta = std::fs::metadata(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    Ok(Value::Float(mtime))
}

fn file_list_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let entries = std::fs::read_dir(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    let items: Vec<Value> = entries
        .filter_map(|e| e.ok())
        .map(|e| {
            Value::Str(Rc::from(
                e.file_name().to_string_lossy().to_string().as_str(),
            ))
        })
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn file_walk_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let mut out: Vec<Value> = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<Value>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                let s = Value::Str(Rc::from(p.to_string_lossy().to_string().as_str()));
                if p.is_dir() {
                    out.push(s);
                    walk(&p, out);
                } else {
                    out.push(s);
                }
            }
        }
    }
    walk(std::path::Path::new(path), &mut out);
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn file_copy_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let src = arg_str(args, 0)?;
    let dst = arg_str(args, 1)?;
    std::fs::copy(src, dst)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Copied {src} to {dst}").as_str(),
    )))
}

fn file_move_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let src = arg_str(args, 0)?;
    let dst = arg_str(args, 1)?;
    std::fs::rename(src, dst)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Moved {src} to {dst}").as_str(),
    )))
}

fn file_delete_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    std::fs::remove_file(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Deleted {path}").as_str())))
}

fn file_delete_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let recursive = arg_bool(args, 1).unwrap_or(false);
    if recursive {
        std::fs::remove_dir_all(path).map_err(|e| {
            ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None)
        })?;
    } else {
        std::fs::remove_dir(path).map_err(|e| {
            ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None)
        })?;
    }
    Ok(Value::Str(Rc::from(
        format!("Deleted directory {path}").as_str(),
    )))
}

fn file_temp_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let suffix = arg_opt_str(args, 0)?.unwrap_or("");
    let prefix = arg_opt_str(args, 1)?.unwrap_or("tmp");
    let default_dir = std::env::temp_dir().to_string_lossy().to_string();
    let dir = arg_opt_str(args, 2)?.unwrap_or(&default_dir);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = format!("{dir}/{prefix}{nanos}{suffix}");
    std::fs::write(&path, "")
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(path.as_str())))
}

fn file_temp_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let suffix = arg_opt_str(args, 0)?.unwrap_or("");
    let prefix = arg_opt_str(args, 1)?.unwrap_or("tmp");
    let default_dir = std::env::temp_dir().to_string_lossy().to_string();
    let dir = arg_opt_str(args, 2)?.unwrap_or(&default_dir);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = format!("{dir}/{prefix}{nanos}{suffix}");
    std::fs::create_dir(&path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(path.as_str())))
}

fn file_glob_files(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let pattern = arg_opt_str(args, 1)?.unwrap_or("*");
    let re = regex::Regex::new(&glob_to_regex(pattern)).unwrap();
    let mut out: Vec<Value> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if re.is_match(&name) {
                out.push(Value::Str(Rc::from(
                    e.path().to_string_lossy().to_string().as_str(),
                )));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn glob_to_regex(glob: &str) -> String {
    let mut re = String::from("^");
    for c in glob.chars() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            _ => re.push(c),
        }
    }
    re.push('$');
    re
}

fn file_grep_files(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let pattern = arg_str(args, 1)?;
    let re = regex::Regex::new(pattern).map_err(|e| {
        ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None)
    })?;
    let mut out: Vec<Value> = Vec::new();
    if let Ok(content) = std::fs::read_to_string(path) {
        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                out.push(Value::Str(Rc::from(format!("{}:{}", i + 1, line).as_str())));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

// ---------------------------------------------------------------------------
// std.system
// ---------------------------------------------------------------------------

fn system_env_get(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str(args, 0)?;
    let default = arg_opt_str(args, 1)?;
    match std::env::var(key) {
        Ok(v) => Ok(Value::Str(Rc::from(v.as_str()))),
        Err(_) => match default {
            Some(d) => Ok(Value::Str(Rc::from(d))),
            None => Ok(Value::Null),
        },
    }
}

fn system_env_set(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str(args, 0)?;
    let value = arg_str(args, 1)?;
    unsafe { std::env::set_var(key, value) };
    Ok(Value::Str(Rc::from(format!("Set {key}={value}").as_str())))
}

fn system_env_delete(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str(args, 0)?;
    unsafe { std::env::remove_var(key) };
    Ok(Value::Str(Rc::from(format!("Deleted {key}").as_str())))
}

fn system_env_list(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut out: Vec<Value> = Vec::new();
    for (k, v) in std::env::vars() {
        out.push(Value::Str(Rc::from(format!("{k}={v}").as_str())));
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn system_get_cli_args(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let items: Vec<Value> = interp
        .program_args
        .iter()
        .map(|a| Value::Str(Rc::from(a.as_str())))
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn system_parse_cli_args(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let args_list = arg_list(args, 0).unwrap_or_default();
    let mut m = indexmap::IndexMap::new();
    let mut i = 0;
    while i < args_list.len() {
        let a = args_list[i].python_str();
        if a.starts_with("--") {
            let key = a.trim_start_matches("--").to_string();
            if i + 1 < args_list.len() && !args_list[i + 1].python_str().starts_with('-') {
                m.insert(Value::Str(Rc::from(key.as_str())), args_list[i + 1].clone());
                i += 2;
            } else {
                m.insert(Value::Str(Rc::from(key.as_str())), Value::Bool(true));
                i += 1;
            }
        } else if a.starts_with('-') && a.len() > 1 {
            let key = a.trim_start_matches('-').to_string();
            if i + 1 < args_list.len() && !args_list[i + 1].python_str().starts_with('-') {
                m.insert(Value::Str(Rc::from(key.as_str())), args_list[i + 1].clone());
                i += 2;
            } else {
                m.insert(Value::Str(Rc::from(key.as_str())), Value::Bool(true));
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    Ok(Value::Map(Rc::new(RefCell::new(m))))
}

fn system_exec(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let command = arg_str(args, 0)?;
    let shell = arg_bool(args, 1).unwrap_or(false);
    let mut cmd = if shell {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(command);
        c
    } else {
        let mut parts = command.split_whitespace();
        let prog = parts.next().unwrap_or("");
        let mut c = std::process::Command::new(prog);
        c.args(parts);
        c
    };
    match cmd.output() {
        Ok(out) => {
            let mut m = indexmap::IndexMap::new();
            m.insert(
                Value::Str(Rc::from("stdout")),
                Value::Str(Rc::from(
                    String::from_utf8_lossy(&out.stdout).to_string().as_str(),
                )),
            );
            m.insert(
                Value::Str(Rc::from("stderr")),
                Value::Str(Rc::from(
                    String::from_utf8_lossy(&out.stderr).to_string().as_str(),
                )),
            );
            m.insert(
                Value::Str(Rc::from("exit_code")),
                Value::Int(BigInt::from(out.status.code().unwrap_or(-1))),
            );
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
        Err(e) => {
            let mut m = indexmap::IndexMap::new();
            m.insert(Value::Str(Rc::from("stdout")), Value::Str(Rc::from("")));
            m.insert(
                Value::Str(Rc::from("stderr")),
                Value::Str(Rc::from(e.to_string().as_str())),
            );
            m.insert(
                Value::Str(Rc::from("exit_code")),
                Value::Int(BigInt::from(-1)),
            );
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
    }
}

fn system_exec_async(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let command = arg_str(args, 0)?;
    let shell = arg_bool(args, 1).unwrap_or(false);
    let mut cmd = if shell {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(command);
        c
    } else {
        let mut parts = command.split_whitespace();
        let prog = parts.next().unwrap_or("");
        let mut c = std::process::Command::new(prog);
        c.args(parts);
        c
    };
    cmd.spawn()
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Started {command}").as_str())))
}

fn system_pid(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Int(BigInt::from(std::process::id() as i64)))
}

fn system_exit(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let code = arg_opt_int(args, 0)?
        .map(|n| n.to_i64().unwrap_or(0))
        .unwrap_or(0);
    Err(ExceptionValue::new("SystemExit", format!("{code}"), None))
}

fn system_hostname(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let h = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "localhost".to_string());
    Ok(Value::Str(Rc::from(h.as_str())))
}

fn system_platform(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Str(Rc::from("linux")))
}

fn system_cpu_count(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Int(BigInt::from(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as i64,
    )))
}

fn system_python_version(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Str(Rc::from("3.11.0")))
}

fn system_platform_version(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Str(Rc::from("")))
}

fn system_memory_info(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut m = indexmap::IndexMap::new();
    m.insert(Value::Str(Rc::from("total")), Value::Int(BigInt::from(0)));
    m.insert(
        Value::Str(Rc::from("available")),
        Value::Int(BigInt::from(0)),
    );
    Ok(Value::Map(Rc::new(RefCell::new(m))))
}

fn system_kill(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let pid = arg_int(args, 0)?.to_i64().unwrap_or(0);
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .status();
    Ok(Value::Null)
}

macro_rules! log_fn {
    ($name:ident, $level:expr) => {
        fn $name(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
            let msg = arg_str(args, 0)?;
            let out = format!("[{level}] {msg}", level = $level);
            interp.stdout.lock().unwrap().push_str(&out);
            interp.stdout.lock().unwrap().push('\n');
            Ok(Value::Str(Rc::from(out.as_str())))
        }
    };
}

log_fn!(system_log_debug, "DEBUG");
log_fn!(system_log_info, "INFO");
log_fn!(system_log_warn, "WARN");
log_fn!(system_log_error, "ERROR");
log_fn!(system_log_critical, "CRITICAL");

fn system_log_set_level(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

fn system_log_to_file(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------
// std.network
// ---------------------------------------------------------------------------

fn network_url_encode(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    Ok(Value::Str(Rc::from(out.as_str())))
}

fn network_url_decode(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    Ok(Value::Str(Rc::from(
        String::from_utf8_lossy(&out).to_string().as_str(),
    )))
}

fn network_url_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let url = arg_str(args, 0)?;
    let mut m = indexmap::IndexMap::new();
    let rest = url.split("://").collect::<Vec<_>>();
    if rest.len() == 2 {
        m.insert(
            Value::Str(Rc::from("scheme")),
            Value::Str(Rc::from(rest[0])),
        );
        let after = rest[1];
        let path_start = after.find('/').unwrap_or(after.len());
        let (host_port, path) = after.split_at(path_start);
        let hp: Vec<&str> = host_port.split(':').collect();
        m.insert(Value::Str(Rc::from("host")), Value::Str(Rc::from(hp[0])));
        if hp.len() > 1 {
            m.insert(
                Value::Str(Rc::from("port")),
                Value::Int(BigInt::from(hp[1].parse::<i64>().unwrap_or(0))),
            );
        }
        m.insert(Value::Str(Rc::from("path")), Value::Str(Rc::from(path)));
    } else {
        m.insert(Value::Str(Rc::from("scheme")), Value::Str(Rc::from("")));
        m.insert(Value::Str(Rc::from("host")), Value::Str(Rc::from("")));
        m.insert(Value::Str(Rc::from("path")), Value::Str(Rc::from(url)));
    }
    Ok(Value::Map(Rc::new(RefCell::new(m))))
}

fn network_url_build(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let scheme = arg_str(args, 0)?;
    let host = arg_str(args, 1)?;
    let path = arg_opt_str(args, 2)?.unwrap_or("");
    let query = args.get(3).cloned().unwrap_or(Value::Null);
    let mut out = format!("{scheme}://{host}{path}");
    if let Value::Map(q) = query {
        let parts: Vec<String> = q
            .borrow()
            .iter()
            .map(|(k, v)| format!("{}={}", k.python_str(), v.python_str()))
            .collect();
        if !parts.is_empty() {
            out.push('?');
            out.push_str(&parts.join("&"));
        }
    }
    Ok(Value::Str(Rc::from(out.as_str())))
}

fn http_request(method: &str, url: &str, data: Option<&Value>) -> Result<Value, ExceptionValue> {
    let body = match data {
        Some(Value::Str(s)) => Some(s.to_string()),
        Some(Value::Null) | None => None,
        Some(v) => Some(v.python_str()),
    };
    let resp = match method {
        "GET" => ureq_get(url),
        "POST" => ureq_post(url, body),
        "PUT" => ureq_put(url, body),
        "DELETE" => ureq_delete(url),
        _ => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("unsupported method {method}"),
                None,
            ))
        }
    };
    match resp {
        Ok((status, text)) => {
            let mut m = indexmap::IndexMap::new();
            m.insert(
                Value::Str(Rc::from("status")),
                Value::Int(BigInt::from(status as i64)),
            );
            m.insert(
                Value::Str(Rc::from("body")),
                Value::Str(Rc::from(text.as_str())),
            );
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
        Err(e) => {
            let mut m = indexmap::IndexMap::new();
            m.insert(Value::Str(Rc::from("status")), Value::Int(BigInt::from(0)));
            m.insert(Value::Str(Rc::from("body")), Value::Str(Rc::from(e)));
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
    }
}

fn ureq_get(url: &str) -> Result<(u16, String), String> {
    let agent = ureq::agent();
    let resp = agent.get(url).call().map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.into_string().map_err(|e| e.to_string())?;
    Ok((status, text))
}

fn ureq_post(url: &str, body: Option<String>) -> Result<(u16, String), String> {
    let agent = ureq::agent();
    let req = agent.post(url);
    let resp = match body {
        Some(b) => req.send_string(&b),
        None => req.send_string(""),
    }
    .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.into_string().map_err(|e| e.to_string())?;
    Ok((status, text))
}

fn ureq_put(url: &str, body: Option<String>) -> Result<(u16, String), String> {
    let agent = ureq::agent();
    let req = agent.put(url);
    let resp = match body {
        Some(b) => req.send_string(&b),
        None => req.send_string(""),
    }
    .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.into_string().map_err(|e| e.to_string())?;
    Ok((status, text))
}

fn ureq_delete(url: &str) -> Result<(u16, String), String> {
    let agent = ureq::agent();
    let resp = agent.delete(url).call().map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.into_string().map_err(|e| e.to_string())?;
    Ok((status, text))
}

fn network_http_get(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let url = arg_str(args, 0)?;
    http_request("GET", url, None)
}

fn network_http_post(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let url = arg_str(args, 0)?;
    let data = args.get(1).cloned().unwrap_or(Value::Null);
    http_request("POST", url, Some(&data))
}

fn network_http_put(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let url = arg_str(args, 0)?;
    let data = args.get(1).cloned().unwrap_or(Value::Null);
    http_request("PUT", url, Some(&data))
}

fn network_http_delete(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let url = arg_str(args, 0)?;
    http_request("DELETE", url, None)
}

fn network_http_download(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let url = arg_str(args, 0)?;
    let path = arg_str(args, 1)?;
    let resp = ureq_get(url)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    std::fs::write(path, resp.1)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Downloaded to {path}").as_str(),
    )))
}

// ---------------------------------------------------------------------------
// Stub module functions (runtime-dependent; documented error until M5+).
// ---------------------------------------------------------------------------

fn stub_unimplemented(name: &str) -> Result<Value, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        format!("{name} is not available in this runtime build (requires M5+ runtime)"),
        None,
    ))
}

macro_rules! stub_fn {
    ($name:ident, $display:expr) => {
        fn $name(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
            stub_unimplemented($display)
        }
    };
}

stub_fn!(stub_clear_context, "clear_context");
stub_fn!(stub_context_stats, "context_stats");
stub_fn!(stub_context_usage, "context_usage");
stub_fn!(stub_get_message, "get_message");
stub_fn!(stub_insert_message, "insert_message");
stub_fn!(stub_delete_message, "delete_message");
stub_fn!(stub_pin_message, "pin_message");
stub_fn!(stub_unpin_message, "unpin_message");
stub_fn!(stub_list_pinned_messages, "list_pinned_messages");
stub_fn!(stub_compress_context, "compress_context");
stub_fn!(stub_search_context, "search_context");
stub_fn!(stub_context_slice, "context_slice");
stub_fn!(stub_export_context, "export_context");
stub_fn!(stub_import_context, "import_context");
stub_fn!(stub_fork_context, "fork_context");
stub_fn!(stub_restore_context, "restore_context");
stub_fn!(stub_replace_message, "replace_message");
stub_fn!(stub_set_context_window, "set_context_window");
stub_fn!(stub_get_context_config, "get_context_config");
stub_fn!(stub_set_cache_aware, "set_cache_aware");
stub_fn!(stub_set_compression_strategy, "set_compression_strategy");
stub_fn!(stub_compress_context_target, "compress_context_target");
stub_fn!(stub_on_compression, "on_compression");
stub_fn!(stub_on_context_overflow, "on_context_overflow");
stub_fn!(stub_working_memory_set, "working_memory_set");
stub_fn!(stub_working_memory_get, "working_memory_get");
stub_fn!(stub_working_memory_remove, "working_memory_remove");
stub_fn!(stub_working_memory_clear, "working_memory_clear");
stub_fn!(
    stub_set_working_memory_enabled,
    "set_working_memory_enabled"
);
stub_fn!(stub_analyze_code, "analyze_code");
stub_fn!(stub_check_security, "check_security");
stub_fn!(stub_quality_score, "quality_score");
stub_fn!(stub_quality_report, "quality_report");
stub_fn!(stub_test_describe, "describe");
stub_fn!(stub_test_it, "it");
stub_fn!(stub_test_it_skip, "it_skip");
stub_fn!(stub_test_assert_true, "assert_true");
stub_fn!(stub_test_assert_equal, "assert_equal");
stub_fn!(stub_test_assert_not_equal, "assert_not_equal");
stub_fn!(stub_test_assert_contains, "assert_contains");
stub_fn!(stub_test_assert_throws, "assert_throws");
stub_fn!(stub_test_expect, "expect");
stub_fn!(stub_test_before_each, "before_each");
stub_fn!(stub_test_after_each, "after_each");
stub_fn!(stub_test_before_all, "before_all");
stub_fn!(stub_test_after_all, "after_all");
stub_fn!(stub_test_run_tests, "run_tests");
stub_fn!(stub_test_run_tests_json, "run_tests_json");
stub_fn!(stub_test_reset, "test_reset");
stub_fn!(stub_test_count, "test_count");
stub_fn!(stub_test_suite, "test_suite");
stub_fn!(stub_test_case, "test_case");
stub_fn!(stub_test_case_skip, "test_case_skip");
stub_fn!(stub_test_fail, "fail");
stub_fn!(stub_test_set_timeout, "set_test_timeout");
stub_fn!(stub_test_end_suite, "test_end_suite");
stub_fn!(stub_web_search, "web_search");
stub_fn!(stub_web_fetch, "web_fetch");
stub_fn!(stub_shell_exec, "shell_exec");
stub_fn!(stub_calculate, "calculate");
stub_fn!(stub_patch_file, "patch_file");
stub_fn!(stub_load_skill, "load_skill");
stub_fn!(stub_list_skill_references, "list_skill_references");
stub_fn!(stub_cancel_llm_call, "cancel_llm_call");
stub_fn!(stub_current_llm_call_id, "current_llm_call_id");
stub_fn!(stub_cancel_all_llm_calls, "cancel_all_llm_calls");
stub_fn!(stub_set_temperature, "set_temperature");
stub_fn!(stub_get_temperature, "get_temperature");
stub_fn!(stub_set_max_turns, "set_max_turns");
stub_fn!(stub_get_max_turns, "get_max_turns");
stub_fn!(stub_set_max_tokens, "set_max_tokens");
stub_fn!(stub_get_max_tokens, "get_max_tokens");
stub_fn!(stub_set_thinking_mode, "set_thinking_mode");
stub_fn!(stub_get_thinking_mode, "get_thinking_mode");
stub_fn!(stub_set_reasoning_effort, "set_reasoning_effort");
stub_fn!(stub_get_reasoning_effort, "get_reasoning_effort");
stub_fn!(stub_get_model, "get_model");
stub_fn!(stub_get_description, "get_description");
stub_fn!(stub_get_provider, "get_provider");
stub_fn!(stub_media, "media");
stub_fn!(stub_media_base64, "media_base64");
stub_fn!(stub_is_media, "is_media");
stub_fn!(stub_media_type, "media_type");
stub_fn!(stub_to_openai_parts, "to_openai_parts");
stub_fn!(stub_to_claude_parts, "to_claude_parts");
stub_fn!(stub_to_gemini_parts, "to_gemini_parts");
stub_fn!(stub_media_to_base64, "media_to_base64");
stub_fn!(stub_save_media, "save_media");
stub_fn!(stub_is_image, "is_image");
stub_fn!(stub_is_video, "is_video");
stub_fn!(stub_is_audio, "is_audio");
stub_fn!(stub_query_transcript, "query_transcript");
stub_fn!(stub_search_transcript, "search_transcript");
stub_fn!(stub_list_invocations, "list_invocations");
stub_fn!(stub_get_invocation, "get_invocation");
stub_fn!(stub_get_invocation_tree, "get_invocation_tree");
stub_fn!(stub_get_spawn_tree, "get_spawn_tree");
stub_fn!(stub_get_spawned_sessions, "get_spawned_sessions");
stub_fn!(stub_export_transcript, "export_transcript");
stub_fn!(stub_replay_transcript, "replay_transcript");
stub_fn!(stub_replay_full_session, "replay_full_session");
stub_fn!(stub_resume_session, "resume_session");
stub_fn!(stub_delete_current_session, "delete_current_session");
stub_fn!(stub_release_session_lock, "release_session_lock");
stub_fn!(stub_invocation_path, "invocation_path");
stub_fn!(stub_get_compression_audit, "get_compression_audit");

// ---------------------------------------------------------------------------
// M8: session management (port of `helen/stdlib/transcript.py` core)
// ---------------------------------------------------------------------------

/// `get_session_id() -> str` — current transcript session ID.
/// Python parity: triggers lazy transcript-store init, creating a session
/// when none exists yet (v1.29.14).
fn impl_get_session_id(i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    if i.session_id.is_empty() {
        // Lazy init: create a new session via the manager (Python
        // `_init_transcript_store(None)` -> `SessionManager.create_session`).
        let mgr = i.session_manager.lock().unwrap();
        let new_id = mgr.create_session(None);
        drop(mgr);
        i.session_id = new_id.clone();
        Ok(Value::Str(Rc::from(new_id.as_str())))
    } else {
        Ok(Value::Str(Rc::from(i.session_id.as_str())))
    }
}

/// `get_session_dir() -> dict` — current session directory.
fn impl_get_session_dir(i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    if i.session_id.is_empty() {
        // Lazy init (Python parity): ensure a session exists before resolving.
        let mgr = i.session_manager.lock().unwrap();
        let new_id = mgr.create_session(None);
        drop(mgr);
        i.session_id = new_id;
    }
    let session_id = i.session_id.clone();
    let mgr = i.session_manager.lock().unwrap();
    let dir = mgr.get_session_dir(&session_id);
    let dir_str = dir.to_string_lossy().to_string();
    Ok(make_str_map(&[("session_dir", &dir_str)]))
}

/// `set_session_dir(path: str) -> dict` — override the session directory
/// (port of transcript.py::set_session_dir; returns status dict).
fn impl_set_session_dir(i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = match args.first() {
        Some(Value::Str(s)) => s.to_string(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "set_session_dir() expected a string path".to_string(),
                None,
            ))
        }
    };
    let new_mgr = helen_runtime::SessionManager::new(Some(std::path::Path::new(&path)));
    *i.session_manager.lock().unwrap() = new_mgr;
    Ok(make_str_map(&[("status", "ok"), ("session_dir", &path)]))
}

/// `list_sessions(scope="") -> list[dict]` — list sessions with metadata.
fn impl_list_sessions(i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let mgr = i.session_manager.lock().unwrap();
    let sessions = mgr.list_sessions();
    let mut items = Vec::new();
    for s in sessions {
        let mut m = indexmap::IndexMap::new();
        m.insert(
            Value::Str(Rc::from("session_id")),
            Value::Str(Rc::from(s.session_id.as_str())),
        );
        m.insert(
            Value::Str(Rc::from("modified_at")),
            Value::Float(s.modified_at),
        );
        m.insert(
            Value::Str(Rc::from("size_bytes")),
            Value::Int(s.size_bytes.into()),
        );
        m.insert(
            Value::Str(Rc::from("message_count")),
            Value::Int(s.message_count.into()),
        );
        items.push(Value::Map(Rc::new(RefCell::new(
            m.into_iter().collect(),
        ))));
    }
    let _ = args; // scope filter: both dirs merged in Python; we use the configured one.
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

/// `delete_session(session_id: str) -> bool` — delete a session.
fn impl_delete_session(i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let session_id = match args.first() {
        Some(Value::Str(s)) => s.to_string(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "delete_session() expected a string session_id".to_string(),
                None,
            ))
        }
    };
    let mgr = i.session_manager.lock().unwrap();
    Ok(Value::Bool(mgr.delete_session(&session_id)))
}

/// `cleanup_sessions(keep_count=100) -> int` — delete old sessions.
fn impl_cleanup_sessions(i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let keep = match args.first() {
        Some(Value::Int(n)) => n.to_string().parse::<usize>().unwrap_or(100),
        Some(_) => 100,
        None => 100,
    };
    let mgr = i.session_manager.lock().unwrap();
    Ok(Value::Int(mgr.cleanup_old_sessions(keep).into()))
}

/// `get_session_meta(session_id="") -> dict` — session metadata dict.
fn impl_get_session_meta(i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let session_id = match args.first() {
        Some(Value::Str(s)) if !s.is_empty() => s.to_string(),
        _ => i.session_id.clone(),
    };
    let mgr = i.session_manager.lock().unwrap();
    if session_id.is_empty() || !mgr.session_exists(&session_id) {
        return Ok(make_str_map(&[("status", "error"), ("error", "Session not found")]));
    }
    // Metadata lives in the transcript's first line; without a transcript
    // store we return a minimal stub (M8 transcript integration fills this).
    Ok(make_str_map(&[
        ("status", "ok"),
        ("session_id", &session_id),
    ]))
}

/// `mailbox_select(channels, timeout=None)` (Task 7.5) — port of
/// `helen/stdlib/mailbox.py::_mailbox_select`.
///
/// Polls each channel endpoint in order (10ms interval) and returns the first
/// available message as `{"endpoint": ..., "message": ...}`, or null on
/// timeout. Non-list input returns null.
fn builtin_mailbox_select(
    _interp: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    use std::time::{Duration, Instant};
    let channels = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        _ => return Ok(Value::Null),
    };
    let timeout = match args.get(1) {
        Some(Value::Float(f)) if f.is_finite() && *f >= 0.0 => Some(*f),
        Some(Value::Int(i)) => i.to_f64(),
        _ => None,
    };
    let deadline = timeout.map(|t| Instant::now() + Duration::from_secs_f64(t));

    loop {
        for endpoint in &channels {
            if let Value::Channel(ep) = endpoint {
                if let Some(msg) = ep.try_receive() {
                    // Python: `if msg is not None` — a None message (or the
                    // close sentinel) is skipped, indistinguishable in Python.
                    if !matches!(msg.0, Value::Null) {
                        let mut m = indexmap::IndexMap::new();
                        m.insert(Value::Str(Rc::from("endpoint")), Value::Channel(ep.clone()));
                        m.insert(Value::Str(Rc::from("message")), msg.0);
                        return Ok(Value::Map(Rc::new(RefCell::new(m))));
                    }
                }
            }
        }
        if let Some(dl) = deadline {
            if Instant::now() >= dl {
                return Ok(Value::Null);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

// Debug observability (runtime-dependent, but `debug` itself works).
stub_fn!(stub_trace_on, "trace_on");
stub_fn!(stub_trace_off, "trace_off");
stub_fn!(stub_get_trace, "get_trace");
stub_fn!(stub_get_llm_log, "get_llm_log");
stub_fn!(stub_get_call_stack, "get_call_stack");
stub_fn!(stub_get_last_error, "get_last_error");
stub_fn!(stub_last_error_detail, "last_error_detail");
stub_fn!(stub_error_category, "error_category");
stub_fn!(stub_error_suggestion, "error_suggestion");
stub_fn!(stub_error_data_flow, "error_data_flow");
stub_fn!(stub_record_data_flow, "record_data_flow");
stub_fn!(stub_trace_value_origin, "trace_value_origin");
stub_fn!(stub_trace_value_consumers, "trace_value_consumers");
stub_fn!(stub_record_session, "record_session");
stub_fn!(stub_replay_session, "replay_session");
stub_fn!(stub_stop_recording, "stop_recording");
stub_fn!(stub_validate_output, "validate_output");
stub_fn!(stub_coverage_on, "coverage_on");
stub_fn!(stub_coverage_off, "coverage_off");
stub_fn!(stub_coverage_summary, "coverage_summary");
stub_fn!(stub_coverage_report, "coverage_report");
stub_fn!(stub_get_data_lineage, "get_data_lineage");

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
        name: "rsplit",
        func: str_rsplit,
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
        name: "lstrip",
        func: str_lstrip,
    },
    StdlibExport {
        name: "rstrip",
        func: str_rstrip,
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
        name: "find_from",
        func: str_find_from,
    },
    StdlibExport {
        name: "chr",
        func: str_chr,
    },
    StdlibExport {
        name: "ord",
        func: str_ord,
    },
    StdlibExport {
        name: "trim_prefix",
        func: str_trim_prefix,
    },
    StdlibExport {
        name: "trim_suffix",
        func: str_trim_suffix,
    },
    StdlibExport {
        name: "pad_left",
        func: str_pad_left,
    },
    StdlibExport {
        name: "pad_right",
        func: str_pad_right,
    },
    StdlibExport {
        name: "center",
        func: str_center,
    },
    StdlibExport {
        name: "format_float",
        func: str_format_float,
    },
    StdlibExport {
        name: "levenshtein",
        func: str_levenshtein,
    },
    StdlibExport {
        name: "similarity",
        func: str_similarity,
    },
    StdlibExport {
        name: "tokenize",
        func: str_tokenize,
    },
    StdlibExport {
        name: "word_count",
        func: str_word_count,
    },
    StdlibExport {
        name: "normalize_whitespace",
        func: str_normalize_whitespace,
    },
    StdlibExport {
        name: "remove_punctuation",
        func: str_remove_punctuation,
    },
    StdlibExport {
        name: "interpolate",
        func: str_interpolate,
    },
    StdlibExport {
        name: "base64_encode",
        func: str_base64_encode,
    },
    StdlibExport {
        name: "base64_decode",
        func: str_base64_decode,
    },
    StdlibExport {
        name: "html_escape",
        func: str_html_escape,
    },
    StdlibExport {
        name: "html_unescape",
        func: str_html_unescape,
    },
    StdlibExport {
        name: "extract_urls",
        func: str_extract_urls,
    },
    StdlibExport {
        name: "extract_emails",
        func: str_extract_emails,
    },
    StdlibExport {
        name: "regex_match",
        func: str_regex_match,
    },
    StdlibExport {
        name: "regex_search",
        func: str_regex_search,
    },
    StdlibExport {
        name: "regex_test",
        func: str_regex_test,
    },
    StdlibExport {
        name: "regex_replace",
        func: str_regex_replace,
    },
    StdlibExport {
        name: "regex_split",
        func: str_regex_split,
    },
    StdlibExport {
        name: "regex_findall",
        func: str_regex_findall,
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
    StdlibExport {
        name: "flatten",
        func: list_flatten,
    },
    StdlibExport {
        name: "chunk",
        func: list_chunk,
    },
    StdlibExport {
        name: "zip",
        func: list_zip,
    },
    StdlibExport {
        name: "every",
        func: list_every,
    },
    StdlibExport {
        name: "some",
        func: list_some,
    },
    StdlibExport {
        name: "find_if",
        func: list_find_if,
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
    StdlibExport {
        name: "omit",
        func: dict_omit,
    },
    StdlibExport {
        name: "pick",
        func: dict_pick,
    },
];

pub static SET_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "make_set",
        func: set_make_set,
    },
    StdlibExport {
        name: "set_union",
        func: set_union,
    },
    StdlibExport {
        name: "set_intersection",
        func: set_intersection,
    },
    StdlibExport {
        name: "set_difference",
        func: set_difference,
    },
    StdlibExport {
        name: "set_has",
        func: set_has,
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
    StdlibExport {
        name: "mean",
        func: math_mean,
    },
    StdlibExport {
        name: "median",
        func: math_median,
    },
    StdlibExport {
        name: "mode",
        func: math_mode,
    },
    StdlibExport {
        name: "variance",
        func: math_variance,
    },
    StdlibExport {
        name: "stddev",
        func: math_stddev,
    },
    StdlibExport {
        name: "correlation",
        func: math_correlation,
    },
    StdlibExport {
        name: "percentile",
        func: math_percentile,
    },
    StdlibExport {
        name: "sum",
        func: math_sum,
    },
    StdlibExport {
        name: "product",
        func: math_product,
    },
    StdlibExport {
        name: "stats_min",
        func: math_stats_min,
    },
    StdlibExport {
        name: "stats_max",
        func: math_stats_max,
    },
    StdlibExport {
        name: "cos",
        func: math_cos,
    },
    StdlibExport {
        name: "sin",
        func: math_sin,
    },
    StdlibExport {
        name: "tan",
        func: math_tan,
    },
    StdlibExport {
        name: "acos",
        func: math_acos,
    },
    StdlibExport {
        name: "asin",
        func: math_asin,
    },
    StdlibExport {
        name: "atan",
        func: math_atan,
    },
    StdlibExport {
        name: "atan2",
        func: math_atan2,
    },
    StdlibExport {
        name: "log",
        func: math_log,
    },
    StdlibExport {
        name: "log2",
        func: math_log2,
    },
    StdlibExport {
        name: "log10",
        func: math_log10,
    },
    StdlibExport {
        name: "exp",
        func: math_exp,
    },
    StdlibExport {
        name: "bit_and",
        func: math_bit_and,
    },
    StdlibExport {
        name: "bit_or",
        func: math_bit_or,
    },
    StdlibExport {
        name: "bit_xor",
        func: math_bit_xor,
    },
    StdlibExport {
        name: "bit_not",
        func: math_bit_not,
    },
    StdlibExport {
        name: "bit_shift_left",
        func: math_bit_shift_left,
    },
    StdlibExport {
        name: "bit_shift_right",
        func: math_bit_shift_right,
    },
];

pub static DATA_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "json_parse",
        func: data_json_parse,
    },
    StdlibExport {
        name: "json_parse_lenient",
        func: data_json_parse_lenient,
    },
    StdlibExport {
        name: "json_stringify",
        func: data_json_stringify,
    },
    StdlibExport {
        name: "json_load",
        func: data_json_load,
    },
    StdlibExport {
        name: "json_save",
        func: data_json_save,
    },
    StdlibExport {
        name: "csv_parse",
        func: data_csv_parse,
    },
    StdlibExport {
        name: "csv_stringify",
        func: data_csv_stringify,
    },
    StdlibExport {
        name: "csv_load",
        func: data_csv_load,
    },
    StdlibExport {
        name: "csv_save",
        func: data_csv_save,
    },
];

pub static TIME_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "now",
        func: time_now,
    },
    StdlibExport {
        name: "time",
        func: time_time,
    },
    StdlibExport {
        name: "time_func",
        func: time_time_func,
    },
    StdlibExport {
        name: "sleep",
        func: time_sleep,
    },
    StdlibExport {
        name: "date",
        func: time_date,
    },
    StdlibExport {
        name: "datetime",
        func: time_datetime,
    },
    StdlibExport {
        name: "fromtimestamp",
        func: time_fromtimestamp,
    },
    StdlibExport {
        name: "date_format",
        func: time_date_format,
    },
    StdlibExport {
        name: "date_parse",
        func: time_date_parse,
    },
    StdlibExport {
        name: "date_add",
        func: time_date_add,
    },
    StdlibExport {
        name: "date_diff",
        func: time_date_diff,
    },
    StdlibExport {
        name: "date_year",
        func: time_date_year,
    },
    StdlibExport {
        name: "date_month",
        func: time_date_month,
    },
    StdlibExport {
        name: "date_day",
        func: time_date_day,
    },
    StdlibExport {
        name: "date_weekday",
        func: time_date_weekday,
    },
    StdlibExport {
        name: "stopwatch_start",
        func: time_stopwatch_start,
    },
    StdlibExport {
        name: "stopwatch_elapsed",
        func: time_stopwatch_elapsed,
    },
    StdlibExport {
        name: "stopwatch_lap",
        func: time_stopwatch_lap,
    },
];

pub static CRYPTO_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "md5",
        func: crypto_md5,
    },
    StdlibExport {
        name: "sha1",
        func: crypto_sha1,
    },
    StdlibExport {
        name: "sha256",
        func: crypto_sha256,
    },
    StdlibExport {
        name: "sha512",
        func: crypto_sha512,
    },
    StdlibExport {
        name: "hmac_sha256",
        func: crypto_hmac_sha256,
    },
    StdlibExport {
        name: "hash_file",
        func: crypto_hash_file,
    },
    StdlibExport {
        name: "random",
        func: crypto_random,
    },
    StdlibExport {
        name: "randint",
        func: crypto_randint,
    },
    StdlibExport {
        name: "choice",
        func: crypto_choice,
    },
    StdlibExport {
        name: "shuffle",
        func: crypto_shuffle,
    },
    StdlibExport {
        name: "sample",
        func: crypto_sample,
    },
    StdlibExport {
        name: "uuid_generate",
        func: crypto_uuid_generate,
    },
    StdlibExport {
        name: "uuid_from_string",
        func: crypto_uuid_from_string,
    },
    StdlibExport {
        name: "uuid_nil",
        func: crypto_uuid_nil,
    },
    StdlibExport {
        name: "random_bytes",
        func: crypto_random_bytes,
    },
    StdlibExport {
        name: "random_hex",
        func: crypto_random_hex,
    },
    StdlibExport {
        name: "random_base64",
        func: crypto_random_base64,
    },
];

pub static PATH_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "path_join",
        func: path_join,
    },
    StdlibExport {
        name: "path_basename",
        func: path_basename,
    },
    StdlibExport {
        name: "path_dirname",
        func: path_dirname,
    },
    StdlibExport {
        name: "path_exists",
        func: path_exists,
    },
    StdlibExport {
        name: "path_is_file",
        func: path_is_file,
    },
    StdlibExport {
        name: "path_is_dir",
        func: path_is_dir,
    },
];

pub static IO_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "write_file",
        func: io_write_file,
    },
    StdlibExport {
        name: "append_file",
        func: io_append_file,
    },
    StdlibExport {
        name: "mkdir",
        func: io_mkdir,
    },
    StdlibExport {
        name: "mkdir_p",
        func: io_mkdir_p,
    },
    StdlibExport {
        name: "stream_print",
        func: io_stream_print,
    },
    StdlibExport {
        name: "stream_clear",
        func: io_stream_clear,
    },
    StdlibExport {
        name: "stream_cursor_up",
        func: io_stream_cursor_up,
    },
    StdlibExport {
        name: "stream_cursor_down",
        func: io_stream_cursor_down,
    },
    StdlibExport {
        name: "progress_bar",
        func: io_progress_bar,
    },
];

pub static FILE_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "file_size",
        func: file_file_size,
    },
    StdlibExport {
        name: "file_modified",
        func: file_file_modified,
    },
    StdlibExport {
        name: "list_dir",
        func: file_list_dir,
    },
    StdlibExport {
        name: "walk_dir",
        func: file_walk_dir,
    },
    StdlibExport {
        name: "copy_file",
        func: file_copy_file,
    },
    StdlibExport {
        name: "move_file",
        func: file_move_file,
    },
    StdlibExport {
        name: "delete_file",
        func: file_delete_file,
    },
    StdlibExport {
        name: "delete_dir",
        func: file_delete_dir,
    },
    StdlibExport {
        name: "temp_file",
        func: file_temp_file,
    },
    StdlibExport {
        name: "temp_dir",
        func: file_temp_dir,
    },
    StdlibExport {
        name: "glob_files",
        func: file_glob_files,
    },
    StdlibExport {
        name: "grep_files",
        func: file_grep_files,
    },
];

pub static SYSTEM_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "env_get",
        func: system_env_get,
    },
    StdlibExport {
        name: "env_set",
        func: system_env_set,
    },
    StdlibExport {
        name: "env_delete",
        func: system_env_delete,
    },
    StdlibExport {
        name: "env_list",
        func: system_env_list,
    },
    StdlibExport {
        name: "get_cli_args",
        func: system_get_cli_args,
    },
    StdlibExport {
        name: "parse_cli_args",
        func: system_parse_cli_args,
    },
    StdlibExport {
        name: "exec",
        func: system_exec,
    },
    StdlibExport {
        name: "exec_async",
        func: system_exec_async,
    },
    StdlibExport {
        name: "pid",
        func: system_pid,
    },
    StdlibExport {
        name: "exit",
        func: system_exit,
    },
    StdlibExport {
        name: "hostname",
        func: system_hostname,
    },
    StdlibExport {
        name: "platform",
        func: system_platform,
    },
    StdlibExport {
        name: "cpu_count",
        func: system_cpu_count,
    },
    StdlibExport {
        name: "python_version",
        func: system_python_version,
    },
    StdlibExport {
        name: "platform_version",
        func: system_platform_version,
    },
    StdlibExport {
        name: "memory_info",
        func: system_memory_info,
    },
    StdlibExport {
        name: "kill",
        func: system_kill,
    },
    StdlibExport {
        name: "log_debug",
        func: system_log_debug,
    },
    StdlibExport {
        name: "log_info",
        func: system_log_info,
    },
    StdlibExport {
        name: "log_warn",
        func: system_log_warn,
    },
    StdlibExport {
        name: "log_error",
        func: system_log_error,
    },
    StdlibExport {
        name: "log_critical",
        func: system_log_critical,
    },
    StdlibExport {
        name: "log_set_level",
        func: system_log_set_level,
    },
    StdlibExport {
        name: "log_to_file",
        func: system_log_to_file,
    },
];

pub static NETWORK_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "http_get",
        func: network_http_get,
    },
    StdlibExport {
        name: "http_post",
        func: network_http_post,
    },
    StdlibExport {
        name: "http_put",
        func: network_http_put,
    },
    StdlibExport {
        name: "http_delete",
        func: network_http_delete,
    },
    StdlibExport {
        name: "http_download",
        func: network_http_download,
    },
    StdlibExport {
        name: "url_parse",
        func: network_url_parse,
    },
    StdlibExport {
        name: "url_build",
        func: network_url_build,
    },
    StdlibExport {
        name: "url_encode",
        func: network_url_encode,
    },
    StdlibExport {
        name: "url_decode",
        func: network_url_decode,
    },
];

pub static DEBUG_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "debug",
        func: debug_debug,
    },
    StdlibExport {
        name: "trace_on",
        func: stub_trace_on,
    },
    StdlibExport {
        name: "trace_off",
        func: stub_trace_off,
    },
    StdlibExport {
        name: "get_trace",
        func: stub_get_trace,
    },
    StdlibExport {
        name: "get_llm_log",
        func: stub_get_llm_log,
    },
    StdlibExport {
        name: "get_call_stack",
        func: stub_get_call_stack,
    },
    StdlibExport {
        name: "get_last_error",
        func: stub_get_last_error,
    },
    StdlibExport {
        name: "last_error_detail",
        func: stub_last_error_detail,
    },
    StdlibExport {
        name: "error_category",
        func: stub_error_category,
    },
    StdlibExport {
        name: "error_suggestion",
        func: stub_error_suggestion,
    },
    StdlibExport {
        name: "error_data_flow",
        func: stub_error_data_flow,
    },
    StdlibExport {
        name: "record_data_flow",
        func: stub_record_data_flow,
    },
    StdlibExport {
        name: "trace_value_origin",
        func: stub_trace_value_origin,
    },
    StdlibExport {
        name: "trace_value_consumers",
        func: stub_trace_value_consumers,
    },
    StdlibExport {
        name: "record_session",
        func: stub_record_session,
    },
    StdlibExport {
        name: "replay_session",
        func: stub_replay_session,
    },
    StdlibExport {
        name: "stop_recording",
        func: stub_stop_recording,
    },
    StdlibExport {
        name: "validate_output",
        func: stub_validate_output,
    },
    StdlibExport {
        name: "coverage_on",
        func: stub_coverage_on,
    },
    StdlibExport {
        name: "coverage_off",
        func: stub_coverage_off,
    },
    StdlibExport {
        name: "coverage_summary",
        func: stub_coverage_summary,
    },
    StdlibExport {
        name: "coverage_report",
        func: stub_coverage_report,
    },
    StdlibExport {
        name: "get_data_lineage",
        func: stub_get_data_lineage,
    },
];

pub static CONTEXT_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "clear_context",
        func: stub_clear_context,
    },
    StdlibExport {
        name: "context_stats",
        func: stub_context_stats,
    },
    StdlibExport {
        name: "context_usage",
        func: stub_context_usage,
    },
    StdlibExport {
        name: "get_message",
        func: stub_get_message,
    },
    StdlibExport {
        name: "insert_message",
        func: stub_insert_message,
    },
    StdlibExport {
        name: "delete_message",
        func: stub_delete_message,
    },
    StdlibExport {
        name: "pin_message",
        func: stub_pin_message,
    },
    StdlibExport {
        name: "unpin_message",
        func: stub_unpin_message,
    },
    StdlibExport {
        name: "list_pinned_messages",
        func: stub_list_pinned_messages,
    },
    StdlibExport {
        name: "compress_context",
        func: stub_compress_context,
    },
    StdlibExport {
        name: "search_context",
        func: stub_search_context,
    },
    StdlibExport {
        name: "context_slice",
        func: stub_context_slice,
    },
    StdlibExport {
        name: "export_context",
        func: stub_export_context,
    },
    StdlibExport {
        name: "import_context",
        func: stub_import_context,
    },
    StdlibExport {
        name: "fork_context",
        func: stub_fork_context,
    },
    StdlibExport {
        name: "restore_context",
        func: stub_restore_context,
    },
    StdlibExport {
        name: "replace_message",
        func: stub_replace_message,
    },
    StdlibExport {
        name: "set_context_window",
        func: stub_set_context_window,
    },
    StdlibExport {
        name: "get_context_config",
        func: stub_get_context_config,
    },
    StdlibExport {
        name: "set_cache_aware",
        func: stub_set_cache_aware,
    },
    StdlibExport {
        name: "set_compression_strategy",
        func: stub_set_compression_strategy,
    },
    StdlibExport {
        name: "compress_context_target",
        func: stub_compress_context_target,
    },
    StdlibExport {
        name: "on_compression",
        func: stub_on_compression,
    },
    StdlibExport {
        name: "on_context_overflow",
        func: stub_on_context_overflow,
    },
    StdlibExport {
        name: "working_memory_set",
        func: stub_working_memory_set,
    },
    StdlibExport {
        name: "working_memory_get",
        func: stub_working_memory_get,
    },
    StdlibExport {
        name: "working_memory_remove",
        func: stub_working_memory_remove,
    },
    StdlibExport {
        name: "working_memory_clear",
        func: stub_working_memory_clear,
    },
    StdlibExport {
        name: "set_working_memory_enabled",
        func: stub_set_working_memory_enabled,
    },
];

pub static QUALITY_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "analyze_code",
        func: stub_analyze_code,
    },
    StdlibExport {
        name: "check_security",
        func: stub_check_security,
    },
    StdlibExport {
        name: "quality_score",
        func: stub_quality_score,
    },
    StdlibExport {
        name: "quality_report",
        func: stub_quality_report,
    },
];

pub static TEST_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "describe",
        func: stub_test_describe,
    },
    StdlibExport {
        name: "it",
        func: stub_test_it,
    },
    StdlibExport {
        name: "it_skip",
        func: stub_test_it_skip,
    },
    StdlibExport {
        name: "assert_true",
        func: stub_test_assert_true,
    },
    StdlibExport {
        name: "assert_equal",
        func: stub_test_assert_equal,
    },
    StdlibExport {
        name: "assert_not_equal",
        func: stub_test_assert_not_equal,
    },
    StdlibExport {
        name: "assert_contains",
        func: stub_test_assert_contains,
    },
    StdlibExport {
        name: "assert_throws",
        func: stub_test_assert_throws,
    },
    StdlibExport {
        name: "expect",
        func: stub_test_expect,
    },
    StdlibExport {
        name: "before_each",
        func: stub_test_before_each,
    },
    StdlibExport {
        name: "after_each",
        func: stub_test_after_each,
    },
    StdlibExport {
        name: "before_all",
        func: stub_test_before_all,
    },
    StdlibExport {
        name: "after_all",
        func: stub_test_after_all,
    },
    StdlibExport {
        name: "run_tests",
        func: stub_test_run_tests,
    },
    StdlibExport {
        name: "run_tests_json",
        func: stub_test_run_tests_json,
    },
    StdlibExport {
        name: "test_reset",
        func: stub_test_reset,
    },
    StdlibExport {
        name: "test_count",
        func: stub_test_count,
    },
    StdlibExport {
        name: "test_suite",
        func: stub_test_suite,
    },
    StdlibExport {
        name: "test_case",
        func: stub_test_case,
    },
    StdlibExport {
        name: "test_case_skip",
        func: stub_test_case_skip,
    },
    StdlibExport {
        name: "fail",
        func: stub_test_fail,
    },
    StdlibExport {
        name: "set_test_timeout",
        func: stub_test_set_timeout,
    },
    StdlibExport {
        name: "test_end_suite",
        func: stub_test_end_suite,
    },
];

pub static TOOLS_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "web_search",
        func: stub_web_search,
    },
    StdlibExport {
        name: "web_fetch",
        func: stub_web_fetch,
    },
    StdlibExport {
        name: "shell_exec",
        func: stub_shell_exec,
    },
    StdlibExport {
        name: "calculate",
        func: stub_calculate,
    },
    StdlibExport {
        name: "patch_file",
        func: stub_patch_file,
    },
    StdlibExport {
        name: "load_skill",
        func: stub_load_skill,
    },
    StdlibExport {
        name: "list_skill_references",
        func: stub_list_skill_references,
    },
];

pub static LLM_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "cancel_llm_call",
        func: stub_cancel_llm_call,
    },
    StdlibExport {
        name: "current_llm_call_id",
        func: stub_current_llm_call_id,
    },
    StdlibExport {
        name: "cancel_all_llm_calls",
        func: stub_cancel_all_llm_calls,
    },
    StdlibExport {
        name: "set_temperature",
        func: stub_set_temperature,
    },
    StdlibExport {
        name: "get_temperature",
        func: stub_get_temperature,
    },
    StdlibExport {
        name: "set_max_turns",
        func: stub_set_max_turns,
    },
    StdlibExport {
        name: "get_max_turns",
        func: stub_get_max_turns,
    },
    StdlibExport {
        name: "set_max_tokens",
        func: stub_set_max_tokens,
    },
    StdlibExport {
        name: "get_max_tokens",
        func: stub_get_max_tokens,
    },
    StdlibExport {
        name: "set_thinking_mode",
        func: stub_set_thinking_mode,
    },
    StdlibExport {
        name: "get_thinking_mode",
        func: stub_get_thinking_mode,
    },
    StdlibExport {
        name: "set_reasoning_effort",
        func: stub_set_reasoning_effort,
    },
    StdlibExport {
        name: "get_reasoning_effort",
        func: stub_get_reasoning_effort,
    },
    StdlibExport {
        name: "get_model",
        func: stub_get_model,
    },
    StdlibExport {
        name: "get_description",
        func: stub_get_description,
    },
    StdlibExport {
        name: "get_provider",
        func: stub_get_provider,
    },
];

pub static MEDIA_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "media",
        func: stub_media,
    },
    StdlibExport {
        name: "media_base64",
        func: stub_media_base64,
    },
    StdlibExport {
        name: "is_media",
        func: stub_is_media,
    },
    StdlibExport {
        name: "media_type",
        func: stub_media_type,
    },
    StdlibExport {
        name: "to_openai_parts",
        func: stub_to_openai_parts,
    },
    StdlibExport {
        name: "to_claude_parts",
        func: stub_to_claude_parts,
    },
    StdlibExport {
        name: "to_gemini_parts",
        func: stub_to_gemini_parts,
    },
    StdlibExport {
        name: "media_to_base64",
        func: stub_media_to_base64,
    },
    StdlibExport {
        name: "save_media",
        func: stub_save_media,
    },
    StdlibExport {
        name: "is_image",
        func: stub_is_image,
    },
    StdlibExport {
        name: "is_video",
        func: stub_is_video,
    },
    StdlibExport {
        name: "is_audio",
        func: stub_is_audio,
    },
];

pub static TRANSCRIPT_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "query_transcript",
        func: stub_query_transcript,
    },
    StdlibExport {
        name: "search_transcript",
        func: stub_search_transcript,
    },
    StdlibExport {
        name: "get_session_id",
        func: impl_get_session_id,
    },
    StdlibExport {
        name: "get_session_dir",
        func: impl_get_session_dir,
    },
    StdlibExport {
        name: "set_session_dir",
        func: impl_set_session_dir,
    },
    StdlibExport {
        name: "list_sessions",
        func: impl_list_sessions,
    },
    StdlibExport {
        name: "list_invocations",
        func: stub_list_invocations,
    },
    StdlibExport {
        name: "get_invocation",
        func: stub_get_invocation,
    },
    StdlibExport {
        name: "get_invocation_tree",
        func: stub_get_invocation_tree,
    },
    StdlibExport {
        name: "get_spawn_tree",
        func: stub_get_spawn_tree,
    },
    StdlibExport {
        name: "get_spawned_sessions",
        func: stub_get_spawned_sessions,
    },
    StdlibExport {
        name: "get_session_meta",
        func: impl_get_session_meta,
    },
    StdlibExport {
        name: "export_transcript",
        func: stub_export_transcript,
    },
    StdlibExport {
        name: "replay_transcript",
        func: stub_replay_transcript,
    },
    StdlibExport {
        name: "replay_full_session",
        func: stub_replay_full_session,
    },
    StdlibExport {
        name: "resume_session",
        func: stub_resume_session,
    },
    StdlibExport {
        name: "delete_session",
        func: impl_delete_session,
    },
    StdlibExport {
        name: "delete_current_session",
        func: stub_delete_current_session,
    },
    StdlibExport {
        name: "cleanup_sessions",
        func: impl_cleanup_sessions,
    },
    StdlibExport {
        name: "release_session_lock",
        func: stub_release_session_lock,
    },
    StdlibExport {
        name: "invocation_path",
        func: stub_invocation_path,
    },
    StdlibExport {
        name: "get_compression_audit",
        func: stub_get_compression_audit,
    },
];

pub static CONCURRENCY_EXPORTS: &[StdlibExport] = &[StdlibExport {
    name: "mailbox_select",
    func: builtin_mailbox_select,
}];

/// Resolve a stdlib module name to its export table.
pub fn module_exports(module: &str) -> Option<&'static [StdlibExport]> {
    match module {
        "std.str" => Some(STR_EXPORTS),
        "std.list" => Some(LIST_EXPORTS),
        "std.dict" => Some(DICT_EXPORTS),
        "std.math" => Some(MATH_EXPORTS),
        "std.data" => Some(DATA_EXPORTS),
        "std.time" => Some(TIME_EXPORTS),
        "std.crypto" => Some(CRYPTO_EXPORTS),
        "std.path" => Some(PATH_EXPORTS),
        "std.io" => Some(IO_EXPORTS),
        "std.file" => Some(FILE_EXPORTS),
        "std.system" => Some(SYSTEM_EXPORTS),
        "std.network" => Some(NETWORK_EXPORTS),
        "std.debug" => Some(DEBUG_EXPORTS),
        "std.context" => Some(CONTEXT_EXPORTS),
        "std.quality" => Some(QUALITY_EXPORTS),
        "std.test" => Some(TEST_EXPORTS),
        "std.tools" => Some(TOOLS_EXPORTS),
        "std.llm" => Some(LLM_EXPORTS),
        "std.media" => Some(MEDIA_EXPORTS),
        "std.transcript" => Some(TRANSCRIPT_EXPORTS),
        "std.concurrency" => Some(CONCURRENCY_EXPORTS),
        _ => None,
    }
}

/// `'std.X'` module tag used by `BuiltinFn.module`.
pub fn module_tag(module: &str) -> &'static str {
    match module {
        "std.str" => "str",
        "std.list" => "list",
        "std.dict" => "dict",
        "std.math" => "math",
        "std.data" => "data",
        "std.time" => "time",
        "std.crypto" => "crypto",
        "std.path" => "path",
        "std.io" => "io",
        "std.file" => "file",
        "std.system" => "system",
        "std.network" => "network",
        "std.debug" => "debug",
        "std.context" => "context",
        "std.quality" => "quality",
        "std.test" => "test",
        "std.tools" => "tools",
        "std.llm" => "llm",
        "std.media" => "media",
        "std.transcript" => "transcript",
        "std.concurrency" => "concurrency",
        _ => "core",
    }
}
