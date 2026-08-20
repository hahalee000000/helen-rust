//! REPL assistant (`:ask` command) — port of `helen/cli/ask_assistant.py`.
//!
//! L1: Direct LLM call with system prompt assembled from framework
//!     instructions + Helen conventions + REPL context block.
//! L2: Four REPL-state tools (repl_definitions / repl_last_error /
//!     repl_history / repl_read_file) injected via dispatch_fn.
//! L3: AssistantSession with multi-turn chat sub-REPL.

use helen_interpreter::interpreter::Interpreter;
use helen_runtime::http_llm::HttpLLMRuntime;
use helen_runtime::llm::LlmRuntime;
use helen_runtime::session::SessionManager;
use serde_json::{json, Value};
use std::io::{self, Write};

// ---------------------------------------------------------------------------
// ReplState — captured REPL state passed into the assistant prompt
// ---------------------------------------------------------------------------

/// Captured REPL state. The REPL module updates this after every input
/// turn so the assistant sees fresh data on each `:ask` invocation.
pub struct ReplState {
    /// Bounded buffer of recent REPL output lines (most-recent-last).
    pub output_buffer: Vec<String>,
    /// Maximum number of output lines to retain.
    pub output_buffer_max: usize,
    /// Persistent "last error" — survives errors.reset() between REPL turns.
    pub last_error_text: Option<String>,
}

impl Default for ReplState {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplState {
    pub fn new() -> Self {
        ReplState {
            output_buffer: Vec::new(),
            output_buffer_max: 50,
            last_error_text: None,
        }
    }

    /// Append REPL output lines, evicting oldest if over capacity.
    pub fn record_output(&mut self, text: &str) {
        for line in text.lines() {
            self.output_buffer.push(line.to_string());
        }
        if self.output_buffer.len() > self.output_buffer_max {
            let drain_count = self.output_buffer.len() - self.output_buffer_max;
            self.output_buffer.drain(..drain_count);
        }
    }

    /// Record a persistent last-error snapshot.
    pub fn record_error(&mut self, text: &str) {
        self.last_error_text = Some(text.to_string());
    }

