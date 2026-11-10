//! Integration tests for agent transcript persistence (v1.29).
//! Tests that agents with `transcript "persistent"` write messages to transcript.jsonl.
//! 
//! v1.46.22: Agents with explicit logic (main block) should NOT write internal
//! call summaries to transcript, as these leak into the chat UI.

use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use helen_core::lexer::Scanner;

/// Test that calling an agent with transcript="persistent" and explicit logic (main block)
/// does NOT write internal call summaries to transcript (v1.46.22 fix).
/// 
/// Rationale: The "Calling AgentName(...)" message was leaking into the chat UI
/// as a user message. Only actual user input and LLM responses should be in the transcript.
#[test]
fn test_agent_with_persistent_transcript_writes_messages() {
    let mut interp = Interpreter::new();
    
    // Create a temporary session
    let session_id = format!("test_agent_transcript_{}", std::process::id());
    let manager = interp.session_manager.lock().unwrap();
    let path = manager.get_session_path(&session_id);
    drop(manager);
    
    // Set session_id
    interp.session_id = session_id.clone();
    
    // Create a simple agent with transcript="persistent" and explicit logic
    let agent_code = r#"
agent TestAgent(msg: str) {
    description "Test agent with persistent transcript"
    transcript "persistent"
    
    main {
        return "Response: " + msg
    }
}

TestAgent("Hello")
"#;
    
    // Parse and execute
    let tokens = Scanner::new(agent_code, "test.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    assert!(parser.errors().is_empty(), "Parse errors: {:?}", parser.errors());
    
    let result = interp.interpret(&program);
    assert!(result.is_ok(), "Failed to execute: {:?}", result.err());
    
    // v1.46.22: Transcript file should NOT be created for agents with explicit logic.
    // Internal call summaries should not leak into the chat UI.
    // If the file exists, it should be empty (no messages written).
    if path.exists() {
        let content = std::fs::read_to_string(&path).expect("should read transcript file");
        assert!(content.is_empty(), 
            "transcript should be empty for agents with explicit logic (no user-facing messages)");
    }
    
    // Clean up
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// Test that agents without transcript="persistent" do NOT write to transcript
#[test]
fn test_agent_without_persistent_transcript_does_not_write() {
    let mut interp = Interpreter::new();
    
    let session_id = format!("test_agent_no_transcript_{}", std::process::id());
    let manager = interp.session_manager.lock().unwrap();
    let path = manager.get_session_path(&session_id);
    drop(manager);
    
    interp.session_id = session_id.clone();
    
    // Create an agent without transcript="persistent" (default is "none")
    let agent_code = r#"
agent TestAgentNoTranscript(msg: str) {
    description "Test agent without persistent transcript"
    
    main {
        return "Response: " + msg
    }
}

TestAgentNoTranscript("Hello")
"#;
    
    // Parse and execute
    let tokens = Scanner::new(agent_code, "test.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    assert!(parser.errors().is_empty(), "Parse errors: {:?}", parser.errors());
    
    let result = interp.interpret(&program);
    assert!(result.is_ok(), "Failed to execute: {:?}", result.err());
    
    // Verify transcript file was NOT created (or is empty)
    if path.exists() {
        let content = std::fs::read_to_string(&path).expect("should read transcript file");
        assert!(content.is_empty(), "transcript should be empty for non-persistent agent");
    }
    
    // Clean up
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
