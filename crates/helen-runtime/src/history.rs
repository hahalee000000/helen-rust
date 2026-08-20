//! History manager (Task 8.2) — port of `helen/runtime/history.py`.
//!
//! Token-budget checking, oldest-first trimming (system-preserving),
//! three-tier compression (recent/middle-summarize/oldest-drop),
//! conversation summary, usage stats, and save/load persistence.

use crate::token::estimate_tokens_simple;
use std::fs;

// ---------------------------------------------------------------------------
// Model context window lookup
// ---------------------------------------------------------------------------

/// Known model families and their context windows (Python parity).
pub const MODEL_CONTEXT_WINDOWS: &[(&str, usize)] = &[
    // Qwen family (DashScope)
    ("qwen3.7-plus", 131072),
    ("qwen3.7", 131072),
    ("qwen-plus", 131072),
    ("qwen-max", 32768),
    ("qwen-turbo", 131072),
    ("qwen-long", 1000000),
    // OpenAI family
    ("gpt-4", 8192),
    ("gpt-4-turbo", 128000),
    ("gpt-4o", 128000),
    ("gpt-4o-mini", 128000),
    ("gpt-4.1", 1047576),
    ("gpt-4.1-mini", 1047576),
    ("gpt-4.1-nano", 1047576),
    ("gpt-3.5-turbo", 16385),
    ("o1", 200000),
    ("o1-mini", 128000),
    ("o1-pro", 200000),
    ("o3", 200000),
    ("o3-mini", 200000),
    ("o4-mini", 200000),
    // Claude family (Anthropic)
    ("claude-3-opus", 200000),
    ("claude-3-sonnet", 200000),
    ("claude-3-haiku", 200000),
    ("claude-3-5-sonnet", 200000),
    ("claude-3-5-haiku", 200000),
    ("claude-opus-4", 200000),
    ("claude-sonnet-4", 200000),
    ("claude-fable-5", 200000),
    // Gemini family
    ("gemini-pro", 32768),
    ("gemini-1.5-pro", 2097152),
    ("gemini-1.5-flash", 1048576),
    ("gemini-2.0-flash", 1048576),
    ("gemini-2.5-pro", 1048576),
];

/// Default fallback context window when model is unknown (history.py: 128000).
pub const DEFAULT_CONTEXT_WINDOW: usize = 128000;

/// History size limit: keep at most 60% of context window for history.
pub const HISTORY_BUDGET_RATIO: f64 = 0.6;

/// Summary target: when compressing, aim for this many tokens.
pub const COMPRESSION_SUMMARY_TOKENS: usize = 2048;

/// P3: Compression modes.
pub const COMPRESSION_MODE_SUMMARIZE: &str = "summarize";
pub const COMPRESSION_MODE_TRUNCATE: &str = "truncate";
pub const COMPRESSION_MODE_NONE: &str = "none";

/// P3: Minimum recent messages to always keep (never compressed/dropped).
pub const MIN_RECENT_MESSAGES: usize = 5;

/// Get the context window size (in tokens) for a given model.
///
/// Lookup order: exact match → prefix match → default fallback.
pub fn get_model_context_window(model: Option<&str>) -> usize {
    let Some(model) = model else {
        return DEFAULT_CONTEXT_WINDOW;
    };
    if model.is_empty() {
        return DEFAULT_CONTEXT_WINDOW;
    }
    // Exact match
    if let Some((_, w)) = MODEL_CONTEXT_WINDOWS.iter().find(|(m, _)| *m == model) {
        return *w;
    }
    // Prefix match: try progressively shorter prefixes
    let parts: Vec<&str> = model.split('-').collect();
    for i in (1..=parts.len()).rev() {
        let prefix = parts[..i].join("-");
        if let Some((_, w)) = MODEL_CONTEXT_WINDOWS.iter().find(|(m, _)| *m == prefix) {
            return *w;
        }
    }
    DEFAULT_CONTEXT_WINDOW
}

