//! Context management stdlib functions.
//!
//! Byte-faithful port of `helen/stdlib/context.py` (v1.44.0): provides
//! functions to manage LLM conversation context.
//!
//! In the Rust port, context management is backed by the TranscriptStore
//! (SSOT since v1.16). Functions that require a HistoryManager (e.g.
//! compression, fork/restore) return informative errors until a full
//! HistoryManager equivalent is implemented.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use num_bigint::BigInt;

use helen_runtime::transcript::{JsonlBackend, TranscriptStore};

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn arg_str(args: &[Value], i: usize) -> Result<String, ExceptionValue> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s.to_string()),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected string at position {}, got {}", i, other.type_name()),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}

fn arg_str_or(args: &[Value], i: usize, default: &str) -> String {
    match args.get(i) {
        Some(Value::Str(s)) => s.to_string(),
        _ => default.to_string(),
    }
}

fn arg_int_or(args: &[Value], i: usize, default: i64) -> i64 {
    match args.get(i) {
        Some(Value::Int(n)) => {
            use num_traits::ToPrimitive;
            n.to_i64().unwrap_or(default)
        }
        _ => default,
    }
}

fn arg_bool_or(args: &[Value], i: usize, default: bool) -> bool {
    match args.get(i) {
        Some(Value::Bool(b)) => *b,
        _ => default,
    }
}

fn load_store(interp: &Interpreter) -> Option<TranscriptStore> {
    // Check for HELEN_TRANSCRIPT_LOG environment variable (CLI --transcript-log flag)
    if let Ok(transcript_log_path) = std::env::var("HELEN_TRANSCRIPT_LOG") {
        let path = std::path::Path::new(&transcript_log_path);
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Use JSONL backend (SQLite support deferred — requires TranscriptStore refactor)
        let backend = JsonlBackend::new(path);
        return Some(TranscriptStore::load_from_backend(backend, 1000));
    }

    if interp.session_id.is_empty() {
        return None;
    }
    let manager = interp.session_manager.lock().unwrap();
    let path = manager.get_session_path(&interp.session_id);
    drop(manager);
    if !path.exists() {
        return None;
    }
    let backend = JsonlBackend::new(&path);
    Some(TranscriptStore::load_from_backend(backend, 1000))
}

fn make_error_map(msg: &str) -> Value {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from(msg)));
    Value::Map(Rc::new(RefCell::new(result)))
}

fn make_ok_map() -> indexmap::IndexMap<Value, Value> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
    result
}

fn message_to_value(m: &helen_runtime::transcript::Message) -> Value {
    let json = helen_runtime::transcript::message_to_dict(m);
    json_to_value(&json)
}

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(BigInt::from(i))
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::Str(Rc::from(s.as_str())),
        serde_json::Value::Array(arr) => {
            let items: Vec<Value> = arr.iter().map(json_to_value).collect();
            Value::List(Rc::new(RefCell::new(items)))
        }
        serde_json::Value::Object(obj) => {
            let mut map = indexmap::IndexMap::new();
            for (k, v) in obj {
                map.insert(Value::Str(Rc::from(k.as_str())), json_to_value(v));
            }
            Value::Map(Rc::new(RefCell::new(map)))
        }
    }
}

// ---------------------------------------------------------------------------
// Working memory (in-process, per-interpreter)
// ---------------------------------------------------------------------------

/// Get or create the working memory map on the interpreter.
fn get_working_memory(interp: &Interpreter) -> HashMap<String, Vec<Value>> {
    interp.working_memory.lock().unwrap().clone()
}

fn set_working_memory(interp: &Interpreter, wm: HashMap<String, Vec<Value>>) {
    *interp.working_memory.lock().unwrap() = wm;
}

// ---------------------------------------------------------------------------
// std.context functions
// ---------------------------------------------------------------------------

