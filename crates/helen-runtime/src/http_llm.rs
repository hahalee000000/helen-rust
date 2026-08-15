//! HTTP LLM client (Task 5.2) — port of `helen/runtime/http_llm.py`.
//!
//! Blocking OpenAI-compatible chat completions over `ureq`, with:
//! - provider protocol payload transform / response parsing (Task 5.3)
//! - retry/backoff for transient errors
//! - tool-calling loop (concurrent + sequential)
//! - SSE streaming with incremental delta parsing + usage extraction
//! - message sanitization + tool result truncation

use crate::llm::{LlmResponse, LlmRuntime};
use crate::provider::{detect_protocol, PlatformKind, PlatformProtocol};
use serde_json::{json, Value};
use std::time::Duration;

pub const MAX_TOOL_RESULT_CHARS: usize = 3000;
pub const MAX_TOOL_RESULTS_PER_TURN: usize = 12;

// ---------------------------------------------------------------------------
// Helpers (module-level functions from http_llm.py)
// ---------------------------------------------------------------------------

/// `_last_user_message_matches` — True if the last message is a user message
/// with content equal to `prompt` (string or multimodal list).
fn last_user_message_matches(messages: &[Value], prompt: &str) -> bool {
    let Some(last) = messages.last() else {
        return false;
    };
    if last.get("role").and_then(|v| v.as_str()) != Some("user") {
        return false;
    }
    match last.get("content") {
        Some(Value::String(s)) => s == prompt,
        Some(Value::Array(items)) => {
            // Multimodal: match if any text part equals the prompt.
            items.iter().any(|part| {
                part.get("type").and_then(|v| v.as_str()) == Some("text")
                    && part.get("text").and_then(|v| v.as_str()) == Some(prompt)
            })
        }
        _ => false,
    }
}

/// `_sanitize_messages` — replace lone surrogate code points (Python parity).
fn sanitize_messages(messages: &mut [Value]) -> usize {
    let mut count = 0;
    for m in messages.iter_mut() {
        if let Some(content) = m.get_mut("content") {
            if let Value::String(s) = content {
                if s.contains('\u{FFFD}')
                    || s.chars().any(|c| (0xD800..0xE000).contains(&(c as u32)))
                {
                    let cleaned: String = s
                        .chars()
                        .map(|c| {
                            let cp = c as u32;
                            if (0xD800..0xE000).contains(&cp) {
                                count += 1;
                                '\u{FFFD}'
                            } else {
                                c
                            }
                        })
                        .collect();
                    *content = Value::String(cleaned);
                }
            }
        }
    }
    count
}

/// `_repair_message_sequence` — fix role alternation (no consecutive same role
/// except tool->assistant patterns). Returns number of repairs.
fn repair_message_sequence(messages: &mut Vec<Value>) -> usize {
    let mut repairs = 0;
    let mut i = 1;
    while i < messages.len() {
        let prev_role = messages[i - 1]
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let cur_role = messages[i]
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // tool messages are always allowed after assistant (tool results)
        if cur_role == prev_role && cur_role != "tool" {
            // Insert an empty assistant message to break alternation
            messages.insert(i, json!({"role": "assistant", "content": ""}));
            repairs += 1;
        }
        i += 1;
    }
    repairs
}

/// `_truncate_tool_result` — truncate oversized tool results.
fn truncate_tool_result(result: &str) -> String {
    if result.chars().count() > MAX_TOOL_RESULT_CHARS {
        let truncated: String = result.chars().take(MAX_TOOL_RESULT_CHARS).collect();
        format!("{truncated}\n...[truncated]")
    } else {
        result.to_string()
    }
}

/// `_enforce_tool_results_per_turn` — cap parallel tool calls per turn.
fn enforce_tool_results_per_turn(tool_calls: Vec<Value>) -> Vec<Value> {
    tool_calls
        .into_iter()
        .take(MAX_TOOL_RESULTS_PER_TURN)
        .collect()
}

