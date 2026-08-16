//! helen-rust CLI — M1: `--lex` token-stream dump; M2: `--semantic-only`;
//! M3: `--run` execution (all for differential testing); M12: full CLI
//! (`helen <file>`, `check`, `test`, `repl`, `doc`, `coverage`, `provider`,
//! `--version`, `--help`).
//!
//! Differential modes emit the same JSON schemas as
//! `tests/conformance/reference.py`:
//! - `--lex`: array of `{type, lexeme, line, col, end_line, end_col, literal}`
//! - `--parse`: `{"ast": ...}`
//! - `--semantic-only`: `{"exit_code":N, "e_codes":[...]}`
//! - `--run`: `{"stdout", "stderr", "exit_code", "error_classes"}`
//!
//! Float literals are formatted with Rust's shortest-round-trip `Debug`
//! repr; the comparison script parses both sides numerically.

mod docgen;
mod formatter;
mod repl;
mod test;

use helen_core::ast_printer::{py_str_float, AstPrinter};
use helen_core::lexer::Scanner;
use helen_core::tokens::LiteralValue;
use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use helen_semantic::{analyze_codes, analyze_messages};

/// The Helen version string (matches Python `helen.__version__`).
pub const HELEN_VERSION: &str = "1.45.0";

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

fn run_mode(path: &str, mock_llm: bool) {
    #[cfg(feature = "python-ffi")]
    {
        // M10: install the Python FFI runtime (import hook + custom
        // provider loader). Best-effort: a missing Python embedding should
        // not prevent plain-Helen programs from running.
        let _ = helen_ffi::install();
    }
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
    interp.set_source_file(path);
    if mock_llm {
        // reference.py --mock-llm: MockLLMRuntime(act_return="MOCK_REPLY",
        // route_return="__mock__")
        let mock = helen_interpreter::llm_runtime::MockLlmRuntime::new(
            Some("__mock__".to_string()),
            Some(helen_interpreter::llm_runtime::LlmResponse {
                text: Some("MOCK_REPLY".to_string()),
                ..Default::default()
            }),
        );
        interp.set_llm_runtime(std::sync::Arc::new(mock));
    }
    let result = interp.interpret(&program);
    let stdout = interp.stdout.lock().unwrap().clone();
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

// ---------------------------------------------------------------------------
// M12: real CLI (run / check / test / repl / doc / coverage / provider)
// ---------------------------------------------------------------------------

/// `helen <file> [args...]` — run a program (port of `cli.__main__.run_command`).
/// Exit codes: 0 success, 1 file-not-found/syntax, 2 semantic, 3 runtime.
fn run_command(file: &str) -> i32 {
    let source_path = std::path::Path::new(file);
    if !source_path.exists() {
        eprintln!("Error: file not found: {file}");
        return 1;
    }
    let Ok(source_text) = std::fs::read_to_string(source_path) else {
        eprintln!("Error: cannot read {file}");
        return 1;
    };
    let source_lines: Vec<String> = source_text.lines().map(|s| s.to_string()).collect();

    // Lex
    let mut scanner = Scanner::new(&source_text, file);
    let tokens = scanner.scan_all();

    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let parse_errors = parser.errors();
    if !parse_errors.is_empty() {
        for e in parse_errors {
            let diag =
                helen_semantic::Diagnostic::new(e.code(), e.message().to_string(), Some(e.span()));
            eprintln!("{}", formatter::format_error(&diag, Some(&source_lines)));
        }
        return 1;
    }

    // Analyze
    let mut analyzer =
        helen_semantic::SemanticAnalyzer::new(helen_semantic::ErrorReporter::new(), ".");
    analyzer.analyze(&program);
    if analyzer.errors.has_errors() {
        for d in analyzer.errors.errors() {
            eprintln!("{}", formatter::format_error(d, Some(&source_lines)));
        }
        return 2;
    }

    // Interpret
    let mut interp = Interpreter::new();
    interp.set_source_file(file);
    let result = interp.interpret(&program);
    let stdout = interp.stdout.lock().unwrap().clone();
    print!("{stdout}");
    match result {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("RuntimeError: {}", e.to_display_string());
            3
        }
    }
}

