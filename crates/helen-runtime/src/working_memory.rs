//! Working memory (Task 8.4) — port of `helen/runtime/working_memory.py`.
//!
//! Compact high-priority context buffer: task description, active files,
//! recent decisions, pending TODOs, error history. Token-budget-aware
//! section dropping and three-channel context building.

use crate::token::{estimate_tokens_simple, is_cjk_char};

/// Response buffer ratio (10% reserved for model response).
pub const RESPONSE_BUFFER_RATIO: f64 = 0.10;

/// Three-channel budget: system 10% / working 45% / history 35%.
pub const THREE_CHANNEL_BUDGET: &[(&str, f64)] =
    &[("system", 0.10), ("working", 0.45), ("history", 0.35)];

/// Convert token budget to character budget (CJK-aware).
pub fn tokens_to_chars(text: &str, token_budget: usize) -> usize {
    if text.is_empty() {
        return 0;
    }
    let cjk_count = text.chars().filter(|c| is_cjk_char(*c)).count();
    let total_len = text.chars().count();
    if total_len == 0 {
        return 0;
    }
    // Weighted chars-per-token ratio based on actual composition
    let ratio = (cjk_count as f64 * 1.2 + (total_len - cjk_count) as f64 * 4.0) / total_len as f64;
    (token_budget as f64 * ratio) as usize
}

/// Truncate text to fit within token budget (CJK-aware).
pub fn truncate_to_token_budget(text: &str, max_tokens: usize) -> String {
    if estimate_tokens_simple(text) <= max_tokens {
        return text.to_string();
    }
    let char_budget = tokens_to_chars(text, max_tokens);
    let end = char_budget.min(text.len());
    text[..end].to_string()
}

/// Compact working memory for tracking essential context.
#[derive(Debug, Clone, Default)]
pub struct WorkingMemory {
    pub task_description: String,
    pub active_files: Vec<String>,
    pub recent_decisions: Vec<String>,
    pub pending_todos: Vec<String>,
    pub error_history: Vec<MemoryError>,
    /// Token budget for working memory (default 5000).
    pub max_tokens: usize,
}

/// An error entry in working memory.
#[derive(Debug, Clone, Default)]
pub struct MemoryError {
    pub command: String,
    pub error: String,
}

impl WorkingMemory {
    pub fn new() -> Self {
        WorkingMemory {
            task_description: String::new(),
            active_files: Vec::new(),
            recent_decisions: Vec::new(),
            pending_todos: Vec::new(),
            error_history: Vec::new(),
            max_tokens: 5000,
        }
    }

