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
use crate::interpreter::Interpreter;
use crate::interpreter_builtins::BuiltinImpl;
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


use crate::stdlib_helpers::{arg_str, arg_int, arg_f64, arg_bool, arg_list, arg_map, arg_opt_str, arg_opt_int, err_expected};

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
// Export tables
// ---------------------------------------------------------------------------

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
    // HTML (regex-based, Python data.py parity).
    StdlibExport {
        name: "html_parse",
        func: crate::data_formats::data_html_parse,
    },
    StdlibExport {
        name: "html_text",
        func: crate::data_formats::data_html_text,
    },
    StdlibExport {
        name: "html_links",
        func: crate::data_formats::data_html_links,
    },
    StdlibExport {
        name: "html_select",
        func: crate::data_formats::data_html_select,
    },
    // Markdown (line-based, Python data.py parity).
    StdlibExport {
        name: "markdown_to_html",
        func: crate::data_formats::data_markdown_to_html,
    },
    StdlibExport {
        name: "markdown_extract_headings",
        func: crate::data_formats::data_markdown_extract_headings,
    },
    StdlibExport {
        name: "markdown_parse",
        func: crate::data_formats::data_markdown_parse,
    },
    // TOML.
    StdlibExport {
        name: "toml_parse",
        func: crate::data_formats::data_toml_parse,
    },
    StdlibExport {
        name: "toml_stringify",
        func: crate::data_formats::data_toml_stringify,
    },
    StdlibExport {
        name: "toml_load",
        func: crate::data_formats::data_toml_load,
    },
    StdlibExport {
        name: "toml_save",
        func: crate::data_formats::data_toml_save,
    },
    // XML.
    StdlibExport {
        name: "xml_parse",
        func: crate::data_formats::data_xml_parse,
    },
    StdlibExport {
        name: "xml_stringify",
        func: crate::data_formats::data_xml_stringify,
    },
    StdlibExport {
        name: "xml_load",
        func: crate::data_formats::data_xml_load,
    },
    StdlibExport {
        name: "xml_save",
        func: crate::data_formats::data_xml_save,
    },
    // YAML.
    StdlibExport {
        name: "yaml_parse",
        func: crate::data_formats::data_yaml_parse,
    },
    StdlibExport {
        name: "yaml_stringify",
        func: crate::data_formats::data_yaml_stringify,
    },
    StdlibExport {
        name: "yaml_load",
        func: crate::data_formats::data_yaml_load,
    },
    StdlibExport {
        name: "yaml_save",
        func: crate::data_formats::data_yaml_save,
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



pub static DEBUG_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "debug",
        func: debug_debug,
    },
    StdlibExport {
        name: "trace_on",
        func: crate::debug::debug_trace_on,
    },
    StdlibExport {
        name: "trace_off",
        func: crate::debug::debug_trace_off,
    },
    StdlibExport {
        name: "get_trace",
        func: crate::debug::debug_get_trace,
    },
    StdlibExport {
        name: "get_llm_log",
        func: crate::debug::debug_get_llm_log,
    },
    StdlibExport {
        name: "get_call_stack",
        func: crate::debug::debug_get_call_stack,
    },
    StdlibExport {
        name: "get_last_error",
        func: crate::debug::debug_get_last_error,
    },
    StdlibExport {
        name: "last_error_detail",
        func: crate::debug::debug_last_error_detail,
    },
    StdlibExport {
        name: "error_category",
        func: crate::debug::debug_error_category,
    },
    StdlibExport {
        name: "error_suggestion",
        func: crate::debug::debug_error_suggestion,
    },
    StdlibExport {
        name: "error_data_flow",
        func: crate::debug::debug_error_data_flow,
    },
    StdlibExport {
        name: "record_data_flow",
        func: crate::debug::debug_record_data_flow,
    },
    StdlibExport {
        name: "trace_value_origin",
        func: crate::debug::debug_trace_value_origin,
    },
    StdlibExport {
        name: "trace_value_consumers",
        func: crate::debug::debug_trace_value_consumers,
    },
    StdlibExport {
        name: "record_session",
        func: crate::debug::debug_record_session,
    },
    StdlibExport {
        name: "replay_session",
        func: crate::debug::debug_replay_session,
    },
    StdlibExport {
        name: "stop_recording",
        func: crate::debug::debug_stop_recording,
    },
    StdlibExport {
        name: "validate_output",
        func: crate::debug::debug_validate_output,
    },
    StdlibExport {
        name: "coverage_on",
        func: crate::debug::debug_coverage_on,
    },
    StdlibExport {
        name: "coverage_off",
        func: crate::debug::debug_coverage_off,
    },
    StdlibExport {
        name: "coverage_summary",
        func: crate::debug::debug_coverage_summary,
    },
    StdlibExport {
        name: "coverage_report",
        func: crate::debug::debug_coverage_report,
    },
    StdlibExport {
        name: "get_data_lineage",
        func: crate::debug::debug_get_data_lineage,
    },
];

