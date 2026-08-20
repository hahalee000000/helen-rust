//! Tests for transcript module — transcript query and session management.

use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::transcript::*;
use helen_interpreter::value::Value;
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
    // Python parity: returns {} when no session or no invocations.
    match result.unwrap() {
        Value::Map(map) => {
            let borrowed = map.borrow();
            assert_eq!(borrowed.len(), 0, "expected empty map, got {borrowed:?}");
        }
        _ => panic!("Expected Map, got other type"),
    }
}

#[test]
fn test_get_invocation_tree_nested_children() {
    use helen_runtime::transcript::{JsonlBackend, Message, TranscriptStore};
    let mut interp = make_interp();

    // Seed a session store with a parent + child invocation.
    let sid = format!("session_tree_{}", std::process::id());
    let manager = interp.session_manager.lock().unwrap();
    let path = manager.get_session_path(&sid);
    drop(manager);
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let backend = JsonlBackend::new(&path);
    let mut store = TranscriptStore::load_from_backend(backend, 1000);
    let parent_uuid = helen_runtime::transcript::generate_uuid();
    let child_uuid = helen_runtime::transcript::generate_uuid();

    store.append(
        &mut Message::new(
            "user",
            serde_json::json!("hi"),
            Vec::new(),
            None,
            helen_runtime::transcript::generate_uuid(),
            None,
            0,
            false,
            false,
            None,
            parent_uuid.clone(),
            String::new(),
            Vec::new(),
        ),
        true,
    );
    store.append(
        &mut Message::new(
            "assistant",
            serde_json::json!("response"),
            Vec::new(),
            None,
            helen_runtime::transcript::generate_uuid(),
            None,
            0,
            false,
            false,
            Some("agentA".to_string()),
            child_uuid.clone(),
            parent_uuid.clone(),
            Vec::new(),
        ),
        true,
    );

    let result = transcript_get_invocation_tree(&mut interp, &[Value::Str(Rc::from(sid.as_str()))]);
    assert!(result.is_ok(), "tree failed: {:?}", result.err());
    let tree = result.unwrap();
    match &tree {
        Value::Map(map) => {
            let b = map.borrow();
            // Single root = parent invocation.
            assert_eq!(
                b.get(&Value::Str(Rc::from("invocation_id"))),
                Some(&Value::Str(Rc::from(parent_uuid.as_str())))
            );
            assert_eq!(
                b.get(&Value::Str(Rc::from("message_count"))),
                Some(&Value::Int(num_bigint::BigInt::from(1)))
            );
            // One child.
            let kids = b.get(&Value::Str(Rc::from("children"))).unwrap();
            if let Value::List(list) = kids {
                let children = list.borrow();
                assert_eq!(children.len(), 1);
                if let Value::Map(cmap) = &children[0] {
                    let cb = cmap.borrow();
                    assert_eq!(
                        cb.get(&Value::Str(Rc::from("agent_name"))),
                        Some(&Value::Str(Rc::from("agentA")))
                    );
                    assert_eq!(
                        cb.get(&Value::Str(Rc::from("parent_invocation_id"))),
                        Some(&Value::Str(Rc::from(parent_uuid.as_str())))
                    );
                } else {
                    panic!("child not a map");
                }
            } else {
                panic!("children not a list");
            }
        }
        _ => panic!("Expected Map tree, got {:?}", tree),
    }

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
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
fn test_export_transcript_empty_path() {
    // No args → output_path is "" → returns ""
    let mut interp = make_interp();
    let result = transcript_export_transcript(&mut interp, &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Str(Rc::from("")));
}

#[test]
fn test_export_transcript_no_session() {
    // Valid path but no session → returns ""
    let mut interp = make_interp();
    let tmp = std::env::temp_dir().join("helen_test_export_nosession.json");
    let path_str = tmp.to_str().unwrap();
    let result = transcript_export_transcript(&mut interp, &[Value::Str(Rc::from(path_str))]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Str(Rc::from("")));
    // Clean up if file was created
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_export_transcript_unknown_format() {
    // Unknown format → returns ""
    let mut interp = make_interp();
    let tmp = std::env::temp_dir().join("helen_test_export_badfmt.xyz");
    let path_str = tmp.to_str().unwrap();
    let result = transcript_export_transcript(
        &mut interp,
        &[Value::Str(Rc::from(path_str)), Value::Str(Rc::from("xml"))],
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Str(Rc::from("")));
    let _ = std::fs::remove_file(&tmp);
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

// ── format_context_stats (HistoryManager wiring) ─────────────────────────

#[test]
fn test_format_context_stats_empty_history() {
    let interp = make_interp();
    let s = interp.format_context_stats();
    // Python-parity box header + 0 tokens.
    assert!(s.contains("Context Usage Statistics"), "{s}");
    assert!(s.contains("Tokens:"), "{s}");
    assert!(s.contains("0 /"), "{s}");
}

#[test]
fn test_format_context_stats_after_llm_call() {
    // Seed history directly (llm calls recorded by visit_llm_act; here we
    // simulate the recorded entries).
    let interp = make_interp();
    {
        let mut h = interp.history.borrow_mut();
        h.push(serde_json::json!({"role": "user", "content": "hello world"}));
        h.push(serde_json::json!({"role": "assistant", "content": "hi there"}));
    }
    let s = interp.format_context_stats();
    assert!(s.contains("Messages:"), "{s}");
    assert!(s.contains("2"), "{s}"); // message_count = 2
    assert!(s.contains("USER"), "{s}"); // capitalized role label
    assert!(s.contains("ASSISTANT"), "{s}");
}

#[test]
fn test_transcript_log_env_var() {
    let dir =
        std::env::temp_dir().join(format!("helen_test_transcript_log_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let log_path = dir.join("custom_transcript.jsonl");

    // Set env var
    std::env::set_var("HELEN_TRANSCRIPT_LOG", log_path.to_str().unwrap());

    let mut interp = make_interp();
    interp.session_id = "test_session".to_string();

    // Call a function that uses load_store internally
    let result = helen_interpreter::context::context_context_stats(&mut interp, &[]);
    assert!(
        result.is_ok(),
        "context_stats should work with env var path"
    );

    // Clean up
    std::env::remove_var("HELEN_TRANSCRIPT_LOG");
    let _ = std::fs::remove_dir_all(&dir);
}