/// `helen check <file>` — frontend validation only (port of `check_command`).
/// Exit codes: 0 clean, 1 file-not-found/syntax, 2 semantic.
fn check_command(file: &str) -> i32 {
    let source_path = std::path::Path::new(file);
    if !source_path.exists() {
        eprintln!("Error: file not found: {file}");
        return 1;
    }
    let Ok(source_text) = std::fs::read_to_string(source_path) else {
        eprintln!("Error: cannot read {file}");
        return 1;
    };
    let source_lines: Vec<String> = source_text.lines().map(|s| s.to_string()).collect();

    // Lex
    let mut scanner = Scanner::new(&source_text, file);
    let tokens = scanner.scan_all();

    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let parse_errors = parser.errors();
    if !parse_errors.is_empty() {
        for e in parse_errors {
            let diag =
                helen_semantic::Diagnostic::new(e.code(), e.message().to_string(), Some(e.span()));
            eprintln!("{}", formatter::format_error(&diag, Some(&source_lines)));
        }
        return 1;
    }

    // Analyze
    let mut analyzer =
        helen_semantic::SemanticAnalyzer::new(helen_semantic::ErrorReporter::new(), ".");
    analyzer.analyze(&program);
    if analyzer.errors.has_errors() {
        for d in analyzer.errors.errors() {
            eprintln!("{}", formatter::format_error(d, Some(&source_lines)));
        }
        return 2;
    }

    println!("✓ {file}: OK");
    0
}

/// `helen provider list` — list installed custom providers (port).
fn provider_command(argv: &[String]) -> i32 {
    if argv.is_empty() {
        println!("Usage: helen provider <subcommand>");
        println!();
        println!("Subcommands:");
        println!("  list    List installed custom providers");
        return 2;
    }
    match argv[0].as_str() {
        "list" => {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let providers_dir = std::path::Path::new(&home).join(".helen").join("providers");
            if !providers_dir.exists() {
                println!("No custom providers installed / 未安装自定义 Provider");
                return 0;
            }
            let mut adapters: Vec<_> = std::fs::read_dir(&providers_dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|x| x == "py").unwrap_or(false))
                        .collect()
                })
                .unwrap_or_default();
            adapters.sort_by_key(|e| e.file_name());
            if adapters.is_empty() {
                println!("No custom providers installed / 未安装自定义 Provider");
                return 0;
            }
            println!("Installed providers ({}):", adapters.len());
            for adapter in adapters {
                let name = adapter
                    .path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                println!("  • {name}  ({})", adapter.path().display());
            }
            0
        }
        other => {
            eprintln!("Unknown subcommand: {other}");
            eprintln!("Available: list");
            2
        }
    }
}

/// `helen coverage <file> [opts]` — run tests with a text coverage summary.
/// Minimal port: runs the test file and prints the report (detailed HTML/JSON
/// coverage tracking is a later milestone).
fn coverage_command(argv: &[String]) -> i32 {
    let files: Vec<String> = argv
        .iter()
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .collect();
    if files.is_empty() {
        eprintln!("Error: 'coverage' requires at least one file argument");
        eprintln!("Usage: helen coverage <test_file> [test_file2 ...] [--format text|json|html]");
        return 1;
    }
    let code = test::test_command(&files);
    if code != 0 {
        return code;
    }
    println!();
    println!("📊 Coverage:");
    println!("   Use 'helen coverage --html <dir>' for detailed coverage:");
    println!("   (HTML/JSON coverage reports are a later milestone)");
    0
}

/// `helen doc [files...] [opts]` — docgen CLI (port of `docgen.generate_cli`).
fn docgen_command(argv: &[String]) -> i32 {
    let mut files: Vec<String> = Vec::new();
    let mut fmt = "markdown".to_string();
    let mut with_builtins = false;
    let mut output: Option<String> = None;

    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--format" => {
                i += 1;
                if i < argv.len() {
                    fmt = argv[i].clone();
                }
            }
            "--with-builtins" => with_builtins = true,
            "-o" | "--output" => {
                i += 1;
                if i < argv.len() {
                    output = Some(argv[i].clone());
                }
            }
            a if a.starts_with('-') => {
                eprintln!("Unknown option: {a}");
                return 2;
            }
            f => files.push(f.to_string()),
        }
        i += 1;
    }

    let docs = docgen::generate_docs(&files, with_builtins);
    let out = if fmt == "json" {
        serde_json::to_string_pretty(&docs).unwrap_or_default()
    } else {
        docgen::format_markdown(&docs)
    };

    match output {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, &out) {
                eprintln!("Error: cannot write {path}: {e}");
                return 1;
            }
        }
        None => println!("{out}"),
    }
    0
}

