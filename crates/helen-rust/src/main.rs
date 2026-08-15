//! helen-rust CLI — M1: `--lex` token-stream dump; M2: `--semantic-only`;
//! M3: `--run` execution — all for differential testing.
//!
//! Emits the same JSON schemas as `tests/conformance/reference.py`:
//! - `--lex`: array of `{type, lexeme, line, col, end_line, end_col, literal}`
//! - `--parse`: `{"ast": ...}`
//! - `--semantic-only`: `{"exit_code":N, "e_codes":[...]}`
//! - `--run`: `{"stdout", "stderr", "exit_code", "error_classes"}`
//!
//! Float literals are formatted with Rust's shortest-round-trip `Debug`
//! repr; the comparison script parses both sides numerically.

use helen_core::ast_printer::AstPrinter;
use helen_core::lexer::Scanner;
use helen_core::tokens::LiteralValue;
use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use helen_semantic::{analyze_codes, analyze_messages};

/// Python `reference.py` strips " at <path>:<line>:<col>-<col>" suffixes via
/// `_SPAN_RE = re.compile(r" at \S+:\d+:\d+-\d+")`. Mirror that regex exactly
/// so stderr compares equal.
fn normalize_stderr(s: &str) -> String {
    // Port of Python's `re.sub(r" at \S+:\d+:\d+-\d+", "", s)`. The greedy
    // `\S+` backtracks so the match ends at the LAST `:\d+:\d+-\d+` within
    // the non-space run (the run may continue with e.g. ':' after the span).
    let mut out = String::new();
    let mut rest = s;
    while let Some(idx) = rest.find(" at ") {
        let after = &rest[idx + 4..];
        // Find the end of the \S+ run.
        let ws = after.find(char::is_whitespace).unwrap_or(after.len());
        let nonspace = &after[..ws];
        // Find the end of the last `:\d+:\d+-\d+` within the run.
        let span_end = last_span_end(nonspace.as_bytes());
        match span_end {
            Some(end) => {
                out.push_str(&rest[..idx]);
                // Consume " at " + the span (up to `end` within `after`),
                // keeping the remainder of the run AND the message tail.
                rest = &after[end..];
            }
            None => {
                out.push_str(&rest[..idx + 4]);
                rest = &rest[idx + 4..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Byte index after the LAST `:\d+:\d+-\d+` in `b`, if any (Python's greedy
/// `\S+` leaves the rightmost span pattern as the match tail).
fn last_span_end(b: &[u8]) -> Option<usize> {
    let n = b.len();
    // Scan for each `:` then try to match `\d+:\d+-\d+`; keep the rightmost.
    let mut best: Option<usize> = None;
    let mut i = 0;
    while i + 2 < n {
        if b[i] == b':' {
            // try `:\d+:\d+-\d+` starting at i
            let mut j = i + 1;
            while j < n && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < n && b[j] == b':' {
                let mut k = j + 1;
                while k < n && b[k].is_ascii_digit() {
                    k += 1;
                }
                if k > j + 1 && k < n && b[k] == b'-' {
                    let mut m = k + 1;
                    while m < n && b[m].is_ascii_digit() {
                        m += 1;
                    }
                    if m > k + 1 {
                        best = Some(m);
                        i = m;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    best
}

/// Render an uncaught exception the way Python's CLI does: `RuntimeError: {e}`.
fn render_uncaught(e: &ExceptionValue) -> String {
    format!("RuntimeError: {}\n", e.to_display_string())
}

/// Emit the run-result JSON in reference.py's exact layout:
/// `{"stdout": "...", "stderr": "...", "exit_code": N, "error_classes": [...]}`
/// (spaces after separators, stdout/stderr/exit_code/error_classes order).
fn run_json(stdout: &str, stderr: &str, exit_code: i64, error_classes: &[String]) -> String {
    let classes = error_classes
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{{\"stdout\": {stdout}, \"stderr\": {stderr}, \"exit_code\": {exit_code}, \"error_classes\": [{classes}]}}",
        stdout = serde_json::to_string(stdout).unwrap(),
        stderr = serde_json::to_string(stderr).unwrap(),
        exit_code = exit_code,
        classes = classes,
    )
}

fn run_mode(path: &str) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("helen: cannot read {path}: {e}");
            std::process::exit(2);
        }
    };

    // Lex — Python's scan_all collects errors without raising (reference.py's
    // E300 branch is effectively dead code), so proceed to parse regardless.
    let mut scanner = Scanner::new(&source, path);
    let tokens = scanner.scan_all();

    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let parse_errors = parser.errors();
    if !parse_errors.is_empty() {
        let e_str: Vec<String> = parse_errors.iter().map(|e| e.to_string()).collect();
        println!(
            "{}",
            run_json("", &normalize_stderr(&e_str.join("\n")), 1, &[])
        );
        return;
    }

    // Analyze (exit 2 on semantic errors) — full `E{code}: message` strings,
    // span-normalized like the reference CLI.
    let messages = analyze_messages(&program);
    if !messages.is_empty() {
        let e_str = messages.join("\n");
        println!("{}", run_json("", &normalize_stderr(&e_str), 2, &[]));
        return;
    }

    // Interpret
    let mut interp = Interpreter::new();
    let result = interp.interpret(&program);
    let stdout = interp.stdout.borrow().clone();
    match result {
        Ok(_) => {
            println!("{}", run_json(&stdout, "", 0, &[]));
        }
        Err(e) => {
            println!(
                "{}",
                run_json(
                    &stdout,
                    &normalize_stderr(&render_uncaught(&e)),
                    3,
                    std::slice::from_ref(&e.class_name)
                )
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 3 && args[1] == "--run" {
        run_mode(&args[2]);
        return;
    }

    if args.len() >= 3 && args[1] == "--semantic-only" {
        let path = &args[2];
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("helen: cannot read {path}: {e}");
                std::process::exit(2);
            }
        };

        let mut scanner = Scanner::new(&source, path);
        let tokens = scanner.scan_all();
        let mut parser = Parser::new(tokens);
        let program = parser.parse();

        let codes = analyze_codes(&program);
        // serde_json::json! sorts object keys (BTreeMap); emit manually to
        // match reference.py's exact byte layout: {"exit_code":N,"e_codes":[...]}
        let codes_json = codes
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{{\"exit_code\":{},\"e_codes\":[{}]}}",
            if codes.is_empty() { 0 } else { 2 },
            codes_json
        );
        return;
    }

    if args.len() >= 3 && args[1] == "--parse" {
        let path = &args[2];
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("helen: cannot read {path}: {e}");
                std::process::exit(2);
            }
        };

        let mut scanner = Scanner::new(&source, path);
        let tokens = scanner.scan_all();
        let mut parser = Parser::new(tokens);
        let program = parser.parse();
        let printer = AstPrinter::new();
        let out = printer.print_program(&program);
        println!("{}", serde_json::json!({ "ast": out }));
        return;
    }

    if args.len() >= 3 && args[1] == "--lex" {
        let path = &args[2];
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("helen: cannot read {path}: {e}");
                std::process::exit(2);
            }
        };

        let mut scanner = Scanner::new(&source, path);
        let tokens = scanner.scan_all();

        let out: Vec<serde_json::Value> = tokens
            .iter()
            .map(|t| {
                let literal = match &t.literal {
                    LiteralValue::Null => serde_json::json!({"kind": "null"}),
                    LiteralValue::Bool(b) => serde_json::json!({"kind": "bool", "value": b}),
                    LiteralValue::Str(s) => serde_json::json!({"kind": "str", "value": s}),
                    LiteralValue::Int(i) => {
                        serde_json::json!({"kind": "int", "value": i.to_string()})
                    }
                    LiteralValue::Float(f) => {
                        serde_json::json!({"kind": "float", "value": format!("{f:?}")})
                    }
                };
                serde_json::json!({
                    "type": t.kind.name(),
                    "lexeme": t.lexeme,
                    "line": t.line,
                    "col": t.col,
                    "end_line": t.end_line,
                    "end_col": t.end_col,
                    "literal": literal,
                })
            })
            .collect();

        println!("{}", serde_json::to_string(&out).unwrap());
        return;
    }

    eprintln!("helen: usage: helen --lex <file.helen>  (M1 lexer differential mode)");
    std::process::exit(2);
}