pub static CONTEXT_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "clear_context",
        func: crate::context::context_clear_context,
    },
    StdlibExport {
        name: "context_stats",
        func: crate::context::context_context_stats,
    },
    StdlibExport {
        name: "context_usage",
        func: crate::context::context_context_usage,
    },
    StdlibExport {
        name: "get_message",
        func: crate::context::context_get_message,
    },
    StdlibExport {
        name: "insert_message",
        func: crate::context::context_insert_message,
    },
    StdlibExport {
        name: "delete_message",
        func: crate::context::context_delete_message,
    },
    StdlibExport {
        name: "pin_message",
        func: crate::context::context_pin_message,
    },
    StdlibExport {
        name: "unpin_message",
        func: crate::context::context_unpin_message,
    },
    StdlibExport {
        name: "list_pinned_messages",
        func: crate::context::context_list_pinned_messages,
    },
    StdlibExport {
        name: "compress_context",
        func: crate::context::context_compress_context,
    },
    StdlibExport {
        name: "search_context",
        func: crate::context::context_search_context,
    },
    StdlibExport {
        name: "context_slice",
        func: crate::context::context_context_slice,
    },
    StdlibExport {
        name: "export_context",
        func: crate::context::context_export_context,
    },
    StdlibExport {
        name: "import_context",
        func: crate::context::context_import_context,
    },
    StdlibExport {
        name: "fork_context",
        func: crate::context::context_fork_context,
    },
    StdlibExport {
        name: "restore_context",
        func: crate::context::context_restore_context,
    },
    StdlibExport {
        name: "replace_message",
        func: crate::context::context_replace_message,
    },
    StdlibExport {
        name: "set_context_window",
        func: crate::context::context_set_context_window,
    },
    StdlibExport {
        name: "get_context_config",
        func: crate::context::context_get_context_config,
    },
    StdlibExport {
        name: "set_cache_aware",
        func: crate::context::context_set_cache_aware,
    },
    StdlibExport {
        name: "set_compression_strategy",
        func: crate::context::context_set_compression_strategy,
    },
    StdlibExport {
        name: "compress_context_target",
        func: crate::context::context_compress_context_target,
    },
    StdlibExport {
        name: "on_compression",
        func: crate::context::context_on_compression,
    },
    StdlibExport {
        name: "on_context_overflow",
        func: crate::context::context_on_context_overflow,
    },
    StdlibExport {
        name: "working_memory_set",
        func: crate::context::context_working_memory_set,
    },
    StdlibExport {
        name: "working_memory_get",
        func: crate::context::context_working_memory_get,
    },
    StdlibExport {
        name: "working_memory_remove",
        func: crate::context::context_working_memory_remove,
    },
    StdlibExport {
        name: "working_memory_clear",
        func: crate::context::context_working_memory_clear,
    },
    StdlibExport {
        name: "set_working_memory_enabled",
        func: crate::context::context_set_working_memory_enabled,
    },
];

pub static QUALITY_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "analyze_code",
        func: crate::quality::quality_analyze_code,
    },
    StdlibExport {
        name: "check_security",
        func: crate::quality::quality_check_security,
    },
    StdlibExport {
        name: "quality_score",
        func: crate::quality::quality_quality_score,
    },
    StdlibExport {
        name: "dimension_scores",
        func: crate::quality::quality_dimension_scores,
    },
    StdlibExport {
        name: "quality_report",
        func: crate::quality::quality_quality_report,
    },
];