    /// Format working memory as context for the LLM.
    ///
    /// Sections are progressively dropped (lowest-priority first) to fit
    /// within the budget. Priority order (highest first): Current Task >
    /// Recent Errors > Active Files > Recent Decisions > Pending TODOs.
    pub fn to_context(&self, budget_tokens: Option<usize>) -> String {
        let effective_budget = if self.max_tokens > 0 {
            match budget_tokens {
                None => Some(self.max_tokens),
                Some(b) => Some(b.min(self.max_tokens)),
            }
        } else {
            budget_tokens
        };

        // Build sections in priority order (highest priority first).
        // Each entry: (section_header_lines, section_body_lines)
        let mut sections: Vec<(Vec<String>, Vec<String>)> = Vec::new();

        if !self.task_description.is_empty() {
            sections.push((
                vec!["## Current Task".to_string()],
                vec![self.task_description.clone(), String::new()],
            ));
        }

        if !self.error_history.is_empty() {
            let mut body: Vec<String> = Vec::new();
            for e in self.error_history.iter().rev().take(3).rev() {
                let cmd = if e.command.is_empty() {
                    "unknown".to_string()
                } else {
                    e.command.clone()
                };
                let err: String = if e.error.is_empty() {
                    "unknown".to_string()
                } else {
                    e.error.clone()
                };
                let err: String = err.chars().take(100).collect();
                body.push(format!("- Command: {cmd}"));
                body.push(format!("  Error: {err}"));
            }
            body.push(String::new());
            sections.push((vec!["## Recent Errors".to_string()], body));
        }

        if !self.active_files.is_empty() {
            let mut body: Vec<String> = Vec::new();
            for f in self.active_files.iter().rev().take(5).rev() {
                body.push(format!("- {f}"));
            }
            body.push(String::new());
            sections.push((vec!["## Active Files".to_string()], body));
        }

        if !self.recent_decisions.is_empty() {
            let mut body: Vec<String> = Vec::new();
            for d in self.recent_decisions.iter().rev().take(5).rev() {
                body.push(format!("- {d}"));
            }
            body.push(String::new());
            sections.push((vec!["## Recent Decisions".to_string()], body));
        }

        if !self.pending_todos.is_empty() {
            let mut body: Vec<String> = Vec::new();
            for t in self.pending_todos.iter().take(10) {
                body.push(format!("- [ ] {t}"));
            }
            body.push(String::new());
            sections.push((vec!["## Pending TODOs".to_string()], body));
        }

        let Some(effective_budget) = effective_budget else {
            // No budget — include everything
            let mut parts: Vec<String> = Vec::new();
            for (header, body) in &sections {
                parts.extend(header.clone());
                parts.extend(body.clone());
            }
            return parts.join("\n");
        };

        // With budget: drop lowest-priority sections until we fit.
        let mut included: Vec<usize> = (0..sections.len()).collect();
        let section_text = |i: usize| -> String {
            format!("{}\n{}", sections[i].0.join("\n"), sections[i].1.join("\n"))
        };
        let mut total_tokens: usize = included
            .iter()
            .map(|i| estimate_tokens_simple(&section_text(*i)))
            .sum();

        while total_tokens > effective_budget && included.len() > 1 {
            let dropped = included.pop().expect("non-empty");
            total_tokens =
                total_tokens.saturating_sub(estimate_tokens_simple(&section_text(dropped)));
        }

        // If even the highest-priority section alone exceeds budget,
        // truncate its body content to fit.
        let mut parts: Vec<String> = Vec::new();
        let mut remaining_tokens = effective_budget;
        for idx in &included {
            let (header, body) = &sections[*idx];
            let header_str = header.join("\n");
            let body_str = body.join("\n");
            let section_str = format!("{header_str}\n{body_str}");
            let section_tokens = estimate_tokens_simple(&section_str);

            if section_tokens <= remaining_tokens {
                parts.push(section_str);
                remaining_tokens -= section_tokens;
            } else {
                let header_tokens = estimate_tokens_simple(&header_str);
                if remaining_tokens > header_tokens + 2 {
                    // Can fit header + partial body; truncate body
                    let body_budget_tokens = remaining_tokens - header_tokens - 1;
                    let char_budget = tokens_to_chars(&body_str, body_budget_tokens);
                    let mut truncated_body = body_str;
                    truncated_body.truncate(char_budget.min(truncated_body.len()));
                    // Cut at last complete line to avoid mid-character break
                    if let Some(pos) = truncated_body.rfind('\n') {
                        if pos > 0 {
                            truncated_body.truncate(pos);
                        }
                    }
                    parts.push(format!("{header_str}\n{truncated_body}"));
                    break;
                } else {
                    break;
                }
            }
        }

        parts.join("\n")
    }

    /// Update working memory based on a tool call and its result.
    pub fn update_from_tool_call(
        &mut self,
        tool_call: &serde_json::Value,
        tool_result: &serde_json::Value,
    ) {
        let tool_name = tool_call.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = tool_call
            .get("args")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        match tool_name {
            "read_file" => {
                let file_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                if !file_path.is_empty() {
                    self.add_active_file(file_path);
                }
            }
            "write_file" | "patch_file" => {
                let file_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                if !file_path.is_empty() {
                    self.add_active_file(file_path);
                    self.add_decision(&format!("Modified file: {file_path}"));
                }
            }
            "shell_exec" => {
                let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                // Check if command failed (returncode != 0)
                let failed = tool_result
                    .get("returncode")
                    .and_then(|v| v.as_i64())
                    .map(|c| c != 0)
                    .unwrap_or(false);
                if failed {
                    let error_msg = tool_result
                        .get("stderr")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .or_else(|| tool_result.get("stdout").and_then(|v| v.as_str()))
                        .unwrap_or("Unknown error")
                        .to_string();
                    self.add_error(command, &error_msg);
                }
            }
            _ => {}
        }
    }