    pub fn clear(&mut self) {
        self.output_buffer.clear();
        self.last_error_text = None;
    }
}

// ---------------------------------------------------------------------------
// System prompt building (L1)
// ---------------------------------------------------------------------------

/// Format REPL state into an XML-tagged context block for the assistant
/// system prompt. Empty sections are omitted to keep the block compact.
pub fn format_repl_context_block(
    definitions: &std::collections::HashMap<String, Vec<String>>,
    last_error_text: Option<&str>,
    recent_output: &[String],
    cwd: &str,
) -> String {
    let mut sections: Vec<String> = Vec::new();

    // Definitions
    let fns = definitions.get("functions").cloned().unwrap_or_default();
    let ags = definitions.get("agents").cloned().unwrap_or_default();
    if !fns.is_empty() || !ags.is_empty() {
        let mut lines = Vec::new();
        if !fns.is_empty() {
            let display: Vec<&str> = fns.iter().take(30).map(|s| s.as_str()).collect();
            let suffix = if fns.len() > 30 { "..." } else { "" };
            lines.push(format!("Functions: {}{}", display.join(", "), suffix));
        }
        if !ags.is_empty() {
            let display: Vec<&str> = ags.iter().take(30).map(|s| s.as_str()).collect();
            let suffix = if ags.len() > 30 { "..." } else { "" };
            lines.push(format!("Agents:    {}{}", display.join(", "), suffix));
        }
        sections.push(format!("## Current Definitions\n{}", lines.join("\n")));
    }

    // Last error
    if let Some(err_text) = last_error_text {
        let truncated = if err_text.len() > 1500 {
            format!("{}\n... (truncated)", &err_text[..1500])
        } else {
            err_text.to_string()
        };
        sections.push(format!("## Last Error\n```\n{}\n```", truncated));
    }

    // Recent output
    if !recent_output.is_empty() {
        let last_15: Vec<&str> = recent_output
            .iter()
            .rev()
            .take(15)
            .rev()
            .map(|s| s.as_str())
            .collect();
        let joined = last_15.join("\n");
        let truncated = if joined.len() > 2000 {
            format!("{}\n... (truncated)", &joined[joined.len() - 2000..])
        } else {
            joined
        };
        sections.push(format!("## Recent REPL Output\n```\n{}\n```", truncated));
    }

    // CWD
    if !cwd.is_empty() {
        sections.push(format!("## Working Directory\n`{}`", cwd));
    }

    if sections.is_empty() {
        return String::new();
    }
    format!("<repl_context>\n{}\n</repl_context>", sections.join("\n\n"))
}

/// Build the combined system prompt for the REPL assistant.
/// Combines framework instructions + Helen conventions + REPL context.
pub fn build_assistant_system_prompt(repl_context: &str) -> String {
    let mut parts: Vec<&str> = vec![FRAMEWORK_INSTRUCTIONS];
    parts.push(HELEN_CONVENTIONS);

    let mut result = parts.join("\n\n");
    if !repl_context.is_empty() {
        result.push_str("\n\n");
        result.push_str(repl_context);
    }
    result
}

/// Build the full assistant prompt: system prompt + REPL context.
pub fn build_assistant_prompt(interp: &Interpreter, repl_state: &ReplState, cwd: &str) -> String {
    let definitions = interp.list_definitions();
    let repl_block = format_repl_context_block(
        &definitions,
        repl_state.last_error_text.as_deref(),
        &repl_state.output_buffer,
        cwd,
    );
    build_assistant_system_prompt(&repl_block)
}

// ---------------------------------------------------------------------------
// L2: REPL state tools (exposed to the LLM via dispatch_fn)
// ---------------------------------------------------------------------------

/// Build tool definitions for the REPL tools (OpenAI-compatible format).
pub fn build_repl_tools() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "repl_definitions",
                "description": "List all functions and agents currently defined in the user's REPL session. Call this when the user refers to 'my function X' or 'the agent I defined' so you know what actually exists.",
                "parameters": {"type": "object", "properties": {}}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "repl_last_error",
                "description": "Return the last error the user hit in the REPL (type, message, location). Use this when the user asks 'why did this fail' or 'fix my error'.",
                "parameters": {"type": "object", "properties": {}}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "repl_history",
                "description": "Return the last N lines of REPL output (default 10). Use this to see what the user's code actually produced.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "n": {"type": "integer", "description": "Number of recent lines (default 10, max 50)."}
                    }
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "repl_read_file",
                "description": "Read a file from the user's current working directory. Path is relative to the REPL's cwd. Use this to inspect source code the user is working on. Restricted to cwd for safety.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Relative path within the REPL cwd."}
                    },
                    "required": ["path"]
                }
            }
        }),
    ]
}

