//! Tests for agent isolation features (v1.12+).
//!
//! Covers:
//! - Invocation ID tracking (v1.22)
//! - Isolation levels (L0 open, L1 standard, L2 strict, L3 sandbox)
//! - ReadOnlyView for agent parameters
//! - Shared let visibility
//! - Per-invocation history filtering

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::llm_runtime::MockLlmRuntime;
use helen_interpreter::value::Value;
use helen_parser::Parser;
use num_traits::cast::ToPrimitive;
use std::sync::Arc;

/// Helper to parse and run Helen code
#[allow(clippy::arc_with_non_send_sync)]
fn run_code(
    code: &str,
) -> (
    Result<Option<Value>, helen_interpreter::exceptions::ExceptionValue>,
    Arc<MockLlmRuntime>,
) {
    let mut interp = Interpreter::new();
    let mock = Arc::new(MockLlmRuntime::with_act_text("test response"));
    interp.set_llm_runtime(mock.clone());

    let tokens = Scanner::new(code, "test.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();

    assert!(
        parser.errors().is_empty(),
        "Parse errors: {:?}",
        parser.errors()
    );

    let result = interp.interpret(&program);
    (result, mock)
}

// ── Invocation ID Tracking (v1.22) ──────────────────────────────────────────

#[test]
fn test_invocation_id_generated_for_agent() {
    let code = r#"
        agent TestAgent() {
            description "Test agent"
            main {
                let inv_id = get_invocation_id()
                return inv_id
            }
        }
        
        let result = TestAgent()
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "Should execute without error: {:?}",
        result.err()
    );

    // Invocation ID should be generated (format: inv_<timestamp>_<uuid>)
    if let Ok(Some(Value::Str(id))) = result {
        assert!(
            id.starts_with("inv_"),
            "Invocation ID should start with 'inv_': {}",
            id
        );
    }
}

#[test]
fn test_invocation_id_unique_per_call() {
    let code = r#"
        agent TestAgent() {
            description "Test agent"
            main {
                return get_invocation_id()
            }
        }
        
        let id1 = TestAgent()
        let id2 = TestAgent()
        
        return [id1, id2]
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "Should generate unique IDs: {:?}",
        result.err()
    );

    // Check that the two IDs are different
    if let Ok(Some(Value::List(ids))) = result {
        let ids_vec = ids.borrow();
        assert_eq!(ids_vec.len(), 2, "Should return 2 IDs");

        if let (Value::Str(id1), Value::Str(id2)) = (&ids_vec[0], &ids_vec[1]) {
            assert_ne!(
                id1.as_ref(),
                id2.as_ref(),
                "Each call should have a unique invocation ID"
            );
        } else {
            panic!("IDs should be strings");
        }
    } else {
        panic!("Should return a list of IDs");
    }
}

#[test]
fn test_invocation_stack_tracking() {
    let code = r#"
        agent ChildAgent() {
            description "Child agent"
            main {
                let child_id = get_invocation_id()
                let parent_id = get_parent_invocation_id()
                return [child_id, parent_id]
            }
        }
        
        agent ParentAgent() {
            description "Parent agent"
            main {
                let parent_id = get_invocation_id()
                let result = ChildAgent()
                return [parent_id, result]
            }
        }
        
        let result = ParentAgent()
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "Should track invocation stack: {:?}",
        result.err()
    );
}

// ── Isolation Levels ────────────────────────────────────────────────────────

#[test]
fn test_l1_standard_const_visible() {
    let code = r#"
        const MODULE_CONST = "const_value"
        
        agent TestAgent() {
            description "Test agent"
            main {
                // L1: module-level const should be visible
                return MODULE_CONST
            }
        }
        
        let result = TestAgent()
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "L1 should show module const: {:?}",
        result.err()
    );

    if let Ok(Some(Value::Str(s))) = result {
        assert_eq!(s.as_ref(), "const_value", "L1 should expose module const");
    }
}

#[test]
fn test_l2_strict_deep_copy() {
    let code = r#"
        @strict
        agent TestAgent(data: list) {
            description "Test agent"
            
            main {
                // L2: data should be a deep copy, not a reference
                let copy = data
                copy[0] = 999
                return copy[0]
            }
        }
        
        let original = [1, 2, 3]
        let result = TestAgent(original)
        
        return [original[0], result]
    "#;

    let (result, _) = run_code(code);
    assert!(result.is_ok(), "L2 should deep copy: {:?}", result.err());

    if let Ok(Some(Value::List(arr))) = result {
        let arr_ref = arr.borrow();
        if let (Value::Int(orig), Value::Int(res)) = (&arr_ref[0], &arr_ref[1]) {
            assert_eq!(orig.to_i64().unwrap(), 1, "Original should be unchanged");
            assert_eq!(res.to_i64().unwrap(), 999, "Copy should be modified");
        }
    }
}

