//! Interactive REPL — port of `cli/repl.py`.
//!
//! Multiline input detection (unbalanced braces/parens/brackets), persistent
//! interpreter state, `:help`-style commands, error printing. Mirrors the
//! Python REPL's behavior for the core loop; `:ask`/LLM-dependent commands
//! are stubbed (LLM milestone M6/M7 surface).

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use helen_semantic::SemanticAnalyzer;

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

/// REPL command handlers: `:help`, `:reset`, `:list`, `:undefine`.
/// Returns true if `line` was a command (consumed).
fn handle_repl_command(
    line: &str,
    interp: &mut Interpreter,
    analyzer: &mut SemanticAnalyzer,
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
            println!("  exit              Exit the REPL");
        }
        ":reset" => {
            analyzer.reset();
            interp.functions.clear();
            interp.agents.clear();
            println!("All definitions cleared.");
        }
        ":list" => {
            let mut fns: Vec<String> = interp.functions.keys().cloned().collect();
            fns.sort();
            let mut agents: Vec<String> = interp.agents.keys().cloned().collect();
            agents.sort();
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
                let removed_fn = interp.functions.remove(arg).is_some();
                let removed_agent = interp.agents.remove(arg).is_some();
                let removed_sym = analyzer.undefine(arg);
                if removed_fn || removed_agent || removed_sym {
                    println!("Removed '{arg}'.");
                } else {
                    println!("'{arg}' not found.");
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
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
            .as_str(),
    );

    let mut buffer_lines: Vec<String> = Vec::new();
    let mut empty_line_count = 0;

    loop {
        let prompt = if buffer_lines.is_empty() {
            ">>> "
        } else {
            "... "
        };

        // Read a line (interactive stdin).
        let mut line = String::new();
        use std::io::Write;
        print!("{prompt}");
        let _ = std::io::stdout().flush();
        let n = match std::io::stdin().read_line(&mut line) {
            Ok(0) => {
                // EOF
                println!();
                break;
            }
            Ok(_) => {
                // strip trailing newline
                while line.ends_with('\n') || line.ends_with('\r') {
                    line.pop();
                }
                line.len()
            }
            Err(_) => break,
        };
        if n == 0 {
            break;
        }

        if line.trim() == "exit" {
            break;
        }

        // REPL commands only at top level
        if buffer_lines.is_empty() && handle_repl_command(&line, &mut interp, &mut analyzer) {
            continue;
        }

        // Track empty lines
        if line.trim().is_empty() {
            empty_line_count += 1;
        } else {
            empty_line_count = 0;
        }

        // Two consecutive empty lines in multi-line mode force execution
        if !buffer_lines.is_empty() && empty_line_count >= 2 {
            let buffer = buffer_lines.join("\n");
            if !buffer.trim().is_empty() {
                let result = execute_input(&buffer, &mut interp, &mut analyzer);
                if result.success {
                    if let Some(r) = result.result {
                        println!("{r}");
                    }
                } else if let Some(e) = result.error {
                    eprintln!("Error: {e}");
                }
            }
            buffer_lines.clear();
            empty_line_count = 0;
            continue;
        }

        buffer_lines.push(line.clone());

        let buffer = buffer_lines.join("\n");
        if needs_continuation(&buffer) {
            continue;
        }

        if !buffer.trim().is_empty() {
            let result = execute_input(&buffer, &mut interp, &mut analyzer);
            if result.success {
                if let Some(r) = result.result {
                    println!("{r}");
                }
            } else if let Some(e) = result.error {
                eprintln!("Error: {e}");
            }
        }

        buffer_lines.clear();
        empty_line_count = 0;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_statement() {
        assert!(!needs_continuation("let x = 1;"));
        assert!(!needs_continuation("let x = {1, 2, 3};"));
    }

    #[test]
    fn test_unbalanced_brace() {
        assert!(needs_continuation("agent Test {"));
        assert!(needs_continuation("if (x > 0) {"));
    }

    #[test]
    fn test_unbalanced_paren() {
        assert!(needs_continuation("let x = func("));
    }

    #[test]
    fn test_unbalanced_bracket() {
        assert!(needs_continuation("let x = [1, 2,"));
    }

    #[test]
    fn test_string_with_brace() {
        assert!(!needs_continuation("let msg = \"hello {world}\";"));
    }

    #[test]
    fn test_nested_braces() {
        assert!(!needs_continuation("agent A { main { let x = 1; } }"));
        assert!(needs_continuation("agent A { main { let x = 1; }"));
    }

    #[test]
    fn test_execute_simple() {
        let mut interp = Interpreter::new();
        let mut analyzer = SemanticAnalyzer::new(helen_semantic::ErrorReporter::new(), ".");
        let r = execute_input("let x = 1", &mut interp, &mut analyzer);
        assert!(r.success, "{:?}", r.error);
    }

    #[test]
    fn test_execute_syntax_error() {
        let mut interp = Interpreter::new();
        let mut analyzer = SemanticAnalyzer::new(helen_semantic::ErrorReporter::new(), ".");
        let r = execute_input("agent {", &mut interp, &mut analyzer);
        assert!(!r.success);
        assert!(r.error.is_some());
    }

    #[test]
    fn test_execute_arithmetic() {
        let mut interp = Interpreter::new();
        let mut analyzer = SemanticAnalyzer::new(helen_semantic::ErrorReporter::new(), ".");
        let r = execute_input("let x = 1 + 2\nlet y = x * 3", &mut interp, &mut analyzer);
        assert!(r.success, "{:?}", r.error);
    }
}