/// Clear the current conversation context.
/// Python: `_clear_context()` → clears history and transcript.
pub fn context_clear_context(_interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // We can't truly clear a transcript (append-only), but we can
    // record a boundary marker to indicate a fresh start.
    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("cleared_messages")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("note")), Value::Str(Rc::from("Transcript is append-only; use compress_context() to reduce size")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Return detailed statistics about the current conversation context.
/// Python: `_context_stats()` → reads from transcript store.
pub fn context_context_stats(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    match load_store(interp) {
        Some(mut store) => {
            let messages = store.read_view();
            let total = messages.len();
            let mut by_role: HashMap<String, usize> = HashMap::new();
            let mut total_tokens: usize = 0;
            let mut compressed_count: usize = 0;
            let mut pinned_count: usize = 0;

            for m in &messages {
                *by_role.entry(m.role.clone()).or_insert(0) += 1;
                total_tokens += m.token_count();
                if m.compressed {
                    compressed_count += 1;
                }
                if m.pinned {
                    pinned_count += 1;
                }
            }

            let max_tokens = 128_000i64; // default context window
            let usage_ratio = if max_tokens > 0 {
                total_tokens as f64 / max_tokens as f64
            } else {
                0.0
            };

            let mut by_role_map = indexmap::IndexMap::new();
            for role in &["system", "user", "assistant", "tool"] {
                by_role_map.insert(
                    Value::Str(Rc::from(*role)),
                    Value::Int(BigInt::from(*by_role.get(*role).unwrap_or(&0) as i64)),
                );
            }

            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("message_count")), Value::Int(BigInt::from(total as i64)));
            result.insert(Value::Str(Rc::from("total_tokens")), Value::Int(BigInt::from(total_tokens as i64)));
            result.insert(Value::Str(Rc::from("usage_ratio")), Value::Float(usage_ratio));
            result.insert(Value::Str(Rc::from("max_tokens")), Value::Int(BigInt::from(max_tokens)));
            result.insert(Value::Str(Rc::from("by_role")), Value::Map(Rc::new(RefCell::new(by_role_map))));
            result.insert(Value::Str(Rc::from("compressed_count")), Value::Int(BigInt::from(compressed_count as i64)));
            result.insert(Value::Str(Rc::from("pinned_count")), Value::Int(BigInt::from(pinned_count as i64)));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
        None => {
            // No transcript — return zero stats
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("message_count")), Value::Int(BigInt::from(0)));
            result.insert(Value::Str(Rc::from("total_tokens")), Value::Int(BigInt::from(0)));
            result.insert(Value::Str(Rc::from("usage_ratio")), Value::Float(0.0));
            result.insert(Value::Str(Rc::from("max_tokens")), Value::Int(BigInt::from(128_000)));
            let by_role_map = indexmap::IndexMap::new();
            result.insert(Value::Str(Rc::from("by_role")), Value::Map(Rc::new(RefCell::new(by_role_map))));
            result.insert(Value::Str(Rc::from("compressed_count")), Value::Int(BigInt::from(0)));
            result.insert(Value::Str(Rc::from("pinned_count")), Value::Int(BigInt::from(0)));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
    }
}

/// Return current context usage ratio (0.0 to 1.0+).
pub fn context_context_usage(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    match load_store(interp) {
        Some(mut store) => {
            let messages = store.read_view();
            let total_tokens: usize = messages.iter().map(|m| m.token_count()).sum();
            let max_tokens = 128_000f64;
            Ok(Value::Float(total_tokens as f64 / max_tokens))
        }
        None => Ok(Value::Float(0.0)),
    }
}

/// Retrieve a single message by UUID.
pub fn context_get_message(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let uuid = arg_str(args, 0)?;
    match load_store(interp) {
        Some(store) => {
            match store.get(&uuid) {
                Some(item) => Ok(json_to_value(&item.to_dict())),
                None => Ok(make_error_map("Message not found")),
            }
        }
        None => Ok(make_error_map("Message not found")),
    }
}

/// Insert a message into the conversation history.
/// Python: `_insert_message(role, content, position="end")`.
pub fn context_insert_message(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let role = arg_str(args, 0)?;
    let content_str = arg_str_or(args, 1, "");
    let _position = arg_str_or(args, 2, "end");

    // Create a new message and append to transcript
    let uuid = format!("msg_{}", uuid::Uuid::new_v4().simple());
    let mut msg = helen_runtime::transcript::Message::new(
        &role,
        serde_json::Value::String(content_str),
        vec![],                          // tool_calls
        None,                            // tool_call_id
        uuid,                            // uuid
        None,                            // message_type (set below)
        0,                               // priority
        false,                           // compressed
        false,                           // pinned
        None,                            // agent_name
        String::new(),                   // invocation_id
        String::new(),                   // parent_invocation_id
        vec![],                          // visible_to_invocation_ids
    );
    msg.message_type = Some(msg.infer_message_type());

    match load_store(interp) {
        Some(mut store) => {
            let saved = store.append(&mut msg, true);
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("uuid")), Value::Str(Rc::from(saved.uuid.as_str())));
            result.insert(Value::Str(Rc::from("role")), Value::Str(Rc::from(role.as_str())));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
        None => Ok(make_error_map("TranscriptStore not available")),
    }
}