fn print_help() {
    println!("helen {HELEN_VERSION} — Helen Agent Programming Language");
    println!();
    println!("Usage:");
    println!("  helen                          Interactive REPL (default)");
    println!("  helen <file> [args...]         Run a Helen program (args become `argv`)");
    println!("  helen check <file> [args...]   Check without executing");
    println!("  helen test <file> [opts]       Run Helen test file(s)");
    println!("  helen coverage <file> [opts]   Run tests with coverage measurement");
    println!("  helen doc [files]              Generate API documentation");
    println!("  helen provider <cmd> [opts]    Manage custom LLM provider adapters");
    println!("  helen agent                    Launch the Helen Web UI");
    println!("  helen repl                     Start the interactive REPL");
    println!("  helen --version                Show version number");
    println!();
    println!("Test Options:");
    println!("  --json                    Output results as JSON");
    println!("  --verbose                 Show detailed output");
    println!("  --only <name>             Run only the test with this exact name");
    println!("  --suite <name>            Run only tests in this suite");
    println!("  --filter <pattern>        Run only tests matching this pattern (regex)");
    println!();
    println!("Doc Options:");
    println!("  --format <markdown|json>  Output format (default: markdown)");
    println!("  --with-builtins           Include built-in functions");
    println!("  -o, --output <path>       Write output to file (default: stdout)");
}

/// CLI preflight config check (port of `_preflight_config_check`): exits 1
/// when not configured and stdin is not a TTY. The harness sets a dummy
/// `HELEN_API_KEY`, mirroring `tests/conftest.py`.
fn preflight_config_check() -> Result<(), i32> {
    if std::env::var("HELEN_API_KEY")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        return Ok(());
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config = std::path::Path::new(&home)
        .join(".helen")
        .join("config.yaml");
    if config.exists() {
        return Ok(());
    }
    // Not configured. Non-interactive → error + exit 1.
    eprintln!("❌ Error: Helen is not configured");
    eprintln!();
    eprintln!("Configuration required. Run one of:");
    eprintln!("  helen init              # Interactive setup");
    eprintln!("  helen                   # Start REPL (will prompt for config)");
    eprintln!();
    eprintln!("Or set environment variable: HELEN_API_KEY");
    Err(1)
}

