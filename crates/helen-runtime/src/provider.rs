//! Platform-level protocol abstraction (Task 5.3) — port of
//! `helen/runtime/provider_protocol.py`.
//!
//! Protocol is determined by the PLATFORM (base_url), not the model.
//! - DashScope: ALL models use same protocol (Qwen + DeepSeek unified)
//! - Volcengine Ark: ALL models use same protocol (Doubao + third-party)
//! - Direct APIs: each provider has its own protocol

use serde_json::{json, Value};

/// `PlatformProtocol` — platform-specific protocol handling.
/// Default implementation = standard OpenAI protocol.
pub trait PlatformProtocol: Send + Sync {
    fn name(&self) -> &'static str {
        "openai"
    }

    /// Transform payload into platform-specific format. Default: as-is.
    fn build_request_payload(
        &self,
        base_payload: Value,
        model_id: &str,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Value {
        let _ = (model_id, thinking_enabled, reasoning_effort);
        base_payload
    }

    /// Check if platform supports given tool_choice value.
    fn supports_tool_choice(&self, _value: &str) -> bool {
        true
    }

    /// Transform messages into platform-specific format. Default: as-is.
    fn sanitize_messages(&self, messages: Vec<Value>) -> Vec<Value> {
        messages
    }

    /// Parse platform-specific response into standard format:
    /// {content, reasoning_content, tool_calls, finish_reason, usage}.
    fn parse_response(&self, response_data: &Value) -> Value {
        default_parse_response(response_data)
    }

    /// Parse streaming delta into standard format.
    fn parse_streaming_delta(&self, delta: &Value, _context: &mut Value) -> Value {
        json!({
            "content": delta.get("content").and_then(|v| v.as_str()).unwrap_or(""),
            "reasoning_content": delta.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or(""),
            "tool_calls": delta.get("tool_calls").cloned().unwrap_or_else(|| json!([])),
            "finish_reason": delta.get("finish_reason").and_then(|v| v.as_str()),
        })
    }

    /// Extract usage from streaming chunk. Kimi: choices[0].usage.
    fn extract_streaming_usage(&self, chunk: &Value) -> Option<Value> {
        chunk.get("usage").cloned()
    }

    /// Parse platform-specific error format into human-readable string.
    fn parse_error(&self, _status_code: u16, response_body: &Value) -> String {
        let error = response_body.get("error");
        match error {
            Some(Value::Object(m)) => m
                .get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| response_body.to_string()),
            _ => response_body.to_string(),
        }
    }

    /// Check if error indicates context window overflow.
    fn is_context_overflow_error(&self, error_msg: &str) -> bool {
        let markers = [
            "context length",
            "maximum context",
            "too many tokens",
            "reduce your prompt",
            "context overflow",
            "max_tokens",
        ];
        let lower = error_msg.to_lowercase();
        markers.iter().any(|m| lower.contains(m))
    }
}

/// Shared default response parser (extracted so protocol overrides can call
/// the base logic without UFCS dispatching back to the override).
fn default_parse_response(response_data: &Value) -> Value {
    let choices = response_data.get("choices").and_then(|c| c.as_array());
    let choice = choices
        .and_then(|c| c.first())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let message = choice.get("message").cloned().unwrap_or_else(|| json!({}));
    json!({
        "content": message.get("content").and_then(|v| v.as_str()).unwrap_or(""),
        "reasoning_content": message.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or(""),
        "tool_calls": message.get("tool_calls").cloned().unwrap_or_else(|| json!([])),
        "finish_reason": choice.get("finish_reason").and_then(|v| v.as_str()).unwrap_or("stop"),
        "usage": response_data.get("usage").cloned().unwrap_or_else(|| json!({})),
    })
}

// ---------------------------------------------------------------------------
// Platform implementations
// ---------------------------------------------------------------------------

/// 阿里云百炼 (DashScope) — unified protocol for all models.
pub struct DashScopeProtocol;