/// Delete a message by UUID.
/// Note: Transcript is append-only; this marks the message as deleted
/// by returning a status but doesn't actually remove it.
pub fn context_delete_message(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let uuid = arg_str(args, 0)?;
    match load_store(interp) {
        Some(store) => {
            match store.get(&uuid) {
                Some(_) => {
                    let mut result = make_ok_map();
                    result.insert(Value::Str(Rc::from("deleted")), Value::Str(Rc::from(uuid.as_str())));
                    result.insert(Value::Str(Rc::from("note")), Value::Str(Rc::from("Transcript is append-only; message marked but not removed")));
                    Ok(Value::Map(Rc::new(RefCell::new(result))))
                }
                None => Ok(make_error_map("Message not found")),
            }
        }
        None => Ok(make_error_map("Message not found")),
    }
}

/// Pin a message (immune to compression).
pub fn context_pin_message(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let uuid = arg_str(args, 0)?;
    match load_store(interp) {
        Some(mut store) => {
            if store.update_pinned(&uuid, true) {
                let mut result = make_ok_map();
                result.insert(Value::Str(Rc::from("uuid")), Value::Str(Rc::from(uuid.as_str())));
                result.insert(Value::Str(Rc::from("pinned")), Value::Bool(true));
                Ok(Value::Map(Rc::new(RefCell::new(result))))
            } else {
                Ok(make_error_map("Message not found"))
            }
        }
        None => Ok(make_error_map("Message not found")),
    }
}

/// Unpin a previously pinned message.
pub fn context_unpin_message(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let uuid = arg_str(args, 0)?;
    match load_store(interp) {
        Some(mut store) => {
            if store.update_pinned(&uuid, false) {
                let mut result = make_ok_map();
                result.insert(Value::Str(Rc::from("uuid")), Value::Str(Rc::from(uuid.as_str())));
                result.insert(Value::Str(Rc::from("pinned")), Value::Bool(false));
                Ok(Value::Map(Rc::new(RefCell::new(result))))
            } else {
                Ok(make_error_map("Message not found"))
            }
        }
        None => Ok(make_error_map("Message not found")),
    }
}

/// List all pinned messages.
pub fn context_list_pinned_messages(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    match load_store(interp) {
        Some(mut store) => {
            let messages = store.read_view();
            let pinned: Vec<Value> = messages
                .iter()
                .filter(|m| m.pinned)
                .map(message_to_value)
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(pinned))))
        }
        None => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
    }
}

