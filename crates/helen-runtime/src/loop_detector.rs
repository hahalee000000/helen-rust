//! Tool call loop detection for LLM agent execution.
//!
//! Detects when an agent is stuck in a repetitive tool call pattern
//! and provides mechanisms to break the loop.
//!
//! Detection strategies:
//! 1. Exact match: Same function + same arguments repeated N times
//! 2. Near-match: Same function + similar arguments (for shell commands with minor variations)
//! 3. Sliding window: K duplicates in last M calls
//! 4. Tool-name-repeat: Same tool called N times with varying arguments
//!
//! v1.46.28: Ported from Python loop_detector.py
//! v1.46.37: Three-level graduated intervention + per-tool thresholds + informative hints

use serde_json::Value;
use std::collections::HashMap;

/// Per-tool thresholds: different tools have different repeat tolerance.
/// Read/execution tools are more tolerant, write/modification tools are stricter.
pub const DEFAULT_TOOL_THRESHOLDS: &[(&str, usize)] = &[
    // Read/execution tools — high tolerance (legitimate repeated use)
    ("shell_exec", 25),
    ("read_file", 20),
    ("list_directory", 20),
    ("search_files", 15),
    ("find_files", 15),
    ("web_search", 15),
    ("web_fetch", 15),
    // Write/modification tools — strict (repeated writes are suspicious)
    ("write_file", 5),
    ("edit_file", 5),
    ("patch_file", 5),
];

/// Default threshold for unknown tools.
pub const DEFAULT_THRESHOLD: usize = 10;

/// A normalized signature for a tool call, used for deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolCallSignature {
    pub name: String,
    pub args_key: String,
}

impl ToolCallSignature {
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

        let args_key = if let Some(args_str) = args_raw.as_str() {
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
            Value::Array(arr) => Value::Array(arr.iter().map(Self::normalize_value).collect()),
            _ => value.clone(),
        }
    }
}

/// Result of loop detection analysis.
#[derive(Debug, Clone)]
pub struct LoopDetectionResult {
    pub is_looping: bool,
    pub loop_type: Option<String>,
    pub repeated_count: usize,
    pub signature: Option<ToolCallSignature>,
    pub hint_message: Option<String>,
    pub threshold_used: usize,
}

impl LoopDetectionResult {
    pub fn not_looping() -> Self {
        Self {
            is_looping: false,
            loop_type: None,
            repeated_count: 0,
            signature: None,
            hint_message: None,
            threshold_used: 0,
        }
    }
}

/// Detects repetitive tool call patterns that indicate an agent is stuck.
///
/// v1.46.37: Mixed approach — per-tool thresholds + informative hints.
pub struct ToolCallLoopDetector {
    pub exact_repeat_threshold: usize,
    pub window_size: usize,
    pub window_repeat_threshold: usize,
    pub tool_only_repeat_threshold: usize,
    pub tool_thresholds: HashMap<String, usize>,
    history: Vec<ToolCallSignature>,
}

impl ToolCallLoopDetector {
    pub fn new() -> Self {
        let mut tool_thresholds = HashMap::new();
        for (name, threshold) in DEFAULT_TOOL_THRESHOLDS {
            tool_thresholds.insert(name.to_string(), *threshold);
        }
        Self {
            exact_repeat_threshold: 3,
            window_size: 10,
            window_repeat_threshold: 5,
            tool_only_repeat_threshold: 15,
            tool_thresholds,
            history: Vec::new(),
        }
    }

    /// Get the repeat threshold for a specific tool.
    pub fn get_tool_threshold(&self, tool_name: &str) -> usize {
        self.tool_thresholds
            .get(tool_name)
            .copied()
            .unwrap_or(DEFAULT_THRESHOLD)
    }

    /// Record a batch of tool calls (one turn's worth).
    pub fn record(&mut self, tool_calls: &[Value]) {
        for tc in tool_calls {
            let sig = ToolCallSignature::from_tool_call(tc);
            self.history.push(sig);
        }
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
        // Strategy 1: exact consecutive repeats
        let r = self.check_exact_repeats();
        if r.is_looping {
            return r;
        }
        // Strategy 2: sliding window pattern
        let r = self.check_window_pattern();
        if r.is_looping {
            return r;
        }
        // Strategy 3: tool-name-repeat (same tool, varying args)
        let r = self.check_tool_name_repeat();
        if r.is_looping {
            return r;
        }
        LoopDetectionResult::not_looping()
    }

