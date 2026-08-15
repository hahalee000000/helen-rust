//! Context awareness (Task 8.6) — port of `helen/runtime/context_awareness.py`.
//!
//! Injects token-budget awareness into LLM context:
//! 1. `<budget:token_budget>N</budget:token_budget>` in the system prompt
//! 2. `<system_warning>Token usage: X%; N remaining</system_warning>` injected
//!    after tool calls at warning (50%) / critical (75%) / emergency (90%).

use serde_json::Value;

pub const USAGE_NORMAL: &str = "normal";
pub const USAGE_WARNING: &str = "warning";
pub const USAGE_CRITICAL: &str = "critical";
pub const USAGE_EMERGENCY: &str = "emergency";

pub const WARNING_THRESHOLD: f64 = 0.50;
pub const CRITICAL_THRESHOLD: f64 = 0.75;
pub const EMERGENCY_THRESHOLD: f64 = 0.90;

/// `ContextAwareness` — inject token budget awareness into LLM context.
pub struct ContextAwareness {
    pub max_tokens: u64,
}

impl Default for ContextAwareness {
    fn default() -> Self {
        Self {
            max_tokens: crate::token::DEFAULT_CONTEXT_WINDOW as u64,
        }
    }
}

impl ContextAwareness {
    pub fn new(max_tokens: Option<u64>) -> Self {
        Self {
            max_tokens: max_tokens.unwrap_or(crate::token::DEFAULT_CONTEXT_WINDOW as u64),
        }
    }

    /// Inject `<budget:token_budget>` tag into the system prompt.
    pub fn inject_budget_tag(&self, system_prompt: &str) -> String {
        if system_prompt.contains("<budget:token_budget>") {
            return system_prompt.to_string();
        }
        format!(
            "{system_prompt}\n<budget:token_budget>{}</budget:token_budget>",
            self.max_tokens
        )
    }

    /// `build_usage_warning` — warning message if usage >= 50%, else None.
    pub fn build_usage_warning(&self, messages: &[Value]) -> Option<String> {
        let current_tokens = self.calculate_tokens(messages);
        let ratio = if self.max_tokens > 0 {
            current_tokens as f64 / self.max_tokens as f64
        } else {
            0.0
        };
        let level = self.get_usage_level(ratio);

        if level == USAGE_NORMAL {
            return None;
        }

        let remaining = self.max_tokens.saturating_sub(current_tokens as u64);
        let pct = (ratio * 100.0) as i64;

        match level {
            USAGE_WARNING => Some(format!(
                "<system_warning>Token usage: {current_tokens}/{}; {pct}% used; {remaining} remaining</system_warning>",
                self.max_tokens
            )),
            USAGE_CRITICAL => Some(format!(
                "<system_warning>Token usage: {current_tokens}/{}; {pct}% used; {remaining} remaining. Consider summarizing previous work before continuing.</system_warning>",
                self.max_tokens
            )),
            USAGE_EMERGENCY => Some(format!(
                "<system_warning>CRITICAL: Token usage at {pct}% ({current_tokens}/{}); only {remaining} tokens remaining. You MUST be concise. Avoid redundant explanations.</system_warning>",
                self.max_tokens
            )),
            _ => None,
        }
    }

    /// `get_usage_level` — classify a usage ratio into a level.
    pub fn get_usage_level(&self, ratio: f64) -> &'static str {
        if ratio >= EMERGENCY_THRESHOLD {
            USAGE_EMERGENCY
        } else if ratio >= CRITICAL_THRESHOLD {
            USAGE_CRITICAL
        } else if ratio >= WARNING_THRESHOLD {
            USAGE_WARNING
        } else {
            USAGE_NORMAL
        }
    }

    /// `_calculate_tokens` — sum content tokens + 4 overhead per message.
    pub fn calculate_tokens(&self, messages: &[Value]) -> usize {
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
    use serde_json::json;

    #[test]
    fn default_max_tokens() {
        let a = ContextAwareness::default();
        assert_eq!(a.max_tokens, crate::token::DEFAULT_CONTEXT_WINDOW as u64);
    }

    #[test]
    fn usage_level_classification() {
        let a = ContextAwareness::new(Some(1000));
        assert_eq!(a.get_usage_level(0.1), USAGE_NORMAL);
        assert_eq!(a.get_usage_level(0.5), USAGE_WARNING);
        assert_eq!(a.get_usage_level(0.75), USAGE_CRITICAL);
        assert_eq!(a.get_usage_level(0.9), USAGE_EMERGENCY);
        assert_eq!(a.get_usage_level(1.5), USAGE_EMERGENCY);
    }

    #[test]
    fn budget_tag_injected() {
        let a = ContextAwareness::new(Some(8000));
        let out = a.inject_budget_tag("You are helpful.");
        assert!(
            out.contains("<budget:token_budget>8000</budget:token_budget>"),
            "{out}"
        );
        // Idempotent
        let again = a.inject_budget_tag(&out);
        assert_eq!(again, out);
    }

    #[test]
    fn normal_usage_no_warning() {
        let a = ContextAwareness::new(Some(1000));
        let msgs = vec![json!({"role": "user", "content": "hello world"})];
        assert!(a.build_usage_warning(&msgs).is_none());
    }

    #[test]
    fn warning_at_50_percent() {
        // 100 tokens content (400 chars EN) + 4 overhead = 104/1000 = 10.4% — too low.
        // Use max_tokens=208 so ratio=0.5 exactly.
        let a = ContextAwareness::new(Some(208));
        let msgs = vec![json!({"role": "user", "content": "a".repeat(400)})];
        let w = a.build_usage_warning(&msgs).unwrap();
        assert!(
            w.contains(
                "<system_warning>Token usage: 104/208; 50% used; 104 remaining</system_warning>"
            ),
            "{w}"
        );
    }

    #[test]
    fn emergency_format() {
        // 54 tokens content (200 EN chars /4 + 4) / 60 max = 0.9 -> emergency.
        let a = ContextAwareness::new(Some(60));
        let msgs = vec![json!({"role": "user", "content": "a".repeat(200)})];
        let w = a.build_usage_warning(&msgs).unwrap();
        assert!(w.contains("CRITICAL: Token usage at 90%"), "{w}");
        assert!(w.contains("You MUST be concise"), "{w}");
    }

    #[test]
    fn token_count_includes_overhead() {
        let a = ContextAwareness::new(Some(1000));
        let msgs = vec![
            json!({"role": "system", "content": "hello world"}), // 2 + 4
            json!({"role": "user", "content": ""}),              // 0 (empty skipped)
        ];
        assert_eq!(a.calculate_tokens(&msgs), 6);
    }
}