impl PlatformProtocol for DashScopeProtocol {
    fn name(&self) -> &'static str {
        "dashscope"
    }

    fn build_request_payload(
        &self,
        mut base_payload: Value,
        _model_id: &str,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Value {
        if thinking_enabled {
            if let Value::Object(m) = &mut base_payload {
                m.insert("enable_thinking".into(), Value::Bool(true));
                if let Some(effort) = reasoning_effort {
                    let budget_map = [
                        ("low", 1024),
                        ("medium", 4096),
                        ("high", 16384),
                        ("max", 32768),
                    ];
                    let budget = budget_map
                        .iter()
                        .find(|(k, _)| *k == effort)
                        .map(|(_, v)| *v)
                        .unwrap_or(4096);
                    m.insert("thinking_budget".into(), json!(budget));
                }
            }
        }
        base_payload
    }
}

/// 火山引擎方舟 (Volcengine Ark) — Endpoint ID (ep-XXXXX) in model field.
pub struct VolcengineProtocol;

impl PlatformProtocol for VolcengineProtocol {
    fn name(&self) -> &'static str {
        "volcengine"
    }

    fn build_request_payload(
        &self,
        mut base_payload: Value,
        _model_id: &str,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Value {
        // v1.37: validate endpoint id format (ep-XXXXX); log warning otherwise.
        if thinking_enabled {
            if let Value::Object(m) = &mut base_payload {
                m.insert("thinking".into(), json!({"type": "enabled"}));
                if let Some(effort) = reasoning_effort {
                    m.insert("reasoning_effort".into(), json!(effort));
                }
            }
        }
        base_payload
    }
}

/// 智谱 (Zhipu) — only supports tool_choice "auto".
pub struct ZhipuProtocol;

impl PlatformProtocol for ZhipuProtocol {
    fn name(&self) -> &'static str {
        "zhipu"
    }

    fn build_request_payload(
        &self,
        mut base_payload: Value,
        _model_id: &str,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Value {
        if thinking_enabled {
            if let Value::Object(m) = &mut base_payload {
                m.insert("thinking".into(), json!({"type": "enabled"}));
                if let Some(effort) = reasoning_effort {
                    m.insert("reasoning_effort".into(), json!(effort));
                }
            }
        }
        base_payload
    }

    fn supports_tool_choice(&self, value: &str) -> bool {
        value == "auto"
    }
}

/// DeepSeek — direct API. reasoning_content/content mutually exclusive per
/// chunk; requires reasoning_content in multi-turn tool calls.
pub struct DeepSeekProtocol;

impl PlatformProtocol for DeepSeekProtocol {
    fn name(&self) -> &'static str {
        "deepseek"
    }

    fn build_request_payload(
        &self,
        mut base_payload: Value,
        _model_id: &str,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Value {
        if let Value::Object(m) = &mut base_payload {
            if thinking_enabled {
                m.insert("thinking".into(), json!({"type": "enabled"}));
            }
            if let Some(effort) = reasoning_effort {
                m.insert("reasoning_effort".into(), json!(effort));
            }
        }
        base_payload
    }
}

/// MiniMax — reasoning_split=true; reasoning_details CUMULATIVE in streaming.
pub struct MinimaxProtocol;

