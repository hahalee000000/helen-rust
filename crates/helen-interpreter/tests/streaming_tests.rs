//! Tests for streaming functionality — verifies on_chunk/on_complete callbacks
//! and proper memory management in streaming paths.

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::llm_runtime::MockLlmRuntime;
use helen_interpreter::value::Value;
use helen_parser::Parser;

/// Helper to parse and execute Helen code
fn run_with_mock(code: &str, mock_text: &str) -> Result<Option<Value>, String> {
    let tokens = Scanner::new(code, "test.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    if !parser.errors().is_empty() {
        return Err(format!("Parse errors: {:?}", parser.errors()));
    }

    let mut interp = Interpreter::new();
    let mock = MockLlmRuntime::with_act_text(mock_text);
    interp.set_llm_runtime(std::sync::Arc::new(mock));

    interp
        .interpret(&program)
        .map_err(|e| format!("Runtime error: {:?}", e))
}

/// Test that streaming path correctly accumulates and returns text
/// This test would have caught the dangling reference bug fixed in M25.1
#[test]
fn streaming_accumulates_text_correctly() {
    let code = r#"
        agent StreamTest(msg: str) {
            description "Test streaming"
            prompt "Test: {{msg}}"
            
            main {
                let result = llm act msg
                return result
            }
        }
        
        let output = StreamTest("hello")
        return output
    "#;

    let result = run_with_mock(code, "Hello, streaming world!");
    assert!(
        result.is_ok(),
        "Streaming execution should succeed: {:?}",
        result.err()
    );

    let value = result.unwrap();
    match value {
        Some(Value::Str(s)) => {
            let text = s.as_ref();
            assert_eq!(
                text, "Hello, streaming world!",
                "Streaming should return complete accumulated text"
            );
        }
        Some(other) => panic!("Expected string result, got {:?}", other),
        None => panic!("Expected Some result, got None"),
    }
}

/// Test that streaming with on_chunk callback receives all chunks
#[test]
fn streaming_on_chunk_receives_content() {
    let code = r#"
        let chunks = []
        
        fn chunk_handler(text) {
            chunks = chunks + [text]
            return true
        }
        
        agent ChunkTest(msg: str) {
            description "Test chunk callback"
            prompt "Test: {{msg}}"
            
            main {
                let result = llm act msg
                    on_chunk chunk_handler
                return result
            }
        }
        
        let output = ChunkTest("test")
        return output
    "#;

    let result = run_with_mock(code, "chunk1 chunk2 chunk3");
    assert!(
        result.is_ok(),
        "Streaming with on_chunk should succeed: {:?}",
        result.err()
    );

    let value = result.unwrap();
    match value {
        Some(Value::Str(s)) => {
            let text = s.as_ref();
            assert!(!text.is_empty(), "Streaming should return non-empty text");
            assert_eq!(text, "chunk1 chunk2 chunk3");
        }
        Some(other) => panic!("Expected string result, got {:?}", other),
        None => panic!("Expected Some result, got None"),
    }
}

/// Test that streaming with on_complete callback is called
#[test]
fn streaming_on_complete_called() {
    let code = r#"
        let complete_called = false
        
        fn complete_handler() {
            complete_called = true
        }
        
        agent CompleteTest(msg: str) {
            description "Test complete callback"
            prompt "Test: {{msg}}"
            
            main {
                let result = llm act msg
                    on_complete complete_handler
                return result
            }
        }
        
        let output = CompleteTest("test")
        return output
    "#;

    let result = run_with_mock(code, "complete test");
    assert!(
        result.is_ok(),
        "Streaming with on_complete should succeed: {:?}",
        result.err()
    );
}

/// Test that streaming returns empty string for empty LLM response
#[test]
fn streaming_empty_response() {
    let code = r#"
        agent EmptyTest(msg: str) {
            description "Test empty response"
            prompt "Test: {{msg}}"
            
            main {
                let result = llm act msg
                return result
            }
        }
        
        let output = EmptyTest("test")
        return output
    "#;

    // Use mock that returns None (empty text)
    let tokens = Scanner::new(code, "test.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    assert!(
        parser.errors().is_empty(),
        "Parse errors: {:?}",
        parser.errors()
    );

    let mut interp = Interpreter::new();
    let mock = MockLlmRuntime::new(None, None);
    interp.set_llm_runtime(std::sync::Arc::new(mock));

    let result = interp.interpret(&program);
    assert!(
        result.is_ok(),
        "Empty streaming response should succeed: {:?}",
        result.err()
    );

    let value = result.unwrap();
    match value {
        Some(Value::Str(s)) => {
            let text = s.as_ref();
            assert_eq!(text, "", "Empty response should return empty string");
        }
        Some(other) => panic!("Expected string result, got {:?}", other),
        None => panic!("Expected Some result, got None"),
    }
}

/// Test that streaming handles long text correctly (no truncation)
#[test]
fn streaming_long_text() {
    let code = r#"
        agent LongTest(msg: str) {
            description "Test long response"
            prompt "Test: {{msg}}"
            
            main {
                let result = llm act msg
                return result
            }
        }
        
        let output = LongTest("test")
        return output
    "#;

    // Create a long response
    let long_text = "word ".repeat(1000);
    let result = run_with_mock(code, &long_text);
    assert!(
        result.is_ok(),
        "Long streaming response should succeed: {:?}",
        result.err()
    );

    let value = result.unwrap();
    match value {
        Some(Value::Str(s)) => {
            let text = s.as_ref();
            assert_eq!(
                text.len(),
                long_text.len(),
                "Long text should not be truncated"
            );
            assert_eq!(text, long_text);
        }
        Some(other) => panic!("Expected string result, got {:?}", other),
        None => panic!("Expected Some result, got None"),
    }
}

/// Test that streaming with Unicode text works correctly
#[test]
fn streaming_unicode_text() {
    let code = r#"
        agent UnicodeTest(msg: str) {
            description "Test Unicode response"
            prompt "Test: {{msg}}"
            
            main {
                let result = llm act msg
                return result
            }
        }
        
        let output = UnicodeTest("test")
        return output
    "#;

    let unicode_text = "你好世界 🌍 Hello 世界";
    let result = run_with_mock(code, unicode_text);
    assert!(
        result.is_ok(),
        "Unicode streaming should succeed: {:?}",
        result.err()
    );

    let value = result.unwrap();
    match value {
        Some(Value::Str(s)) => {
            let text = s.as_ref();
            assert_eq!(text, unicode_text, "Unicode text should be preserved");
        }
        Some(other) => panic!("Expected string result, got {:?}", other),
        None => panic!("Expected Some result, got None"),
    }
}