pub static TEST_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "describe",
        func: crate::test_framework::test_describe,
    },
    StdlibExport {
        name: "it",
        func: crate::test_framework::test_it,
    },
    StdlibExport {
        name: "it_skip",
        func: crate::test_framework::test_it_skip,
    },
    StdlibExport {
        name: "assert_true",
        func: crate::test_framework::test_assert_true,
    },
    StdlibExport {
        name: "assert_equal",
        func: crate::test_framework::test_assert_equal,
    },
    StdlibExport {
        name: "assert_not_equal",
        func: crate::test_framework::test_assert_not_equal,
    },
    StdlibExport {
        name: "assert_contains",
        func: crate::test_framework::test_assert_contains,
    },
    StdlibExport {
        name: "assert_throws",
        func: crate::test_framework::test_assert_throws,
    },
    StdlibExport {
        name: "expect",
        func: crate::test_framework::test_expect,
    },
    StdlibExport {
        name: "before_each",
        func: crate::test_framework::test_before_each,
    },
    StdlibExport {
        name: "after_each",
        func: crate::test_framework::test_after_each,
    },
    StdlibExport {
        name: "before_all",
        func: crate::test_framework::test_before_all,
    },
    StdlibExport {
        name: "after_all",
        func: crate::test_framework::test_after_all,
    },
    StdlibExport {
        name: "run_tests",
        func: crate::test_framework::test_run_tests,
    },
    StdlibExport {
        name: "run_tests_json",
        func: crate::test_framework::test_run_tests_json,
    },
    StdlibExport {
        name: "test_reset",
        func: crate::test_framework::test_reset,
    },
    StdlibExport {
        name: "test_count",
        func: crate::test_framework::test_count,
    },
    StdlibExport {
        name: "test_suite",
        func: crate::test_framework::test_suite,
    },
    StdlibExport {
        name: "test_case",
        func: crate::test_framework::test_suite,
    },
    StdlibExport {
        name: "test_case_skip",
        func: crate::test_framework::test_it_skip,
    },
    StdlibExport {
        name: "fail",
        func: crate::test_framework::test_fail,
    },
    StdlibExport {
        name: "set_test_timeout",
        func: crate::test_framework::test_set_timeout,
    },
    StdlibExport {
        name: "test_end_suite",
        func: crate::test_framework::test_end_suite,
    },
];

pub static TOOLS_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "web_search",
        func: crate::tools::tools_web_search,
    },
    StdlibExport {
        name: "web_fetch",
        func: crate::tools::tools_web_fetch,
    },
    StdlibExport {
        name: "shell_exec",
        func: crate::tools::tools_shell_exec,
    },
    StdlibExport {
        name: "calculate",
        func: crate::tools::tools_calculate,
    },
    StdlibExport {
        name: "patch_file",
        func: crate::tools::tools_patch_file,
    },
    StdlibExport {
        name: "load_skill",
        func: crate::tools::tools_load_skill,
    },
    StdlibExport {
        name: "list_skill_references",
        func: crate::tools::tools_list_skill_references,
    },
    StdlibExport {
        name: "read_file",
        func: crate::tools::tools_read_file,
    },
    StdlibExport {
        name: "write_file",
        func: crate::tools::tools_write_file,
    },
    StdlibExport {
        name: "shell_exec_full",
        func: crate::tools::tools_shell_exec_full,
    },
];

pub static LLM_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "cancel_llm_call",
        func: crate::llm_control::llm_cancel_llm_call,
    },
    StdlibExport {
        name: "current_llm_call_id",
        func: crate::llm_control::llm_current_llm_call_id,
    },
    StdlibExport {
        name: "cancel_all_llm_calls",
        func: crate::llm_control::llm_cancel_all_llm_calls,
    },
    StdlibExport {
        name: "set_temperature",
        func: crate::llm_control::llm_set_temperature,
    },
    StdlibExport {
        name: "get_temperature",
        func: crate::llm_control::llm_get_temperature,
    },
    StdlibExport {
        name: "set_max_turns",
        func: crate::llm_control::llm_set_max_turns,
    },
    StdlibExport {
        name: "get_max_turns",
        func: crate::llm_control::llm_get_max_turns,
    },
    StdlibExport {
        name: "set_max_tokens",
        func: crate::llm_control::llm_set_max_tokens,
    },
    StdlibExport {
        name: "get_max_tokens",
        func: crate::llm_control::llm_get_max_tokens,
    },
    StdlibExport {
        name: "set_thinking_mode",
        func: crate::llm_control::llm_set_thinking_mode,
    },
    StdlibExport {
        name: "get_thinking_mode",
        func: crate::llm_control::llm_get_thinking_mode,
    },
    StdlibExport {
        name: "set_reasoning_effort",
        func: crate::llm_control::llm_set_reasoning_effort,
    },
    StdlibExport {
        name: "get_reasoning_effort",
        func: crate::llm_control::llm_get_reasoning_effort,
    },
    StdlibExport {
        name: "get_model",
        func: crate::llm_control::llm_get_model,
    },
    StdlibExport {
        name: "get_description",
        func: crate::llm_control::llm_get_description,
    },
    StdlibExport {
        name: "get_provider",
        func: crate::llm_control::llm_get_provider,
    },
];