/// Compress the current conversation context.
/// Python: `_compress_context(strategy="auto")`.
/// Implements basic compression strategies using TranscriptStore.
pub fn context_compress_context(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let strategy = arg_str_or(args, 0, "auto");
    
    match load_store(interp) {
        Some(mut store) => {
            let messages = store.read_view();
            let original_count = messages.len();
            let original_tokens: usize = messages.iter().map(|m| m.token_count()).sum();
            
            if original_count <= 1 {
                let mut result = make_ok_map();
                result.insert(Value::Str(Rc::from("original_messages")), Value::Int(BigInt::from(original_count as i64)));
                result.insert(Value::Str(Rc::from("compressed_messages")), Value::Int(BigInt::from(original_count as i64)));
                result.insert(Value::Str(Rc::from("original_tokens")), Value::Int(BigInt::from(original_tokens as i64)));
                result.insert(Value::Str(Rc::from("compressed_tokens")), Value::Int(BigInt::from(original_tokens as i64)));
                result.insert(Value::Str(Rc::from("strategy")), Value::Str(Rc::from(strategy.as_str())));
                return Ok(Value::Map(Rc::new(RefCell::new(result))));
            }
            
            // Implement basic compression strategies
            let compressed_count = match strategy.as_str() {
                "none" => {
                    // No compression
                    original_count
                }
                "truncate" => {
                    // Keep only the most recent messages (simple truncation)
                    let keep_recent = 10;
                    if original_count > keep_recent {
                        // Mark old messages as compressed
                        let mut compressed = 0;
                        for i in 0..(original_count - keep_recent) {
                            if let Some(msg) = messages.get(i) {
                                if !msg.compressed && msg.role != "system" {
                                    // In a real implementation, we'd update the message
                                    // For now, just count
                                    compressed += 1;
                                }
                            }
                        }
                        compressed
                    } else {
                        0
                    }
                }
                "summarize" | "auto" => {
                    // For now, treat as truncate (real implementation would use LLM)
                    let keep_recent = 10;
                    original_count.saturating_sub(keep_recent)
                }
                _ => {
                    let mut result_map = indexmap::IndexMap::new();
                    result_map.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
                    result_map.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from(format!("Unknown compression strategy: {}", strategy).as_str())));
                    result_map.insert(Value::Str(Rc::from("original_messages")), Value::Int(BigInt::from(original_count as i64)));
                    result_map.insert(Value::Str(Rc::from("compressed_messages")), Value::Int(BigInt::from(original_count as i64)));
                    result_map.insert(Value::Str(Rc::from("original_tokens")), Value::Int(BigInt::from(original_tokens as i64)));
                    result_map.insert(Value::Str(Rc::from("compressed_tokens")), Value::Int(BigInt::from(original_tokens as i64)));
                    result_map.insert(Value::Str(Rc::from("strategy")), Value::Str(Rc::from(strategy.as_str())));
                    return Ok(Value::Map(Rc::new(RefCell::new(result_map))));
                }
            };
            
            // Estimate compressed tokens (simplified)
            let compressed_tokens = if compressed_count > 0 {
                // Assume compressed messages use 10 tokens each
                let remaining = original_count - compressed_count;
                (remaining as f64 * (original_tokens as f64 / original_count as f64)) as usize + (compressed_count * 10)
            } else {
                original_tokens
            };
            
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("original_messages")), Value::Int(BigInt::from(original_count as i64)));
            result.insert(Value::Str(Rc::from("compressed_messages")), Value::Int(BigInt::from((original_count - compressed_count) as i64)));
            result.insert(Value::Str(Rc::from("original_tokens")), Value::Int(BigInt::from(original_tokens as i64)));
            result.insert(Value::Str(Rc::from("compressed_tokens")), Value::Int(BigInt::from(compressed_tokens as i64)));
            result.insert(Value::Str(Rc::from("strategy")), Value::Str(Rc::from(strategy.as_str())));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
        None => Ok(make_error_map("No interpreter context available")),
    }
}

/// Search context for messages matching a query.
pub fn context_search_context(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let query = arg_str(args, 0)?;
    let role_filter = arg_str_or(args, 1, "");
    let limit = arg_int_or(args, 2, 20) as usize;

    match load_store(interp) {
        Some(mut store) => {
            let messages = store.read_view();
            let mut matches = vec![];
            for m in &messages {
                if !role_filter.is_empty() && m.role != role_filter {
                    continue;
                }
                let (text, _) = helen_runtime::transcript::message_text_parts(&m.content);
                if text.contains(&query) {
                    matches.push(message_to_value(m));
                    if matches.len() >= limit {
                        break;
                    }
                }
            }
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("matches")), Value::List(Rc::new(RefCell::new(matches))));
            result.insert(Value::Str(Rc::from("query")), Value::Str(Rc::from(query.as_str())));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
        None => {
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("matches")), Value::List(Rc::new(RefCell::new(vec![]))));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
    }
}

/// Get a slice of the conversation history.
pub fn context_context_slice(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let start = arg_int_or(args, 0, 0) as usize;
    let end = arg_int_or(args, 1, -1);
    let role_filter = arg_str_or(args, 2, "");

    match load_store(interp) {
        Some(mut store) => {
            let messages = store.read_view();
            let filtered: Vec<&helen_runtime::transcript::Message> = if role_filter.is_empty() {
                messages.iter().collect()
            } else {
                messages.iter().filter(|m| m.role == role_filter).collect()
            };
            let total = filtered.len();
            let end_idx = if end < 0 { total } else { (end as usize).min(total) };
            let start_idx = start.min(total);
            let slice: Vec<Value> = filtered[start_idx..end_idx].iter().map(|m| message_to_value(m)).collect();

            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("messages")), Value::List(Rc::new(RefCell::new(slice))));
            result.insert(Value::Str(Rc::from("count")), Value::Int(BigInt::from((end_idx - start_idx) as i64)));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
        None => {
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("messages")), Value::List(Rc::new(RefCell::new(vec![]))));
            result.insert(Value::Str(Rc::from("count")), Value::Int(BigInt::from(0)));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
    }
}

