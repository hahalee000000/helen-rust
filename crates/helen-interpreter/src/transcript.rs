//! Transcript and session stdlib functions.
//!
//! Byte-faithful port of `helen/stdlib/transcript.py` (v1.44.0): provides
//! transcript query and session management functions.
//!
//! These functions access the interpreter's `session_manager` and
//! `session_id` fields to interact with the transcript store.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use helen_runtime::transcript::{JsonlBackend, TranscriptStore};

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn arg_str_or<'a>(args: &'a [Value], i: usize, default: &'a str) -> &'a str {
    match args.get(i) {
        Some(Value::Str(s)) => s.as_ref(),
        _ => default,
    }
}

fn arg_bool_or(args: &[Value], i: usize, default: bool) -> bool {
    match args.get(i) {
        Some(Value::Bool(b)) => *b,
        _ => default,
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

/// Load a TranscriptStore for the given session_id (or current session).
fn load_store(interp: &Interpreter, session_id: &str) -> Option<(TranscriptStore, PathBuf)> {
    let sid = if session_id.is_empty() {
        &interp.session_id
    } else {
        session_id
    };
    if sid.is_empty() {
        return None;
    }
    let manager = interp.session_manager.lock().unwrap();
    let path = manager.get_session_path(sid);
    drop(manager);
    if !path.exists() {
        return None;
    }
    let backend = JsonlBackend::new(&path);
    let store = TranscriptStore::load_from_backend(backend, 1000);
    Some((store, path))
}

/// Convert a serde_json::Value to a Helen Value.
fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(num_bigint::BigInt::from(i))
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
// std.transcript functions
// ---------------------------------------------------------------------------

/// Get current session ID.
/// Python: `get_session_id()` → returns `agent_ctx.session_id` or `""`.
pub fn transcript_get_session_id(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    if interp.session_id.is_empty() {
        // Lazy init: create a new session via the manager (Python
        // `_init_transcript_store(None)` -> `SessionManager.create_session`).
        let mgr = interp.session_manager.lock().unwrap();
        let new_id = mgr.create_session(None);
        drop(mgr);
        interp.session_id = new_id.clone();
        Ok(Value::Str(Rc::from(new_id.as_str())))
    } else {
        Ok(Value::Str(Rc::from(interp.session_id.as_str())))
    }
}

/// Get session metadata.
/// Python: `get_session_meta(session_id="")` → reads meta from transcript.
pub fn transcript_get_session_meta(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    match load_store(interp, sid) {
        Some((store, _)) => {
            match store.read_meta() {
                Some(meta) => {
                    let mut data = indexmap::IndexMap::new();
                    data.insert(Value::Str(Rc::from("argv")), json_to_value(&serde_json::to_value(&meta.argv).unwrap_or(serde_json::Value::Array(vec![]))));
                    data.insert(Value::Str(Rc::from("timestamp")), Value::Float(meta.timestamp));
                    data.insert(Value::Str(Rc::from("helen_version")), Value::Str(Rc::from(meta.helen_version.as_str())));
                    data.insert(Value::Str(Rc::from("platform")), Value::Str(Rc::from(meta.platform.as_str())));
                    data.insert(Value::Str(Rc::from("cwd")), Value::Str(Rc::from(meta.cwd.as_str())));
                    data.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from(meta.session_id.as_str())));
                    data.insert(Value::Str(Rc::from("session_scope")), Value::Str(Rc::from(meta.session_scope.as_str())));
                    let mut result = indexmap::IndexMap::new();
                    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
                    result.insert(Value::Str(Rc::from("data")), Value::Map(Rc::new(RefCell::new(data))));
                    Ok(Value::Map(Rc::new(RefCell::new(result))))
                }
                None => {
                    let mut result = indexmap::IndexMap::new();
                    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
                    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No session metadata available")));
                    Ok(Value::Map(Rc::new(RefCell::new(result))))
                }
            }
        }
        None => {
            let mut result = indexmap::IndexMap::new();
            result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
            result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("TranscriptStore not enabled")));
            Ok(Value::Map(Rc::new(RefCell::new(result))))
        }
    }
}

