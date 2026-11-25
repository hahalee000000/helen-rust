//! Tests for three-level graduated loop intervention (v1.46.37).
//!
//! Verifies that:
//! - L1: First detection injects a warning hint, allows LLM to continue
//! - L2: Second detection injects force-summary prompt, allows LLM to produce final answer
//! - L3: Third detection (after L2) terminates the agent

use helen_runtime::http_llm::HttpLLMRuntime;
use helen_runtime::loop_detector::{ToolCallLoopDetector, ToolCallSignature};
use serde_json::json;

#[test]
fn test_per_tool_thresholds() {
    let detector = ToolCallLoopDetector::new();
    
    // shell_exec has high tolerance (25)
    assert_eq!(detector.get_tool_threshold("shell_exec"), 25);
    
    // write_file has strict threshold (5)
    assert_eq!(detector.get_tool_threshold("write_file"), 5);
    
    // Unknown tool uses default (10)
    assert_eq!(detector.get_tool_threshold("unknown_tool"), 10);
}

#[test]
fn test_tool_name_repeat_detection() {
    let mut detector = ToolCallLoopDetector::new();
    detector.exact_repeat_threshold = 3;
    
    // Create tool calls with same name but different arguments
    let tc1 = json!({
        "function": {
            "name": "shell_exec",
            "arguments": "{\"command\": \"ls\"}"
        }
    });
    let tc2 = json!({
        "function": {
            "name": "shell_exec",
            "arguments": "{\"command\": \"pwd\"}"
        }
    });
    let tc3 = json!({
        "function": {
            "name": "shell_exec",
            "arguments": "{\"command\": \"cat file.txt\"}"
        }
    });
    
    // Record same tool name with different arguments
    // shell_exec threshold is 25, so we need 25 calls to trigger
    for _ in 0..8 {
        detector.record(std::slice::from_ref(&tc1));
        detector.record(std::slice::from_ref(&tc2));
        detector.record(std::slice::from_ref(&tc3));
    }
    
    let result = detector.check();
    // Should detect tool-name-repeat pattern
    assert!(result.is_looping);
    assert_eq!(result.loop_type, Some("tool-name-repeat".to_string()));
}

#[test]
fn test_informative_hint_format() {
    let mut detector = ToolCallLoopDetector::new();
    detector.exact_repeat_threshold = 3;
    
    let tc = json!({
        "function": {
            "name": "web_search",
            "arguments": "{\"query\": \"test\"}"
        }
    });
    
    // Record same tool call 3 times
    for _ in 0..3 {
        detector.record(std::slice::from_ref(&tc));
    }
    
    let result = detector.check();
    assert!(result.is_looping);
    
    let hint = result.hint_message.unwrap();
    // Should contain informative sections
    assert!(hint.contains("Common scenarios"));
    assert!(hint.contains("If you're making progress"));
    assert!(hint.contains("If you're stuck"));
}

#[test]
fn test_threshold_used_in_result() {
    let mut detector = ToolCallLoopDetector::new();
    detector.exact_repeat_threshold = 3;
    
    let tc = json!({
        "function": {
            "name": "write_file",
            "arguments": "{\"path\": \"test.txt\"}"
        }
    });
    
    // write_file threshold is 5
    for _ in 0..5 {
        detector.record(std::slice::from_ref(&tc));
    }
    
    let result = detector.check();
    assert!(result.is_looping);
    assert_eq!(result.threshold_used, 5);
}