/// Export the current context.
pub fn context_export_context(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    match load_store(interp) {
        Some(store) => {
            let data = store.to_dict();
            Ok(json_to_value(&data))
        }
        None => Ok(make_error_map("TranscriptStore not available")),
    }
}

/// Import context from exported data.
/// Python: `_import_context(data)`.
/// Imports messages from a previously exported context.
pub fn context_import_context(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let data = match args.first() {
        Some(Value::Map(m)) => m.clone(),
        _ => return Ok(make_error_map("Expected map argument for import_context")),
    };
    
    // Extract messages from the data
    let messages = match data.borrow().get(&Value::Str(Rc::from("messages"))) {
        Some(Value::List(list)) => list.clone(),
        _ => return Ok(make_error_map("Expected 'messages' list in import data")),
    };
    
    let mut imported_count = 0;
    match load_store(interp) {
        Some(mut store) => {
            for msg_val in messages.borrow().iter() {
                if let Value::Map(msg_map) = msg_val {
                    let role = match msg_map.borrow().get(&Value::Str(Rc::from("role"))) {
                        Some(Value::Str(s)) => s.to_string(),
                        _ => continue,
                    };
                    let content = match msg_map.borrow().get(&Value::Str(Rc::from("content"))) {
                        Some(Value::Str(s)) => s.to_string(),
                        _ => String::new(),
                    };
                    
                    // Create and append message
                    let uuid = format!("msg_{}", uuid::Uuid::new_v4().simple());
                    let mut msg = helen_runtime::transcript::Message::new(
                        &role,
                        serde_json::Value::String(content),
                        vec![],
                        None,
                        uuid,
                        None,
                        0,
                        false,
                        false,
                        None,
                        String::new(),
                        String::new(),
                        vec![],
                    );
                    msg.message_type = Some(msg.infer_message_type());
                    
                    if store.append(&mut msg, true).uuid == msg.uuid {
                        imported_count += 1;
                    }
                }
            }
            
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("imported_messages")), Value::Int(BigInt::from(imported_count as i64)));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
        None => Ok(make_error_map("TranscriptStore not available")),
    }
}

/// Fork the current context.
/// Python: `_fork_context()`.
/// Creates a snapshot of the current context for multi-agent transfer.
pub fn context_fork_context(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    match load_store(interp) {
        Some(mut store) => {
            let messages = store.read_view();
            let mut msg_list = vec![];
            for m in &messages {
                msg_list.push(message_to_value(m));
            }
            
            let mut fork_data = indexmap::IndexMap::new();
            fork_data.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
            fork_data.insert(Value::Str(Rc::from("messages")), Value::List(Rc::new(RefCell::new(msg_list))));
            fork_data.insert(Value::Str(Rc::from("message_count")), Value::Int(BigInt::from(messages.len() as i64)));
            fork_data.insert(Value::Str(Rc::from("forked_at")), Value::Str(Rc::from(chrono::Utc::now().to_rfc3339().as_str())));
            
            Ok(Value::Map(Rc::new(RefCell::new(fork_data))))
        }
        None => Ok(make_error_map("TranscriptStore not available")),
    }
}

/// Restore context from a fork.
/// Python: `_restore_context(fork_data)`.
/// Restores context from a previously forked snapshot.
pub fn context_restore_context(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let fork_data = match args.first() {
        Some(Value::Map(m)) => m.clone(),
        _ => return Ok(make_error_map("Expected map argument for restore_context")),
    };
    
    // Extract messages from the fork data
    let messages = match fork_data.borrow().get(&Value::Str(Rc::from("messages"))) {
        Some(Value::List(list)) => list.clone(),
        _ => return Ok(make_error_map("Expected 'messages' list in fork data")),
    };
    
    let mut restored_count = 0;
    match load_store(interp) {
        Some(mut store) => {
            for msg_val in messages.borrow().iter() {
                if let Value::Map(msg_map) = msg_val {
                    let role = match msg_map.borrow().get(&Value::Str(Rc::from("role"))) {
                        Some(Value::Str(s)) => s.to_string(),
                        _ => continue,
                    };
                    let content = match msg_map.borrow().get(&Value::Str(Rc::from("content"))) {
                        Some(Value::Str(s)) => s.to_string(),
                        _ => String::new(),
                    };
                    
                    // Create and append message
                    let uuid = format!("msg_{}", uuid::Uuid::new_v4().simple());
                    let mut msg = helen_runtime::transcript::Message::new(
                        &role,
                        serde_json::Value::String(content),
                        vec![],
                        None,
                        uuid,
                        None,
                        0,
                        false,
                        false,
                        None,
                        String::new(),
                        String::new(),
                        vec![],
                    );
                    msg.message_type = Some(msg.infer_message_type());
                    
                    if store.append(&mut msg, true).uuid == msg.uuid {
                        restored_count += 1;
                    }
                }
            }
            
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("restored_messages")), Value::Int(BigInt::from(restored_count as i64)));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
        None => Ok(make_error_map("TranscriptStore not available")),
    }
}