/// List all transcript sessions.
/// Python: `list_sessions(scope="")` → uses SessionManager.list_sessions().
pub fn transcript_list_sessions(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _scope = arg_str_or(args, 0, "");
    let manager = interp.session_manager.lock().unwrap();
    let sessions = manager.list_sessions();
    let items: Vec<Value> = sessions
        .iter()
        .map(|s| {
            let mut map = indexmap::IndexMap::new();
            map.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from(s.session_id.as_str())));
            map.insert(Value::Str(Rc::from("created_at")), Value::Float(s.created_at));
            map.insert(Value::Str(Rc::from("modified_at")), Value::Float(s.modified_at));
            map.insert(Value::Str(Rc::from("size_bytes")), Value::Int(num_bigint::BigInt::from(s.size_bytes as i64)));
            map.insert(Value::Str(Rc::from("message_count")), Value::Int(num_bigint::BigInt::from(s.message_count as i64)));
            Value::Map(Rc::new(RefCell::new(map)))
        })
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

/// Get spawned sessions (children of a given session).
/// Python: `get_spawned_sessions(session_id="")` → searches for parent_session_id matches.
pub fn transcript_get_spawned_sessions(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    let target_sid = if sid.is_empty() {
        interp.session_id.clone()
    } else {
        sid.to_string()
    };
    if target_sid.is_empty() {
        return Ok(Value::List(Rc::new(RefCell::new(vec![]))));
    }

    let manager = interp.session_manager.lock().unwrap();
    let all_sessions = manager.list_sessions();
    let mut results = vec![];

    for s in &all_sessions {
        let path = manager.get_session_path(&s.session_id);
        if !path.exists() {
            continue;
        }
        let backend = JsonlBackend::new(&path);
        let store = TranscriptStore::load_from_backend(backend, 100);
        if let Some(meta) = store.read_meta() {
            if meta.parent_session_id == target_sid {
                let mut map = indexmap::IndexMap::new();
                map.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from(s.session_id.as_str())));
                map.insert(Value::Str(Rc::from("parent_session_id")), Value::Str(Rc::from(meta.parent_session_id.as_str())));
                map.insert(Value::Str(Rc::from("timestamp")), Value::Float(meta.timestamp));
                results.push(Value::Map(Rc::new(RefCell::new(map))));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(results))))
}

/// Get spawn tree (recursive).
/// Python: `get_spawn_tree(session_id="")` → builds tree recursively.
pub fn transcript_get_spawn_tree(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    let target_sid = if sid.is_empty() {
        interp.session_id.clone()
    } else {
        sid.to_string()
    };
    if target_sid.is_empty() {
        let mut map = indexmap::IndexMap::new();
        map.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from("")));
        map.insert(Value::Str(Rc::from("children")), Value::List(Rc::new(RefCell::new(vec![]))));
        return Ok(Value::Map(Rc::new(RefCell::new(map))));
    }

    // Build tree by collecting all sessions and their parent relationships
    let manager = interp.session_manager.lock().unwrap();
    let all_sessions = manager.list_sessions();
    let mut parent_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    for s in &all_sessions {
        let path = manager.get_session_path(&s.session_id);
        if !path.exists() {
            continue;
        }
        let backend = JsonlBackend::new(&path);
        let store = TranscriptStore::load_from_backend(backend, 100);
        if let Some(meta) = store.read_meta() {
            if !meta.parent_session_id.is_empty() {
                parent_map
                    .entry(meta.parent_session_id.clone())
                    .or_default()
                    .push(s.session_id.clone());
            }
        }
    }
    drop(manager);

    fn build_tree(sid: &str, parent_map: &std::collections::HashMap<String, Vec<String>>) -> Value {
        let children_sids = parent_map.get(sid).cloned().unwrap_or_default();
        let children: Vec<Value> = children_sids
            .iter()
            .map(|child_sid| build_tree(child_sid, parent_map))
            .collect();
        let mut map = indexmap::IndexMap::new();
        map.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from(sid)));
        map.insert(Value::Str(Rc::from("children")), Value::List(Rc::new(RefCell::new(children))));
        Value::Map(Rc::new(RefCell::new(map)))
    }

    Ok(build_tree(&target_sid, &parent_map))
}

