//! Tests for transcript module — transcript query and session management.

use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::transcript::*;
use helen_interpreter::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

fn make_interp() -> Interpreter {
    Interpreter::new()
}

// ── Query functions (return empty lists when no session) ─────────────────

#[test]
fn test_query_transcript_empty() {
    let mut interp = make_interp();
    let result = transcript_query_transcript(&mut interp, &[]);
    assert!(result.is_ok());
    match result.unwrap() {
        Value::List(list) => {
            let borrowed = list.borrow();
            assert_eq!(borrowed.len(), 0);
        }
        _ => panic!("Expected List, got other type"),
    }
}

#[test]
fn test_search_transcript_empty() {
    let mut interp = make_interp();
    let result = transcript_search_transcript(&mut interp, &[]);
    assert!(result.is_ok());
    match result.unwrap() {
        Value::List(list) => {
            let borrowed = list.borrow();
            assert_eq!(borrowed.len(), 0);
        }
        _ => panic!("Expected List, got other type"),
    }
}

#[test]
fn test_list_invocations_empty() {
    let mut interp = make_interp();
    let result = transcript_list_invocations(&mut interp, &[]);
    assert!(result.is_ok());
    match result.unwrap() {
        Value::List(list) => {
            let borrowed = list.borrow();
            assert_eq!(borrowed.len(), 0);
        }
        _ => panic!("Expected List, got other type"),
    }
}

// ── Get functions (return Null or empty structures when no session) ──────

#[test]
fn test_get_invocation_null() {
    let mut interp = make_interp();
    let result = transcript_get_invocation(&mut interp, &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
fn test_get_invocation_tree_empty_map() {
    let mut interp = make_interp();
    let result = transcript_get_invocation_tree(&mut interp, &[]);
    assert!(result.is_ok());
    // Returns a map with empty invocations when no invocation_id provided
    match result.unwrap() {
        Value::Map(map) => {
            let borrowed = map.borrow();
            assert!(borrowed.contains_key(&Value::Str(Rc::from("session_id"))));
            assert!(borrowed.contains_key(&Value::Str(Rc::from("invocations"))));
        }
        _ => panic!("Expected Map, got other type"),
    }
}

#[test]
fn test_get_spawn_tree_empty_map() {
    let mut interp = make_interp();
    let result = transcript_get_spawn_tree(&mut interp, &[]);
    assert!(result.is_ok());
    // Returns a map with empty children when no session
    match result.unwrap() {
        Value::Map(map) => {
            let borrowed = map.borrow();
            assert!(borrowed.contains_key(&Value::Str(Rc::from("session_id"))));
            assert!(borrowed.contains_key(&Value::Str(Rc::from("children"))));
        }
        _ => panic!("Expected Map, got other type"),
    }
}

#[test]
fn test_get_compression_audit_empty_list() {
    let mut interp = make_interp();
    let result = transcript_get_compression_audit(&mut interp, &[]);
    assert!(result.is_ok());
    // Returns empty list when no session
    match result.unwrap() {
        Value::List(list) => {
            let borrowed = list.borrow();
            assert_eq!(borrowed.len(), 0);
        }
        _ => panic!("Expected List, got other type"),
    }
}

// ── Session management ───────────────────────────────────────────────────

#[test]
fn test_get_spawned_sessions_empty() {
    let mut interp = make_interp();
    let result = transcript_get_spawned_sessions(&mut interp, &[]);
    assert!(result.is_ok());
    match result.unwrap() {
        Value::List(list) => {
            let borrowed = list.borrow();
            assert_eq!(borrowed.len(), 0);
        }
        _ => panic!("Expected List, got other type"),
    }
}

#[test]
fn test_export_transcript_empty_json() {
    let mut interp = make_interp();
    let result = transcript_export_transcript(&mut interp, &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Str(Rc::from("[]")));
}

#[test]
fn test_replay_transcript_empty_list() {
    let mut interp = make_interp();
    let result = transcript_replay_transcript(&mut interp, &[]);
    assert!(result.is_ok());
    // Returns empty list when no session
    match result.unwrap() {
        Value::List(list) => {
            let borrowed = list.borrow();
            assert_eq!(borrowed.len(), 0);
        }
        _ => panic!("Expected List, got other type"),
    }
}

#[test]
fn test_replay_full_session_empty_list() {
    let mut interp = make_interp();
    let result = transcript_replay_full_session(&mut interp, &[]);
    assert!(result.is_ok());
    // Returns empty list when no session
    match result.unwrap() {
        Value::List(list) => {
            let borrowed = list.borrow();
            assert_eq!(borrowed.len(), 0);
        }
        _ => panic!("Expected List, got other type"),
    }
}

#[test]
fn test_resume_session_error() {
    let mut interp = make_interp();
    let result = transcript_resume_session(&mut interp, &[]);
    assert!(result.is_ok());
    // Should return an error map when no session_id provided
    match result.unwrap() {
        Value::Map(map) => {
            let borrowed = map.borrow();
            assert!(borrowed.contains_key(&Value::Str(Rc::from("status"))));
        }
        _ => panic!("Expected Map, got other type"),
    }
}

#[test]
fn test_delete_current_session_error() {
    let mut interp = make_interp();
    let result = transcript_delete_current_session(&mut interp, &[]);
    assert!(result.is_ok());
    // Should return an error map when no session
    match result.unwrap() {
        Value::Map(map) => {
            let borrowed = map.borrow();
            assert!(borrowed.contains_key(&Value::Str(Rc::from("status"))));
        }
        _ => panic!("Expected Map, got other type"),
    }
}

#[test]
fn test_release_session_lock_error() {
    let mut interp = make_interp();
    let result = transcript_release_session_lock(&mut interp, &[]);
    assert!(result.is_ok());
    // Should return an error map when no session_id provided
    match result.unwrap() {
        Value::Map(map) => {
            let borrowed = map.borrow();
            assert!(borrowed.contains_key(&Value::Str(Rc::from("status"))));
        }
        _ => panic!("Expected Map, got other type"),
    }
}

#[test]
fn test_invocation_path_empty_string() {
    let mut interp = make_interp();
    let result = transcript_invocation_path(&mut interp, &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Str(Rc::from("")));
}