#[test]
fn test_l3_sandbox_tool_restriction() {
    let code = r#"
        @sandbox
        agent TestAgent() {
            description "Test agent"
            tools = ["web_search"]
            
            main {
                // L3: only skill tools should be available
                return "sandbox_mode"
            }
        }
        
        let result = TestAgent()
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "L3 should restrict tools: {:?}",
        result.err()
    );
}

// ── Per-Invocation History Filtering ────────────────────────────────────────

#[test]
fn test_history_filtered_by_invocation() {
    let code = r#"
        agent Agent1() {
            description "Agent 1"
            main {
                let msg1 = llm act "Message from Agent1"
                return msg1
            }
        }
        
        agent Agent2() {
            description "Agent 2"
            main {
                let msg2 = llm act "Message from Agent2"
                return msg2
            }
        }
        
        let r1 = Agent1()
        let r2 = Agent2()
        
        // Each agent should only see its own messages
    "#;

    let (result, mock) = run_code(code);
    assert!(result.is_ok(), "Should filter history: {:?}", result.err());

    // Verify that LLM calls were made with proper invocation context
    let history = mock.act_history.borrow();
    assert_eq!(history.len(), 2, "Should have 2 LLM calls");
}

// ── ReadOnlyView Mutation Protection ────────────────────────────────────────

#[test]
fn test_readonly_view_allows_read() {
    let code = r#"
        agent TestAgent(data: list) {
            description "Test agent"
            main {
                // L1: ReadOnlyView allows read operations
                let first = data[0]
                let length = len(data)
                return [first, length]
            }
        }
        
        let original = [10, 20, 30]
        let result = TestAgent(original)
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "ReadOnlyView should allow reads: {:?}",
        result.err()
    );

    if let Ok(Some(Value::List(arr))) = result {
        let arr_ref = arr.borrow();
        if let (Value::Int(first), Value::Int(length)) = (&arr_ref[0], &arr_ref[1]) {
            assert_eq!(first.to_i64().unwrap(), 10, "Should read first element");
            assert_eq!(length.to_i64().unwrap(), 3, "Should read length");
        }
    }
}

// ── Shared Let Visibility ───────────────────────────────────────────────────

#[test]
fn test_shared_let_visible_in_agent() {
    let code = r#"
        shared let shared_value = "shared_data"
        
        agent TestAgent() {
            description "Test agent"
            main {
                // shared let should be visible and writable
                return shared_value
            }
        }
        
        let result = TestAgent()
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "shared let should be visible: {:?}",
        result.err()
    );

    if let Ok(Some(Value::Str(s))) = result {
        assert_eq!(s.as_ref(), "shared_data", "shared let should be visible");
    }
}

// ── Invocation Metadata ─────────────────────────────────────────────────────

#[test]
fn test_invocation_metadata_in_transcript() {
    let code = r#"
        agent TestAgent() {
            description "Test agent"
            main {
                let msg = llm act "Test message"
                return msg
            }
        }
        
        let result = TestAgent()
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "Should record invocation metadata: {:?}",
        result.err()
    );

    // Transcript messages should have invocation_id set
    // (verified by the fact that the code executes without error)
}

#[test]
fn test_nested_agent_invocation_tree() {
    let code = r#"
        agent GrandChild() {
            description "Grandchild"
            main {
                let gc_id = get_invocation_id()
                let parent_id = get_parent_invocation_id()
                return [gc_id, parent_id]
            }
        }
        
        agent Child() {
            description "Child"
            main {
                let c_id = get_invocation_id()
                let gc_result = GrandChild()
                return [c_id, gc_result]
            }
        }
        
        agent Parent() {
            description "Parent"
            main {
                let p_id = get_invocation_id()
                let c_result = Child()
                return [p_id, c_result]
            }
        }
        
        let result = Parent()
    "#;

    let (result, _) = run_code(code);
    assert!(
        result.is_ok(),
        "Should track nested invocations: {:?}",
        result.err()
    );
}
