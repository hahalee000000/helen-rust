//! Context overflow recovery cascade (Task 8.6) —
//! port of `helen/runtime/context_recovery.py`.
//!
//! Multi-step recovery when LLM API returns prompt-too-long errors:
//! 1. Context Collapse overflow recovery (zero-cost timeline archive)
//! 2. Reactive structural compaction (zero-cost)
//! 3. Reactive semantic compaction (LLM-based, optional)
//! 4. Aggressive trim (last resort)

use regex::Regex;
use serde_json::{json, Value};

/// `RecoveryResult` — result of a recovery attempt.
#[derive(Debug, Clone)]
pub struct RecoveryResult {
    pub messages: Vec<Value>,
    pub strategy: String,
    pub success: bool,
    pub tokens_reduced: usize,
}

impl RecoveryResult {
    fn failed(strategy: &str, messages: Vec<Value>) -> Self {
        Self {
            messages,
            strategy: strategy.into(),
            success: false,
            tokens_reduced: 0,
        }
    }
    fn success(strategy: &str, messages: Vec<Value>, tokens_reduced: usize) -> Self {
        Self {
            messages,
            strategy: strategy.into(),
            success: true,
            tokens_reduced,
        }
    }
}

/// `PromptTooLongRecovery` — multi-step recovery cascade for context overflow.
pub struct PromptTooLongRecovery {
    pub max_tokens: usize,
    pub llm_client: Option<()>, // Rust port: semantic recovery is LLM-optional
    pub max_recovery_attempts: usize,
}

impl Default for PromptTooLongRecovery {
    fn default() -> Self {
        Self {
            max_tokens: crate::token::DEFAULT_CONTEXT_WINDOW,
            llm_client: None,
            max_recovery_attempts: 3,
        }
    }
}

impl PromptTooLongRecovery {
    pub fn new(
        max_tokens: Option<usize>,
        llm_client: Option<()>,
        max_recovery_attempts: Option<usize>,
    ) -> Self {
        Self {
            max_tokens: max_tokens.unwrap_or(crate::token::DEFAULT_CONTEXT_WINDOW),
            llm_client,
            max_recovery_attempts: max_recovery_attempts.unwrap_or(3),
        }
    }

    /// `recover` — execute the full recovery cascade.
    pub fn recover(&self, messages: &[Value], max_tokens: Option<usize>) -> RecoveryResult {
        let effective_max = max_tokens.unwrap_or(self.max_tokens);
        let initial_tokens = self.estimate_total_tokens(messages);

        // Step 1: Context Collapse recovery.
        let result = self.context_collapse_recovery(messages);
        if result.success && self.is_smaller(&result.messages, messages) {
            let reduced =
                initial_tokens.saturating_sub(self.estimate_total_tokens(&result.messages));
            return RecoveryResult::success("context_collapse", result.messages, reduced);
        }

        // Step 2: Reactive structural compaction.
        let result = self.reactive_structural_recovery(messages, effective_max);
        if result.success && self.is_smaller(&result.messages, messages) {
            let reduced =
                initial_tokens.saturating_sub(self.estimate_total_tokens(&result.messages));
            return RecoveryResult::success("reactive_structural", result.messages, reduced);
        }

        // Step 3: Reactive semantic compaction (if LLM available).
        if self.llm_client.is_some() {
            let result = self.reactive_semantic_recovery(messages, effective_max);
            if result.success && self.is_smaller(&result.messages, messages) {
                let reduced =
                    initial_tokens.saturating_sub(self.estimate_total_tokens(&result.messages));
                return RecoveryResult::success("reactive_semantic", result.messages, reduced);
            }
        }

        // Step 4: Aggressive trim (last resort).
        let result = self.aggressive_trim(messages);
        if result.success && self.is_smaller(&result.messages, messages) {
            let reduced =
                initial_tokens.saturating_sub(self.estimate_total_tokens(&result.messages));
            return RecoveryResult::success("aggressive_trim", result.messages, reduced);
        }

        RecoveryResult::failed("exhausted", messages.to_vec())
    }

