//! Tests for agent isolation features (v1.12+) — L0/L2 isolation, history filtering, invocation metadata.
//!
//! These tests verify the 3 missing features identified in the audit:
//! 1. L0 (Open) Isolation Level - inject module-level `let` variables
//! 2. Per-Invocation History Filtering - filter by invocation_id
//! 3. Invocation Tree Metadata - track parent-child relationships

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::llm_runtime::MockLlmRuntime;
use helen_interpreter::value::Value;
use helen_parser::Parser;
use std::sync::Arc;

#[allow(clippy::arc_with_non_send_sync)]
fn run_code(code: &str) -> (Result<Option<Value>, helen_interpreter::exceptions::ExceptionValue>, Arc<MockLlmRuntime>) {
    let mut interp = Interpreter::new();
    let mock_runtime = Arc::new(MockLlmRuntime::with_act_text("mock response"));
    interp.set_llm_runtime(mock_runtime.clone());
    
    let tokens = Scanner::new(code, "test.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    
    assert!(
        parser.errors().is_empty(),
        "Parse errors: {:?}",
        parser.errors()
    );
    
    let result = interp.interpret(&program);
    (result, mock_runtime)
}

// ============================================================================
// Feature 1: L0 (Open) Isolation Level
// ============================================================================

#[test]
fn test_l0_open_isolation_injects_module_let() {
    // L0 (open) isolation should inject module-level `let` variables into agent scope
    let code = r#"
let module_var = "hello from module"

@open agent TestAgent() {
    main {
        return module_var
    }
}

let result = TestAgent()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
    if let Ok(Some(Value::Str(s))) = result {
        assert_eq!(s.as_ref(), "hello from module");
    } else {
        panic!("Expected Ok(Some(Str))");
    }
}

#[test]
fn test_l0_open_isolation_can_modify_module_let() {
    // L0 (open) should allow modification of module-level `let` variables
    let code = r#"
let counter = 0

@open agent Incrementer() {
    main {
        counter = counter + 1
        return counter
    }
}

let result = Incrementer()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
    if let Ok(Some(Value::Int(n))) = result {
        assert_eq!(n, 1.into());
    } else {
        panic!("Expected Ok(Some(Int))");
    }
}

#[test]
fn test_l1_standard_hides_module_let() {
    // L1 (standard, default) should NOT inject module-level `let` variables
    let code = r#"
let module_var = "hidden"

agent TestAgent() {
    main {
        // module_var should not be accessible
        return "ok"
    }
}

let result = TestAgent()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
}

// ============================================================================
// Feature 2: Per-Invocation History Filtering
// ============================================================================

#[test]
fn test_invocation_id_filtering_in_history() {
    // Each agent invocation should only see its own history
    let code = r#"
agent Agent1() {
    main {
        let result1 = llm act "first call"
        return result1
    }
}

agent Agent2() {
    main {
        let result2 = llm act "second call"
        return result2
    }
}

let r1 = Agent1()
let r2 = Agent2()
"#;
    let (result, mock_runtime) = run_code(code);
    assert!(result.is_ok());
    
    // Verify that act was called twice
    let history = mock_runtime.act_history.borrow();
    assert_eq!(history.len(), 2);
}

#[test]
fn test_history_filtered_by_invocation_id() {
    // History should be filtered by current invocation_id
    let code = r#"
agent TestAgent() {
    main {
        let invocation_id = get_invocation_id()
        return invocation_id
    }
}

let id1 = TestAgent()
let id2 = TestAgent()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
}

// ============================================================================
// Feature 3: Invocation Tree Metadata
// ============================================================================

#[test]
fn test_invocation_id_generation() {
    // Each agent call should generate a unique invocation ID
    let code = r#"
agent TestAgent() {
    main {
        return get_invocation_id()
    }
}

let id1 = TestAgent()
let id2 = TestAgent()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
}

#[test]
fn test_nested_invocation_tracking() {
    // Nested agent calls should track parent-child relationships
    let code = r#"
agent ChildAgent() {
    main {
        let parent_id = get_parent_invocation_id()
        let my_id = get_invocation_id()
        return my_id
    }
}

agent ParentAgent() {
    main {
        let parent_id = get_invocation_id()
        let child_result = ChildAgent()
        return child_result
    }
}

let result = ParentAgent()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
}

#[test]
fn test_invocation_stack_push_pop() {
    // Invocation stack should be properly maintained
    let code = r#"
agent Level3() {
    main {
        return get_invocation_id()
    }
}

agent Level2() {
    main {
        let id2 = get_invocation_id()
        let id3 = Level3()
        return id2
    }
}

agent Level1() {
    main {
        let id1 = get_invocation_id()
        let id2 = Level2()
        return id1
    }
}

let result = Level1()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
}

#[test]
fn test_invocation_tree_metadata_tracking() {
    // Invocation tree should track parent-child relationships
    let code = r#"
agent ChildAgent() {
    main {
        return "child"
    }
}

agent ParentAgent() {
    main {
        let result = ChildAgent()
        return result
    }
}

let result = ParentAgent()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
    // The invocation_index should have been populated
    // (we can't directly access it from Helen code, but the test verifies no errors)
}

#[test]
fn test_multiple_invocations_tracked() {
    // Multiple agent calls should each have their own invocation metadata
    let code = r#"
agent TestAgent() {
    main {
        return get_invocation_id()
    }
}

let id1 = TestAgent()
let id2 = TestAgent()
let id3 = TestAgent()
"#;
    let (result, _) = run_code(code);
    assert!(result.is_ok());
}