    /// Strategy 1: Check if the last N calls are all identical.
    fn check_exact_repeats(&self) -> LoopDetectionResult {
        if self.history.is_empty() {
            return LoopDetectionResult::not_looping();
        }
        let last_tool_name = &self.history.last().unwrap().name;
        let threshold = self.get_tool_threshold(last_tool_name);
        if self.history.len() < threshold {
            return LoopDetectionResult::not_looping();
        }
        let recent = &self.history[self.history.len() - threshold..];
        let first = &recent[0];
        if recent.iter().all(|sig| sig == first) {
            let mut count = 1;
            for i in (0..self.history.len() - 1).rev() {
                if self.history[i] == *first {
                    count += 1;
                } else {
                    break;
                }
            }
            let hint = self.build_informative_hint("exact", first, count, threshold);
            return LoopDetectionResult {
                is_looping: true,
                loop_type: Some("exact".to_string()),
                repeated_count: count,
                signature: Some(first.clone()),
                hint_message: Some(hint),
                threshold_used: threshold,
            };
        }
        LoopDetectionResult::not_looping()
    }

    /// Strategy 2: Check for repeated calls within a sliding window.
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
        let mut counts: HashMap<&ToolCallSignature, usize> = HashMap::new();
        for sig in window {
            *counts.entry(sig).or_insert(0) += 1;
        }
        if let Some((sig, count)) = counts.iter().max_by_key(|(_, c)| *c) {
            if *count >= self.window_repeat_threshold {
                let hint = self.build_informative_hint(
                    "pattern",
                    sig,
                    *count,
                    self.window_repeat_threshold,
                );
                return LoopDetectionResult {
                    is_looping: true,
                    loop_type: Some("pattern".to_string()),
                    repeated_count: *count,
                    signature: Some((*sig).clone()),
                    hint_message: Some(hint),
                    threshold_used: self.window_repeat_threshold,
                };
            }
        }
        LoopDetectionResult::not_looping()
    }

    /// Strategy 3: Check if the last N calls all use the same tool (any arguments).
    fn check_tool_name_repeat(&self) -> LoopDetectionResult {
        if self.history.is_empty() {
            return LoopDetectionResult::not_looping();
        }
        let last_tool_name = &self.history.last().unwrap().name;
        let threshold = self.get_tool_threshold(last_tool_name);
        if self.history.len() < threshold {
            return LoopDetectionResult::not_looping();
        }
        let recent = &self.history[self.history.len() - threshold..];
        if recent.iter().all(|sig| sig.name == *last_tool_name) {
            let mut count = threshold;
            for i in (0..self.history.len() - threshold).rev() {
                if self.history[i].name == *last_tool_name {
                    count += 1;
                } else {
                    break;
                }
            }
            let synthetic_sig = ToolCallSignature {
                name: last_tool_name.clone(),
                args_key: "(various arguments)".to_string(),
            };
            let hint = self.build_tool_name_hint(last_tool_name, count, threshold);
            return LoopDetectionResult {
                is_looping: true,
                loop_type: Some("tool-name-repeat".to_string()),
                repeated_count: count,
                signature: Some(synthetic_sig),
                hint_message: Some(hint),
                threshold_used: threshold,
            };
        }
        LoopDetectionResult::not_looping()
    }

    /// Build an informative hint that provides information and suggestions.
    fn build_informative_hint(
        &self,
        loop_type: &str,
        signature: &ToolCallSignature,
        repeated_count: usize,
        _threshold: usize,
    ) -> String {
        let args_display = self.format_args(&signature.args_key);
        let header = format!("[Loop Detection Notice: {} pattern]", loop_type);
        let detail = format!(
            "Tool: {}({}) called {} times",
            signature.name, args_display, repeated_count
        );
        let message = format!(
            "You have called `{}` {} times with this pattern.\n\n\
            **Common scenarios:**\n\
            - ✅ OK: Reading different parts of a large file\n\
            - ✅ OK: Processing multiple items in a loop\n\
            - ✅ OK: Iterative refinement with visible progress\n\
            - ❌ Not OK: Repeating the same failed command\n\
            - ❌ Not OK: Calling with identical arguments expecting different results\n\n\
            **If you're making progress**, continue with your next step.\n\n\
            **If you're stuck**, consider:\n\
            1. Summarizing what you've learned so far\n\
            2. Trying a completely different approach\n\
            3. Asking the user for clarification\n\n\
            Note: If this pattern continues without progress, you may be asked to provide your final answer.",
            signature.name, repeated_count
        );
        format!("{}\n{}\n\n{}", header, detail, message)
    }

    /// Build an informative hint for tool-name-only repeat detection.
    fn build_tool_name_hint(
        &self,
        tool_name: &str,
        repeated_count: usize,
        _threshold: usize,
    ) -> String {
        let header = format!(
            "[Loop Detection Notice: repeated {} calls]",
            tool_name
        );
        let detail = format!(
            "Tool: {} called {} times with varying arguments",
            tool_name, repeated_count
        );
        let context_help = if matches!(tool_name, "shell_exec" | "read_file") {
            "This often happens when:\n\
            - Reading a large file chunk by chunk (consider reading the whole file or using grep)\n\
            - Running similar commands with small variations (consider a single comprehensive command)\n\
            - Checking the same condition repeatedly (the result won't change mid-task)"
        } else if matches!(tool_name, "write_file" | "edit_file" | "patch_file") {
            "This often happens when:\n\
            - Trying to fix a file with multiple small edits (consider a single comprehensive edit)\n\
            - Writing intermediate results (consider accumulating changes before writing)\n\
            - Retry after write failure (check permissions and disk space)"
        } else {
            "This often happens when:\n\
            - Making incremental progress but not consolidating\n\
            - Repeating similar operations without a clear plan\n\
            - Not recognizing when enough information has been gathered"
        };
        let message = format!(
            "{}\n\n\
            **If you're making progress**, continue but consider consolidating your approach.\n\n\
            **If you're stuck**, consider:\n\
            1. Summarizing what you've learned and providing your final answer\n\
            2. Taking a step back and planning a different approach\n\
            3. Asking the user for guidance\n\n\
            Note: If this pattern continues without progress, you may be asked to provide your final answer.",
            context_help
        );
        format!("{}\n{}\n\n{}", header, detail, message)
    }

    /// Format arguments for display.
    fn format_args(&self, args_key: &str) -> String {
        if let Ok(args) = serde_json::from_str::<Value>(args_key) {
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
                args_key.chars().take(100).collect()
            }
        } else {
            args_key.chars().take(100).collect()
        }
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
    fn test_per_tool_thresholds() {
        let detector = ToolCallLoopDetector::new();
        assert_eq!(detector.get_tool_threshold("shell_exec"), 25);
        assert_eq!(detector.get_tool_threshold("write_file"), 5);
        assert_eq!(detector.get_tool_threshold("unknown_tool"), 10);
    }

    #[test]
    fn test_exact_repeat_detection() {
        let mut detector = ToolCallLoopDetector::new();
        // Use write_file (threshold=5) for faster testing
        let tc = json!({
            "function": {
                "name": "write_file",
                "arguments": "{\"path\": \"test.txt\"}"
            }
        });
        for _ in 0..5 {
            detector.record(std::slice::from_ref(&tc));
        }
        let result = detector.check();
        assert!(result.is_looping);
        assert_eq!(result.loop_type, Some("exact".to_string()));
        assert_eq!(result.threshold_used, 5);
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
        detector.record(std::slice::from_ref(&tc1));
        detector.record(&[tc2]);
        detector.record(&[tc1]);
        let result = detector.check();
        assert!(!result.is_looping);
    }

    #[test]
    fn test_tool_name_repeat_detection() {
        let mut detector = ToolCallLoopDetector::new();
        // Use write_file (threshold=5) for tool-name-repeat
        let tc1 = json!({"function": {"name": "write_file", "arguments": "{\"path\": \"a.txt\"}"}});
        let tc2 = json!({"function": {"name": "write_file", "arguments": "{\"path\": \"b.txt\"}"}});
        let tc3 = json!({"function": {"name": "write_file", "arguments": "{\"path\": \"c.txt\"}"}});
        // 5 calls with different args but same tool name
        detector.record(std::slice::from_ref(&tc1));
        detector.record(std::slice::from_ref(&tc2));
        detector.record(std::slice::from_ref(&tc3));
        detector.record(std::slice::from_ref(&tc1));
        detector.record(std::slice::from_ref(&tc2));
        let result = detector.check();
        assert!(result.is_looping);
        assert_eq!(result.loop_type, Some("tool-name-repeat".to_string()));
    }

    #[test]
    fn test_informative_hint_format() {
        let mut detector = ToolCallLoopDetector::new();
        let tc = json!({
            "function": {
                "name": "write_file",
                "arguments": "{\"path\": \"test.txt\"}"
            }
        });
        for _ in 0..5 {
            detector.record(std::slice::from_ref(&tc));
        }
        let result = detector.check();
        assert!(result.is_looping);
        let hint = result.hint_message.unwrap();
        assert!(hint.contains("Common scenarios"));
        assert!(hint.contains("If you're making progress"));
        assert!(hint.contains("If you're stuck"));
    }

    #[test]
    fn test_reset() {
        let mut detector = ToolCallLoopDetector::new();
        let tc = json!({
            "function": {
                "name": "write_file",
                "arguments": "{\"path\": \"test.txt\"}"
            }
        });
        for _ in 0..5 {
            detector.record(std::slice::from_ref(&tc));
        }
        assert!(detector.check().is_looping);
        detector.reset();
        assert!(!detector.check().is_looping);
    }
}
