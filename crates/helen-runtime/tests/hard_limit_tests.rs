//! Hard Limit Trim tests (P1 fix)
//!
//! Tests for the 100% safety net that prevents context overflow
//! when all graduated compression layers are insufficient.

use helen_runtime::compression::{graduated_compress, hard_limit_trim};
use helen_runtime::transcript::Message;

fn make_message(role: &str, content: &str) -> Message {
    Message::new(
        role,
        serde_json::Value::String(content.to_string()),
        Vec::new(),
        None,
        String::new(),
        None,
        50,
        false,
        false,
        None,
        String::new(),
        String::new(),
        Vec::new(),
    )
}

fn make_large_message(role: &str, size: usize) -> Message {
    let content = "x".repeat(size);
    make_message(role, &content)
}

#[test]
fn hard_limit_trim_preserves_system_messages() {
    let history = vec![
        make_message("system", "System prompt"),
        make_message("user", "User message 1"),
        make_message("assistant", "Assistant response 1"),
        make_message("user", "User message 2"),
        make_message("assistant", "Assistant response 2"),
    ];

    let result = hard_limit_trim(&history, 1000);

    // System messages should always be preserved
    assert!(result.iter().any(|m| m.role == "system"));
}

#[test]
fn hard_limit_trim_preserves_pinned_messages() {
    let mut pinned_msg = make_message("user", "Important pinned message");
    pinned_msg.pinned = true;

    let history = vec![
        make_message("user", "Regular message 1"),
        pinned_msg.clone(),
        make_message("user", "Regular message 2"),
        make_message("assistant", "Response"),
    ];

    let result = hard_limit_trim(&history, 1000);

    // Pinned messages should be preserved
    assert!(result.iter().any(|m| m.pinned));
}

#[test]
fn hard_limit_trim_keeps_only_last_two_conversation_messages() {
    let history = vec![
        make_message("system", "System"),
        make_message("user", "Message 1"),
        make_message("assistant", "Response 1"),
        make_message("user", "Message 2"),
        make_message("assistant", "Response 2"),
        make_message("user", "Message 3"),
        make_message("assistant", "Response 3"),
    ];

    let result = hard_limit_trim(&history, 10000);

    // Should keep system + last 2 conversation messages
    let conversation_msgs: Vec<_> = result.iter().filter(|m| m.role != "system").collect();
    assert!(conversation_msgs.len() <= 2);
}

#[test]
fn hard_limit_trim_truncates_large_messages_when_still_over_budget() {
    // Create messages that exceed the budget even after keeping only 2
    let history = vec![
        make_message("system", "System"),
        make_large_message("user", 5000),
        make_large_message("assistant", 5000),
    ];

    // Budget is very small (100 tokens ≈ 400 chars)
    let result = hard_limit_trim(&history, 100);

    // Messages should be truncated
    for msg in &result {
        if msg.role != "system" {
            let text = match &msg.content {
                serde_json::Value::String(s) => s.len(),
                _ => 0,
            };
            // Should be truncated to ~200 chars
            assert!(text < 500, "Message should be truncated");
        }
    }
}

#[test]
fn hard_limit_trim_returns_empty_for_empty_history() {
    let history: Vec<Message> = vec![];
    let result = hard_limit_trim(&history, 1000);
    assert!(result.is_empty());
}

#[test]
fn hard_limit_trim_returns_original_when_under_budget() {
    let history = vec![make_message("user", "Short message")];

    let result = hard_limit_trim(&history, 10000);
    assert_eq!(result.len(), history.len());
}

#[test]
fn graduated_compress_applies_hard_limit_at_100_percent() {
    // Create a scenario where all layers are insufficient
    let history = vec![
        make_message("system", "System prompt"),
        make_large_message("user", 50000), // Very large single message
    ];

    // Usage ratio is way over 100%
    let (result, layer) = graduated_compress(&history, 1.5, Some(1000));

    // Should apply hard_limit layer
    assert_eq!(layer, "hard_limit");

    // Result should be smaller than input
    let result_tokens: usize = result.iter().map(|m| m.token_count()).sum();
    assert!(result_tokens < 50000 / 4); // Should be significantly reduced
}

#[test]
fn graduated_compress_hard_limit_preserves_critical_messages() {
    let mut pinned_msg = make_message("user", "Critical pinned info");
    pinned_msg.pinned = true;

    let history = vec![
        make_message("system", "System prompt"),
        pinned_msg,
        make_large_message("user", 50000),
        make_large_message("assistant", 50000),
    ];

    let (result, layer) = graduated_compress(&history, 2.0, Some(1000));

    assert_eq!(layer, "hard_limit");

    // System and pinned messages should be preserved
    assert!(result.iter().any(|m| m.role == "system"));
    assert!(result.iter().any(|m| m.pinned));
}
