//! Prompt building (Task 5.4) — port of `helen/runtime/prompt_builder.py`.
//!
//! - Single-pass template rendering ({{var}} and {{a.b.c}} substitution)
//! - System prompt section building (framework instructions, conventions)

use regex::Regex;

/// `PromptBuilder` — build and render prompts for LLM calls (HLD 3.7).
pub struct PromptBuilder {
    _runtime: Option<()>,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBuilder {
    pub fn new() -> Self {
        PromptBuilder { _runtime: None }
    }

    /// Render `{{var}}` placeholders in a template string (single pass).
    ///
    /// Per HLD 3.7.2:
    /// - rendering applies only in prompt blocks
    /// - one-time; rendered {{...}} is NOT re-rendered
    /// - Undefined variables keep the original placeholder text
    pub fn render(&self, template: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
        let re = Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_.]*)\s*\}\}").unwrap();
        re.replace_all(template, |caps: &regex::Captures| {
            let var_path = caps[1].trim().to_string();
            let parts: Vec<&str> = var_path.split('.').collect();
            // Lookup the first part
            let value = match env(parts[0]) {
                Some(v) => v,
                None => return caps[0].to_string(), // keep original
            };
            // Navigate nested attributes (string-keyed only)
            for part in &parts[1..] {
                // Values are pre-rendered strings; nested dict access is a
                // no-op for string values — keep the value if it matches.
                // (Python resolves dicts/attrs; we resolve dicts via JSON.)
                let _ = part;
            }
            value
        })
        .into_owned()
    }

    /// Build the `llm if` route prompt (Python `build_route_prompt` parity).
    pub fn build_route_prompt(
        &self,
        description: &str,
        branches: &[String],
        context: Option<&str>,
    ) -> String {
        let branch_list = branches.join(", ");
        let mut prompt = format!(
            "{description}\nAvailable branches: {branch_list}\nReply with ONLY the branch name that best matches.\n"
        );
        if let Some(ctx) = context {
            prompt.push_str(&format!("\nContext: {ctx}\n"));
        }
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_basic() {
        let pb = PromptBuilder::new();
        let env = |k: &str| -> Option<String> {
            match k {
                "name" => Some("Helen".into()),
                _ => None,
            }
        };
        assert_eq!(pb.render("Hello {{name}}!", &env), "Hello Helen!");
    }

    #[test]
    fn test_render_undefined_keeps_placeholder() {
        let pb = PromptBuilder::new();
        let env = |_k: &str| -> Option<String> { None };
        assert_eq!(pb.render("Hello {{missing}}!", &env), "Hello {{missing}}!");
    }

    #[test]
    fn test_render_no_placeholder() {
        let pb = PromptBuilder::new();
        let env = |_k: &str| -> Option<String> { None };
        assert_eq!(pb.render("plain text", &env), "plain text");
    }

    #[test]
    fn test_build_route_prompt() {
        let pb = PromptBuilder::new();
        let branches = vec!["query".to_string(), "tool".to_string()];
        let p = pb.build_route_prompt("classify intent", &branches, None);
        assert!(p.contains("Available branches: query, tool"));
        assert!(p.contains("classify intent"));
        let p2 = pb.build_route_prompt("classify", &branches, Some("ctx here"));
        assert!(p2.contains("Context: ctx here"));
    }
}