/// Estimate token count for a text string (heuristic; tiktoken absent in Rust).
pub fn estimate_tokens(text: &str) -> usize {
    estimate_tokens_simple(text)
}

/// Extract plain text from message content (handles multimodal lists).
pub fn message_text(content: &serde_json::Value) -> String {
    crate::transcript::message_text_parts(content).0
}

// ---------------------------------------------------------------------------
// HistoryManager
// ---------------------------------------------------------------------------

/// Manage conversation history with token budget enforcement.
pub struct HistoryManager {
    /// Context window size (tokens).
    pub max_tokens: usize,
    model: Option<String>,
    /// How to handle history overflow.
    pub compression_mode: String,
    /// Conversation summary token cap.
    pub summary_max_tokens: usize,
}

impl HistoryManager {
    pub fn new(
        model: Option<String>,
        context_window: Option<usize>,
        compression_mode: Option<String>,
    ) -> Self {
        let max_tokens = if let Some(cw) = context_window {
            cw
        } else if let Some(m) = &model {
            get_model_context_window(Some(m.as_str()))
        } else {
            DEFAULT_CONTEXT_WINDOW
        };
        HistoryManager {
            max_tokens,
            model,
            compression_mode: compression_mode
                .unwrap_or_else(|| COMPRESSION_MODE_SUMMARIZE.to_string()),
            summary_max_tokens: 4096,
        }
    }