/// `_is_context_length_error` — markers from provider_protocol.
pub fn is_context_length_error(error_msg: &str) -> bool {
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

/// Iteration budget — prevents infinite tool loops (Python class).
pub struct IterationBudget {
    max_total: usize,
    used: usize,
}

impl IterationBudget {
    pub fn new(max_total: usize) -> Self {
        IterationBudget { max_total, used: 0 }
    }

    pub fn consume(&mut self) -> bool {
        if self.used >= self.max_total {
            false
        } else {
            self.used += 1;
            true
        }
    }

    pub fn remaining(&self) -> usize {
        self.max_total.saturating_sub(self.used)
    }
}

// ---------------------------------------------------------------------------
// Error classification (port of resilience.py classify_error subset)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Transient, // retryable: timeout, 429, 5xx
    Permanent, // non-retryable: 4xx (except 429)
}

pub fn classify_error(status_code: Option<u16>, _error_msg: &str) -> ErrorCategory {
    match status_code {
        Some(429) | Some(500) | Some(502) | Some(503) | Some(504) => ErrorCategory::Transient,
        Some(408) => ErrorCategory::Transient,
        Some(code) if code >= 500 => ErrorCategory::Transient,
        Some(_) => ErrorCategory::Permanent,
        None => ErrorCategory::Transient, // network/timeout — retry
    }
}

pub fn is_retryable(category: ErrorCategory) -> bool {
    matches!(category, ErrorCategory::Transient)
}

/// Exponential backoff with full jitter (Python compute_backoff parity).
pub fn compute_backoff(category: ErrorCategory, attempt: usize, retry_after: Option<f64>) -> f64 {
    let base = match category {
        ErrorCategory::Transient => 1.0,
        ErrorCategory::Permanent => 0.0,
    };
    let exp = base * 2f64.powi(attempt as i32);
    let jittered = exp * 0.5 + (exp * 0.5) * (rand::random::<f64>());
    if let Some(ra) = retry_after {
        jittered.max(ra)
    } else {
        jittered
    }
}

// ---------------------------------------------------------------------------
// HttpLLMRuntime
// ---------------------------------------------------------------------------

/// HTTP-based LLM runtime using OpenAI-compatible API.
pub struct HttpLLMRuntime {
    pub base_url: String,
    pub api_key: String,
    pub default_model: Option<String>,
    pub timeout: u64,
    pub max_retries: usize,
    pub enable_message_sanitization: bool,
    pub enable_tool_truncation: bool,
    pub protocol: PlatformKind,
    /// Explicit protocol name (M10 custom-provider lookup).
    pub protocol_name: Option<String>,
    pub last_error: Option<String>,
    pub last_status_code: Option<u16>,
}

