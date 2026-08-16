//! Tests for provider module — platform protocol abstraction.

use helen_runtime::provider::*;
use serde_json::json;

// ── PlatformKind ────────────────────────────────────────────────────────

#[test]
fn platform_kind_name_openai() {
    let pk = PlatformKind::OpenAI;
    assert_eq!(pk.name(), "openai");
}

#[test]
fn platform_kind_name_dashscope() {
    let pk = PlatformKind::DashScope;
    assert_eq!(pk.name(), "dashscope");
}

#[test]
fn platform_kind_name_volcengine() {
    let pk = PlatformKind::Volcengine;
    assert_eq!(pk.name(), "volcengine");
}

#[test]
fn platform_kind_name_zhipu() {
    let pk = PlatformKind::Zhipu;
    assert_eq!(pk.name(), "zhipu");
}

#[test]
fn platform_kind_name_deepseek() {
    let pk = PlatformKind::DeepSeek;
    assert_eq!(pk.name(), "deepseek");
}

#[test]
fn platform_kind_name_minimax() {
    let pk = PlatformKind::Minimax;
    assert_eq!(pk.name(), "minimax");
}

#[test]
fn platform_kind_name_kimi() {
    let pk = PlatformKind::Kimi;
    assert_eq!(pk.name(), "kimi");
}

// ── protocol_by_name ────────────────────────────────────────────────────

#[test]
fn protocol_by_name_openai() {
    let pk = protocol_by_name("openai");
    assert_eq!(pk.name(), "openai");
}

#[test]
fn protocol_by_name_dashscope() {
    let pk = protocol_by_name("dashscope");
    assert_eq!(pk.name(), "dashscope");
}

#[test]
fn protocol_by_name_volcengine() {
    let pk = protocol_by_name("volcengine");
    assert_eq!(pk.name(), "volcengine");
}

#[test]
fn protocol_by_name_zhipu() {
    let pk = protocol_by_name("zhipu");
    assert_eq!(pk.name(), "zhipu");
}

#[test]
fn protocol_by_name_deepseek() {
    let pk = protocol_by_name("deepseek");
    assert_eq!(pk.name(), "deepseek");
}

#[test]
fn protocol_by_name_minimax() {
    let pk = protocol_by_name("minimax");
    assert_eq!(pk.name(), "minimax");
}

#[test]
fn protocol_by_name_kimi() {
    let pk = protocol_by_name("kimi");
    assert_eq!(pk.name(), "kimi");
}

#[test]
fn protocol_by_name_unknown() {
    let pk = protocol_by_name("nonexistent");
    assert_eq!(pk.name(), "openai");
}

// ── detect_protocol ─────────────────────────────────────────────────────

#[test]
fn detect_protocol_dashscope_url() {
    let pk = detect_protocol("https://dashscope.aliyuncs.com", None);
    assert_eq!(pk.name(), "dashscope");
}

#[test]
fn detect_protocol_volcengine_url() {
    let pk = detect_protocol("https://ark.cn-beijing.volces.com", None);
    assert_eq!(pk.name(), "volcengine");
}

#[test]
fn detect_protocol_zhipu_url() {
    let pk = detect_protocol("https://open.bigmodel.cn", None);
    assert_eq!(pk.name(), "zhipu");
}

#[test]
fn detect_protocol_deepseek_url() {
    let pk = detect_protocol("https://api.deepseek.com", None);
    assert_eq!(pk.name(), "deepseek");
}

#[test]
fn detect_protocol_minimax_url() {
    let pk = detect_protocol("https://api.minimaxi.com", None);
    assert_eq!(pk.name(), "minimax");
}

#[test]
fn detect_protocol_kimi_url() {
    let pk = detect_protocol("https://api.moonshot.ai", None);
    assert_eq!(pk.name(), "kimi");
}

#[test]
fn detect_protocol_explicit_name() {
    let pk = detect_protocol("https://example.com", Some("dashscope"));
    assert_eq!(pk.name(), "dashscope");
}

#[test]
fn detect_protocol_unknown_url() {
    let pk = detect_protocol("https://example.com", None);
    assert_eq!(pk.name(), "openai");
}

// ── PlatformProtocol trait ──────────────────────────────────────────────

#[test]
fn openai_protocol_build_request() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    let payload = json!({"model": "gpt-4", "messages": []});
    let result = protocol.build_request_payload(payload.clone(), "gpt-4", false, None);
    assert_eq!(result, payload);
}