    pub fn add_active_file(&mut self, file_path: &str) {
        if !self.active_files.iter().any(|f| f == file_path) {
            self.active_files.push(file_path.to_string());
            self.evict_to_budget();
        }
    }

    pub fn add_decision(&mut self, decision: &str) {
        self.recent_decisions.push(decision.to_string());
        self.evict_to_budget();
    }

    pub fn add_error(&mut self, command: &str, error: &str) {
        self.error_history.push(MemoryError {
            command: command.to_string(),
            error: error.to_string(),
        });
        self.evict_to_budget();
    }

    pub fn add_todo(&mut self, todo: &str) {
        if !self.pending_todos.iter().any(|t| t == todo) {
            self.pending_todos.push(todo.to_string());
            self.evict_to_budget();
        }
    }

    pub fn complete_todo(&mut self, todo: &str) {
        self.pending_todos.retain(|t| t != todo);
    }

    /// Evict oldest entries to stay within token budget.
    ///
    /// Priority (highest first): task_description > error_history >
    /// active_files > recent_decisions > pending_todos.
    pub fn evict_to_budget(&mut self) {
        if self.max_tokens == 0 {
            return;
        }
        let estimate_list_tokens = |items: &[String]| -> usize {
            // Python parity: sum all chars, then divide by 4 once
            let total_chars: usize = items.iter().map(|s| s.chars().count()).sum();
            total_chars / 4
        };

        let task_tokens = if self.task_description.is_empty() {
            0
        } else {
            self.task_description.chars().count() / 4
        };
        let error_tokens: usize = self
            .error_history
            .iter()
            .map(|e| (e.command.chars().count() + e.error.chars().count()) / 4)
            .sum();
        let file_tokens = estimate_list_tokens(&self.active_files);
        let decision_tokens = estimate_list_tokens(&self.recent_decisions);
        let todo_tokens = estimate_list_tokens(&self.pending_todos);

        let mut total_tokens =
            task_tokens + error_tokens + file_tokens + decision_tokens + todo_tokens;
        if total_tokens <= self.max_tokens {
            return;
        }

        // Phase 1: Evict pending_todos (lowest priority)
        let mut todo_t = todo_tokens;
        while todo_t > 0 && total_tokens > self.max_tokens && !self.pending_todos.is_empty() {
            let removed = self.pending_todos.remove(0);
            let t = removed.chars().count() / 4;
            todo_t = todo_t.saturating_sub(t);
            total_tokens = total_tokens.saturating_sub(t);
        }

        // Phase 2: Evict recent_decisions
        let mut decision_t = decision_tokens;
        while decision_t > 0 && total_tokens > self.max_tokens && !self.recent_decisions.is_empty()
        {
            let removed = self.recent_decisions.remove(0);
            let t = removed.chars().count() / 4;
            decision_t = decision_t.saturating_sub(t);
            total_tokens = total_tokens.saturating_sub(t);
        }

        // Phase 3: Evict active_files
        let mut file_t = file_tokens;
        while file_t > 0 && total_tokens > self.max_tokens && !self.active_files.is_empty() {
            let removed = self.active_files.remove(0);
            let t = removed.chars().count() / 4;
            file_t = file_t.saturating_sub(t);
            total_tokens = total_tokens.saturating_sub(t);
        }

        // Phase 4: Evict error_history (highest priority list, evict last)
        let mut error_t = error_tokens;
        while error_t > 0 && total_tokens > self.max_tokens && !self.error_history.is_empty() {
            let removed = self.error_history.remove(0);
            let t = (removed.command.chars().count() + removed.error.chars().count()) / 4;
            error_t = error_t.saturating_sub(t);
            total_tokens = total_tokens.saturating_sub(t);
        }
    }

