//! std.string — String manipulation stdlib functions.
//!
//! Ports Python's `str_*` functions: upper, lower, split, join, regex, etc.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::stdlib::StdlibExport;
use crate::stdlib_helpers::{arg_f64, arg_int, arg_map, arg_opt_str, arg_str, py_slice};
use crate::value::Value;

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
