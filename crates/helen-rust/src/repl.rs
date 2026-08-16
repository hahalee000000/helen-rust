//! Interactive REPL — port of `cli/repl.py`.
//!
//! Multiline input detection (unbalanced braces/parens/brackets), persistent
//! interpreter state, `:help`-style commands, error printing. Full `:ask`
//! integration via `ask_assistant` module (L1/L2/L3).

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use helen_runtime::transcript::{JsonlBackend, TranscriptStore};
use helen_semantic::SemanticAnalyzer;
use chrono;
use std::io::Write;

use crate::ask_assistant::{self, ReplState};

/// `_needs_continuation(buffer)` — unclosed braces/parens/brackets, with
/// string-literal awareness (braces inside strings don't count).
pub fn needs_continuation(buffer: &str) -> bool {
    let mut brace_count: i64 = 0;
    let mut paren_count: i64 = 0;
    let mut bracket_count: i64 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for ch in buffer.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if ch == '"' && !escape_next {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '{' => brace_count += 1,
            '}' => brace_count -= 1,
            '(' => paren_count += 1,
            ')' => paren_count -= 1,
            '[' => bracket_count += 1,
            ']' => bracket_count -= 1,
            _ => {}
        }
    }

    brace_count > 0 || paren_count > 0 || bracket_count > 0
}

/// Result of executing one REPL input.
pub struct ReplExecResult {
    pub success: bool,
    pub result: Option<String>, // repr of result value (Python `repr(result)`)
    pub error: Option<String>,
}

/// `_execute_input(source, interp, analyzer)` — lex → parse → analyze →
/// interpret, returning (success, result).
pub fn execute_input(
    source: &str,
    interp: &mut Interpreter,
    analyzer: &mut SemanticAnalyzer,
) -> ReplExecResult {
    // Lex
    let mut scanner = Scanner::new(source, "<repl>");
    let tokens = scanner.scan_all();

    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let parse_errors = parser.errors();
    if !parse_errors.is_empty() {
        let e_str = parse_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return ReplExecResult {
            success: false,
            result: None,
            error: Some(e_str),
        };
    }

    // Analyze (semantic checks)
    analyzer.analyze(&program);
    if analyzer.errors.has_errors() {
        let msgs: Vec<String> = analyzer
            .errors
            .errors()
            .iter()
            .map(|d| d.to_string())
            .collect();
        return ReplExecResult {
            success: false,
            result: None,
            error: Some(msgs.join("\n")),
        };
    }

    // Interpret
    match interp.interpret(&program) {
        Ok(result) => ReplExecResult {
            success: true,
            result: result.map(|v| v.python_repr()),
            error: None,
        },
        Err(e) => ReplExecResult {
            success: false,
            result: None,
            error: Some(format!("RuntimeError: {}", e.to_display_string())),
        },
    }
}