/// Query transcript messages.
/// Python: `query_transcript(session_id="", role="", limit=0)` → uses store.query().
pub fn transcript_query_transcript(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    let role = arg_str_or(args, 1, "");
    let limit = arg_int_or(args, 2, 0) as usize;

    match load_store(interp, sid) {
        Some((mut store, _)) => {
            let messages = if role.is_empty() {
                store.read_view()
            } else {
                store.read_view().into_iter().filter(|m| m.role == role).collect()
            };
            let messages = if limit > 0 && messages.len() > limit {
                messages[..limit].to_vec()
            } else {
                messages
            };
            let items: Vec<Value> = messages
                .iter()
                .map(|m| json_to_value(&helen_runtime::transcript::message_to_dict(m)))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        None => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
    }
}

/// Search transcript by content.
/// Python: `search_transcript(query, role="", limit=20)` → filters messages.
pub fn transcript_search_transcript(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let query = arg_str_or(args, 0, "");
    let role = arg_str_or(args, 1, "");
    let limit = arg_int_or(args, 2, 20) as usize;

    if query.is_empty() {
        return Ok(Value::List(Rc::new(RefCell::new(vec![]))));
    }

    match load_store(interp, "") {
        Some((mut store, _)) => {
            let messages = store.read_view();
            let mut results = vec![];
            for m in &messages {
                if !role.is_empty() && m.role != role {
                    continue;
                }
                let (text, _) = helen_runtime::transcript::message_text_parts(&m.content);
                if text.contains(query) {
                    results.push(json_to_value(&helen_runtime::transcript::message_to_dict(m)));
                    if results.len() >= limit {
                        break;
                    }
                }
            }
            Ok(Value::List(Rc::new(RefCell::new(results))))
        }
        None => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
    }
}

/// List invocations (LLM calls) in a session.
/// Python: `list_invocations(session_id=None, agent=None, limit=50)`.
pub fn transcript_list_invocations(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    let _agent = arg_str_or(args, 1, "");
    let limit = arg_int_or(args, 2, 50) as usize;

    match load_store(interp, sid) {
        Some((mut store, _)) => {
            let messages = store.read_view();
            let mut invocations = vec![];
            for m in &messages {
                if m.role == "assistant" || m.role == "tool" {
                    let msg_type = m.infer_message_type();
                    if msg_type == "llm_response" || msg_type == "tool_result" {
                        let mut map = indexmap::IndexMap::new();
                        map.insert(Value::Str(Rc::from("uuid")), Value::Str(Rc::from(m.uuid.as_str())));
                        map.insert(Value::Str(Rc::from("role")), Value::Str(Rc::from(m.role.as_str())));
                        map.insert(Value::Str(Rc::from("message_type")), Value::Str(Rc::from(msg_type.as_str())));
                        invocations.push(Value::Map(Rc::new(RefCell::new(map))));
                        if invocations.len() >= limit {
                            break;
                        }
                    }
                }
            }
            Ok(Value::List(Rc::new(RefCell::new(invocations))))
        }
        None => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
    }
}

/// Get a specific invocation by UUID.
pub fn transcript_get_invocation(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let uuid = arg_str_or(args, 0, "");
    if uuid.is_empty() {
        return Ok(Value::Null);
    }
    match load_store(interp, "") {
        Some((store, _)) => {
            match store.get(uuid) {
                Some(item) => Ok(json_to_value(&item.to_dict())),
                None => Ok(Value::Null),
            }
        }
        None => Ok(Value::Null),
    }
}

/// Get invocation tree (agent call hierarchy within a session).
/// Simplified: returns a flat list of invocations with parent references.
pub fn transcript_get_invocation_tree(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    match load_store(interp, sid) {
        Some((mut store, _)) => {
            let messages = store.read_view();
            let mut invocations = vec![];
            for m in &messages {
                if m.role == "assistant" {
                    let mut map = indexmap::IndexMap::new();
                    map.insert(Value::Str(Rc::from("uuid")), Value::Str(Rc::from(m.uuid.as_str())));
                    map.insert(Value::Str(Rc::from("role")), Value::Str(Rc::from(m.role.as_str())));
                    map.insert(Value::Str(Rc::from("children")), Value::List(Rc::new(RefCell::new(vec![]))));
                    invocations.push(Value::Map(Rc::new(RefCell::new(map))));
                }
            }
            let mut tree = indexmap::IndexMap::new();
            tree.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from(interp.session_id.as_str())));
            tree.insert(Value::Str(Rc::from("invocations")), Value::List(Rc::new(RefCell::new(invocations))));
            Ok(Value::Map(Rc::new(RefCell::new(tree))))
        }
        None => {
            let mut tree = indexmap::IndexMap::new();
            tree.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from("")));
            tree.insert(Value::Str(Rc::from("invocations")), Value::List(Rc::new(RefCell::new(vec![]))));
            Ok(Value::Map(Rc::new(RefCell::new(tree))))
        }
    }
}