impl PlatformProtocol for MinimaxProtocol {
    fn name(&self) -> &'static str {
        "minimax"
    }

    fn build_request_payload(
        &self,
        mut base_payload: Value,
        _model_id: &str,
        thinking_enabled: bool,
        _reasoning_effort: Option<&str>,
    ) -> Value {
        if let Value::Object(m) = &mut base_payload {
            m.insert("reasoning_split".into(), Value::Bool(true));
            if thinking_enabled {
                m.insert("thinking".into(), json!({"type": "adaptive"}));
            }
        }
        base_payload
    }

    fn parse_response(&self, response_data: &Value) -> Value {
        let mut result = default_parse_response(response_data);
        let choices = response_data.get("choices").and_then(|c| c.as_array());
        let choice = choices
            .and_then(|c| c.first())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let message = choice.get("message").cloned().unwrap_or_else(|| json!({}));
        if message.get("reasoning_details").is_some() {
            if let Value::Object(m) = &mut result {
                m.insert(
                    "reasoning_details".into(),
                    message.get("reasoning_details").cloned().unwrap_or_default(),
                );
            }
        }
        result
    }

    fn parse_streaming_delta(&self, delta: &Value, context: &mut Value) -> Value {
        let mut result = json!({
            "content": delta.get("content").and_then(|v| v.as_str()).unwrap_or(""),
            "reasoning_content": delta.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or(""),
            "tool_calls": delta.get("tool_calls").cloned().unwrap_or_else(|| json!([])),
            "finish_reason": delta.get("finish_reason").and_then(|v| v.as_str()),
        });
        // reasoning_details is cumulative — compute incremental delta.
        if let Some(current) = delta.get("reasoning_details") {
            let prev_total = context
                .get("reasoning_details_total")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(cur_str) = current.as_str() {
                if cur_str.starts_with(&prev_total) {
                    if let Value::Object(m) = &mut result {
                        m.insert(
                            "reasoning_content".into(),
                            json!(cur_str[prev_total.len()..].to_string()),
                        );
                    }
                }
            }
            context["reasoning_details_total"] = current.clone();
        }
        result
    }
}

/// Kimi/Moonshot — usage at choices[0].usage in streaming.
pub struct KimiProtocol;

impl PlatformProtocol for KimiProtocol {
    fn name(&self) -> &'static str {
        "kimi"
    }

    fn build_request_payload(
        &self,
        mut base_payload: Value,
        _model_id: &str,
        _thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Value {
        if let Some(effort) = reasoning_effort {
            if let Value::Object(m) = &mut base_payload {
                m.insert("reasoning_effort".into(), json!(effort));
            }
        }
        base_payload
    }
    fn extract_streaming_usage(&self, chunk: &Value) -> Option<Value> {
        // Kimi: usage at choices[0].usage, not top-level
        if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
            if let Some(first) = choices.first() {
                if let Some(usage) = first.get("usage") {
                    return Some(usage.clone());
                }
            }
        }
        chunk.get("usage").cloned()
    }
}

/// Standard OpenAI protocol (default).
pub struct OpenAIProtocol;

impl PlatformProtocol for OpenAIProtocol {}

// ---------------------------------------------------------------------------
// Auto-detection
// ---------------------------------------------------------------------------

/// URL pattern -> protocol constructor.
pub const PLATFORM_PATTERNS: &[(&str, PlatformKind)] = &[
    ("dashscope.aliyuncs.com", PlatformKind::DashScope),
    ("ark.cn-beijing.volces.com", PlatformKind::Volcengine),
    ("open.bigmodel.cn", PlatformKind::Zhipu),
    ("api.deepseek.com", PlatformKind::DeepSeek),
    ("api.minimaxi.com", PlatformKind::Minimax),
    ("api.minimax.io", PlatformKind::Minimax),
    ("api.moonshot.ai", PlatformKind::Kimi),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformKind {
    DashScope,
    Volcengine,
    Zhipu,
    DeepSeek,
    Minimax,
    Kimi,
    OpenAI,
}

impl PlatformKind {
    pub fn name(&self) -> &'static str {
        match self {
            PlatformKind::DashScope => "dashscope",
            PlatformKind::Volcengine => "volcengine",
            PlatformKind::Zhipu => "zhipu",
            PlatformKind::DeepSeek => "deepseek",
            PlatformKind::Minimax => "minimax",
            PlatformKind::Kimi => "kimi",
            PlatformKind::OpenAI => "openai",
        }
    }

    pub fn protocol(&self) -> Box<dyn PlatformProtocol> {
        match self {
            PlatformKind::DashScope => Box::new(DashScopeProtocol),
            PlatformKind::Volcengine => Box::new(VolcengineProtocol),
            PlatformKind::Zhipu => Box::new(ZhipuProtocol),
            PlatformKind::DeepSeek => Box::new(DeepSeekProtocol),
            PlatformKind::Minimax => Box::new(MinimaxProtocol),
            PlatformKind::Kimi => Box::new(KimiProtocol),
            PlatformKind::OpenAI => Box::new(OpenAIProtocol),
        }
    }
}