/// REPL command handlers. Returns true if `line` was a command (consumed).
fn handle_repl_command(
    line: &str,
    interp: &mut Interpreter,
    analyzer: &mut SemanticAnalyzer,
    repl_state: &mut ReplState,
) -> bool {
    let stripped = line.trim();
    if !stripped.starts_with(':') {
        return false;
    }

    let parts: Vec<&str> = stripped.splitn(2, char::is_whitespace).collect();
    let cmd = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd.as_str() {
        ":help" => {
            println!("REPL commands:");
            println!("  :help             Show this help message");
            println!("  :reset            Clear all definitions (functions, agents)");
            println!("  :list             List all defined functions and agents");
            println!("  :undefine <name>  Remove a function or agent definition");
            println!("  :ask <question>   Ask the Helen assistant (single question)");
            println!("  :ask              Enter multi-turn :ask chat mode");
            println!("  :ask --list       List recent :ask chat sessions");
            println!("  :ask --resume <sid>  Resume a previous :ask chat session");
            println!("  :trace on|off     Enable/disable execution tracing");
            println!("  :trace show [n]   Show last n trace entries (default 50)");
            println!("  :last_error [-v]  Show last error (verbose with -v)");
            println!("  :llm_log [n] [-v] Show last n LLM calls (verbose with -v)");
            println!("  :stats            Show context window usage statistics");
            println!("  :transcript       Show current transcript");
            println!("  :transcript --full  Show full transcript including compressed");
            println!("  :transcript --audit Show compression audit trail");
            println!("  :sessions         List transcript sessions");
            println!("  :session_id       Show current session ID");
            println!("  :resume <id>      Resume a previous transcript session");
            println!("  exit              Exit the REPL");
        }
        ":reset" => {
            analyzer.reset();
            interp.reset_definitions();
            repl_state.clear();
            println!("All definitions cleared.");
        }
        ":list" => {
            let defs = interp.list_definitions();
            let fns = defs.get("functions").cloned().unwrap_or_default();
            let agents = defs.get("agents").cloned().unwrap_or_default();
            if fns.is_empty() {
                println!("Functions: (none)");
            } else {
                println!("Functions: {}", fns.join(", "));
            }
            if agents.is_empty() {
                println!("Agents:    (none)");
            } else {
                println!("Agents:    {}", agents.join(", "));
            }
        }
        ":undefine" => {
            if arg.is_empty() {
                println!("Usage: :undefine <name>");
            } else {
                let removed_fn = interp.undefine_function(arg);
                let removed_agent = interp.undefine_agent(arg);
                let removed_sym = analyzer.undefine(arg);
                if removed_fn || removed_agent || removed_sym {
                    println!("Removed '{arg}'.");
                } else {
                    println!("'{arg}' not found.");
                }
            }
        }

        // ── :ask — Helen Assistant (L1/L2/L3) ──────────────────────
        ":ask" => {
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());

            // :ask --list — show recent assistant sessions
            if arg == "--list" {
                let sessions = ask_assistant::list_assistant_sessions();
                if sessions.is_empty() {
                    println!("(no sessions found)");
                } else {
                    println!("{:<40} {:<22} {:>8}", "session_id", "created", "messages");
                    for s in &sessions {
                        let created = chrono::DateTime::from_timestamp(s.created_at as i64, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        println!("{:<40} {:<22} {:>8}", s.session_id, created, s.message_count);
                    }
                }
                return true;
            }

            // :ask --resume <sid> — enter chat mode resuming a session
            if arg.starts_with("--resume") {
                let resume_parts: Vec<&str> = arg.splitn(2, char::is_whitespace).collect();
                if resume_parts.len() < 2 || resume_parts[1].trim().is_empty() {
                    println!("Usage: :ask --resume <session_id>");
                    return true;
                }
                let sid = resume_parts[1].trim();
                ask_assistant::run_chat_mode(sid, &cwd);
                return true;
            }

            // :ask (no question) — enter multi-turn chat mode
            if arg.is_empty() {
                let sid = if interp.session_id.is_empty() {
                    format!("ask-{}", chrono::Utc::now().timestamp())
                } else {
                    interp.session_id.clone()
                };
                ask_assistant::run_chat_mode(&sid, &cwd);
                return true;
            }

            // :ask <question> — single-turn question
            ask_assistant::ask_single(arg, interp, repl_state, &cwd);
        }

        // ── :trace — execution tracing ──────────────────────────────
        ":trace" => {
            if arg == "on" {
                interp.observability.tracer.enabled = true;
                interp.observability.call_stack.enabled = true;
                println!("Execution tracing enabled.");
            } else if arg == "off" {
                interp.observability.tracer.enabled = false;
                interp.observability.call_stack.enabled = false;
                println!("Execution tracing disabled.");
            } else if arg.starts_with("show") {
                let n = if arg.len() > 5 {
                    arg[5..].trim().parse::<usize>().unwrap_or(50)
                } else {
                    50
                };
                println!("{}", interp.observability.tracer.format_trace(n));
            } else {
                println!("Usage: :trace on|off|show [n]");
            }
        }

        // ── :last_error — persistent error snapshot ─────────────────
        ":last_error" => {
            let verbose = arg.contains("-v") || arg.contains("--verbose");
            if let Some(ref snap) = interp.observability.last_error {
                println!("{}", snap.format_text(verbose));
                if !verbose {
                    println!("\nTip: use :last_error -v to show execution trace");
                }
            } else {
                println!("No error captured yet.");
            }
        }

        // ── :llm_log — LLM call audit ──────────────────────────────
        ":llm_log" => {
            let mut n = 10;
            let mut verbose = false;
            for part in arg.split_whitespace() {
                if part == "-v" || part == "--verbose" {
                    verbose = true;
                } else if let Ok(num) = part.parse::<usize>() {
                    n = num;
                }
            }
            let entries = interp.observability.llm_audit.entries();
            if entries.is_empty() {
                println!("No LLM calls recorded yet.");
            } else {
                let start = if entries.len() > n { entries.len() - n } else { 0 };
                println!("Last {} LLM calls:", entries.len() - start);
                for (i, entry) in entries[start..].iter().enumerate() {
                    let status = if entry.error.is_some() { "❌" } else { "✅" };
                    if verbose {
                        let ts = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        println!("\n  [{}] {} {}", i + 1, status, ts);
                        println!("      Type: {}", entry.call_type);
                        println!("      Agent: {}", entry.agent_name.as_deref().unwrap_or("anonymous"));
                        println!("      Model: {}", entry.model.as_deref().unwrap_or("default"));
                        let prompt_preview = entry.prompt.chars().take(100).collect::<String>();
                        let prompt_suffix = if entry.prompt.chars().count() > 100 { "..." } else { "" };
                        println!("      Prompt: {}{}", prompt_preview, prompt_suffix);
                        if let Some(ref resp) = entry.response {
                            let resp_preview = resp.chars().take(100).collect::<String>();
                            let resp_suffix = if resp.chars().count() > 100 { "..." } else { "" };
                            println!("      Response: {}{}", resp_preview, resp_suffix);
                        }
                        println!("      Tokens: {} in / {} out", entry.tokens_in, entry.tokens_out);
                        println!("      Duration: {:.0}ms", entry.duration_ms);
                        if !entry.tool_calls.is_empty() {
                            println!("      Tool calls: {}", entry.tool_calls.len());
                            for tc in entry.tool_calls.iter().take(3) {
                                if let Some(name) = tc.get("name").and_then(|v| v.as_str()) {
                                    println!("        - {}", name);
                                }
                            }
                        }
                        if let Some(ref err) = entry.error {
                            println!("      Error: {}", err);
                        }
                    } else {
                        let model_str = entry.model.as_ref().map(|m| format!(" @{}", m)).unwrap_or_default();
                        println!("  {} [{}] {}{} ({}+{} tokens, {:.0}ms)",
                            status, entry.call_type,
                            entry.agent_name.as_deref().unwrap_or("anonymous"),
                            model_str, entry.tokens_in, entry.tokens_out, entry.duration_ms);
                        if !entry.tool_calls.is_empty() {
                            println!("      🔧 {} tool call(s)", entry.tool_calls.len());
                        }
                        if let Some(ref err) = entry.error {
                            println!("      ❗ {}", err);
                        }
                    }
                }
            }
        }

        // ── :stats — context usage statistics ───────────────────────
        ":stats" => {
            println!("{}", interp.format_context_stats());
        }

        // ── :sessions — list transcript sessions ────────────────────
        ":sessions" => {
            let sessions = interp.session_manager.lock().unwrap().list_sessions();
            if sessions.is_empty() {
                println!("(no sessions found)");
            } else {
                println!("{:<40} {:<22} {:>8}", "session_id", "created", "messages");
                for s in sessions {
                    let created = chrono::DateTime::from_timestamp(s.created_at as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    println!("{:<40} {:<22} {:>8}", s.session_id, created, s.message_count);
                }
            }
        }

        // ── :session_id — show current session ID ───────────────────
        ":session_id" => {
            if interp.session_id.is_empty() {
                println!("(no active session)");
            } else {
                println!("{}", interp.session_id);
            }
        }

        // ── :transcript — show current transcript ───────────────────
        ":transcript" => {
            handle_transcript(interp, arg);
        }

        // ── :resume — resume a previous transcript session ──────────
        ":resume" => {
            if arg.is_empty() {
                println!("Usage: :resume <session_id>");
            } else {
                let session_id = arg.trim();
                let manager = interp.session_manager.lock().unwrap();
                if manager.session_exists(session_id) {
                    // Load the session's transcript
                    let session_path = manager.get_session_path(session_id);
                    drop(manager);
                    match load_and_resume_transcript(interp, &session_path, session_id) {
                        Ok(count) => {
                            println!("Resumed session: {} ({} messages loaded)", session_id, count);
                        }
                        Err(e) => {
                            println!("Error resuming session: {}", e);
                        }
                    }
                } else {
                    println!("Session not found: {}", session_id);
                }
            }
        }

        _ => {
            println!("Unknown command: {cmd}");
            println!("Type ':help' for available commands.");
        }
    }
    true
}

/// Handle `:transcript` command with optional flags.
fn handle_transcript(interp: &mut Interpreter, arg: &str) {
    if interp.session_id.is_empty() {
        println!("TranscriptStore is not enabled.");
        return;
    }

    // Try to load the transcript for the current session
    let manager = interp.session_manager.lock().unwrap();
    let session_path = manager.get_session_path(&interp.session_id);
    drop(manager);

    if !session_path.exists() {
        println!("TranscriptStore is not enabled.");
        return;
    }

    let backend = JsonlBackend::new(&session_path);
    let mut store = TranscriptStore::load_from_backend(backend, 1000);

    if arg.contains("--audit") {
        // Show compression audit trail
        let audit = store.get_compression_audit();
        if audit.is_empty() {
            println!("No compression events recorded.");
        } else {
            println!("Compression audit trail ({} events):", audit.len());
            for (i, entry) in audit.iter().enumerate() {
                let ts = entry.get("timestamp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let before = entry.get("items_before").and_then(|v| v.as_u64()).unwrap_or(0);
                let after = entry.get("items_after").and_then(|v| v.as_u64()).unwrap_or(0);
                let strategy = entry.get("strategy").and_then(|v| v.as_str()).unwrap_or("unknown");
                let ts_str = chrono::DateTime::from_timestamp(ts as i64, 0)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| "??:??:??".to_string());
                println!("  [{}] {} — {} → {} items ({})", i + 1, ts_str, before, after, strategy);
            }
        }
    } else if arg.contains("--full") {
        // Show full transcript including compressed items
        let items = &store.transcript;
        if items.is_empty() {
            println!("(transcript is empty)");
        } else {
            println!("Full transcript ({} items):", items.len());
            for item in items.iter() {
                match item {
                    helen_runtime::transcript::Item::Message(msg) => {
                        let role = &msg.role;
                        let (text, _) = helen_runtime::transcript::message_text_parts(&msg.content);
                        let preview: String = text.chars().take(120).collect();
                        let suffix = if text.chars().count() > 120 { "..." } else { "" };
                        println!("  [{}] {}{}", role, preview, suffix);
                    }
                    helen_runtime::transcript::Item::Boundary(b) => {
                        println!("  ── boundary: {} ──", b.summary);
                    }
                }
            }
        }
    } else {
        // Default: show active transcript (read_view)
        let messages = store.read_view();
        if messages.is_empty() {
            println!("(transcript is empty)");
        } else {
            println!("Transcript for session: {} ({} messages)", interp.session_id, messages.len());
            for msg in messages.iter().take(50) {
                let role = &msg.role;
                let (text, _) = helen_runtime::transcript::message_text_parts(&msg.content);
                let preview: String = text.chars().take(120).collect();
                let suffix = if text.chars().count() > 120 { "..." } else { "" };
                println!("  [{}] {}{}", role, preview, suffix);
            }
            if messages.len() > 50 {
                println!("  ... and {} more messages", messages.len() - 50);
            }
        }
    }
}

/// Load a transcript from a session path and replay it into the interpreter.
fn load_and_resume_transcript(
    interp: &mut Interpreter,
    session_path: &std::path::Path,
    session_id: &str,
) -> Result<usize, String> {
    if !session_path.exists() {
        return Err(format!("session path does not exist: {:?}", session_path));
    }
    let backend = JsonlBackend::new(session_path);
    let mut store = TranscriptStore::load_from_backend(backend, 1000);
    let messages = store.read_view();
    let count = messages.len();

    // Set the interpreter's session_id to the resumed session
    interp.session_id = session_id.to_string();

    // Note: full transcript replay into the interpreter's LLM history
    // would require integration with the interpreter's agent_context.
    // For now, we set the session_id so subsequent operations use it.
    Ok(count)
}

/// `repl_command()` — the interactive loop. Returns process exit code.
pub fn repl_command() -> i32 {
    println!("Helen REPL v1.2");
    println!("Type 'exit' or Ctrl+D to quit, ':help' for commands");
    println!(
        "In multi-line mode (...), press Enter twice on empty line to execute, or Ctrl+C to cancel"
    );
    println!();

    let mut interp = Interpreter::new();
    let mut analyzer = SemanticAnalyzer::new(
        helen_semantic::ErrorReporter::new(),
        &std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()),
    );
    let mut repl_state = ReplState::new();
    let mut buffer = String::new();

    loop {
        // Prompt
        let prompt = if buffer.is_empty() { ">>> " } else { "... " };
        print!("{}", prompt);
        if std::io::stdout().flush().is_err() {
            break;
        }

        // Read line
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => {
                // EOF
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }

        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');

        // Exit
        if buffer.is_empty() && trimmed == "exit" {
            break;
        }

        // REPL commands only at top-level (no buffer)
        if buffer.is_empty() && handle_repl_command(trimmed, &mut interp, &mut analyzer, &mut repl_state) {
            continue;
        }

        // Accumulate buffer
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(trimmed);

        // Multi-line detection
        if needs_continuation(&buffer) {
            continue;
        }

        // Empty input in continuation mode → execute
        if trimmed.is_empty() && !buffer.is_empty() {
            // fall through to execute
        } else if trimmed.is_empty() {
            buffer.clear();
            continue;
        }

        // Execute
        let source = buffer.clone();
        let result = execute_input(&source, &mut interp, &mut analyzer);

        // Record into ReplState for :ask context
        if result.success {
            if let Some(ref val) = result.result {
                if !val.is_empty() && val != "None" {
                    repl_state.record_output(val);
                    println!("{}", val);
                }
            }
        } else {
            if let Some(ref err) = result.error {
                eprintln!("{}", err);
                repl_state.record_error(err);
            }
        }

        buffer.clear();
    }

    0
}