/// Export transcript to JSON string.
pub fn transcript_export_transcript(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    let include_compressed = arg_bool_or(args, 1, false);

    match load_store(interp, sid) {
        Some((mut store, _)) => {
            let data = if include_compressed {
                store.to_dict()
            } else {
                let messages = store.read_view();
                let items: Vec<serde_json::Value> = messages
                    .iter()
                    .map(|m| helen_runtime::transcript::message_to_dict(m))
                    .collect();
                serde_json::json!({ "messages": items })
            };
            let json_str = serde_json::to_string_pretty(&data).unwrap_or_else(|_| "[]".to_string());
            Ok(Value::Str(Rc::from(json_str.as_str())))
        }
        None => Ok(Value::Str(Rc::from("[]"))),
    }
}

/// Replay transcript messages.
pub fn transcript_replay_transcript(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    let include_compressed = arg_bool_or(args, 1, false);

    match load_store(interp, sid) {
        Some((mut store, _)) => {
            let messages = if include_compressed {
                // Return all items as messages
                store.read_view()
            } else {
                store.read_view()
            };
            let items: Vec<Value> = messages
                .iter()
                .map(|m| json_to_value(&helen_runtime::transcript::message_to_dict(m)))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        None => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
    }
}

/// Replay full session (including compressed).
pub fn transcript_replay_full_session(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    match load_store(interp, sid) {
        Some((mut store, _)) => {
            let messages = store.read_view();
            let items: Vec<Value> = messages
                .iter()
                .map(|m| json_to_value(&helen_runtime::transcript::message_to_dict(m)))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        None => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
    }
}

/// Resume a session (set interpreter's session_id).
pub fn transcript_resume_session(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    if sid.is_empty() {
        let mut result = indexmap::IndexMap::new();
        result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
        result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("session_id is required")));
        return Ok(Value::Map(Rc::new(RefCell::new(result))));
    }

    let manager = interp.session_manager.lock().unwrap();
    if !manager.session_exists(sid) {
        let mut result = indexmap::IndexMap::new();
        result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
        result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from(format!("Session not found: {sid}").as_str())));
        return Ok(Value::Map(Rc::new(RefCell::new(result))));
    }
    drop(manager);

    interp.session_id = sid.to_string();
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
    result.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from(sid)));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Delete current session.