impl HttpLLMRuntime {
    /// Construct from explicit config or auto-load `~/.helen/config.yaml`
    /// (Python `__post_init__` parity).
    pub fn new(
        base_url: Option<String>,
        api_key: Option<String>,
        default_model: Option<String>,
    ) -> Self {
        let config = crate::config::load_config();
        let base_url = base_url.unwrap_or_else(|| {
            config
                .get("base_url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://coding.dashscope.aliyuncs.com/v1")
                .to_string()
        });
        let api_key = api_key.unwrap_or_else(|| {
            config
                .get("api_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        });
        let default_model = default_model.or_else(|| {
            config
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });
        let protocol_name = config.get("protocol").and_then(|v| v.as_str());
        let protocol = detect_protocol(&base_url, protocol_name);
        HttpLLMRuntime {
            base_url,
            api_key,
            default_model,
            timeout: config
                .get("timeout")
                .and_then(|v| v.as_i64())
                .unwrap_or(120) as u64,
            max_retries: 3,
            enable_message_sanitization: true,
            enable_tool_truncation: true,
            protocol,
            protocol_name: protocol_name.map(|s| s.to_string()),
            last_error: None,
            last_status_code: None,
        }
    }

    fn protocol(&self) -> Box<dyn PlatformProtocol> {
        // M10: custom (Python-loaded) protocols take priority over built-ins.
        if let Some(name) = self.protocol_name.as_deref() {
            if let Some(custom) = crate::provider::custom_protocol_by_name(name) {
                return crate::provider::box_custom_protocol(custom);
            }
        }
        self.protocol.protocol()
    }

    fn url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// Build the Authorization header.
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    /// `_chat_with_messages` — send a chat completion request, return the
    /// assistant message dict (content/tool_calls) or None on failure.
    #[allow(clippy::too_many_arguments)]
    fn chat_with_messages(
        &mut self,
        messages: &[Value],
        model: &str,
        temperature: f64,
        tools: Option<&[Value]>,
        max_tokens: Option<u64>,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Option<Value> {
        let mut payload = json!({
            "model": model,
            "messages": messages,
            "temperature": temperature,
        });
        if let Some(t) = tools {
            if !t.is_empty() {
                payload["tools"] = Value::Array(t.to_vec());
            }
        }
        if let Some(mt) = max_tokens {
            payload["max_tokens"] = json!(mt);
        }
        // v1.35: Let platform protocol transform payload
        payload = self.protocol().build_request_payload(
            payload,
            model,
            thinking_enabled,
            reasoning_effort,
        );

        let body = payload.to_string();
        let resp = ureq::post(&self.url())
            .timeout(Duration::from_secs(self.timeout))
            .set("Content-Type", "application/json")
            .set("Authorization", &self.auth_header())
            .send_string(&body);

        match resp {
            Ok(response) => {
                let status = response.status();
                if status != 200 {
                    let error_body: Value = response
                        .into_string()
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_else(|| json!({}));
                    let msg = self.protocol().parse_error(status, &error_body);
                    self.last_error = Some(msg.clone());
                    self.last_status_code = Some(status);
                    return None;
                }
                let result: Value = match response.into_string() {
                    Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| json!({})),
                    Err(e) => {
                        self.last_error = Some(format!("Failed to read response: {e}"));
                        self.last_status_code = Some(status);
                        return None;
                    }
                };
                self.last_error = None;
                self.last_status_code = None;
                let choices = result.get("choices").and_then(|c| c.as_array());
                if choices.map(|c| c.is_empty()).unwrap_or(true) {
                    return None;
                }
                let parsed = self.protocol().parse_response(&result);
                let mut message = json!({
                    "role": "assistant",
                    "content": parsed.get("content").and_then(|v| v.as_str()).unwrap_or(""),
                    "reasoning_content": parsed.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or(""),
                    "tool_calls": parsed.get("tool_calls").cloned().unwrap_or_else(|| json!([])),
                });
                // v1.34.1: GLM/DeepSeek fall back to reasoning_content.
                if message
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty()
                {
                    let reasoning = message
                        .get("reasoning_content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !reasoning.is_empty() {
                        message["content"] = json!(reasoning);
                    }
                }
                let _ = parsed;
                Some(message)
            }
            Err(ureq::Error::Status(code, response)) => {
                let error_body: Value = response
                    .into_string()
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_else(|| json!({}));
                let msg = self.protocol().parse_error(code, &error_body);
                self.last_error = Some(msg.clone());
                self.last_status_code = Some(code);
                None
            }
            Err(e) => {
                self.last_error = Some(format!("Network error: {e}"));
                self.last_status_code = None;
                None
            }
        }
    }

    /// `_chat_with_messages_retry` — retry loop with context-overflow recovery
    /// (trim messages) and backoff.
    #[allow(clippy::too_many_arguments)]
    fn chat_with_messages_retry(
        &mut self,
        messages: &mut Vec<Value>,
        model: &str,
        temperature: f64,
        tools: Option<&[Value]>,
        max_tokens: Option<u64>,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Option<Value> {
        let mut context_overflow_retried = false;
        for attempt in 0..=self.max_retries {
            let result = self.chat_with_messages(
                messages,
                model,
                temperature,
                tools,
                max_tokens,
                thinking_enabled,
                reasoning_effort,
            );
            if result.is_some() {
                return result;
            }
            let error = self.last_error.clone().unwrap_or_default();

            // P1: context overflow auto-recovery — trim older messages.
            if !context_overflow_retried && is_context_length_error(&error) {
                context_overflow_retried = true;
                if messages.len() > 2 {
                    // Drop the oldest non-system message (simple trim parity
                    // with PromptTooLongRecovery's aggressive last resort).
                    let sys_idx = messages
                        .iter()
                        .position(|m| m.get("role").and_then(|v| v.as_str()) == Some("system"));
                    let drop_idx = match sys_idx {
                        Some(0) if messages.len() > 1 => 1,
                        _ => 0,
                    };
                    if messages.len() > 1 {
                        messages.remove(drop_idx);
                    }
                    continue;
                }
            }

            let category = classify_error(self.last_status_code, &error);
            if attempt < self.max_retries && is_retryable(category) {
                let wait = compute_backoff(category, attempt, None).min(120.0);
                if wait > 0.0 {
                    std::thread::sleep(Duration::from_secs_f64(wait));
                }
                continue;
            }
            break;
        }
        None
    }
}

impl LlmRuntime for HttpLLMRuntime {
    fn route(
        &mut self,
        description: &str,
        branches: &[String],
        context: Option<&str>,
    ) -> Result<Option<String>, String> {
        let pb = crate::prompt::PromptBuilder::new();
        let prompt = pb.build_route_prompt(description, branches, context);
        let model = self
            .default_model
            .clone()
            .unwrap_or_else(|| "default".into());
        let mut messages = vec![json!({"role": "user", "content": prompt})];
        let response =
            self.chat_with_messages_retry(&mut messages, &model, 0.0, None, Some(10), false, None);
        let Some(response) = response else {
            let err = self
                .last_error
                .clone()
                .unwrap_or_else(|| "Unknown API error".into());
            return Err(err);
        };
        let text = response
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if text.is_empty() {
            return Ok(branches.first().cloned());
        }
        // Try to match the response to a valid branch (Python parity).
        let cleaned = text
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_lowercase();
        for branch in branches {
            let b = branch.to_lowercase();
            if cleaned == b || cleaned.starts_with(&b) {
                return Ok(Some(branch.clone()));
            }
        }
        Ok(branches.first().cloned())
    }

    fn act(
        &mut self,
        prompt: &str,
        tools: Option<&[Value]>,
        model: Option<&str>,
        temperature: f64,
        max_turns: usize,
        max_tokens: Option<u64>,
        history: Option<&[Value]>,
        system_prompt: Option<&str>,
        dispatch_fn: Option<&dyn Fn(&str, &Value) -> String>,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Result<LlmResponse, String> {
        let use_model: String = model
            .map(|s| s.to_string())
            .or_else(|| self.default_model.clone())
            .unwrap_or_else(|| "default".into());
        let mut messages: Vec<Value> = Vec::new();
        if let Some(sp) = system_prompt {
            messages.push(json!({"role": "system", "content": sp}));
        }
        if let Some(h) = history {
            messages.extend_from_slice(h);
        }
        // P0: only append user message if not already last in history.
        if messages.is_empty() || !last_user_message_matches(&messages, prompt) {
            messages.push(json!({"role": "user", "content": prompt}));
        }

        let mut final_text = String::new();
        let mut tool_calls_log: Vec<Value> = Vec::new();
        let mut budget = IterationBudget::new(max_turns + 2);
        let mut empty_response_retries = 0;
        let max_empty_retries = 2;

        while budget.consume() {
            if self.enable_message_sanitization {
                sanitize_messages(&mut messages);
                repair_message_sequence(&mut messages);
            }

            let response_msg = self.chat_with_messages_retry(
                &mut messages,
                &use_model,
                temperature,
                tools,
                max_tokens,
                thinking_enabled,
                reasoning_effort,
            );
            let Some(response_msg) = response_msg else {
                let err = self
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Unknown API error".into());
                return Err(format!("LLM API call failed: {err}"));
            };

            // Check if LLM wants tool calls
            let tool_calls: Vec<Value> = response_msg
                .get("tool_calls")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            if !tool_calls.is_empty() {
                let tool_calls = enforce_tool_results_per_turn(tool_calls);

                // Append assistant message with tool_calls
                messages.push(response_msg.clone());

                // Execute tool calls
                let mut tool_results: Vec<(Value, String)> = Vec::new();
                for tc in &tool_calls {
                    let fn_name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let fn_args: Value = tc
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_else(|| json!({}));
                    let result = match &dispatch_fn {
                        Some(d) => d(&fn_name, &fn_args),
                        None => {
                            if let Ok(tools_mod) = crate::tools_dispatch(fn_name.as_str(), &fn_args)
                            {
                                tools_mod
                            } else {
                                format!("Tool '{fn_name}' not found")
                            }
                        }
                    };
                    tool_results.push((tc.clone(), result));
                }

                // Append tool results with truncation
                for (tc, result) in &tool_results {
                    let content = if self.enable_tool_truncation {
                        truncate_tool_result(result)
                    } else {
                        result.clone()
                    };
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.get("id").cloned().unwrap_or_else(|| json!("")),
                        "content": content,
                    }));
                    let truncated: String = result.chars().take(500).collect();
                    tool_calls_log.push(json!({
                        "name": tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or(""),
                        "args": tc.get("function").and_then(|f| f.get("arguments")).cloned().unwrap_or_else(|| json!("{}")),
                        "result": truncated,
                    }));
                }
                continue;
            }

            // No tool calls — final text.
            let text = response_msg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if text.is_empty() && empty_response_retries < max_empty_retries {
                empty_response_retries += 1;
                continue; // nudge
            }
            final_text = text.to_string();
            break;
        }

        let _ = tool_calls_log;
        Ok(LlmResponse {
            text: if final_text.is_empty() {
                None
            } else {
                Some(final_text)
            },
            tool_calls: Vec::new(),
            model: Some(use_model),
        })
    }

    fn act_stream(
        &mut self,
        prompt: &str,
        model: Option<&str>,
        temperature: f64,
        system_prompt: Option<&str>,
        tools: Option<&[Value]>,
        max_turns: usize,
        history: Option<&[Value]>,
        dispatch_fn: Option<&dyn Fn(&str, &Value) -> String>,
        mut on_event: &mut dyn FnMut(Value) -> bool,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Result<(), String> {
        let use_model: String = model
            .map(|s| s.to_string())
            .or_else(|| self.default_model.clone())
            .unwrap_or_else(|| "default".into());
        let mut messages: Vec<Value> = Vec::new();
        if let Some(sp) = system_prompt {
            messages.push(json!({"role": "system", "content": sp}));
        }
        if let Some(h) = history {
            messages.extend_from_slice(h);
        }
        if messages.is_empty() || !last_user_message_matches(&messages, prompt) {
            messages.push(json!({"role": "user", "content": prompt}));
        }

        let mut budget = IterationBudget::new(max_turns + 2);
        let mut empty_response_retries = 0;
        let max_empty_retries = 2;
        let protocol = self.protocol();
        let protocol_ref = protocol.as_ref();

        while budget.consume() {
            if self.enable_message_sanitization {
                sanitize_messages(&mut messages);
                repair_message_sequence(&mut messages);
            }

            // Build streaming request
            let mut payload = json!({
                "model": use_model,
                "messages": messages,
                "temperature": temperature,
                "stream": true,
                "stream_options": {"include_usage": true},
            });
            if let Some(t) = tools {
                if !t.is_empty() {
                    payload["tools"] = Value::Array(t.to_vec());
                }
            }
            if let Some(mt) = max_tokens_for_stream() {
                payload["max_tokens"] = json!(mt);
            }
            payload = self.protocol().build_request_payload(
                payload,
                &use_model,
                thinking_enabled,
                reasoning_effort,
            );

            let body = payload.to_string();
            let resp = ureq::post(&self.url())
                .timeout(Duration::from_secs(self.timeout))
                .set("Content-Type", "application/json")
                .set("Authorization", &self.auth_header())
                .send_string(&body);

            let mut reader = match resp {
                Ok(r) => r.into_reader(),
                Err(ureq::Error::Status(code, response)) => {
                    let error_body: Value = response
                        .into_string()
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_else(|| json!({}));
                    let msg = self.protocol().parse_error(code, &error_body);
                    self.last_error = Some(msg.clone());
                    self.last_status_code = Some(code);
                    if !on_event(json!({"type": "error", "message": msg})) {
                        return Ok(());
                    }
                    return Err(msg);
                }
                Err(e) => {
                    let msg = format!("Network error: {e}");
                    self.last_error = Some(msg.clone());
                    if !on_event(json!({"type": "error", "message": msg})) {
                        return Ok(());
                    }
                    return Err(msg);
                }
            };

            // SSE parse loop
            let mut full_text = String::new();
            let mut reasoning_chunks = String::new();
            let mut tool_calls_acc: indexmap::IndexMap<usize, Value> = indexmap::IndexMap::new();
            let mut usage_info = json!({});
            let mut finish_reason: Option<String> = None;
            let mut stream_context = json!({});
            let mut final_message: Option<Value> = None;

            use std::io::Read;
            let mut buf = [0u8; 8192];
            let mut line = String::new();
            loop {
                // Read one byte at a time until \n (SSE lines)
                let mut byte = [0u8; 1];
                match reader.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) => {
                        if byte[0] == b'\n' {
                            process_sse_line(
                                &line,
                                &mut full_text,
                                &mut reasoning_chunks,
                                &mut tool_calls_acc,
                                &mut usage_info,
                                &mut finish_reason,
                                &mut stream_context,
                                &mut final_message,
                                &mut on_event,
                                protocol_ref,
                            );
                            line.clear();
                        } else {
                            line.push(byte[0] as char);
                        }
                    }
                    Err(_) => break,
                }
                let _ = &mut buf;
            }
            // Process any trailing line
            if !line.trim().is_empty() {
                process_sse_line(
                    &line,
                    &mut full_text,
                    &mut reasoning_chunks,
                    &mut tool_calls_acc,
                    &mut usage_info,
                    &mut finish_reason,
                    &mut stream_context,
                    &mut final_message,
                    &mut on_event,
                    protocol_ref,
                );
            }

            // Tool-call handling in streaming: if tool calls accumulated, run them.
            if !tool_calls_acc.is_empty() {
                let mut tool_results: Vec<(Value, String)> = Vec::new();
                for (_, tc) in &tool_calls_acc {
                    let fn_name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let fn_args: Value = tc
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_else(|| json!({}));
                    let result = match &dispatch_fn {
                        Some(d) => d(&fn_name, &fn_args),
                        None => {
                            crate::tools_dispatch(fn_name.as_str(), &fn_args).unwrap_or_else(|e| e)
                        }
                    };
                    if !on_event(json!({"type": "tool_result", "name": fn_name, "result": result}))
                    {
                        return Ok(());
                    }
                    tool_results.push((tc.clone(), result));
                }
                // Append assistant + tool messages for the next turn
                let mut assistant_msg = json!({"role": "assistant", "content": full_text, "tool_calls": Value::Array(tool_calls_acc.values().cloned().collect())});
                if !reasoning_chunks.is_empty() {
                    assistant_msg["reasoning_content"] = json!(reasoning_chunks);
                }
                messages.push(assistant_msg);
                for (tc, result) in &tool_results {
                    let content = if self.enable_tool_truncation {
                        truncate_tool_result(result)
                    } else {
                        result.clone()
                    };
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.get("id").cloned().unwrap_or_else(|| json!("")),
                        "content": content,
                    }));
                }
                full_text.clear();
                reasoning_chunks.clear();
                tool_calls_acc.clear();
                continue;
            }