/// `helen agent` — launch the Helen Web UI (delegates to Python start_webui.py).
/// Port of `helen/cli/agent_launcher.py::launch_agent`.
fn agent_command() -> i32 {
    use std::process::Command;

    println!("============================================================");
    println!("🚀 Helen Programming Assistant");
    println!("============================================================");
    println!();

    // 1. Check Node.js
    let node_check = Command::new("node")
        .arg("--version")
        .output();
    if node_check.is_err() || !node_check.as_ref().unwrap().status.success() {
        eprintln!("❌ Error: Node.js is not installed.");
        eprintln!();
        eprintln!("Helen agent requires Node.js 18+ for the frontend.");
        eprintln!();
        eprintln!("Install Node.js:");
        eprintln!("  https://nodejs.org/");
        eprintln!();
        eprintln!("Or using a version manager:");
        eprintln!("  nvm install 18");
        eprintln!("  nvm use 18");
        return 1;
    }

    // 2. Check Python dependencies
    let dep_check_script = r#"
import sys
missing = []
for mod, pkg in [("fastapi","fastapi"),("uvicorn","uvicorn"),("websockets","websockets"),("pydantic","pydantic"),("pydantic_settings","pydantic-settings"),("dotenv","python-dotenv"),("multipart","python-multipart")]:
    try:
        __import__(mod)
    except ImportError:
        missing.append(pkg)
if missing:
    print("MISSING:" + ",".join(missing))
    sys.exit(1)
print("OK")
"#;
    let dep_check = Command::new("python3")
        .args(["-c", dep_check_script])
        .output();
    match dep_check {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(missing_str) = stdout.strip_prefix("MISSING:") {
                let missing: Vec<&str> = missing_str.trim().split(',').collect();
                eprintln!("❌ Error: Helen agent requires additional Python packages.");
                eprintln!();
                eprintln!("Missing packages:");
                for pkg in &missing {
                    eprintln!("  - {pkg}");
                }
                eprintln!();
                eprintln!("Install with:");
                eprintln!("  pip install helen-lang[agent]");
                eprintln!();
                eprintln!("Or install individually:");
                eprintln!("  pip install {}", missing.join(" "));
                return 1;
            }
            eprintln!("❌ Error: Python dependency check failed.");
            return 1;
        }
        Err(_) => {
            eprintln!("❌ Error: Python 3 is not installed or not in PATH.");
            return 1;
        }
    }

    // 3. Find agent directory
    // Priority: HELEN_AGENT_DIR env var > relative to binary > relative to CWD
    let agent_dir = if let Ok(dir) = std::env::var("HELEN_AGENT_DIR") {
        std::path::PathBuf::from(dir)
    } else {
        // Try relative to the binary location
        let exe = std::env::current_exe().ok();
        let bin_dir = exe.as_ref().and_then(|p| p.parent().map(|p| p.to_path_buf()));
        let candidate = bin_dir
            .as_ref()
            .map(|d| d.join("../helen/agent"))
            .filter(|p| p.exists());
        if let Some(d) = candidate {
            d
        } else {
            // Fall back to CWD/helen/agent
            let cwd = std::env::current_dir().unwrap_or_default();
            cwd.join("helen/agent")
        }
    };

    if !agent_dir.exists() {
        eprintln!("❌ Error: agent directory not found");
        eprintln!("Expected location: {}", agent_dir.display());
        eprintln!();
        eprintln!("Set HELEN_AGENT_DIR to the agent directory, or run from the helen-rust repo root.");
        return 1;
    }

    // 4. Check node_modules
    let frontend_dir = agent_dir.join("webui/frontend");
    let node_modules = frontend_dir.join("node_modules");
    let vite_bin = if cfg!(windows) {
        node_modules.join(".bin/vite.cmd")
    } else {
        node_modules.join(".bin/vite")
    };
    if !vite_bin.exists() {
        eprintln!("⚠️  Frontend dependencies not found");
        eprintln!();
        eprintln!("Installing...");
        let npm_result = Command::new("npm")
            .arg("install")
            .current_dir(&frontend_dir)
            .status();
        match npm_result {
            Ok(status) if status.success() => {
                eprintln!("✅ Frontend dependencies installed");
                eprintln!();
            }
            _ => {
                eprintln!("❌ Failed to install frontend dependencies.");
                eprintln!();
                eprintln!("Please install manually:");
                eprintln!("  cd {}", frontend_dir.display());
                eprintln!("  npm install");
                return 1;
            }
        }
    }

    // 5. Launch start_webui.py
    let start_script = agent_dir.join("webui/start_webui.py");
    if !start_script.exists() {
        eprintln!("❌ Error: start_webui.py not found");
        eprintln!("Expected: {}", start_script.display());
        return 1;
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    eprintln!("✅ Starting Helen programming assistant...");
    eprintln!();

    let mut child = Command::new("python3")
        .arg(&start_script)
        .current_dir(&agent_dir)
        .env("HELEN_WEBUI_CWD", cwd.to_string_lossy().as_ref())
        .spawn();

    match child {
        Ok(ref mut c) => {
            // Wait for the child process
            match c.wait() {
                Ok(status) => status.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("❌ Error waiting for Web UI: {e}");
                    1
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to start Web UI: {e}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // ── Differential modes (kept for the M1–M3 diff harness) ──
    if args.len() >= 3 && args[1] == "--run" {
        let mut mock = false;
        let mut path = args[2].clone();
        if args.len() >= 4 && args[2] == "--mock-llm" {
            mock = true;
            path = args[3].clone();
        }
        run_mode(&path, mock);
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
                        serde_json::json!({"kind": "float", "value": py_str_float(*f)})
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

    // ── M12: real CLI ─────────────────────────────────────────────
    let argv: Vec<String> = args[1..].to_vec();

    if argv.is_empty() {
        if preflight_config_check().is_err() {
            std::process::exit(1);
        }
        let code = repl::repl_command();
        std::process::exit(code);
    }

    let first = argv[0].as_str();
    match first {
        "-h" | "--help" | "help" => {
            print_help();
            std::process::exit(0);
        }
        "-V" | "--version" => {
            println!("Helen {HELEN_VERSION}");
            std::process::exit(0);
        }
        "repl" => {
            if preflight_config_check().is_err() {
                std::process::exit(1);
            }
            let code = repl::repl_command();
            std::process::exit(code);
        }
        "check" => {
            if argv.len() < 2 {
                eprintln!("Error: 'check' requires a file argument");
                std::process::exit(1);
            }
            let code = check_command(&argv[1]);
            std::process::exit(code);
        }
        "test" => {
            if preflight_config_check().is_err() {
                std::process::exit(1);
            }
            let code = test::test_command(&argv[1..]);
            std::process::exit(code);
        }
        "coverage" => {
            let code = coverage_command(&argv[1..]);
            std::process::exit(code);
        }
        "doc" => {
            let code = docgen_command(&argv[1..]);
            std::process::exit(code);
        }
        "provider" => {
            let code = provider_command(&argv[1..]);
            std::process::exit(code);
        }
        "lsp" => {
            let mut server = helen_lsp::HelenLanguageServer::new();
            server.run();
            std::process::exit(0);
        }
        "agent" => {
            let code = agent_command();
            std::process::exit(code);
        }
        // Default: treat first argument as a file to run.
        _ => {
            if preflight_config_check().is_err() {
                std::process::exit(1);
            }
            let code = run_command(first);
            std::process::exit(code);
        }
    }
}
