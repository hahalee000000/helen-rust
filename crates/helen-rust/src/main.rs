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

mod ask_assistant;
mod docgen;
mod formatter;
mod llm_adapter;
mod repl;
mod test;

use helen_core::ast_printer::{py_str_float, AstPrinter};
use helen_core::lexer::Scanner;
use helen_core::tokens::LiteralValue;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use helen_runtime::http_llm::HttpLLMRuntime;
use helen_rust::cli_utils::{
    normalize_stderr, print_help, render_uncaught, run_json, HELEN_VERSION,
    PYTHON_REFERENCE_VERSION,
};
use helen_rust::llm_adapter::HttpLlmAdapter;
use helen_semantic::{analyze_codes, analyze_messages};

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
        #[allow(clippy::arc_with_non_send_sync)]
        interp.set_llm_runtime(std::sync::Arc::new(mock));
    } else {
        // Set up real LLM runtime from config (so `llm act` works)
        let runtime = HttpLLMRuntime::new(None, None, None);
        if !runtime.api_key.is_empty() && runtime.api_key != "sk-placeholder" {
            let adapter = HttpLlmAdapter::new(runtime);
            #[allow(clippy::arc_with_non_send_sync)]
            interp.set_llm_runtime(std::sync::Arc::new(adapter));
        }
    }
    let result = interp.interpret(&program);
    let stdout = interp.stdout.lock().expect("mutex poisoned").clone();
    // This is the --json output mode, so we use run_json to format output.
    // Note: builtin_print also writes to actual stdout incrementally, but
    // for --json mode we need the captured buffer for the JSON structure.
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

// print_help is now imported from helen_rust::cli_utils

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

/// `helen agent` — launch the Helen Web UI (standalone Rust server).
/// Serves an embedded React frontend via Axum on port 8001.
/// Independent from the Python reference implementation's agent.
fn agent_command() -> i32 {
    use std::net::SocketAddr;

    println!("============================================================");
    println!("🚀 Helen Programming Assistant (Rust WebUI)");
    println!("============================================================");
    println!();

    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut port = 8001u16;
    let mut host = "127.0.0.1".to_string();
    let mut auth_token: Option<String> = None;

    let mut i = 2; // Skip "helen" and "agent"
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or_else(|_| {
                        eprintln!("❌ Invalid port number: {}", args[i + 1]);
                        8001
                    });
                    i += 2;
                } else {
                    eprintln!("❌ --port requires a value");
                    return 1;
                }
            }
            "--host" | "-h" => {
                if i + 1 < args.len() {
                    host = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("❌ --host requires a value");
                    return 1;
                }
            }
            "--auth" => {
                if i + 1 < args.len() {
                    auth_token = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("❌ --auth requires a token value");
                    return 1;
                }
            }
            "--help" => {
                println!("Usage: helen agent [OPTIONS]");
                println!();
                println!("Start the Helen programming assistant web UI.");
                println!();
                println!("Options:");
                println!("  -p, --port <PORT>    Port to listen on (default: 8001)");
                println!("  -h, --host <HOST>    Host to bind to (default: 127.0.0.1)");
                println!("      --auth <TOKEN>   Enable authentication with the given token");
                println!("      --help           Show this help message");
                return 0;
            }
            _ => {
                eprintln!("❌ Unknown option: {}", args[i]);
                return 1;
            }
        }
    }

    let addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap_or_else(|_| {
        eprintln!("❌ Invalid address: {}:{}", host, port);
        "127.0.0.1:8001".parse().unwrap()
    });

    println!("🌐 Starting web server on http://{}", addr);
    if auth_token.is_some() {
        println!("🔒 Authentication enabled");
    }
    println!();
    println!("Press Ctrl+C to stop the server.");
    println!();

    // Build and run the async runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        match helen_agent::server::start_server_with_auth(&addr.to_string(), auth_token).await {
            Ok(server) => {
                println!("✅ Server started successfully");
                println!("   Open http://{} in your browser", addr);
                println!();

                // Wait for shutdown signal
                tokio::signal::ctrl_c()
                    .await
                    .expect("Failed to listen for Ctrl+C");

                println!();
                println!("🛑 Shutting down...");
                server.shutdown().await;
                println!("✅ Server stopped");
                0
            }
            Err(e) => {
                eprintln!("❌ Failed to start server: {}", e);
                1
            }
        }
    })
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

    // Extract --transcript-log flag (sets HELEN_TRANSCRIPT_LOG env var)
    let mut filtered_argv = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        if argv[i] == "--transcript-log" && i + 1 < argv.len() {
            let path = std::path::Path::new(&argv[i + 1]);
            let abs_path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().unwrap_or_default().join(path)
            };
            std::env::set_var(
                "HELEN_TRANSCRIPT_LOG",
                abs_path.to_string_lossy().to_string(),
            );
            i += 2;
        } else if argv[i].starts_with("--transcript-log=") {
            let path_str = &argv[i]["--transcript-log=".len()..];
            let path = std::path::Path::new(path_str);
            let abs_path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().unwrap_or_default().join(path)
            };
            std::env::set_var(
                "HELEN_TRANSCRIPT_LOG",
                abs_path.to_string_lossy().to_string(),
            );
            i += 1;
        } else {
            filtered_argv.push(argv[i].clone());
            i += 1;
        }
    }
    let argv = filtered_argv;

    if argv.is_empty() {
        if preflight_config_check().is_err() {
            std::process::exit(1);
        }
        let (session_id, _) = helen_rust::cli_commands::extract_session_flags(&argv);
        let code = repl::repl_command(session_id.as_deref());
        std::process::exit(code);
    }

    let first = argv[0].as_str();
    match first {
        "-h" | "--help" | "help" => {
            print_help();
            std::process::exit(0);
        }
        "-V" | "--version" => {
            println!("Helen {HELEN_VERSION} (Rust, ported from Python Helen v{PYTHON_REFERENCE_VERSION})");
            std::process::exit(0);
        }
        "repl" => {
            if preflight_config_check().is_err() {
                std::process::exit(1);
            }
            let (session_id, _) = helen_rust::cli_commands::extract_session_flags(&argv[1..]);
            let code = repl::repl_command(session_id.as_deref());
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
        "init" => {
            let code = helen_rust::cli_commands::init_command();
            std::process::exit(code);
        }
        "quality" => {
            let code = helen_rust::cli_commands::quality_command(&argv[1..]);
            std::process::exit(code);
        }
        "watch" => {
            if argv.len() < 2 {
                eprintln!("Error: 'watch' requires a file argument");
                std::process::exit(1);
            }
            let code = helen_rust::cli_commands::watch_command(&argv[1]);
            std::process::exit(code);
        }
        "template" => {
            let code = helen_rust::cli_commands::template_command(&argv[1..]);
            std::process::exit(code);
        }
        "replay" => {
            let code = helen_rust::cli_commands::replay_command(&argv[1..]);
            std::process::exit(code);
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
            // Extract session flags before running
            let (session_id, _remaining_argv) =
                helen_rust::cli_commands::extract_session_flags(&argv[1..]);
            let code = helen_rust::cli_commands::run_command(first, session_id.as_deref());
            std::process::exit(code);
        }
    }
}