            // Normal completion — content chunks were already emitted during
            // SSE parsing (process_sse_line). Only handle the empty-response
            // nudge and the usage event (Python parity).
            if full_text.is_empty() && empty_response_retries < max_empty_retries {
                empty_response_retries += 1;
                continue;
            }
            if let Some(usage) = usage_info.as_object() {
                if !usage.is_empty() && !on_event(json!({"type": "usage", "usage": usage_info})) {
                    return Ok(());
                }
            }
            let _ = finish_reason;
            return Ok(());
        }
        Ok(())
    }
}

/// Placeholder for max_tokens in streaming (Python passes it through).
fn max_tokens_for_stream() -> Option<u64> {
    None
}

/// Process one SSE `data:` line (OpenAI chat.completion.chunk format).
#[allow(clippy::too_many_arguments)]
fn process_sse_line(
    line: &str,
    full_text: &mut String,
    reasoning_chunks: &mut String,
    tool_calls_acc: &mut indexmap::IndexMap<usize, Value>,
    usage_info: &mut Value,
    finish_reason: &mut Option<String>,
    stream_context: &mut Value,
    final_message: &mut Option<Value>,
    on_event: &mut dyn FnMut(Value) -> bool,
    protocol: &dyn PlatformProtocol,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    if !trimmed.starts_with("data:") {
        return;
    }
    let data = trimmed["data:".len()..].trim();
    if data == "[DONE]" {
        return;
    }
    let chunk: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Extract usage (protocol-specific position)
    if let Some(u) = protocol.extract_streaming_usage(&chunk) {
        *usage_info = u;
    }

    let choices = chunk
        .get("choices")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    for choice in choices {
        let delta = choice.get("delta").cloned().unwrap_or_else(|| json!({}));
        if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
            *finish_reason = Some(fr.to_string());
        }
        let parsed = protocol.parse_streaming_delta(&delta, stream_context);
        if let Some(c) = parsed.get("content").and_then(|v| v.as_str()) {
            if !c.is_empty() {
                full_text.push_str(c);
                if !on_event(json!({"type": "content", "content": c})) {
                    return;
                }
            }
        }
        if let Some(rc) = parsed.get("reasoning_content").and_then(|v| v.as_str()) {
            if !rc.is_empty() {
                reasoning_chunks.push_str(rc);
            }
        }
        // Accumulate tool call deltas by index
        if let Some(tcs) = parsed.get("tool_calls").and_then(|v| v.as_array()) {
            for tc in tcs {
                let idx = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let entry = tool_calls_acc.entry(idx).or_insert_with(|| {
                    json!({"id": "", "type": "function", "function": {"name": "", "arguments": ""}})
                });
                if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                    if !id.is_empty() {
                        entry["id"] = json!(id);
                    }
                }
                if let Some(name) = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                {
                    if !name.is_empty() {
                        entry["function"]["name"] = json!(name);
                    }
                }
                if let Some(args) = tc
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|v| v.as_str())
                {
                    if !args.is_empty() {
                        let prev = entry["function"]["arguments"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        entry["function"]["arguments"] = json!(format!("{prev}{args}"));
                    }
                }
            }
        }
    }

    // Record the final message shape for caller use
    if full_text.is_empty() && tool_calls_acc.is_empty() {
        *final_message = None;
    } else {
        *final_message = Some(json!({"content": full_text.clone()}));
    }
    let _ = final_message;
}