/// Dispatch a REPL tool call. Returns the tool result as a string.
pub fn dispatch_repl_tool(
    name: &str,
    args: &Value,
    interp: &Interpreter,
    repl_state: &ReplState,
    cwd: &str,
) -> String {
    match name {
        "repl_definitions" => {
            let defs = interp.list_definitions();
            json!({
                "functions": defs.get("functions").cloned().unwrap_or_default(),
                "agents": defs.get("agents").cloned().unwrap_or_default(),
            })
            .to_string()
        }
        "repl_last_error" => {
            if let Some(ref err) = repl_state.last_error_text {
                err.clone()
            } else if let Some(ref snap) = interp.observability.last_error {
                snap.format_text(false)
            } else {
                "(no error recorded)".to_string()
            }
        }
        "repl_history" => {
            let n = args.get("n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let n = n.clamp(1, 50);
            let lines: Vec<&str> = repl_state
                .output_buffer
                .iter()
                .rev()
                .take(n)
                .rev()
                .map(|s| s.as_str())
                .collect();
            if lines.is_empty() {
                "(no recent output)".to_string()
            } else {
                lines.join("\n")
            }
        }
        "repl_read_file" => {
            let rel_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if rel_path.is_empty() {
                return "(error: 'path' argument required)".to_string();
            }
            // Safety: confine to cwd
            let cwd_path = match std::fs::canonicalize(cwd) {
                Ok(p) => p,
                Err(e) => return format!("(error: cannot resolve cwd: {})", e),
            };
            let target = cwd_path.join(rel_path);
            let target = match std::fs::canonicalize(&target) {
                Ok(p) => p,
                Err(_) => {
                    // File might not exist yet; check if path is within cwd
                    if !target.starts_with(&cwd_path) {
                        return format!("(error: path escapes cwd: {})", rel_path);
                    }
                    return format!("(error: not a file: {})", rel_path);
                }
            };
            if !target.starts_with(&cwd_path) {
                return format!("(error: path escapes cwd: {}", rel_path);
            }
            if !target.is_file() {
                return format!("(error: not a file: {}", rel_path);
            }
            match std::fs::read_to_string(&target) {
                Ok(content) => content,
                Err(e) => format!("(error reading file: {})", e),
            }
        }
        _ => "(error: unknown tool)".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Streaming response helper
// ---------------------------------------------------------------------------

/// Run a streaming LLM call, printing content events to stdout.
/// Returns the full response text.
fn run_streaming(
    runtime: &mut HttpLLMRuntime,
    prompt: &str,
    system_prompt: &str,
    tools: &[Value],
    dispatch_fn: &dyn Fn(&str, &Value) -> String,
) -> String {
    let mut chunks: Vec<String> = Vec::new();
    let stdout = io::stdout();

    let result = runtime.act_stream(
        prompt,
        None, // model
        0.7,  // temperature
        Some(system_prompt),
        Some(tools),
        5,    // max_turns
        None, // max_tokens
        None, // history
        Some(dispatch_fn),
        &mut |event: Value| {
            let etype = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match etype {
                "content" => {
                    if let Some(text) = event.get("content").and_then(|v| v.as_str()) {
                        if !text.is_empty() {
                            let mut out = stdout.lock();
                            let _ = out.write_all(text.as_bytes());
                            let _ = out.flush();
                            chunks.push(text.to_string());
                        }
                    }
                }
                "error" => {
                    let msg = event
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown error");
                    let mut out = stdout.lock();
                    let _ = write!(out, "\n(assistant error: {})", msg);
                    let _ = out.flush();
                }
                _ => {}
            }
            true // continue
        },
        false, // thinking_enabled
        None,  // reasoning_effort
    );

    if let Err(e) = result {
        let mut out = stdout.lock();
        let _ = write!(out, "\n(assistant error: {})", e);
        let _ = out.flush();
    }
    println!(); // final newline
    chunks.join("")
}

// ---------------------------------------------------------------------------
// Single-turn :ask (L1 + L2)
// ---------------------------------------------------------------------------

/// Run a single `:ask` question and print the assistant's response.
pub fn ask_single(question: &str, interp: &Interpreter, repl_state: &ReplState, cwd: &str) {
    let system_prompt = build_assistant_prompt(interp, repl_state, cwd);
    let repl_tools = build_repl_tools();

    let mut runtime = match create_llm_runtime() {
        Some(rt) => rt,
        None => {
            println!("(error: LLM runtime not configured. Set HELEN_LLM_BASE_URL and HELEN_LLM_API_KEY.)");
            return;
        }
    };

    println!("\n🤔 Thinking...\n");

    let dispatch_fn = |name: &str, args: &Value| -> String {
        dispatch_repl_tool(name, args, interp, repl_state, cwd)
    };

    // Try streaming first
    run_streaming(
        &mut runtime,
        question,
        &system_prompt,
        &repl_tools,
        &dispatch_fn,
    );
}

// ---------------------------------------------------------------------------
// Multi-turn chat mode (L3)
// ---------------------------------------------------------------------------

/// Per-session REPL state for chat mode (independent of main REPL's buffer).
struct ChatSessionState {
    repl_state: ReplState,
}

impl ChatSessionState {
    fn new() -> Self {
        ChatSessionState {
            repl_state: ReplState::new(),
        }
    }
}

/// Enter the multi-turn `:ask` sub-REPL.
/// Loops until the user types `:exit`, `exit`, or presses Ctrl+C/D.
pub fn run_chat_mode(session_id: &str, cwd: &str) {
    println!("\n💬 Entered :ask chat mode (session: {})", session_id);
    println!("   Type :exit or Ctrl+C to return to the main REPL.\n");

    // Create a dedicated interpreter for this chat session
    let interp = Interpreter::new();
    let session_state = ChatSessionState::new();

    loop {
        print!(
            "[{}:ask] >>> ",
            session_id.chars().take(8).collect::<String>()
        );
        if io::stdout().flush().is_err() {
            break;
        }

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                println!("\n(read error: {})", e);
                break;
            }
        }

        let line = input.trim();
        if line.is_empty() {
            continue;
        }
        if line == ":exit" || line == "exit" || line == ":quit" || line == "quit" {
            break;
        }

        let system_prompt = build_assistant_prompt(&interp, &session_state.repl_state, cwd);
        let repl_tools = build_repl_tools();

        let mut runtime = match create_llm_runtime() {
            Some(rt) => rt,
            None => {
                println!("(error: LLM runtime not configured)");
                continue;
            }
        };

        let dispatch_fn = |name: &str, args: &Value| -> String {
            dispatch_repl_tool(name, args, &interp, &session_state.repl_state, cwd)
        };

        run_streaming(
            &mut runtime,
            line,
            &system_prompt,
            &repl_tools,
            &dispatch_fn,
        );
    }
}

// ---------------------------------------------------------------------------
// Session listing
// ---------------------------------------------------------------------------

/// List past `:ask` chat sessions from SessionManager.
pub fn list_assistant_sessions() -> Vec<helen_runtime::session::SessionInfo> {
    let manager = SessionManager::new(None);
    let mut sessions = manager.list_sessions();
    // Cap to most recent 20
    sessions.truncate(20);
    sessions
}

// ---------------------------------------------------------------------------
// LLM runtime creation helper
// ---------------------------------------------------------------------------

/// Create an HttpLLMRuntime from environment/config. Returns None if not configured.
fn create_llm_runtime() -> Option<HttpLLMRuntime> {
    let runtime = HttpLLMRuntime::new(None, None, None);
    // Check if we have a valid API key
    if runtime.api_key.is_empty() || runtime.api_key == "sk-placeholder" {
        return None;
    }
    Some(runtime)
}

// ---------------------------------------------------------------------------
// Framework instructions & Helen conventions (embedded constants)
// ---------------------------------------------------------------------------

const FRAMEWORK_INSTRUCTIONS: &str = r#"<framework_instructions>
You are a Helen agent with tools and skills available. Follow these rules:

## 1. Tool Use (CRITICAL)
You MUST use your tools to take action — do not describe what you would do
without actually doing it. When tools are available, use them instead of
telling the user what you would do. Execute, don't describe.

## 2. Skills (CRITICAL)
Before replying, scan <available_skills> below. If a skill matches or is
even partially relevant to your task, you MUST load it with load_skill and
follow its instructions. Err on the side of loading.

## 3. Parallel Tool Calls
When you need multiple independent pieces of information, request them
together in a single response instead of one tool call per turn. Independent
reads, searches, and read-only commands should be batched.

## 4. Completion Criteria
The deliverable is a working artifact backed by real tool output — not a
description of one. Keep working until you have actually exercised the code
or produced the requested result. Don't stop at "I would do X" — actually do X.

## 5. Memory Management
Save durable, reusable knowledge — skip transient or trivial details.
</framework_instructions>"#;

const HELEN_CONVENTIONS: &str = r#"<helen_conventions>
You are generating code for Helen, a prompt-first Agent programming language.

## Core Principles
- Helen is agent-centric: design around `agent` blocks with `prompt`, `tools`, and `main`
- Use `llm act` for LLM interactions (with optional tool calling via `tools` declaration)
- Use `llm if` for LLM-routed branching (classification tasks)
- Prefer composition over inheritance: build small, focused agents that collaborate

## Skill-Driven Development
**CRITICAL**: Before writing ANY code (tests, main program, or utilities):
1. Scan the available skills below
2. If a skill matches your task, call `load_skill(name='skill-name')` FIRST
3. Follow the loaded skill's instructions precisely

## Code Generation Best Practices
- **Test-first**: Write tests before implementation when possible
- **Incremental**: Build and verify in small steps, not all at once
- **Error handling**: Use `try-catch` with specific exception types
</helen_conventions>"#;