#[test]
fn openai_protocol_supports_tool_choice() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    assert!(protocol.supports_tool_choice("auto"));
    assert!(protocol.supports_tool_choice("required"));
}

#[test]
fn openai_protocol_sanitize_messages() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    let messages = vec![json!({"role": "user", "content": "hello"})];
    let result = protocol.sanitize_messages(messages.clone());
    assert_eq!(result, messages);
}

#[test]
fn openai_protocol_parse_response() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    let response = json!({
        "choices": [{
            "message": {"content": "hello"},
            "finish_reason": "stop"
        }]
    });
    let result = protocol.parse_response(&response);
    assert!(result.is_object());
}

#[test]
fn openai_protocol_parse_streaming_delta() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    let delta = json!({
        "content": "hello",
        "finish_reason": null
    });
    let mut context = json!({});
    let result = protocol.parse_streaming_delta(&delta, &mut context);
    assert!(result.is_object());
    assert!(result.get("content").is_some());
}

#[test]
fn openai_protocol_extract_streaming_usage() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    let chunk = json!({
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}
    });
    let usage = protocol.extract_streaming_usage(&chunk);
    assert!(usage.is_some());
}

#[test]
fn openai_protocol_parse_error() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    let body = json!({"error": {"message": "bad request"}});
    let msg = protocol.parse_error(400, &body);
    assert!(msg.contains("bad request"));
}

#[test]
fn openai_protocol_parse_error_no_error_field() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    let body = json!({"message": "something went wrong"});
    let msg = protocol.parse_error(500, &body);
    assert!(!msg.is_empty());
}

#[test]
fn openai_protocol_is_context_overflow_error() {
    let pk = PlatformKind::OpenAI;
    let protocol = pk.protocol();
    assert!(protocol.is_context_overflow_error("context length exceeded"));
    assert!(protocol.is_context_overflow_error("maximum context length"));
    assert!(protocol.is_context_overflow_error("too many tokens"));
    assert!(!protocol.is_context_overflow_error("bad request"));
}

// ── DashScope protocol ──────────────────────────────────────────────────

#[test]
fn dashscope_protocol_name() {
    let pk = PlatformKind::DashScope;
    let protocol = pk.protocol();
    assert_eq!(protocol.name(), "dashscope");
}

#[test]
fn dashscope_protocol_build_request() {
    let pk = PlatformKind::DashScope;
    let protocol = pk.protocol();
    let payload = json!({"model": "qwen", "messages": []});
    let result = protocol.build_request_payload(payload, "qwen", false, None);
    assert!(result.is_object());
}

// ── Volcengine protocol ─────────────────────────────────────────────────

#[test]
fn volcengine_protocol_name() {
    let pk = PlatformKind::Volcengine;
    let protocol = pk.protocol();
    assert_eq!(protocol.name(), "volcengine");
}

// ── DeepSeek protocol ───────────────────────────────────────────────────

#[test]
fn deepseek_protocol_name() {
    let pk = PlatformKind::DeepSeek;
    let protocol = pk.protocol();
    assert_eq!(protocol.name(), "deepseek");
}

// ── Kimi protocol ───────────────────────────────────────────────────────

#[test]
fn kimi_protocol_name() {
    let pk = PlatformKind::Kimi;
    let protocol = pk.protocol();
    assert_eq!(protocol.name(), "kimi");
}

// ── Minimax protocol ────────────────────────────────────────────────────

#[test]
fn minimax_protocol_name() {
    let pk = PlatformKind::Minimax;
    let protocol = pk.protocol();
    assert_eq!(protocol.name(), "minimax");
}

// ── Zhipu protocol ──────────────────────────────────────────────────────

#[test]
fn zhipu_protocol_name() {
    let pk = PlatformKind::Zhipu;
    let protocol = pk.protocol();
    assert_eq!(protocol.name(), "zhipu");
}

// ── custom_protocol_by_name ─────────────────────────────────────────────

#[test]
fn custom_protocol_by_name_unknown() {
    let result = custom_protocol_by_name("nonexistent");
    assert!(result.is_none());
}

// ── PLATFORM_PATTERNS ───────────────────────────────────────────────────

#[test]
fn platform_patterns_not_empty() {
    assert!(!PLATFORM_PATTERNS.is_empty());
}

#[test]
fn platform_patterns_contain_dashscope() {
    let found = PLATFORM_PATTERNS.iter().any(|(_, pk)| *pk == PlatformKind::DashScope);
    assert!(found);
}
