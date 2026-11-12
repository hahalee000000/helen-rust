//! Compression integration tests (P0 fix)
//!
//! Verifies that graduated compression is applied to conversation history
//! before it's sent to the LLM, matching Python's _prepare_history_for_llm behavior.

use helen_runtime::compression::{calculate_usage_ratio, graduated_compress};
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

/// Convert history messages to OpenAI-compatible JSON format (Python parity).
fn messages_to_json(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| {
            let mut msg = serde_json::json!({
                "role": m.role,
                "content": m.content,
            });
            if !m.tool_calls.is_empty() {
                msg["tool_calls"] = serde_json::Value::Array(m.tool_calls.clone());
            }
            if let Some(ref tcid) = m.tool_call_id {
                msg["tool_call_id"] = serde_json::Value::String(tcid.clone());
            }
            msg
        })
        .collect()
}

/// Convert JSON history (from interpreter) to Message objects.
fn json_to_messages(json_history: &[serde_json::Value]) -> Vec<Message> {
    json_history
        .iter()
        .map(|entry| {
            let role = entry.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = entry
                .get("content")
                .cloned()
                .unwrap_or(serde_json::Value::String(String::new()));
            Message::new(
                role,
                content,
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
        })
        .collect()
}

/// Simulate Python's _prepare_history_for_llm flow:
/// 1. Convert JSON history to Messages
/// 2. Calculate usage ratio
/// 3. Apply graduated compression
/// 4. Convert back to JSON
fn prepare_history_for_llm(
    json_history: &[serde_json::Value],
    system_prompt: Option<&str>,
    max_tokens: usize,
) -> Vec<serde_json::Value> {
    let messages = json_to_messages(json_history);
    let system_tokens = system_prompt
        .map(helen_runtime::token::estimate_tokens_simple)
        .unwrap_or(0);

    let total_tokens: usize = messages.iter().map(|m| m.token_count()).sum();
    let usage_ratio = (total_tokens + system_tokens) as f64 / max_tokens as f64;

    let (compressed, _layer) = graduated_compress(&messages, usage_ratio, Some(max_tokens));
    messages_to_json(&compressed)
}

#[test]
fn prepare_history_compresses_large_history() {
    // Simulate a conversation with many large tool results
    let mut json_history = Vec::new();
    for i in 0..20 {
        json_history.push(serde_json::json!({
            "role": "user",
            "content": format!("Question {}", i),
        }));
        json_history.push(serde_json::json!({
            "role": "assistant",
            "content": format!("Answer {}", i),
        }));
        json_history.push(serde_json::json!({
            "role": "tool",
            "content": "x".repeat(5000), // Large tool output
            "tool_call_id": format!("tool_{}", i),
        }));
    }

    let max_tokens = 10000;
    let compressed = prepare_history_for_llm(&json_history, Some("System prompt"), max_tokens);

    // Compressed history should be smaller than original
    let original_tokens: usize = json_history
        .iter()
        .map(|v| {
            v.get("content")
                .and_then(|c| c.as_str())
                .map(helen_runtime::token::estimate_tokens_simple)
                .unwrap_or(0)
        })
        .sum();

    let compressed_tokens: usize = compressed
        .iter()
        .map(|v| {
            v.get("content")
                .and_then(|c| c.as_str())
                .map(helen_runtime::token::estimate_tokens_simple)
                .unwrap_or(0)
        })
        .sum();

    assert!(
        compressed_tokens < original_tokens,
        "Compressed tokens ({}) should be less than original ({})",
        compressed_tokens,
        original_tokens
    );
}

#[test]
fn prepare_history_preserves_recent_messages() {
    let mut json_history = Vec::new();
    for i in 0..10 {
        json_history.push(serde_json::json!({
            "role": "user",
            "content": format!("Question {}", i),
        }));
        json_history.push(serde_json::json!({
            "role": "assistant",
            "content": format!("Answer {}", i),
        }));
    }

    let max_tokens = 100000; // Large budget, no compression needed
    let compressed = prepare_history_for_llm(&json_history, None, max_tokens);

    // With large budget, all messages should be preserved
    assert_eq!(compressed.len(), json_history.len());
}

#[test]
fn prepare_history_handles_empty_history() {
    let json_history: Vec<serde_json::Value> = vec![];
    let compressed = prepare_history_for_llm(&json_history, None, 10000);
    assert!(compressed.is_empty());
}

#[test]
fn graduated_compress_reduces_usage_ratio() {
    // Create history that's over 80% usage
    let mut history = vec![make_message("system", "System prompt")];
    for i in 0..10 {
        history.push(make_message("user", &format!("Question {}", i)));
        history.push(make_large_message("tool", 3000));
    }

    let max_tokens = 5000;
    let initial_ratio = calculate_usage_ratio(&history, max_tokens);
    assert!(initial_ratio > 0.8, "Initial ratio should be high");

    let (compressed, layer) = graduated_compress(&history, initial_ratio, Some(max_tokens));
    let final_ratio = calculate_usage_ratio(&compressed, max_tokens);

    assert!(
        final_ratio < initial_ratio,
        "Final ratio ({}) should be less than initial ({})",
        final_ratio,
        initial_ratio
    );
    assert_ne!(
        layer, "none",
        "Some compression layer should have been applied"
    );
}

#[test]
fn json_roundtrip_preserves_message_content() {
    let messages = vec![
        make_message("user", "Hello"),
        make_message("assistant", "Hi there"),
        make_message("tool", "Tool result"),
    ];

    let json = messages_to_json(&messages);
    let restored = json_to_messages(&json);

    assert_eq!(restored.len(), messages.len());
    for (orig, rest) in messages.iter().zip(restored.iter()) {
        assert_eq!(orig.role, rest.role);
        assert_eq!(orig.content, rest.content);
    }
}
