//! Tests for three-level graduated loop intervention (v1.46.37).
//!
//! Verifies that:
//! - L1: First detection injects a warning hint, allows LLM to continue
//! - L2: Second detection injects force-summary prompt, allows LLM to produce final answer
//! - L3: Third detection (after L2) terminates the agent
//!
//! v1.46.38: Updated thresholds and added argument diversity check.

use helen_runtime::loop_detector::ToolCallLoopDetector;
use serde_json::json;

#[test]
fn test_per_tool_thresholds() {
    let detector = ToolCallLoopDetector::new();

    // shell_exec has high tolerance (50) — v1.46.38: increased from 25
    assert_eq!(detector.get_tool_threshold("shell_exec"), 50);

    // write_file has moderate threshold (15) — v1.46.38: increased from 5
    assert_eq!(detector.get_tool_threshold("write_file"), 15);

    // Unknown tool uses default (20) — v1.46.38: increased from 10
    assert_eq!(detector.get_tool_threshold("unknown_tool"), 20);
}

#[test]
fn test_tool_name_repeat_detection() {
    let mut detector = ToolCallLoopDetector::new();
    detector.exact_repeat_threshold = 3;

    // v1.46.38: Use enough unique arguments to avoid Strategy 1 (exact) and 
    // Strategy 2 (window pattern), but keep diversity below 80% to trigger Strategy 3.
    // Strategy 2 triggers when same call appears 5+ times in window of 10.
    // We'll use 10 unique args repeated to get 10/15 = 67% diversity (<80%).
    let mut tool_calls = Vec::new();
    for i in 0..10 {
        tool_calls.push(json!({
            "function": {
                "name": "write_file",
                "arguments": format!("{{\"path\": \"file_{}.txt\"}}", i)
            }
        }));
    }

    // write_file threshold is 15. Record each of 10 unique calls, then repeat 5.
    // This gives 10 unique args in 15 calls = 67% diversity (<80% → triggers)
    for tc in &tool_calls {
        detector.record(std::slice::from_ref(tc));
    }
    // Repeat first 5 to reach threshold of 15
    for tc in &tool_calls[..5] {
        detector.record(std::slice::from_ref(tc));
    }

    let result = detector.check();
    // Should detect tool-name-repeat pattern (diversity: 10/15 = 67% < 80%)
    assert!(result.is_looping);
    assert_eq!(result.loop_type, Some("tool-name-repeat".to_string()));
}

#[test]
fn test_tool_name_repeat_diverse_args_not_loop() {
    // v1.46.38: Verify that diverse arguments are NOT flagged as a loop.
    let mut detector = ToolCallLoopDetector::new();
    detector.exact_repeat_threshold = 3;

    // Create tool calls with same name but ALL different arguments (diverse)
    let mut tool_calls = Vec::new();
    for i in 0..50 {
        tool_calls.push(json!({
            "function": {
                "name": "shell_exec",
                "arguments": format!("{{\"command\": \"cmd_{}\"}}", i)
            }
        }));
    }

    // Record 50 different shell_exec calls (threshold is 50)
    for tc in &tool_calls {
        detector.record(std::slice::from_ref(tc));
    }

    let result = detector.check();
    // Should NOT detect loop — all arguments are unique (>80% diversity)
    assert!(!result.is_looping, "Diverse args should not trigger loop detection");
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

    // Record same tool call 25 times (web_search threshold is 25) — v1.46.38
    for _ in 0..25 {
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

    // write_file threshold is 15 — v1.46.38: increased from 5
    for _ in 0..15 {
        detector.record(std::slice::from_ref(&tc));
    }

    let result = detector.check();
    assert!(result.is_looping);
    assert_eq!(result.threshold_used, 15);
}