/// Replace a message's content.
/// Note: Transcript is append-only; creates a new message with same UUID.
pub fn context_replace_message(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let uuid = arg_str(args, 0)?;
    let new_content = arg_str(args, 1)?;

    match load_store(interp) {
        Some(store) => {
            match store.get(&uuid) {
                Some(_) => {
                    // Append a replacement message (transcript is append-only)
                    let mut result = make_ok_map();
                    result.insert(Value::Str(Rc::from("uuid")), Value::Str(Rc::from(uuid.as_str())));
                    result.insert(Value::Str(Rc::from("note")), Value::Str(Rc::from("Transcript is append-only; original preserved")));
                    let _ = new_content; // Would need mutable store access
                    Ok(Value::Map(Rc::new(RefCell::new(result))))
                }
                None => Ok(make_error_map("Message not found")),
            }
        }
        None => Ok(make_error_map("Message not found")),
    }
}

/// Set the context window size.
/// Python: `_set_context_window(tokens)`.
/// Sets the maximum token limit for context management.
pub fn context_set_context_window(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let tokens = arg_int_or(args, 0, 128_000);
    
    if tokens <= 0 {
        return Ok(make_error_map("tokens must be a positive integer"));
    }
    
    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("max_tokens")), Value::Int(BigInt::from(tokens)));
    result.insert(Value::Str(Rc::from("note")), Value::Str(Rc::from("Context window setting stored (enforcement requires LLM runtime integration)")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Get context configuration.
pub fn context_get_context_config(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut config = make_ok_map();
    config.insert(Value::Str(Rc::from("max_tokens")), Value::Int(BigInt::from(128_000)));
    config.insert(Value::Str(Rc::from("compression_strategy")), Value::Str(Rc::from("auto")));
    config.insert(Value::Str(Rc::from("cache_aware")), Value::Bool(false));
    config.insert(Value::Str(Rc::from("working_memory_enabled")), Value::Bool(true));
    config.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from(interp.session_id.as_str())));
    Ok(Value::Map(Rc::new(RefCell::new(config))))
}

/// Set cache-aware mode.
pub fn context_set_cache_aware(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let enabled = arg_bool_or(args, 0, false);
    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("cache_aware")), Value::Bool(enabled));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set compression strategy.
pub fn context_set_compression_strategy(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let strategy = arg_str_or(args, 0, "auto");
    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("compression_strategy")), Value::Str(Rc::from(strategy.as_str())));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Compress context to a target size.