/// Protocol name -> kind (includes "openai" fallback).
pub fn protocol_by_name(name: &str) -> PlatformKind {
    match name {
        "dashscope" => PlatformKind::DashScope,
        "volcengine" => PlatformKind::Volcengine,
        "zhipu" => PlatformKind::Zhipu,
        "deepseek" => PlatformKind::DeepSeek,
        "minimax" => PlatformKind::Minimax,
        "kimi" => PlatformKind::Kimi,
        _ => PlatformKind::OpenAI,
    }
}

// ---------------------------------------------------------------------------
// Custom provider registry (M10 — Python FFI custom provider loader).
// Port of `_PROTOCOL_NAME_MAP` custom half + `_load_custom_providers`:
// user-defined `~/.helen/providers/*.py` protocols are loaded through the
// helen-ffi crate and registered here. Built-in names cannot be overridden.
// ---------------------------------------------------------------------------

static CUSTOM_PROTOCOLS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<dyn PlatformProtocol>>>,
> = std::sync::OnceLock::new();

/// Register a custom (Python-loaded) protocol. Built-in names are ignored
/// (Python: "shadows built-in; skipping"). Returns the registered name.
pub fn register_custom_protocol(
    name: &str,
    proto: std::sync::Arc<dyn PlatformProtocol>,
) -> Option<String> {
    let builtin = [
        "dashscope",
        "volcengine",
        "zhipu",
        "deepseek",
        "minimax",
        "kimi",
        "openai",
    ];
    if builtin.contains(&name) {
        return None;
    }
    let map =
        CUSTOM_PROTOCOLS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    map.lock().expect("mutex poisoned").insert(name.to_string(), proto);
    Some(name.to_string())
}

/// Look up a custom protocol by name (None if not registered).
pub fn custom_protocol_by_name(name: &str) -> Option<std::sync::Arc<dyn PlatformProtocol>> {
    let map = CUSTOM_PROTOCOLS.get()?;
    let guard = map.lock().expect("mutex poisoned");
    guard.get(name).cloned()
}

/// Delegating proxy so an `Arc<dyn PlatformProtocol>` can produce a
/// `Box<dyn PlatformProtocol>` (trait objects aren't cloneable directly).
pub struct ProtocolProxy(pub std::sync::Arc<dyn PlatformProtocol>);