    pub fn estimate_tokens(&self) -> usize {
        self.to_context(None).chars().count() / 4
    }

    pub fn clear(&mut self) {
        self.task_description.clear();
        self.active_files.clear();
        self.recent_decisions.clear();
        self.pending_todos.clear();
        self.error_history.clear();
    }
}

/// Build three-channel context for LLM submission.
///
/// Channel 1 (10%): system instructions; Channel 2 (45%): working memory;
/// Channel 3 (35%): long-term memory (compressed history); 10% response buffer.
pub fn build_three_channel_context(
    system_prompt: &str,
    working_memory: &WorkingMemory,
    history: &[crate::transcript::Message],
    max_tokens: Option<usize>,
) -> Vec<serde_json::Value> {
    let max_tokens = max_tokens.unwrap_or(crate::token::DEFAULT_CONTEXT_WINDOW);

    // Reserve response buffer before distributing to channels
    let effective_max = (max_tokens as f64 * (1.0 - RESPONSE_BUFFER_RATIO)) as usize;
    let mut messages: Vec<serde_json::Value> = Vec::new();

    // Channel 1: System instructions
    let system_budget = (effective_max as f64 * 0.10) as usize;
    if !system_prompt.is_empty() {
        let truncated = truncate_to_token_budget(system_prompt, system_budget);
        messages.push(serde_json::json!({ "role": "system", "content": truncated }));
    }

    // Channel 2: Working memory (capped by working_memory.max_tokens)
    let mut working_budget = (effective_max as f64 * 0.45) as usize;
    working_budget = working_budget.min(working_memory.max_tokens);
    let working_context = working_memory.to_context(Some(working_budget));
    if !working_context.is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("[Working Memory]\n{working_context}"),
        }));
    }

    // Channel 3: Conversation history (most recent to oldest within budget)
    let history_budget = (effective_max as f64 * 0.35) as usize;
    let mut selected: Vec<&crate::transcript::Message> = Vec::new();
    let mut used_tokens: usize = 0;
    for msg in history.iter().rev() {
        let msg_tokens = msg.token_count();
        if used_tokens + msg_tokens <= history_budget {
            selected.insert(0, msg);
            used_tokens += msg_tokens;
        } else {
            break;
        }
    }
    for msg in selected {
        messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wm() -> WorkingMemory {
        let mut m = WorkingMemory::new();
        m.task_description = "Port runtime modules to Rust".to_string();
        m
    }

    #[test]
    fn to_context_includes_all_sections() {
        let mut m = wm();
        m.add_active_file("src/a.rs");
        m.add_decision("Use indexmap");
        m.add_todo("Write tests");
        m.add_error("cargo build", "error[E0308]");
        let ctx = m.to_context(None);
        assert!(ctx.contains("## Current Task"));
        assert!(ctx.contains("## Active Files"));
        assert!(ctx.contains("## Recent Decisions"));
        assert!(ctx.contains("## Pending TODOs"));
        assert!(ctx.contains("## Recent Errors"));
        assert!(ctx.contains("- [ ] Write tests"));
        assert!(ctx.contains("- Command: cargo build"));
    }

    #[test]
    fn to_context_drops_low_priority_sections() {
        let mut m = wm();
        for i in 0..10 {
            m.add_todo(&format!(
                "todo item number {i} with a very long description to consume budget"
            ));
        }
        m.max_tokens = 200; // Tight budget
        let ctx = m.to_context(Some(200));
        // Budget forces dropping; Current Task (highest priority) preserved
        assert!(ctx.contains("## Current Task"));
    }

    #[test]
    fn token_truncation_helpers() {
        assert_eq!(truncate_to_token_budget("short", 100), "short");
        let long = "x".repeat(1000);
        let t = truncate_to_token_budget(&long, 10);
        assert!(t.len() < long.len());
        assert!(tokens_to_chars("hello world", 10) > 0);
    }

    #[test]
    fn eviction_removes_oldest_lowest_priority() {
        let mut m = WorkingMemory::new();
        m.max_tokens = 60;
        for i in 0..20 {
            m.add_todo(&format!("todo number {i}"));
        }
        // Todos are lowest priority — evicted first
        assert!(m.pending_todos.len() < 20);
        // Task description preserved
        m.task_description = "important".to_string();
        m.evict_to_budget();
        assert_eq!(m.task_description, "important");
    }

    #[test]
    fn update_from_tool_call_tracks_files() {
        let mut m = wm();
        m.update_from_tool_call(
            &serde_json::json!({"name": "read_file", "args": {"path": "src/main.rs"}}),
            &serde_json::Value::Null,
        );
        assert!(m.active_files.contains(&"src/main.rs".to_string()));
        m.update_from_tool_call(
            &serde_json::json!({"name": "write_file", "args": {"path": "src/lib.rs"}}),
            &serde_json::Value::Null,
        );
        assert!(m.recent_decisions.iter().any(|d| d.contains("src/lib.rs")));
    }

    #[test]
    fn update_from_tool_call_tracks_errors() {
        let mut m = wm();
        m.update_from_tool_call(
            &serde_json::json!({"name": "shell_exec", "args": {"command": "cargo test"}}),
            &serde_json::json!({"returncode": 1, "stderr": "boom"}),
        );
        assert_eq!(m.error_history.len(), 1);
        assert_eq!(m.error_history[0].command, "cargo test");
        assert_eq!(m.error_history[0].error, "boom");
        // Success does not add errors
        m.update_from_tool_call(
            &serde_json::json!({"name": "shell_exec", "args": {"command": "ls"}}),
            &serde_json::json!({"returncode": 0, "stdout": "ok"}),
        );
        assert_eq!(m.error_history.len(), 1);
    }

    #[test]
    fn three_channel_context_structure() {
        let m = wm();
        let history = vec![
            crate::transcript::Message::new(
                "user",
                serde_json::Value::String("hi".into()),
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
            ),
            crate::transcript::Message::new(
                "assistant",
                serde_json::Value::String("hello".into()),
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
            ),
        ];
        let msgs = build_three_channel_context("sys", &m, &history, Some(10000));
        assert!(msgs.len() >= 3);
        assert_eq!(msgs[0]["role"].as_str().expect("as_str"), "system");
        // Working memory message contains [Working Memory]
        assert!(msgs[1]["content"]
            .as_str()
            .unwrap()
            .contains("[Working Memory]"));
        // Last message is the assistant history
        assert_eq!(
            msgs.last().unwrap()["role"].as_str().expect("as_str"),
            "assistant"
        );
    }

    #[test]
    fn clear_resets_all() {
        let mut m = wm();
        m.add_todo("t");
        m.add_error("c", "e");
        m.clear();
        assert!(m.pending_todos.is_empty());
        assert!(m.error_history.is_empty());
        assert!(m.task_description.is_empty());
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;

    #[test]
    fn to_context_matches_python_exactly() {
        let mut m = WorkingMemory::new();
        m.task_description = "Port runtime modules to Rust".to_string();
        m.add_active_file("src/a.rs");
        m.add_decision("Use indexmap");
        m.add_todo("Write tests");
        m.add_error("cargo build", "error[E0308]");

        let ctx = m.to_context(None);
        let expected = "## Current Task\nPort runtime modules to Rust\n\n## Recent Errors\n- Command: cargo build\n  Error: error[E0308]\n\n## Active Files\n- src/a.rs\n\n## Recent Decisions\n- Use indexmap\n\n## Pending TODOs\n- [ ] Write tests\n";
        assert_eq!(ctx, expected, "to_context(None) byte parity with Python");
        assert_eq!(
            m.to_context(Some(200)),
            expected,
            "to_context(200) byte parity with Python"
        );
    }
}