pub fn transcript_delete_current_session(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let confirm = arg_bool_or(args, 0, false);
    if !confirm {
        let mut result = indexmap::IndexMap::new();
        result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
        result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("confirm=true required to delete session")));
        return Ok(Value::Map(Rc::new(RefCell::new(result))));
    }

    let sid = interp.session_id.clone();
    if sid.is_empty() {
        let mut result = indexmap::IndexMap::new();
        result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
        result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No active session")));
        return Ok(Value::Map(Rc::new(RefCell::new(result))));
    }

    let manager = interp.session_manager.lock().unwrap();
    let deleted = manager.delete_session(&sid);
    drop(manager);

    if deleted {
        interp.session_id.clear();
        let mut result = indexmap::IndexMap::new();
        result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
        result.insert(Value::Str(Rc::from("deleted")), Value::Str(Rc::from(sid.as_str())));
        Ok(Value::Map(Rc::new(RefCell::new(result))))
    } else {
        let mut result = indexmap::IndexMap::new();
        result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
        result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Failed to delete session")));
        Ok(Value::Map(Rc::new(RefCell::new(result))))
    }
}

/// Release session lock.
pub fn transcript_release_session_lock(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    let target_sid = if sid.is_empty() {
        interp.session_id.clone()
    } else {
        sid.to_string()
    };
    if target_sid.is_empty() {
        let mut result = indexmap::IndexMap::new();
        result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
        result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No active session")));
        return Ok(Value::Map(Rc::new(RefCell::new(result))));
    }

    let manager = interp.session_manager.lock().unwrap();
    manager.release_session_lock(&target_sid);
    drop(manager);

    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
    result.insert(Value::Str(Rc::from("session_id")), Value::Str(Rc::from(target_sid.as_str())));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Get invocation path (session path for an invocation).
pub fn transcript_invocation_path(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _invocation_id = arg_str_or(args, 0, "");
    let sid = arg_str_or(args, 1, "");
    let target_sid = if sid.is_empty() {
        interp.session_id.clone()
    } else {
        sid.to_string()
    };
    if target_sid.is_empty() {
        return Ok(Value::Str(Rc::from("")));
    }
    let manager = interp.session_manager.lock().unwrap();
    let path = manager.get_session_path(&target_sid);
    drop(manager);
    Ok(Value::Str(Rc::from(path.to_string_lossy().as_ref())))
}

/// Get compression audit trail.
pub fn transcript_get_compression_audit(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    match load_store(interp, sid) {
        Some((store, _)) => {
            let audit = store.get_compression_audit();
            let items: Vec<Value> = audit.iter().map(|v| json_to_value(v)).collect();
            Ok(Value::List(Rc::new(RefCell::new(items))))
        }
        None => Ok(Value::List(Rc::new(RefCell::new(vec![])))),
    }
}

/// Get session directory path.
pub fn transcript_get_session_dir(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let manager = interp.session_manager.lock().unwrap();
    // Use a dummy session to get the base directory
    let path = manager.get_session_path("__dummy__");
    drop(manager);
    let dir = path.parent().unwrap_or(&path);
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
    result.insert(Value::Str(Rc::from("path")), Value::Str(Rc::from(dir.to_string_lossy().as_ref())));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Delete a specific session.
pub fn transcript_delete_session(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let sid = arg_str_or(args, 0, "");
    let _cascade = arg_bool_or(args, 1, true);
    if sid.is_empty() {
        return Err(ExceptionValue::new(
            "TypeError",
            "delete_session() expected a string session_id".to_string(),
            None,
        ));
    }

    let manager = interp.session_manager.lock().unwrap();
    let deleted = manager.delete_session(sid);
    drop(manager);

    if deleted && interp.session_id == sid {
        interp.session_id.clear();
    }
    Ok(Value::Bool(deleted))
}

/// Cleanup old sessions.
pub fn transcript_cleanup_sessions(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let keep_count = arg_int_or(args, 0, 100) as usize;
    let manager = interp.session_manager.lock().unwrap();
    let removed = manager.cleanup_old_sessions(keep_count);
    drop(manager);
    Ok(Value::Int(num_bigint::BigInt::from(removed as i64)))
}
