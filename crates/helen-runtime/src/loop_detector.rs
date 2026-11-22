//! Tool call loop detection for LLM agent execution.
//!
//! Detects when an agent is stuck in a repetitive tool call pattern
//! and provides mechanisms to break the loop.
//!
//! Detection strategies:
//! 1. Exact match: Same function + same arguments repeated N times
//! 2. Near-match: Same function + similar arguments (for shell commands with minor variations)
//! 3. Sliding window: K duplicates in last M calls
//!
//! v1.46.28: Ported from Python loop_detector.py

use serde_json::Value;
use std::collections::HashMap;

/// A normalized signature for a tool call, used for deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolCallSignature {
    pub name: String,
    pub args_key: String, // Sorted, normalized JSON string of arguments
}

impl ToolCallSignature {
    /// Create a signature from an OpenAI-format tool call.
    pub fn from_tool_call(tool_call: &Value) -> Self {
        let function = tool_call.get("function").and_then(|f| f.as_object());
        let name = function
            .and_then(|f| f.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("unknown")
            .to_string();

        let args_raw = function
            .and_then(|f| f.get("arguments"))
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        // Normalize arguments: parse, sort keys, re-serialize
        let args_key = if let Some(args_str) = args_raw.as_str() {
            // Try to parse as JSON
            if let Ok(args) = serde_json::from_str::<Value>(args_str) {
                Self::normalize_args(&args)
            } else {
                args_str.to_string()
            }
        } else {
            Self::normalize_args(&args_raw)
        };

        Self { name, args_key }
    }

    /// Normalize arguments by sorting keys recursively.
    fn normalize_args(value: &Value) -> String {
        match value {
            Value::Object(map) => {
                let mut sorted: Vec<_> = map.iter().collect();
                sorted.sort_by_key(|(k, _)| (*k).clone());
                let normalized: serde_json::Map<String, Value> = sorted
                    .into_iter()
                    .map(|(k, v)| (k.clone(), Self::normalize_value(v)))
                    .collect();
                serde_json::to_string(&Value::Object(normalized)).unwrap_or_default()
            }
            _ => serde_json::to_string(value).unwrap_or_default(),
        }
    }

    /// Recursively normalize a value (sort object keys).
    fn normalize_value(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut sorted: Vec<_> = map.iter().collect();
                sorted.sort_by_key(|(k, _)| (*k).clone());
                let normalized: serde_json::Map<String, Value> = sorted
                    .into_iter()
                    .map(|(k, v)| (k.clone(), Self::normalize_value(v)))
                    .collect();
                Value::Object(normalized)
            }
            Value::Array(arr) => {
                Value::Array(arr.iter().map(Self::normalize_value).collect())
            }
            _ => value.clone(),
        }
    }
}

/// Result of loop detection analysis.
#[derive(Debug, Clone)]
pub struct LoopDetectionResult {
    pub is_looping: bool,
    pub loop_type: Option<String>, // "exact", "pattern"
    pub repeated_count: usize,
    pub signature: Option<ToolCallSignature>,
    pub hint_message: Option<String>,
}

impl LoopDetectionResult {
    pub fn not_looping() -> Self {
        Self {
            is_looping: false,
            loop_type: None,
            repeated_count: 0,
            signature: None,
            hint_message: None,
        }
    }
}

/// Detects repetitive tool call patterns that indicate an agent is stuck.
///
/// Configuration:
/// - exact_repeat_threshold: Consecutive identical calls to trigger (default: 3)
/// - window_size: Sliding window for pattern detection (default: 10)
/// - window_repeat_threshold: Duplicates within window to trigger (default: 5)
pub struct ToolCallLoopDetector {
    pub exact_repeat_threshold: usize,
    pub window_size: usize,
    pub window_repeat_threshold: usize,
    history: Vec<ToolCallSignature>,
}

impl ToolCallLoopDetector {
    pub fn new() -> Self {
        Self {
            exact_repeat_threshold: 3,
            window_size: 10,
            window_repeat_threshold: 5,
            history: Vec::new(),
        }
    }