    /// Step 1: archive old messages as a timeline summary, keep last N recent.
    fn context_collapse_recovery(&self, messages: &[Value]) -> RecoveryResult {
        let preserve_recent = 4usize;
        let system_msgs: Vec<Value> = messages
            .iter()
            .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
            .cloned()
            .collect();
        let conv_msgs: Vec<Value> = messages
            .iter()
            .filter(|m| m.get("role").and_then(|r| r.as_str()) != Some("system"))
            .cloned()
            .collect();

        if conv_msgs.len() <= preserve_recent + 2 {
            return RecoveryResult::failed("context_collapse", messages.to_vec());
        }

        let (old_msgs, recent_msgs) = conv_msgs.split_at(conv_msgs.len() - preserve_recent);

        let mut timeline_parts = vec![format!(
            "[Context Collapse Recovery: {} turns archived]",
            old_msgs.len()
        )];

        let block_size = 10usize;
        let file_re =
            Regex::new(r"[\w./-]+\.(?:py|js|ts|json|yaml|yml|md|txt|helen|rs|go|java|c|cpp|h|hpp)")
                .unwrap();
        for (i, block) in old_msgs.chunks(block_size).enumerate() {
            let start_idx = i * block_size;
            let end_idx = start_idx + block.len();
            let mut block_parts = vec![format!("  [{start_idx}-{end_idx}]")];

            // File references.
            let mut file_refs: Vec<String> = Vec::new();
            for msg in block {
                if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                    for m in file_re.find_iter(content) {
                        if !file_refs.contains(&m.as_str().to_string()) {
                            file_refs.push(m.as_str().to_string());
                        }
                    }
                }
            }
            file_refs.sort();
            if !file_refs.is_empty() {
                block_parts.push(format!(
                    "Files: {}",
                    file_refs
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            // Tool usage.
            let mut tool_counts: Vec<(String, usize)> = Vec::new();
            for msg in block {
                if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                    if let Some(tcs) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                        for tc in tcs {
                            let name = tc
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            if let Some(entry) = tool_counts.iter_mut().find(|(n, _)| *n == name) {
                                entry.1 += 1;
                            } else {
                                tool_counts.push((name, 1));
                            }
                        }
                    }
                }
            }
            tool_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
            if !tool_counts.is_empty() {
                let tool_str = tool_counts
                    .iter()
                    .take(3)
                    .map(|(n, c)| format!("{n}({c})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                block_parts.push(format!("Tools: {tool_str}"));
            }

            // User intents.
            let mut user_intents: Vec<String> = Vec::new();
            for msg in block {
                if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                    if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                        let first_line = content
                            .split('\n')
                            .next()
                            .unwrap_or("")
                            .chars()
                            .take(60)
                            .collect::<String>()
                            .trim()
                            .to_string();
                        if !first_line.is_empty() && !user_intents.contains(&first_line) {
                            user_intents.push(first_line);
                        }
                    }
                }
            }
            if !user_intents.is_empty() {
                block_parts.push(format!(
                    "Tasks: {}",
                    user_intents
                        .iter()
                        .take(2)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }

            if block_parts.len() > 1 {
                timeline_parts.push(block_parts.join(" | "));
            }
        }

        timeline_parts.push(format!("[Preserved: last {} turns]", recent_msgs.len()));
        let summary_text = timeline_parts.join("\n");

        let mut result = system_msgs;
        result.push(json!({"role": "system", "content": summary_text}));
        result.extend_from_slice(recent_msgs);
        RecoveryResult::success("context_collapse", result, 0)
    }

    /// Step 2: reactive structural compaction (aggressive thresholds).
    fn reactive_structural_recovery(
        &self,
        messages: &[Value],
        max_tokens: usize,
    ) -> RecoveryResult {
        let mut compactor = crate::compression::ReactiveCompactor::new(
            Some(0.0), // Always trigger
            None,
            Some(2), // Keep fewer messages
        );
        let (result_msgs, layer) = compactor.check_and_compact(messages, max_tokens);
        if layer.is_some() {
            RecoveryResult::success("reactive_structural", result_msgs, 0)
        } else {
            RecoveryResult::failed("reactive_structural", messages.to_vec())
        }
    }

    /// Step 3: reactive semantic compaction (LLM-based).
    fn reactive_semantic_recovery(&self, messages: &[Value], max_tokens: usize) -> RecoveryResult {
        let mut compactor = crate::compression::ReactiveCompactor::new(
            None,
            Some(0.0), // Always trigger
            Some(2),
        );
        let (result_msgs, layer) = compactor.check_and_compact(messages, max_tokens);
        if layer.is_some() {
            RecoveryResult::success("reactive_semantic", result_msgs, 0)
        } else {
            RecoveryResult::failed("reactive_semantic", messages.to_vec())
        }
    }

    /// Step 4: aggressive trim — keep only system + last 2 conversation msgs.
    fn aggressive_trim(&self, messages: &[Value]) -> RecoveryResult {
        let system_msgs: Vec<Value> = messages
            .iter()
            .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
            .cloned()
            .collect();
        let conv_msgs: Vec<Value> = messages
            .iter()
            .filter(|m| m.get("role").and_then(|r| r.as_str()) != Some("system"))
            .cloned()
            .collect();

        if conv_msgs.len() <= 2 {
            return RecoveryResult::failed("aggressive_trim", messages.to_vec());
        }

        let mut result = system_msgs;
        result.extend_from_slice(&conv_msgs[conv_msgs.len() - 2..]);
        RecoveryResult::success("aggressive_trim", result, 0)
    }

    fn is_smaller(&self, new_messages: &[Value], old_messages: &[Value]) -> bool {
        self.estimate_total_tokens(new_messages) < self.estimate_total_tokens(old_messages)
    }

    fn estimate_total_tokens(&self, messages: &[Value]) -> usize {
        let mut total = 0usize;
        for msg in messages {
            if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                if !content.is_empty() {
                    total += crate::token::estimate_tokens_simple(content);
                    total += 4;
                }
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msgs(n: usize) -> Vec<Value> {
        let mut out = vec![json!({"role": "system", "content": "sys"})];
        for i in 0..n {
            out.push(json!({"role": "user", "content": format!("message number {i} with some content here")}));
            out.push(json!({"role": "assistant", "content": format!("response number {i}")}));
        }
        out
    }

    #[test]
    fn context_collapse_archives_old_turns() {
        let r = PromptTooLongRecovery::default();
        let result = r.context_collapse_recovery(&msgs(20));
        assert!(result.success);
        // system(1) + summary(1) + last 4 recent conv messages.
        assert_eq!(result.messages.len(), 6);
        let summary = &result.messages[1];
        assert!(summary["content"]
            .as_str()
            .unwrap()
            .contains("Context Collapse Recovery"));
        assert!(summary["content"]
            .as_str()
            .unwrap()
            .contains("[Preserved: last 4 turns]"));
    }

    #[test]
    fn context_collapse_insufficient_messages() {
        let r = PromptTooLongRecovery::default();
        let result = r.context_collapse_recovery(&msgs(2));
        assert!(!result.success);
    }

    #[test]
    fn aggressive_trim_keeps_system_and_last_two() {
        let r = PromptTooLongRecovery::default();
        let result = r.aggressive_trim(&msgs(5));
        assert!(result.success);
        assert_eq!(result.messages.len(), 1 + 2);
        assert!(result.messages[1]["content"]
            .as_str()
            .unwrap()
            .contains("message number 4"));
    }

    #[test]
    fn aggressive_trim_insufficient() {
        let r = PromptTooLongRecovery::default();
        let result = r.aggressive_trim(&msgs(0));
        assert!(!result.success);
    }

    #[test]
    fn recover_cascade_succeeds_via_collapse() {
        let r = PromptTooLongRecovery::default();
        let m = msgs(30);
        let result = r.recover(&m, None);
        assert!(result.success);
        assert_eq!(result.strategy, "context_collapse");
        assert!(result.tokens_reduced > 0);
    }

    #[test]
    fn recover_exhausted_for_small_input() {
        let r = PromptTooLongRecovery::default();
        let m = msgs(1);
        let result = r.recover(&m, None);
        assert!(!result.success);
        assert_eq!(result.strategy, "exhausted");
    }

    #[test]
    fn timeline_includes_files_tools_tasks() {
        let r = PromptTooLongRecovery::default();
        let mut m = msgs(15);
        // Add a file ref and tool call in an old block.
        m[2]["content"] = json!("read src/main.py and src/util.rs");
        m[3]["role"] = json!("assistant");
        m[3]["tool_calls"] = json!([
            {"function": {"name": "read_file"}},
            {"function": {"name": "read_file"}}
        ]);
        let result = r.context_collapse_recovery(&m);
        let text = result.messages[1]["content"].as_str().expect("as_str").to_string();
        assert!(text.contains("Files:"), "{text}");
        assert!(text.contains("Tools: read_file(2)"), "{text}");
        assert!(text.contains("Tasks:"), "{text}");
    }
}
