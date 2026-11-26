//! Integration tests for transcript tool_calls persistence (M25.58).
//! Tests that tool_calls are recorded in transcript for both streaming and non-streaming paths.

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::llm_runtime::{LlmResponse, MockLlmRuntime};
use helen_parser::Parser;
use std::sync::Arc;

/// Test that non-streaming path records tool_calls in transcript (M25.58 fix).
///
/// This test verifies that when an agent with transcript="persistent" uses llm act
/// and the LLM returns tool_calls, those tool_calls are written to the transcript.
#[test]
fn test_nonstreaming_transcript_includes_tool_calls() {
    let mut interp = Interpreter::new();

    // Create a mock LLM runtime that returns tool_calls
    let mock_tool_call = serde_json::json!({
        "id": "call_test123",
        "type": "function",
        "function": {
            "name": "read_file",
            "arguments": "{\"path\": \"test.txt\"}"
        }
    });

    let mock_runtime = MockLlmRuntime::new(
        None,
        Some(LlmResponse {
            text: Some("I'll read the file".to_string()),
            tool_calls: vec![mock_tool_call.clone()],
            model: Some("mock-model".to_string()),
        }),
    );
    interp.llm_runtime = Arc::new(mock_runtime);

    // Create a temporary session
    let session_id = format!("test_transcript_tool_calls_{}", std::process::id());
    let manager = interp.session_manager.lock().unwrap();
    let path = manager.get_session_path(&session_id);
    drop(manager);

    interp.session_id = session_id.clone();

    // Create an agent with transcript="persistent" that uses llm act
    let agent_code = r#"
agent TestAgentWithTools(query: str) {
    description "Test agent that uses tools"
    prompt "Query: {{query}}"
    tools ["read_file"]
    transcript "persistent"
    
    main {
        let result = llm act
        return result
    }
}

TestAgentWithTools("test query")
"#;

    // Parse and execute
    let tokens = Scanner::new(agent_code, "test.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    assert!(
        parser.errors().is_empty(),
        "Parse errors: {:?}",
        parser.errors()
    );

    let result = interp.interpret(&program);
    assert!(result.is_ok(), "Failed to execute: {:?}", result.err());

    // Verify transcript file contains tool_calls
    if path.exists() {
        let content = std::fs::read_to_string(&path).expect("should read transcript file");
        
        // Parse transcript as JSONL
        let mut found_tool_calls = false;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) {
                if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                    if let Some(tool_calls) = msg.get("tool_calls") {
                        if let Some(calls_array) = tool_calls.as_array() {
                            if !calls_array.is_empty() {
                                found_tool_calls = true;
                                // Verify the tool_call structure
                                let first_call = &calls_array[0];
                                assert_eq!(
                                    first_call.get("id").and_then(|id| id.as_str()),
                                    Some("call_test123")
                                );
                                assert_eq!(
                                    first_call.get("type").and_then(|t| t.as_str()),
                                    Some("function")
                                );
                                let func = first_call.get("function").unwrap();
                                assert_eq!(
                                    func.get("name").and_then(|n| n.as_str()),
                                    Some("read_file")
                                );
                            }
                        }
                    }
                }
            }
        }
        
        assert!(
            found_tool_calls,
            "Transcript should contain tool_calls in assistant message. Content:\n{}",
            content
        );
    } else {
        panic!("Transcript file should exist at {:?}", path);
    }

    // Clean up
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// Test that write_transcript_message_with_tools correctly writes tool_calls.
#[test]
fn test_write_transcript_message_with_tools() {
    let mut interp = Interpreter::new();

    let session_id = format!("test_write_transcript_{}", std::process::id());
    let manager = interp.session_manager.lock().unwrap();
    let path = manager.get_session_path(&session_id);
    drop(manager);

    interp.session_id = session_id.clone();

    // Create tool_calls
    let tool_calls = vec![serde_json::json!({
        "id": "call_abc123",
        "type": "function",
        "function": {
            "name": "write_file",
            "arguments": "{\"path\": \"output.txt\", \"content\": \"hello\"}"
        }
    })];

    // Write transcript message with tool_calls
    interp.write_transcript_message_with_tools(
        "assistant",
        "I'll write the file",
        "TestAgent",
        tool_calls.clone(),
        None,
    );

    // Verify transcript file contains tool_calls
    assert!(path.exists(), "Transcript file should exist");
    let content = std::fs::read_to_string(&path).expect("should read transcript file");
    
    let mut found = false;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) {
            if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                if let Some(tc) = msg.get("tool_calls") {
                    if let Some(calls) = tc.as_array() {
                        if calls.len() == 1 {
                            found = true;
                            assert_eq!(
                                calls[0].get("id").and_then(|id| id.as_str()),
                                Some("call_abc123")
                            );
                        }
                    }
                }
            }
        }
    }
    
    assert!(found, "Should find assistant message with tool_calls");

    // Clean up
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