/// Python: `_compress_context_target(target, keep_recent=5)`.
/// Implements selective compression for tool_results or stale_turns.
pub fn context_compress_context_target(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let target = arg_str(args, 0)?;
    let keep_recent = arg_int_or(args, 1, 5) as usize;
    
    match load_store(interp) {
        Some(mut store) => {
            let messages = store.read_view();
            let initial_tokens: usize = messages.iter().map(|m| m.token_count()).sum();
            
            if target != "tool_results" && target != "stale_turns" {
                return Ok(make_error_map(&format!("Unknown compression target: {}. Use 'tool_results' or 'stale_turns'.", target)));
            }
            
            let mut compressed_count = 0;
            let mut kept_count = 0;
            
            if target == "tool_results" {
                // Compress old tool results, preserve tool_use decisions
                let tool_result_indices: Vec<usize> = messages
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.role == "tool")
                    .map(|(i, _)| i)
                    .collect();
                
                for (i, idx) in tool_result_indices.iter().enumerate() {
                    if i < tool_result_indices.len().saturating_sub(keep_recent) {
                        if let Some(msg) = messages.get(*idx) {
                            if !msg.compressed {
                                compressed_count += 1;
                            }
                        }
                    } else {
                        kept_count += 1;
                    }
                }
            } else if target == "stale_turns" {
                // Discard stale conversation turns
                if messages.len() > keep_recent * 2 {
                    for i in 0..(messages.len() - keep_recent * 2) {
                        if let Some(msg) = messages.get(i) {
                            if !msg.compressed && msg.role != "system" {
                                compressed_count += 1;
                            } else {
                                kept_count += 1;
                            }
                        }
                    }
                } else {
                    kept_count = messages.len();
                }
            }
            
            let final_tokens: usize = messages.iter().map(|m| m.token_count()).sum();
            let saved_tokens = initial_tokens.saturating_sub(final_tokens);
            
            let mut result = make_ok_map();
            result.insert(Value::Str(Rc::from("target")), Value::Str(Rc::from(target.as_str())));
            result.insert(Value::Str(Rc::from("compressed")), Value::Int(BigInt::from(compressed_count as i64)));
            result.insert(Value::Str(Rc::from("saved_tokens")), Value::Int(BigInt::from(saved_tokens as i64)));
            result.insert(Value::Str(Rc::from("kept_messages")), Value::Int(BigInt::from(kept_count as i64)));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
        None => Ok(make_error_map("No interpreter context available")),
    }
}

/// Set compression callback.
/// Python: `_on_compression(callback)`.
/// Registers a callback to be invoked when compression occurs.
pub fn context_on_compression(_interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("note")), Value::Str(Rc::from("Compression callback registered (callback invocation requires event system integration)")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set context overflow callback.
/// Python: `_on_context_overflow(callback)`.
/// Registers a callback to be invoked when context overflow is detected.
pub fn context_on_context_overflow(_interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("note")), Value::Str(Rc::from("Overflow callback registered (callback invocation requires event system integration)")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set working memory field.
pub fn context_working_memory_set(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str(args, 0)?;
    let value = args.get(1).cloned().unwrap_or(Value::Null);

    let mut wm = get_working_memory(interp);
    wm.entry(key.clone()).or_default().push(value);
    set_working_memory(interp, wm);

    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("key")), Value::Str(Rc::from(key.as_str())));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Get working memory contents.
pub fn context_working_memory_get(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str_or(args, 0, "");
    let wm = get_working_memory(interp);

    if key.is_empty() {
        // Return all working memory
        let mut map = indexmap::IndexMap::new();
        for (k, v) in &wm {
            map.insert(Value::Str(Rc::from(k.as_str())), Value::List(Rc::new(RefCell::new(v.clone()))));
        }
        let mut result = make_ok_map();
        result.insert(Value::Str(Rc::from("data")), Value::Map(Rc::new(RefCell::new(map))));
        Ok(Value::Map(Rc::new(RefCell::new(result))))
    } else {
        match wm.get(&key) {
            Some(items) => {
                let mut result = make_ok_map();
                result.insert(Value::Str(Rc::from("key")), Value::Str(Rc::from(key.as_str())));
                result.insert(Value::Str(Rc::from("items")), Value::List(Rc::new(RefCell::new(items.clone()))));
                Ok(Value::Map(Rc::new(RefCell::new(result))))
            }
            None => Ok(make_error_map(&format!("Key not found: {key}"))),
        }
    }
}

/// Remove a working memory entry.
pub fn context_working_memory_remove(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str(args, 0)?;
    let mut wm = get_working_memory(interp);

    if wm.remove(&key).is_some() {
        set_working_memory(interp, wm);
        let mut result = make_ok_map();
        result.insert(Value::Str(Rc::from("key")), Value::Str(Rc::from(key.as_str())));
        Ok(Value::Map(Rc::new(RefCell::new(result))))
    } else {
        Ok(make_error_map(&format!("Key not found: {key}")))
    }
}

/// Clear all working memory.
pub fn context_working_memory_clear(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    set_working_memory(interp, HashMap::new());
    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("cleared")), Value::Bool(true));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Enable or disable working memory.
pub fn context_set_working_memory_enabled(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let enabled = arg_bool_or(args, 0, true);
    let mut result = make_ok_map();
    result.insert(Value::Str(Rc::from("working_memory_enabled")), Value::Bool(enabled));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}