impl PlatformProtocol for ProtocolProxy {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn build_request_payload(
        &self,
        base_payload: Value,
        model_id: &str,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Value {
        self.0
            .build_request_payload(base_payload, model_id, thinking_enabled, reasoning_effort)
    }
    fn supports_tool_choice(&self, value: &str) -> bool {
        self.0.supports_tool_choice(value)
    }
    fn sanitize_messages(&self, messages: Vec<Value>) -> Vec<Value> {
        self.0.sanitize_messages(messages)
    }
    fn parse_response(&self, response_data: &Value) -> Value {
        self.0.parse_response(response_data)
    }
    fn parse_streaming_delta(&self, delta: &Value, context: &mut Value) -> Value {
        self.0.parse_streaming_delta(delta, context)
    }
    fn extract_streaming_usage(&self, chunk: &Value) -> Option<Value> {
        self.0.extract_streaming_usage(chunk)
    }
    fn parse_error(&self, status_code: u16, response_body: &Value) -> String {
        self.0.parse_error(status_code, response_body)
    }
    fn is_context_overflow_error(&self, error_msg: &str) -> bool {
        self.0.is_context_overflow_error(error_msg)
    }
}

/// Box a custom protocol Arc as a fresh `Box<dyn PlatformProtocol>`.
pub fn box_custom_protocol(arc: std::sync::Arc<dyn PlatformProtocol>) -> Box<dyn PlatformProtocol> {
    Box::new(ProtocolProxy(arc))
}

/// Detect platform protocol from base_url or explicit name.
/// Priority: explicit protocol_name > URL pattern > OpenAI fallback.
pub fn detect_protocol(base_url: &str, protocol_name: Option<&str>) -> PlatformKind {
    if let Some(name) = protocol_name {
        if !name.is_empty() {
            return protocol_by_name(name);
        }
    }
    for (pattern, kind) in PLATFORM_PATTERNS {
        if base_url.contains(pattern) {
            return *kind;
        }
    }
    PlatformKind::OpenAI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_dashscope() {
        assert_eq!(
            detect_protocol("https://dashscope.aliyuncs.com/v1", None),
            PlatformKind::DashScope
        );
    }

    #[test]
    fn test_detect_deepseek() {
        assert_eq!(
            detect_protocol("https://api.deepseek.com", None),
            PlatformKind::DeepSeek
        );
    }

    #[test]
    fn test_detect_explicit_name() {
        assert_eq!(
            detect_protocol("https://unknown.com", Some("kimi")),
            PlatformKind::Kimi
        );
    }

    #[test]
    fn test_detect_fallback_openai() {
        assert_eq!(
            detect_protocol("https://unknown.com", None),
            PlatformKind::OpenAI
        );
    }

    #[test]
    fn test_dashscope_thinking_payload() {
        let p = DashScopeProtocol;
        let payload = p.build_request_payload(
            json!({"model": "qwen-max", "messages": []}),
            "qwen-max",
            true,
            Some("high"),
        );
        assert_eq!(payload["enable_thinking"], true);
        assert_eq!(payload["thinking_budget"], 16384);
    }

    #[test]
    fn test_zhipu_tool_choice() {
        let p = ZhipuProtocol;
        assert!(p.supports_tool_choice("auto"));
        assert!(!p.supports_tool_choice("required"));
    }

    #[test]
    fn test_parse_response_standard() {
        let p = OpenAIProtocol;
        let resp = json!({
            "choices": [{
                "message": {"content": "hi", "tool_calls": []},
                "finish_reason": "stop"
            }],
            "usage": {"total_tokens": 5}
        });
        let parsed = p.parse_response(&resp);
        assert_eq!(parsed["content"], "hi");
        assert_eq!(parsed["finish_reason"], "stop");
    }

    #[test]
    fn test_kimi_streaming_usage() {
        let p = KimiProtocol;
        let chunk = json!({"choices": [{"usage": {"total_tokens": 3}}]});
        let usage = p.extract_streaming_usage(&chunk);
        assert!(usage.is_some());
        assert_eq!(usage.unwrap()["total_tokens"], 3);
    }

    #[test]
    fn test_context_overflow_markers() {
        let p = OpenAIProtocol;
        assert!(p.is_context_overflow_error("This model's maximum context length is 8192 tokens"));
        assert!(!p.is_context_overflow_error("rate limit exceeded"));
    }

    #[test]
    fn test_minimax_cumulative_delta() {
        let p = MinimaxProtocol;
        let mut ctx = json!({});
        let d1 = p.parse_streaming_delta(&json!({"reasoning_details": "think"}), &mut ctx);
        assert_eq!(d1["reasoning_content"], "think");
        let d2 = p.parse_streaming_delta(&json!({"reasoning_details": "thinking"}), &mut ctx);
        assert_eq!(d2["reasoning_content"], "ing");
    }
}