pub static MEDIA_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "media",
        func: crate::media::media_media,
    },
    StdlibExport {
        name: "media_base64",
        func: crate::media::media_media_base64,
    },
    StdlibExport {
        name: "is_media",
        func: crate::media::media_is_media,
    },
    StdlibExport {
        name: "media_type",
        func: crate::media::media_media_type,
    },
    StdlibExport {
        name: "to_openai_parts",
        func: crate::media::media_to_openai_parts,
    },
    StdlibExport {
        name: "to_claude_parts",
        func: crate::media::media_to_claude_parts,
    },
    StdlibExport {
        name: "to_gemini_parts",
        func: crate::media::media_to_gemini_parts,
    },
    StdlibExport {
        name: "media_to_base64",
        func: crate::media::media_media_to_base64,
    },
    StdlibExport {
        name: "save_media",
        func: crate::media::media_save_media,
    },
    StdlibExport {
        name: "is_image",
        func: crate::media::media_is_image,
    },
    StdlibExport {
        name: "is_video",
        func: crate::media::media_is_video,
    },
    StdlibExport {
        name: "is_audio",
        func: crate::media::media_is_audio,
    },
];

pub static TRANSCRIPT_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "query_transcript",
        func: crate::transcript::transcript_query_transcript,
    },
    StdlibExport {
        name: "search_transcript",
        func: crate::transcript::transcript_search_transcript,
    },
    StdlibExport {
        name: "get_session_id",
        func: crate::transcript::transcript_get_session_id,
    },
    StdlibExport {
        name: "get_session_dir",
        func: crate::transcript::transcript_get_session_dir,
    },
    StdlibExport {
        name: "list_sessions",
        func: crate::transcript::transcript_list_sessions,
    },
    StdlibExport {
        name: "list_invocations",
        func: crate::transcript::transcript_list_invocations,
    },
    StdlibExport {
        name: "get_invocation",
        func: crate::transcript::transcript_get_invocation,
    },
    StdlibExport {
        name: "get_invocation_tree",
        func: crate::transcript::transcript_get_invocation_tree,
    },
    StdlibExport {
        name: "get_spawn_tree",
        func: crate::transcript::transcript_get_spawn_tree,
    },
    StdlibExport {
        name: "get_spawned_sessions",
        func: crate::transcript::transcript_get_spawned_sessions,
    },
    StdlibExport {
        name: "get_session_meta",
        func: crate::transcript::transcript_get_session_meta,
    },
    StdlibExport {
        name: "export_transcript",
        func: crate::transcript::transcript_export_transcript,
    },
    StdlibExport {
        name: "replay_transcript",
        func: crate::transcript::transcript_replay_transcript,
    },
    StdlibExport {
        name: "replay_full_session",
        func: crate::transcript::transcript_replay_full_session,
    },
    StdlibExport {
        name: "resume_session",
        func: crate::transcript::transcript_resume_session,
    },
    StdlibExport {
        name: "delete_session",
        func: crate::transcript::transcript_delete_session,
    },
    StdlibExport {
        name: "delete_current_session",
        func: crate::transcript::transcript_delete_current_session,
    },
    StdlibExport {
        name: "cleanup_sessions",
        func: crate::transcript::transcript_cleanup_sessions,
    },
    StdlibExport {
        name: "set_session_dir",
        func: crate::transcript::transcript_set_session_dir,
    },
    StdlibExport {
        name: "release_session_lock",
        func: crate::transcript::transcript_release_session_lock,
    },
    StdlibExport {
        name: "invocation_path",
        func: crate::transcript::transcript_invocation_path,
    },
    StdlibExport {
        name: "get_compression_audit",
        func: crate::transcript::transcript_get_compression_audit,
    },
];

pub static CONCURRENCY_EXPORTS: &[StdlibExport] = &[StdlibExport {
    name: "mailbox_select",
    func: crate::stdlib_network::builtin_mailbox_select,
}];

/// Resolve a stdlib module name to its export table.
pub fn module_exports(module: &str) -> Option<&'static [StdlibExport]> {
    match module {
        "std.str" => Some(crate::stdlib_string::STR_EXPORTS),
        "std.list" => Some(crate::stdlib_list::LIST_EXPORTS),
        "std.dict" => Some(DICT_EXPORTS),
        "std.math" => Some(crate::stdlib_math::MATH_EXPORTS),
        "std.data" => Some(DATA_EXPORTS),
        "std.time" => Some(TIME_EXPORTS),
        "std.crypto" => Some(CRYPTO_EXPORTS),
        "std.path" => Some(crate::stdlib_io::PATH_EXPORTS),
        "std.io" => Some(crate::stdlib_io::IO_EXPORTS),
        "std.file" => Some(crate::stdlib_io::FILE_EXPORTS),
        "std.system" => Some(crate::stdlib_system::SYSTEM_EXPORTS),
        "std.network" => Some(crate::stdlib_network::NETWORK_EXPORTS),
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