    pub fn set_model(&mut self, model: Option<String>) {
        self.model = model;
        self.max_tokens = get_model_context_window(self.model.as_deref());
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Available tokens for history (reserves 1000 buffer for response).
    pub fn check_budget(&self, system_tokens: usize, instruction_tokens: usize) -> usize {
        // Reserve: system prompt + instruction + 1000 for response buffer
        let budget =
            self.max_tokens as i64 - system_tokens as i64 - instruction_tokens as i64 - 1000;
        budget.max(0) as usize
    }

    /// Trim history from oldest to newest to fit within budget.
    /// Never removes the system message if present.
    pub fn trim_history(
        &self,
        history: &[crate::transcript::Message],
        budget: usize,
    ) -> Vec<crate::transcript::Message> {
        if history.is_empty() || budget == 0 {
            return Vec::new();
        }

        // Calculate tokens for each message
        let msg_tokens: Vec<(&crate::transcript::Message, usize)> =
            history.iter().map(|msg| (msg, msg.token_count())).collect();

        // If total is under budget, keep all
        let total: usize = msg_tokens.iter().map(|(_, t)| *t).sum();
        if total <= budget {
            return history.to_vec();
        }

        let mut result: Vec<crate::transcript::Message> = Vec::new();
        let mut result_tokens: Vec<usize> = Vec::new();

        // Identify system messages (they must stay)
        let system_indices: Vec<usize> = msg_tokens
            .iter()
            .enumerate()
            .filter(|(_, (msg, _))| msg.role == "system")
            .map(|(i, _)| i)
            .collect();

        // Build result from newest to oldest, skipping system messages
        for i in (0..msg_tokens.len()).rev() {
            if system_indices.contains(&i) {
                continue;
            }
            let (msg, tokens) = msg_tokens[i];
            if result_tokens.iter().sum::<usize>() + tokens <= budget {
                result.insert(0, msg.clone());
                result_tokens.insert(0, tokens);
            }
        }

        // Prepend system messages at the start
        for i in system_indices {
            let (msg, tokens) = msg_tokens[i];
            result.insert(0, msg.clone());
            result_tokens.insert(0, tokens);
        }

        result
    }

    /// Enforce history size limit using the configured compression mode.
    pub fn enforce_limit(
        &self,
        history: &[crate::transcript::Message],
        budget_ratio: f64,
    ) -> Vec<crate::transcript::Message> {
        if history.is_empty() {
            return Vec::new();
        }

        let budget = (self.max_tokens as f64 * budget_ratio) as usize;
        let total: usize = history.iter().map(|msg| msg.token_count()).sum();

        if total <= budget {
            return history.to_vec(); // Under limit, no compression needed
        }

        if self.compression_mode == COMPRESSION_MODE_NONE {
            return history.to_vec();
        }
        if self.compression_mode == COMPRESSION_MODE_TRUNCATE {
            return self.truncate_compress(history, budget);
        }
        // Default: summarize mode
        self.summarize_compress(history, budget)
    }

    /// P3: Truncate compression — drop oldest messages, keep recent ones.
    pub fn truncate_compress(
        &self,
        history: &[crate::transcript::Message],
        budget: usize,
    ) -> Vec<crate::transcript::Message> {
        if history.len() <= MIN_RECENT_MESSAGES {
            return history.to_vec(); // Can't drop more
        }

        let msg_tokens: Vec<(&crate::transcript::Message, usize)> =
            history.iter().map(|msg| (msg, msg.token_count())).collect();

        let system_indices: Vec<usize> = msg_tokens
            .iter()
            .enumerate()
            .filter(|(_, (msg, _))| msg.role == "system")
            .map(|(i, _)| i)
            .collect();

        let mut recent: Vec<crate::transcript::Message> = Vec::new();
        let mut recent_tokens: usize = 0;
        let min_recent_start = history.len().saturating_sub(MIN_RECENT_MESSAGES);

        for i in (0..history.len()).rev() {
            if system_indices.contains(&i) {
                continue;
            }
            let (msg, tokens) = msg_tokens[i];
            let non_system_kept = recent.iter().filter(|m| m.role != "system").count();
            if recent_tokens + tokens > budget
                && i >= min_recent_start
                && non_system_kept >= MIN_RECENT_MESSAGES
            {
                break;
            }
            recent.insert(0, msg.clone());
            recent_tokens += tokens;
        }

        // Prepend system messages at the start
        for i in system_indices {
            let (msg, _) = msg_tokens[i];
            if !recent.iter().any(|m| std::ptr::eq(m, msg)) {
                recent.insert(0, msg.clone());
            }
        }

        recent
    }

    /// P3: Three-tier summarize compression.
    ///
    /// Tier 1 (recent): keep complete — newest `MIN_RECENT_MESSAGES`.
    /// Tier 2 (middle): summarize into `[Previous conversation summary]`.
    /// Tier 3 (oldest): drop if even summary would exceed budget.
    pub fn summarize_compress(
        &self,
        history: &[crate::transcript::Message],
        budget: usize,
    ) -> Vec<crate::transcript::Message> {
        let keep_budget = (budget as f64 * 0.75) as usize; // 75% for recent
        let summary_budget = budget - keep_budget; // 25% for summary

        // Walk from newest, accumulating until we'd exceed keep_budget
        let mut recent: Vec<crate::transcript::Message> = Vec::new();
        let mut recent_tokens: usize = 0;
        let mut split_idx = 0usize;

        for i in (0..history.len()).rev() {
            let msg = &history[i];
            let tokens = msg.token_count();
            if recent_tokens + tokens > keep_budget {
                split_idx = i + 1;
                break;
            }
            recent.insert(0, msg.clone());
            recent_tokens += tokens;
        }
        if split_idx == 0 && recent_tokens <= keep_budget {
            // All messages fit in keep_budget
            return history.to_vec();
        }

        // Ensure at least MIN_RECENT_MESSAGES are kept
        if recent.len() < MIN_RECENT_MESSAGES && split_idx > 0 {
            let needed = MIN_RECENT_MESSAGES - recent.len();
            let start = split_idx as i64 - 1;
            let end = (split_idx as i64 - 1 - needed as i64 - 5).max(-1);
            let mut i = start;
            while i > end {
                if i < 0 {
                    break;
                }
                let msg = &history[i as usize];
                if msg.role != "system" {
                    recent.insert(0, msg.clone());
                    split_idx = i as usize;
                    if recent.len() >= MIN_RECENT_MESSAGES {
                        break;
                    }
                }
                i -= 1;
            }
        }

        // The old messages to summarize
        let old_messages = &history[..split_idx];
        if old_messages.is_empty() {
            return history.to_vec();
        }

        let summary_text = self.build_summary_text(old_messages, summary_budget);
        let summary_msg = crate::transcript::Message::new(
            "system",
            serde_json::Value::String(format!("[Previous conversation summary]\n{summary_text}")),
            Vec::new(),
            None,
            String::new(),
            Some("system".to_string()),
            100,
            true,
            false,
            None,
            String::new(),
            String::new(),
            Vec::new(),
        );

        let mut out = Vec::with_capacity(1 + recent.len());
        out.push(summary_msg);
        out.extend(recent);
        out
    }

    /// Build a text summary of messages (newest-first until token budget).
    pub fn build_summary_text(
        &self,
        messages: &[crate::transcript::Message],
        max_tokens: usize,
    ) -> String {
        if messages.is_empty() {
            return String::new();
        }

        let mut lines: Vec<String> = Vec::new();
        let mut total_tokens: usize = 0;

        for msg in messages.iter().rev() {
            let text = message_text(&msg.content);
            if text.is_empty() {
                continue;
            }
            let line = format!("[{}] {}", msg.role, text);
            let line_tokens = estimate_tokens(&line);
            if total_tokens + line_tokens > max_tokens {
                break;
            }
            lines.push(line);
            total_tokens += line_tokens;
        }

        lines.reverse();

        if lines.is_empty() {
            // Even a single message exceeded budget — truncate the newest one
            let newest = &messages[messages.len() - 1];
            let text = message_text(&newest.content);
            if !text.is_empty() {
                let approx_chars = max_tokens * 3; // Rough chars-per-token
                if text.len() > approx_chars {
                    return format!("[{}] {}... [truncated]", newest.role, &text[..approx_chars]);
                }
                return format!("[{}] {}", newest.role, text);
            }
            return String::new();
        }

        lines.join("\n")
    }

    /// Build a conversation summary for LLM routing/choose context.
    pub fn build_conversation_summary(
        &self,
        history: &[crate::transcript::Message],
        max_tokens: Option<usize>,
    ) -> String {
        let max_tokens = max_tokens.unwrap_or(self.summary_max_tokens);
        if history.is_empty() {
            return String::new();
        }

        let mut lines: Vec<String> = Vec::new();
        let mut total_tokens: usize = 0;

        for msg in history.iter().rev() {
            let text = message_text(&msg.content);
            let line = format!("[{}] {}", msg.role, text);
            let line_tokens = estimate_tokens(&line);
            if total_tokens + line_tokens > max_tokens {
                continue;
            }
            lines.push(line);
            total_tokens += line_tokens;
        }

        lines.reverse();
        lines.join("\n")
    }

    /// Prepare history for an LLM API call (budget check + trim + convert).
    pub fn prepare_for_llm(
        &self,
        history: &[crate::transcript::Message],
        system_prompt: Option<&str>,
        current_prompt: &str,
    ) -> Vec<serde_json::Value> {
        let system_tokens = system_prompt.map(estimate_tokens).unwrap_or(0);
        let instruction_tokens = estimate_tokens(current_prompt);
        let budget = self.check_budget(system_tokens, instruction_tokens);
        let trimmed = self.trim_history(history, budget);

        let mut messages = Vec::with_capacity(trimmed.len());
        for msg in &trimmed {
            let mut api_msg = serde_json::Map::new();
            api_msg.insert("role".into(), serde_json::Value::String(msg.role.clone()));
            api_msg.insert("content".into(), msg.content.clone());
            if !msg.tool_calls.is_empty() {
                api_msg.insert(
                    "tool_calls".into(),
                    serde_json::Value::Array(msg.tool_calls.clone()),
                );
            }
            if let Some(tcid) = &msg.tool_call_id {
                api_msg.insert(
                    "tool_call_id".into(),
                    serde_json::Value::String(tcid.clone()),
                );
            }
            messages.push(serde_json::Value::Object(api_msg));
        }
        messages
    }

    /// Save conversation history to a JSON file (version 2 format).
    pub fn save_to_file(&self, history: &[crate::transcript::Message], filepath: &str) {
        let messages: Vec<serde_json::Value> = history
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                    "tool_calls": msg.tool_calls,
                    "tool_call_id": msg.tool_call_id,
                    "uuid": msg.uuid,
                })
            })
            .collect();

        let data = serde_json::json!({
            "version": 2,
            "model": self.model,
            "saved_at": crate::config::now_iso_utc(),
            "messages": messages,
        });

        if let Err(e) = fs::write(
            filepath,
            serde_json::to_string_pretty(&data).unwrap_or_default(),
        ) {
            eprintln!("Failed to save history to {filepath}: {e}");
        }
    }

    /// Load conversation history from a JSON file.
    pub fn load_from_file(&self, filepath: &str) -> Vec<crate::transcript::Message> {
        let Ok(content) = fs::read_to_string(filepath) else {
            return Vec::new();
        };
        let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) else {
            return Vec::new();
        };
        let Some(messages) = data.get("messages").and_then(|v| v.as_array()) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(messages.len());
        for m in messages {
            out.push(crate::transcript::Message::new(
                m.get("role").and_then(|v| v.as_str()).unwrap_or("user"),
                m.get("content")
                    .cloned()
                    .unwrap_or(serde_json::Value::String(String::new())),
                m.get("tool_calls")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default(),
                m.get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                m.get("uuid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                m.get("message_type")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                m.get("priority").and_then(|v| v.as_i64()).unwrap_or(50),
                m.get("compressed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                m.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false),
                m.get("agent_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                m.get("invocation_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                m.get("parent_invocation_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                m.get("visible_to_invocation_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            ));
        }
        out
    }

    /// Search history for messages matching criteria (newest first).
    pub fn search(
        &self,
        history: &[crate::transcript::Message],
        query: Option<&str>,
        role: Option<&str>,
        tool_name: Option<&str>,
        limit: usize,
    ) -> Vec<crate::transcript::Message> {
        let mut results: Vec<crate::transcript::Message> = Vec::new();
        let query_l = query.map(|q| q.to_lowercase());

        for msg in history.iter().rev() {
            if results.len() >= limit {
                break;
            }
            if let Some(r) = role {
                if msg.role != r {
                    continue;
                }
            }
            if let Some(q) = &query_l {
                let text_l = message_text(&msg.content).to_lowercase();
                if !text_l.contains(q.as_str()) {
                    let in_tool_results = msg.tool_calls.iter().any(|tc| {
                        tc.get("result")
                            .and_then(|v| v.as_str())
                            .map(|r| r.to_lowercase().contains(q.as_str()))
                            .unwrap_or(false)
                    });
                    if !in_tool_results {
                        continue;
                    }
                }
            }
            if let Some(tn) = tool_name {
                let tn_l = tn.to_lowercase();
                let in_tool_names = msg.tool_calls.iter().any(|tc| {
                    tc.get("name")
                        .and_then(|v| v.as_str())
                        .map(|n| n.to_lowercase().contains(tn_l.as_str()))
                        .unwrap_or(false)
                });
                if !in_tool_names
                    && !message_text(&msg.content)
                        .to_lowercase()
                        .contains(tn_l.as_str())
                {
                    continue;
                }
            }
            results.push(msg.clone());
        }
        results
    }

    /// Get context usage statistics.
    pub fn get_usage_stats(
        &self,
        history: &[crate::transcript::Message],
        system_prompt: Option<&str>,
    ) -> serde_json::Value {
        let msg_tokens: usize = history.iter().map(|m| m.token_count()).sum();
        let system_tokens = system_prompt.map(estimate_tokens).unwrap_or(0);
        let total_tokens = msg_tokens + system_tokens;

        let mut by_role: indexmap::IndexMap<String, usize> = indexmap::IndexMap::new();
        for msg in history {
            *by_role.entry(msg.role.clone()).or_insert(0) += msg.token_count();
        }
        if system_tokens > 0 {
            *by_role.entry("system_prompt".to_string()).or_insert(0) += system_tokens;
        }

        let summary_count = history
            .iter()
            .filter(|msg| {
                msg.role == "system"
                    && message_text(&msg.content).starts_with("[Previous conversation summary]")
            })
            .count();

        let usage_percent = if self.max_tokens > 0 {
            (total_tokens as f64 / self.max_tokens as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        serde_json::json!({
            "total_tokens": total_tokens,
            "context_window": self.max_tokens,
            "usage_percent": usage_percent,
            "message_count": history.len(),
            "by_role": by_role,
            "compressed": summary_count > 0,
            "summary_count": summary_count,
            "model": self.model,
            "compression_mode": self.compression_mode,
        })
    }

    /// Format usage stats as human-readable string for REPL display.
    pub fn format_usage_stats(&self, stats: &serde_json::Value) -> String {
        let mut lines = vec![
            "╔══════════════════════════════════════╗".to_string(),
            "║       Context Usage Statistics        ║".to_string(),
            "╠══════════════════════════════════════╣".to_string(),
        ];

        let total = stats["total_tokens"].as_u64().unwrap_or(0);
        let window = stats["context_window"].as_u64().unwrap_or(0);
        let percent = stats["usage_percent"].as_f64().unwrap_or(0.0);
        let model = stats["model"].as_str().unwrap_or("unknown");

        let bar_width = 40usize;
        let filled = (bar_width as f64 * percent / 100.0) as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
        let status = if percent > 80.0 { "⚠️ " } else { "✅ " };

        lines.push(format!("║ {status} {bar} {percent:5.1}%            ║"));
        lines.push(format!("║ Tokens: {total:>8} / {window:>8}              ║"));
        lines.push(format!("║ Model:  {model:<30} ║"));
        lines.push(format!(
            "║ Messages: {:<27} ║",
            stats["message_count"].as_u64().unwrap_or(0)
        ));

        let by_role = &stats["by_role"];
        if by_role.is_object() && !by_role.as_object().expect("object exists").is_empty() {
            lines.push("╠──────────────────────────────────────╣".to_string());
            let mut pairs: Vec<(String, u64)> = by_role
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0)))
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            for (role, tokens) in pairs {
                let role_label: String = role
                    .chars()
                    .take(15)
                    .map(|c| c.to_ascii_uppercase())
                    .collect();
                lines.push(format!("║  {role_label:<16} {tokens:>8} tokens        ║"));
            }
        }

        if stats["compressed"].as_bool().unwrap_or(false) {
            lines.push("╠──────────────────────────────────────╣".to_string());
            lines.push(format!(
                "║  📦 Compressed: {} summary message(s)   ║",
                stats["summary_count"].as_u64().unwrap_or(0)
            ));
            lines.push(format!(
                "║  Mode: {:<31} ║",
                stats["compression_mode"].as_str().unwrap_or("")
            ));
        }

        lines.push("╚══════════════════════════════════════╝".to_string());
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Message;

    fn msg(role: &str, content: impl Into<String>) -> Message {
        Message::new(
            role,
            serde_json::Value::String(content.into()),
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

    #[test]
    fn context_window_exact_and_prefix() {
        assert_eq!(get_model_context_window(Some("qwen3.7-plus")), 131072);
        assert_eq!(get_model_context_window(Some("qwen3.7-plus-2024")), 131072);
        assert_eq!(get_model_context_window(Some("gpt-4o-mini")), 128000);
        assert_eq!(
            get_model_context_window(Some("unknown-model")),
            DEFAULT_CONTEXT_WINDOW
        );
        assert_eq!(get_model_context_window(None), DEFAULT_CONTEXT_WINDOW);
    }

    #[test]
    fn message_token_count_multimodal() {
        let text = Message::new(
            "user",
            serde_json::Value::String("hello".to_string()),
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
        );
        // estimate("hello") = max(1, 5/4)=1 + 4 overhead = 5
        assert_eq!(text.token_count(), 5);

        let multi = Message::new(
            "user",
            serde_json::json!([
                {"type": "text", "text": "hi"},
                {"type": "image"},
                {"type": "image"}
            ]),
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
        );
        // estimate("hi")=1 + 2*85 media + 4 overhead = 175
        assert_eq!(multi.token_count(), 175);
    }

    #[test]
    fn trim_history_keeps_recent_and_system() {
        let h = HistoryManager::new(None, Some(1000), None);
        let history = vec![
            msg("system", "sys"),
            msg("user", "aaa"),
            msg("assistant", "bbb"),
            msg("user", "ccc"),
        ];
        // Python-parity: budget 5 keeps system + 1 newest non-system only.
        let trimmed = h.trim_history(&history, 5);
        assert!(trimmed.len() < history.len());
        assert_eq!(trimmed[0].role, "system");
        // With loose budget (total-2), Python keeps everything.
        let total: usize = history.iter().map(|m| m.token_count()).sum();
        let all = h.trim_history(&history, total - 2);
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn check_budget_reserves_buffer() {
        let h = HistoryManager::new(Some("gpt-4".to_string()), None, None); // 8192
        assert_eq!(h.max_tokens, 8192);
        let budget = h.check_budget(100, 200);
        assert_eq!(budget, 8192 - 100 - 200 - 1000);
    }

    #[test]
    fn enforce_limit_under_budget_unchanged() {
        let h = HistoryManager::new(None, Some(10000), None);
        let history = vec![msg("user", "hi"), msg("assistant", "there")];
        let out = h.enforce_limit(&history, HISTORY_BUDGET_RATIO);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn summarize_compress_produces_summary_message() {
        let h = HistoryManager::new(None, Some(2000), None);
        // Create enough messages to exceed budget (large contents)
        let mut history = vec![
            msg("user", "x".repeat(300)),
            msg("assistant", "y".repeat(300)),
        ];
        for i in 0..6 {
            history.push(msg("user", format!("message number {i} ").repeat(50)));
            history.push(msg("assistant", format!("reply number {i} ").repeat(50)));
        }
        let out = h.enforce_limit(&history, 0.2); // tiny budget forces compression
        assert!(out.len() < history.len());
        assert!(out.iter().any(|m| m
            .content
            .to_string()
            .contains("[Previous conversation summary]")));
    }

    #[test]
    fn truncate_compress_python_parity() {
        // Verified against Python: the break condition is unreachable with
        // MIN_RECENT_MESSAGES=5 walking from newest (i>=len-5 fails once 5
        // non-system collected), so the reference keeps ALL messages here.
        // Port must match that observable behavior exactly.
        let h = HistoryManager::new(
            None,
            Some(3000),
            Some(COMPRESSION_MODE_TRUNCATE.to_string()),
        );
        let mut history = vec![msg("system", "sys")];
        for i in 0..12 {
            history.push(msg("user", format!("message {i} ").repeat(40)));
            history.push(msg("assistant", format!("reply {i} ").repeat(40)));
        }
        let out = h.enforce_limit(&history, 0.2);
        assert_eq!(out.len(), 25); // Python parity: no trimming
        assert_eq!(out[0].role, "system");
    }

    #[test]
    fn build_conversation_summary_formats() {
        let h = HistoryManager::new(None, None, None);
        let history = vec![msg("user", "first"), msg("assistant", "second")];
        let summary = h.build_conversation_summary(&history, Some(5000));
        assert!(summary.starts_with("[user] first"));
        assert!(summary.contains("[assistant] second"));
    }

    #[test]
    fn save_load_roundtrip() {
        let h = HistoryManager::new(Some("qwen3.7-plus".to_string()), None, None);
        let history = vec![msg("user", "hello world"), msg("assistant", "hi there")];
        let path = std::env::temp_dir().join(format!(
            "helen_history_{}.json",
            crate::transcript::generate_uuid()
        ));
        let path_s = path.to_string_lossy().to_string();
        h.save_to_file(&history, &path_s);
        let loaded = h.load_from_file(&path_s);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].role, "user");
        assert_eq!(
            loaded[0].content.as_str().expect("string value"),
            "hello world"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn search_filters() {
        let h = HistoryManager::new(None, None, None);
        let history = vec![
            msg("user", "please fix the bug"),
            msg("tool", "result: fixed"),
            msg("assistant", "done"),
        ];
        let r = h.search(&history, Some("bug"), None, None, 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].role, "user");
        let by_role = h.search(&history, None, Some("tool"), None, 10);
        assert_eq!(by_role.len(), 1);
    }

    #[test]
    fn usage_stats_and_format() {
        let h = HistoryManager::new(Some("gpt-4".to_string()), None, None);
        let history = vec![msg("user", "hello"), msg("assistant", "world")];
        let stats = h.get_usage_stats(&history, Some("sys prompt"));
        assert_eq!(stats["message_count"].as_u64().expect("u64 value"), 2);
        assert_eq!(stats["model"].as_str().expect("string value"), "gpt-4");
        assert!(!stats["compressed"].as_bool().expect("bool value"));
        let rendered = h.format_usage_stats(&stats);
        assert!(rendered.contains("Context Usage Statistics"));
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    use crate::transcript::Message;

    fn msg(role: &str, content: &str) -> Message {
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

    #[test]
    fn summarize_compress_byte_parity_with_python() {
        let h = HistoryManager::new(None, Some(2000), None);
        let mut history = vec![
            msg("user", &"x".repeat(300)),
            msg("assistant", &"y".repeat(300)),
        ];
        for i in 0..6 {
            history.push(msg("user", &format!("message number {i} ").repeat(50)));
            history.push(msg("assistant", &format!("reply number {i} ").repeat(50)));
        }
        let out = h.enforce_limit(&history, 0.2);
        let sm: Vec<_> = out.iter().filter(|m| m.role == "system").collect();
        let content = sm[0].content.as_str().expect("string value");
        assert!(content.starts_with("[Previous conversation summary]\n[user] message number 3"));
        assert!(content.contains("... [truncated]"));
        // Python: LEN 6 for this corpus.
        assert_eq!(out.len(), 6);
    }
}

#[cfg(test)]
mod exact_parity {
    use super::*;
    use crate::transcript::Message;

    fn msg(role: &str, content: &str) -> Message {
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

    #[test]
    #[ignore] // Requires /tmp/py_summary.json from Python reference (run setup script first)
    fn summary_matches_python_exactly() {
        let h = HistoryManager::new(None, Some(2000), None);
        let mut history = vec![
            msg("user", &"x".repeat(300)),
            msg("assistant", &"y".repeat(300)),
        ];
        for i in 0..6 {
            history.push(msg("user", &format!("message number {i} ").repeat(50)));
            history.push(msg("assistant", &format!("reply number {i} ").repeat(50)));
        }
        let out = h.enforce_limit(&history, 0.2);
        let sm: Vec<_> = out.iter().filter(|m| m.role == "system").collect();
        let content = sm[0].content.as_str().expect("string value");
        let py = std::fs::read_to_string("/tmp/py_summary.json").expect("read file");
        // File contains JSON + a LEN line; extract just the JSON string.
        let json_end = py.find('\n').unwrap_or(py.len());
        let py_line = &py[..json_end];
        let py: serde_json::Value = serde_json::from_str(py_line).expect("parse JSON");
        let py = py.as_str().expect("string value");
        assert_eq!(content, py, "byte-exact summary parity required");
    }
}