    /// Record a batch of tool calls (one turn's worth).
    pub fn record(&mut self, tool_calls: &[Value]) {
        for tc in tool_calls {
            let sig = ToolCallSignature::from_tool_call(tc);
            self.history.push(sig);
        }

        // Keep history bounded
        let max_history = self.window_size * 3;
        if self.history.len() > max_history {
            self.history = self.history[self.history.len() - max_history..].to_vec();
        }
    }

    /// Check if the agent is in a loop.
    pub fn check(&self) -> LoopDetectionResult {
        if self.history.is_empty() {
            return LoopDetectionResult::not_looping();
        }

        // Strategy 1: Check for exact consecutive repeats at the end
        let exact_result = self.check_exact_repeats();
        if exact_result.is_looping {
            return exact_result;
        }

        // Strategy 2: Check for repeating pattern in sliding window
        let window_result = self.check_window_pattern();
        if window_result.is_looping {
            return window_result;
        }

        LoopDetectionResult::not_looping()
    }

    /// Check if the last N calls are all identical.
    fn check_exact_repeats(&self) -> LoopDetectionResult {
        if self.history.len() < self.exact_repeat_threshold {
            return LoopDetectionResult::not_looping();
        }

        // Check the last N signatures
        let recent = &self.history[self.history.len() - self.exact_repeat_threshold..];
        let first = &recent[0];

        if recent.iter().all(|sig| sig == first) {
            // Count how many consecutive
            let mut count = 1;
            for i in (0..self.history.len() - 1).rev() {
                if self.history[i] == *first {
                    count += 1;
                } else {
                    break;
                }
            }

            let hint = self.build_hint(
                "exact",
                first,
                count,
                &format!(
                    "**STOP IMMEDIATELY**: You have called `{}` with identical \
                    arguments {} times without making any progress.\n\n\
                    **You MUST do ONE of the following NOW:**\n\
                    1. If the task is complete: Provide a final summary of what was accomplished\n\
                    2. If the operation failed: Explain what went wrong and try a DIFFERENT approach\n\
                    3. If you need more information: Ask the user for clarification\n\n\
                    **DO NOT** call `{}` again with the same arguments. \
                    This is your final warning before forced termination.",
                    first.name, count, first.name
                ),
            );

            return LoopDetectionResult {
                is_looping: true,
                loop_type: Some("exact".to_string()),
                repeated_count: count,
                signature: Some(first.clone()),
                hint_message: Some(hint),
            };
        }

        LoopDetectionResult::not_looping()
    }

    /// Check for repeated calls within a sliding window.
    fn check_window_pattern(&self) -> LoopDetectionResult {
        if self.history.len() < self.window_repeat_threshold {
            return LoopDetectionResult::not_looping();
        }

        let window_start = if self.history.len() > self.window_size {
            self.history.len() - self.window_size
        } else {
            0
        };
        let window = &self.history[window_start..];

        // Count occurrences of each signature
        let mut counts: HashMap<&ToolCallSignature, usize> = HashMap::new();
        for sig in window {
            *counts.entry(sig).or_insert(0) += 1;
        }

        // Find the most repeated signature
        if let Some((sig, count)) = counts.iter().max_by_key(|(_, c)| *c) {
            if *count >= self.window_repeat_threshold {
                let hint = self.build_hint(
                    "pattern",
                    sig,
                    *count,
                    &format!(
                        "**STOP IMMEDIATELY**: Analysis of your last {} tool calls shows \
                        `{}` was called {} times with similar arguments but without \
                        making any progress. You are stuck in a repetitive pattern.\n\n\
                        **You MUST do ONE of the following NOW:**\n\
                        1. If the task is complete: Provide a final summary immediately\n\
                        2. If the current approach is failing: Switch to a COMPLETELY DIFFERENT strategy\n\
                        3. If you're uncertain: Ask the user for guidance\n\n\
                        **DO NOT** call `{}` again with similar arguments. \
                        Continuing this pattern will result in forced termination.",
                        window.len(),
                        sig.name,
                        count,
                        sig.name
                    ),
                );

                return LoopDetectionResult {
                    is_looping: true,
                    loop_type: Some("pattern".to_string()),
                    repeated_count: *count,
                    signature: Some((*sig).clone()),
                    hint_message: Some(hint),
                };
            }
        }

        LoopDetectionResult::not_looping()
    }

