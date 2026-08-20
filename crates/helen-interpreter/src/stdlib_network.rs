//! std.network — Network and HTTP operations.
//!
//! Ports Python's urllib/requests: HTTP client, URL parsing, encoding.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::stdlib::StdlibExport;
use crate::stdlib_helpers::{arg_opt_str, arg_str};
use crate::value::Value;

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

// `context_stats()` — port of `stdlib/context.py:_context_stats`.
// In batch mode (no transcript store / history wired) Python returns the
// ---------------------------------------------------------------------------
// M8: session management — all session functions live in transcript.rs
// and are exposed via TRANSCRIPT_EXPORTS.
// ---------------------------------------------------------------------------

/// `mailbox_select(channels, timeout=None)` (Task 7.5) — port of
/// `helen/stdlib/mailbox.py::_mailbox_select`.
///
/// Polls each channel endpoint in order (10ms interval) and returns the first
/// available message as `{"endpoint": ..., "message": ...}`, or null on
/// timeout. Non-list input returns null.
pub(crate) fn builtin_mailbox_select(
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

// ---------------------------------------------------------------------------
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