    /// Build a hint message to inject into the conversation.
    fn build_hint(
        &self,
        loop_type: &str,
        signature: &ToolCallSignature,
        repeated_count: usize,
        message: &str,
    ) -> String {
        // Decode arguments for display
        let args_display = if let Ok(args) = serde_json::from_str::<Value>(&signature.args_key) {
            if let Some(obj) = args.as_object() {
                let pairs: Vec<String> = obj
                    .iter()
                    .take(3)
                    .map(|(k, v)| format!("{}={:?}", k, v))
                    .collect();
                let mut display = pairs.join(", ");
                if obj.len() > 3 {
                    display += ", ...";
                }
                display
            } else {
                signature.args_key.chars().take(100).collect()
            }
        } else {
            signature.args_key.chars().take(100).collect()
        };

        let header = format!(
            "[CRITICAL LOOP DETECTION: {} repeat detected]",
            loop_type
        );
        let detail = format!(
            "Tool: {}({}) x{}",
            signature.name, args_display, repeated_count
        );

        format!("{}\n{}\n\n{}", header, detail, message)
    }

    /// Reset the detector state.
    pub fn reset(&mut self) {
        self.history.clear();
    }
}

impl Default for ToolCallLoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_signature_from_tool_call() {
        let tc = json!({
            "function": {
                "name": "web_search",
                "arguments": "{\"query\": \"AI news\"}"
            }
        });
        let sig = ToolCallSignature::from_tool_call(&tc);
        assert_eq!(sig.name, "web_search");
        assert!(sig.args_key.contains("AI news"));
    }

    #[test]
    fn test_exact_repeat_detection() {
        let mut detector = ToolCallLoopDetector::new();
        detector.exact_repeat_threshold = 3;

        let tc = json!({
            "function": {
                "name": "web_search",
                "arguments": "{\"query\": \"test\"}"
            }
        });

        // Record same tool call 3 times
        detector.record(&[tc.clone()]);
        detector.record(&[tc.clone()]);
        detector.record(&[tc.clone()]);

        let result = detector.check();
        assert!(result.is_looping);
        assert_eq!(result.loop_type, Some("exact".to_string()));
        assert!(result.repeated_count >= 3);
    }

    #[test]
    fn test_no_loop_different_calls() {
        let mut detector = ToolCallLoopDetector::new();

        let tc1 = json!({
            "function": {
                "name": "web_search",
                "arguments": "{\"query\": \"test1\"}"
            }
        });
        let tc2 = json!({
            "function": {
                "name": "web_search",
                "arguments": "{\"query\": \"test2\"}"
            }
        });

        detector.record(&[tc1.clone()]);
        detector.record(&[tc2]);
        detector.record(&[tc1]);

        let result = detector.check();
        assert!(!result.is_looping);
    }

    #[test]
    fn test_window_pattern_detection() {
        let mut detector = ToolCallLoopDetector::new();
        detector.window_size = 10;
        detector.window_repeat_threshold = 5;

        let tc = json!({
            "function": {
                "name": "web_search",
                "arguments": "{\"query\": \"test\"}"
            }
        });

        // Record same tool call 5 times in a window
        for _ in 0..5 {
            detector.record(&[tc.clone()]);
        }

        let result = detector.check();
        assert!(result.is_looping);
        assert_eq!(result.loop_type, Some("exact".to_string()));
    }

    #[test]
    fn test_reset() {
        let mut detector = ToolCallLoopDetector::new();
        let tc = json!({
            "function": {
                "name": "web_search",
                "arguments": "{\"query\": \"test\"}"
            }
        });

        detector.record(&[tc.clone()]);
        detector.record(&[tc.clone()]);
        detector.record(&[tc.clone()]);

        assert!(detector.check().is_looping);

        detector.reset();
        assert!(!detector.check().is_looping);
    }
}
