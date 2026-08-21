//! Additional CLI subcommands — port of `helen/cli/__main__.py`:
//! `init`, `quality`, `watch`, `template`, `replay`, plus session-flag
//! extraction (`--session`/`--resume-latest`).

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// `helen init`
// ---------------------------------------------------------------------------

/// `helen init` — initialize the Helen configuration directory.
/// Port of `init_command`: creates `~/.helen/` with `config.yaml` (via a
/// minimal non-interactive wizard) and `skills/`.
pub fn init_command() -> i32 {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let helen_home = PathBuf::from(&home).join(".helen");
    if let Err(e) = std::fs::create_dir_all(&helen_home) {
        eprintln!("Error: cannot create {}: {e}", helen_home.display());
        return 1;
    }
    println!("Helen home: {}", helen_home.display());

    let skills_dir = helen_home.join("skills");
    let _ = std::fs::create_dir_all(&skills_dir);
    println!("Skills directory: {}", skills_dir.display());

    let config_path = helen_home.join("config.yaml");
    if config_path.exists() {
        println!(
            "\n✅ Helen is already configured: {}",
            config_path.display()
        );
        println!("Edit it directly to update settings.");
        return 0;
    }

    // Minimal wizard: read provider base URL / api key from stdin.
    println!("\nHelen setup wizard");
    println!("------------------");
    println!("Configure your LLM provider (leave blank to use HELEN_API_KEY env):");

    let mut provider = String::new();
    print!("Provider base URL (e.g. https://api.openai.com/v1): ");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    if std::io::stdin().read_line(&mut provider).is_ok() {
        provider = provider.trim().to_string();
    }

    let mut model = String::new();
    print!("Default model (e.g. gpt-4o): ");
    let _ = std::io::stdout().flush();
    if std::io::stdin().read_line(&mut model).is_ok() {
        model = model.trim().to_string();
    }

    let mut key = String::new();
    print!("API key (leave blank for HELEN_API_KEY env): ");
    let _ = std::io::stdout().flush();
    if std::io::stdin().read_line(&mut key).is_ok() {
        key = key.trim().to_string();
    }

    let mut lines = String::new();
    lines.push_str("# Helen configuration\n");
    lines.push_str("provider:\n");
    if !provider.is_empty() {
        lines.push_str(&format!("  base_url: \"{provider}\"\n"));
    }
    if !model.is_empty() {
        lines.push_str(&format!("  model: \"{model}\"\n"));
    }
    if !key.is_empty() {
        lines.push_str(&format!("  api_key: \"{key}\"\n"));
    }
    if let Err(e) = std::fs::write(&config_path, lines) {
        eprintln!("Error: cannot write {}: {e}", config_path.display());
        return 1;
    }
    println!("\n✅ Configuration written to {}", config_path.display());
    println!("Skills directory: {}", skills_dir.display());
    0
}

// ---------------------------------------------------------------------------
// `helen quality`
// ---------------------------------------------------------------------------

/// `helen quality <file> [file2 ...]` — 7-dimension quality assessment.
/// Port of `quality_command` (uses the interpreter `quality` module which
/// mirrors `helen/stdlib/quality.py`).
pub fn quality_command(argv: &[String]) -> i32 {
    let mut files: Vec<String> = Vec::new();
    let mut json_output = false;
    let mut dimension: Option<String> = None;
    let mut threshold: f64 = 0.0;

    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--json" => json_output = true,
            "--dimension" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!("Error: --dimension requires a name argument");
                    return 2;
                }
                let dim = argv[i].clone();
                let valid = [
                    "architecture",
                    "code_quality",
                    "security",
                    "test_coverage",
                    "documentation",
                    "maintainability",
                    "engineering",
                ];
                if !valid.contains(&dim.as_str()) {
                    eprintln!("Error: invalid dimension '{dim}'");
                    eprintln!("Valid dimensions: {}", valid.join(", "));
                    return 2;
                }
                dimension = Some(dim);
            }
            "--threshold" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!("Error: --threshold requires a number argument");
                    return 2;
                }
                match argv[i].parse::<f64>() {
                    Ok(t) => threshold = t,
                    Err(_) => {
                        eprintln!("Error: invalid threshold '{}'", argv[i]);
                        return 2;
                    }
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

    if files.is_empty() {
        eprintln!("Error: 'quality' requires at least one file argument");
        eprintln!("Usage: helen quality <file> [file2 ...] [--json] [--dimension <name>] [--threshold <n>]");
        return 1;
    }

    let mut all_results: Vec<serde_json::Value> = Vec::new();
    let mut min_score = 10.0f64;

    for file in &files {
        let source_path = Path::new(file);
        if !source_path.exists() {
            eprintln!("Error: file not found: {file}");
            return 1;
        }
        let Ok(source_text) = std::fs::read_to_string(source_path) else {
            eprintln!("Error: cannot read {file}");
            return 1;
        };

        // Use the interpreter's quality module (mirrors helen/stdlib/quality.py).
        let mut interp = Interpreter::new();
        let source_val =
            helen_interpreter::value::Value::Str(std::rc::Rc::from(source_text.as_str()));
        let name_val = helen_interpreter::value::Value::Str(std::rc::Rc::from(file.as_str()));

        let metrics_val = match helen_interpreter::quality::quality_analyze_code(
            &mut interp,
            &[source_val.clone(), name_val.clone()],
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error analyzing {file}: {}", e.message);
                return 2;
            }
        };
        let security_val = match helen_interpreter::quality::quality_check_security(
            &mut interp,
            std::slice::from_ref(&source_val),
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error checking security for {file}: {}", e.message);
                return 2;
            }
        };
        let score_val = match helen_interpreter::quality::quality_quality_score(
            &mut interp,
            std::slice::from_ref(&source_val),
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error scoring {file}: {}", e.message);
                return 2;
            }
        };
        let dim_scores_val = match helen_interpreter::quality::quality_dimension_scores(
            &mut interp,
            std::slice::from_ref(&source_val),
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error computing dimension scores for {file}: {}", e.message);
                return 2;
            }
        };
        let report_val = helen_interpreter::quality::quality_quality_report(
            &mut interp,
            &[source_val, name_val],
        );

        let total_score = match score_val {
            helen_interpreter::value::Value::Float(f) => f,
            helen_interpreter::value::Value::Int(b) => b.to_string().parse::<f64>().unwrap_or(0.0),
            _ => 0.0,
        };
        if total_score < min_score {
            min_score = total_score;
        }

        if json_output {
            let metrics = value_to_json(&metrics_val);
            let security = value_to_json(&security_val);
            let grade = if total_score >= 9.0 {
                "A"
            } else if total_score >= 8.0 {
                "B"
            } else if total_score >= 7.0 {
                "C"
            } else if total_score >= 6.0 {
                "D"
            } else {
                "F"
            };

            let mut scores = serde_json::json!({
                "total": total_score,
                "grade": grade,
            });
            if let Some(dim) = &dimension {
                // Extract the specific dimension score from the dimension_scores map
                let dim_score = match &dim_scores_val {
                    helen_interpreter::value::Value::Map(m) => {
                        let map = m.borrow();
                        map.get(&helen_interpreter::value::Value::Str(std::rc::Rc::from(
                            dim.as_str(),
                        )))
                        .and_then(|v| match v {
                            helen_interpreter::value::Value::Float(f) => Some(*f),
                            helen_interpreter::value::Value::Int(b) => {
                                b.to_string().parse::<f64>().ok()
                            }
                            _ => None,
                        })
                        .unwrap_or(total_score)
                    }
                    _ => total_score,
                };
                scores[dim] = serde_json::json!(dim_score);
            }

            all_results.push(serde_json::json!({
                "file": file,
                "metrics": metrics.get("metrics").cloned().unwrap_or(serde_json::Value::Null),
                "security_issues": security,
                "scores": scores,
            }));
        } else {
            match report_val {
                Ok(helen_interpreter::value::Value::Str(s)) => println!("{}", s),
                _ => println!("Score: {total_score:.2}/10"),
            }
        }
    }

    if json_output {
        if all_results.len() == 1 {
            println!(
                "{}",
                serde_json::to_string_pretty(&all_results[0]).unwrap_or_default()
            );
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&all_results).unwrap_or_default()
            );
        }
    }

    if threshold > 0.0 && min_score < threshold {
        eprintln!("\n❌ Score {min_score:.2} is below threshold {threshold:.2}");
        return 1;
    }
    0
}

/// Convert a Helen Value to serde_json for CLI output.
fn value_to_json(v: &helen_interpreter::value::Value) -> serde_json::Value {
    use helen_interpreter::value::Value;
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::json!(b),
        Value::Int(b) => serde_json::json!(b.to_string()),
        Value::Float(f) => serde_json::json!(f),
        Value::Str(s) => serde_json::json!(&**s),
        Value::List(l) => serde_json::Value::Array(l.borrow().iter().map(value_to_json).collect()),
        Value::Map(m) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in m.borrow().iter() {
                let key = match k {
                    Value::Str(s) => s.to_string(),
                    other => other.python_str(),
                };
                obj.insert(key, value_to_json(val));
            }
            serde_json::Value::Object(obj)
        }
        other => serde_json::json!(other.python_str()),
    }
}

// ---------------------------------------------------------------------------
// `helen watch`
// ---------------------------------------------------------------------------

/// `helen watch <file>` — re-run the program when the file changes.
/// Port of `watch_command` (poll every 0.5s, re-invokes the run pipeline).
pub fn watch_command(file: &str) -> i32 {
    let source_path = Path::new(file);
    if !source_path.exists() {
        eprintln!("Error: file not found: {file}");
        return 1;
    }
    println!("👀 Watching {file} (press Ctrl+C to stop)...");

    let mut last_mtime = 0.0f64;
    loop {
        let current_mtime = std::fs::metadata(source_path)
            .and_then(|m| m.modified())
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map_err(std::io::Error::other)
            })
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        if current_mtime != last_mtime {
            last_mtime = current_mtime;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let hh = now / 3600 % 24;
            let mm = now / 60 % 60;
            let ss = now % 60;
            println!("\n🔄 [{hh:02}:{mm:02}:{ss:02}] Change detected, running {file}...");
            println!("{}", "=".repeat(60));
            let code = crate::cli_commands::run_command(file, None);
            if code == 0 {
                println!("✅ Program completed successfully");
            } else {
                println!("❌ Program exited with code {code}");
            }
            println!("{}", "=".repeat(60));
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

// ---------------------------------------------------------------------------
// `helen template`
// ---------------------------------------------------------------------------

/// `helen template` — view and copy built-in Helen templates.
/// Port of `template_command`. Templates are embedded at compile time from
/// `crates/helen-rust/templates/`.
pub fn template_command(argv: &[String]) -> i32 {
    const TEMPLATES: &[(&str, &str)] = &[
        (
            "simple_agent",
            include_str!("../templates/simple_agent.helen"),
        ),
        (
            "spawn_channel",
            include_str!("../templates/spawn_channel.helen"),
        ),
        (
            "spawn_with_transcript",
            include_str!("../templates/spawn_with_transcript.helen"),
        ),
        (
            "shared_store",
            include_str!("../templates/shared_store.helen"),
        ),
        (
            "context_object",
            include_str!("../templates/context_object.helen"),
        ),
        ("pipeline", include_str!("../templates/pipeline.helen")),
    ];

    let list_only = argv.is_empty() || argv[0] == "--list" || argv[0] == "-l";

    if list_only {
        println!("Available Helen templates:\n");
        for (name, content) in TEMPLATES {
            let desc = content
                .lines()
                .find(|l| l.contains("// Description:"))
                .map(|l| {
                    l.split("// Description:")
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .to_string()
                })
                .unwrap_or_default();
            println!("  {name:<30} - {desc}");
        }
        println!();
        println!("Usage:");
        println!("  helen template <name>         # Show template content");
        println!("  helen template <name> --copy  # Copy to current directory");
        return 0;
    }

    let template_name = argv[0].as_str();
    let Some((_, content)) = TEMPLATES.iter().find(|(n, _)| *n == template_name) else {
        eprintln!("Error: Template '{template_name}' not found.");
        eprintln!(
            "Available templates: {}",
            TEMPLATES
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(", ")
        );
        return 1;
    };

    if argv.contains(&"--copy".to_string()) {
        let copy_idx = argv.iter().position(|a| a == "--copy").unwrap();
        let output_file = if copy_idx + 1 < argv.len() && !argv[copy_idx + 1].starts_with('-') {
            PathBuf::from(&argv[copy_idx + 1])
        } else {
            PathBuf::from(format!("{template_name}.helen"))
        };
        if let Err(e) = std::fs::write(&output_file, content) {
            eprintln!("Error: cannot write {}: {e}", output_file.display());
            return 1;
        }
        println!("✓ Template copied to: {}", output_file.display());
        println!();
        println!(
            "Edit {} and run with: helen {}",
            output_file.display(),
            output_file.display()
        );
        return 0;
    }

    println!("=== Template: {template_name} ===\n");
    print!("{content}");
    println!("\n=== End of template ===");
    println!();
    println!("Copy this template with: helen template {template_name} --copy");
    0
}

// ---------------------------------------------------------------------------
// `helen replay`
// ---------------------------------------------------------------------------

/// `helen replay <session_id> [--summary] [--dir <path>]` — interactive
/// transcript replay. Port of `replay_command` + `_interactive_replay`.
pub fn replay_command(argv: &[String]) -> i32 {
    let mut session_id: Option<String> = None;
    let mut session_dir: Option<PathBuf> = None;
    let mut show_summary = false;

    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--summary" => show_summary = true,
            "--dir" => {
                i += 1;
                if i < argv.len() {
                    session_dir = Some(PathBuf::from(&argv[i]));
                }
            }
            "--interactive" => {}
            a if a.starts_with("--") => {
                eprintln!("Unknown option: {a}");
                return 1;
            }
            a if session_id.is_none() => session_id = Some(a.to_string()),
            a => {
                eprintln!("Unexpected argument: {a}");
                return 1;
            }
        }
        i += 1;
    }

    let Some(sid) = session_id else {
        eprintln!("Error: 'replay' requires a session ID argument");
        eprintln!("Usage: helen replay <session_id> [--summary] [--dir <path>]");
        return 1;
    };

    // If session_id looks like a path, split into dir + id.
    let (sid, dir) = if session_dir.is_none() && (sid.contains('/') || sid.starts_with('.')) {
        let p = PathBuf::from(&sid);
        if p.is_dir() {
            (
                p.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or(sid),
                Some(p.parent().map(|x| x.to_path_buf()).unwrap_or_default()),
            )
        } else {
            (sid, None)
        }
    } else {
        (sid, session_dir)
    };

    let dir = match dir {
        Some(d) => d,
        None => {
            // Default: ~/.helen/sessions
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(&home).join(".helen").join("sessions")
        }
    };

    let replay = match helen_runtime::transcript_replay::TranscriptReplay::load(&sid, &dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error loading session '{sid}': {e}");
            return 1;
        }
    };

    if show_summary {
        let summary = replay.get_summary();
        let session = summary
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let total = summary
            .get("total_messages")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let roles = summary
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let agents = summary
            .get("agents")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        println!("Session: {session}");
        println!("Total messages: {total}");
        println!("Roles: {roles}");
        if !agents.is_empty() {
            println!("Agents: {agents}");
        }
        return 0;
    }

    interactive_replay(replay)
}

/// Interactive replay loop (port of `_interactive_replay`).
fn interactive_replay(mut replay: helen_runtime::transcript_replay::TranscriptReplay) -> i32 {
    use std::io::Write;

    println!("Transcript Replay - Session: {}", replay.session_id);
    println!("Total messages: {}", replay.len());
    println!();
    println!("Commands:");
    println!("  n, next      - Next message");
    println!("  p, prev      - Previous message");
    println!("  j <n>        - Jump to message n");
    println!("  f, first     - First message");
    println!("  l, last      - Last message");
    println!("  s <query>    - Search for query");
    println!("  summary      - Show summary");
    println!("  q, quit      - Exit replay mode");
    println!();

    replay.first();
    println!("{}", replay.format_current());

    loop {
        print!("\nreplay> ");
        let _ = std::io::stdout().flush();
        let mut cmd = String::new();
        match std::io::stdin().read_line(&mut cmd) {
            Ok(0) | Err(_) => {
                println!();
                break;
            }
            Ok(_) => {}
        }
        let cmd = cmd.trim().to_string();
        if cmd.is_empty() {
            continue;
        }
        let mut parts = cmd.splitn(2, ' ');
        let command = parts.next().unwrap_or("").to_lowercase();
        let arg = parts.next().unwrap_or("").trim().to_string();

        match command.as_str() {
            "q" | "quit" | "exit" => break,
            "n" | "next" => {
                if replay.next().is_some() {
                    println!("{}", replay.format_current());
                } else {
                    println!("Already at last message");
                }
            }
            "p" | "prev" | "previous" => {
                if replay.prev().is_some() {
                    println!("{}", replay.format_current());
                } else {
                    println!("Already at first message");
                }
            }
            "j" | "jump" => match arg.parse::<usize>() {
                Ok(index) if replay.jump(index).is_some() => {
                    println!("{}", replay.format_current());
                }
                _ => println!("Invalid index: {arg}"),
            },
            "f" | "first" => {
                replay.first();
                println!("{}", replay.format_current());
            }
            "l" | "last" => {
                replay.last();
                println!("{}", replay.format_current());
            }
            "s" | "search" => {
                if arg.is_empty() {
                    println!("Usage: s <query>");
                } else {
                    let results = replay.search(&arg, false);
                    if results.is_empty() {
                        println!("No matches for '{arg}'");
                    } else {
                        println!("Found {} match(es):", results.len());
                        for idx in results.iter().take(20) {
                            let msg = replay.get_message_at(*idx);
                            let preview = msg
                                .and_then(|m| m.get("content"))
                                .and_then(|c| c.as_str())
                                .map(|c| {
                                    let t = c.replace('\n', " ");
                                    if t.len() > 60 {
                                        format!("{}...", &t[..60])
                                    } else {
                                        t
                                    }
                                })
                                .unwrap_or_default();
                            println!("  [{idx}] {preview}");
                        }
                    }
                }
            }
            "summary" => {
                let summary = replay.get_summary();
                println!(
                    "Session: {}",
                    summary
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                );
                println!(
                    "Total messages: {}",
                    summary
                        .get("total_messages")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0)
                );
            }
            other => println!("Unknown command: {other}"),
        }
    }
    0
}

// ---------------------------------------------------------------------------
// Session flags
// ---------------------------------------------------------------------------

/// Extract `--session`/`--resume-latest` flags from argv.
/// Port of `_extract_session_flags`: returns (session_id, remaining_argv).
pub fn extract_session_flags(argv: &[String]) -> (Option<String>, Vec<String>) {
    let mut session_id: Option<String> = None;
    let mut resume_latest = false;
    let mut remaining: Vec<String> = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if let Some(v) = arg.strip_prefix("--session=") {
            session_id = Some(v.to_string());
        } else if arg == "--session" && i + 1 < argv.len() {
            session_id = Some(argv[i + 1].clone());
            i += 1;
        } else if arg == "--resume-latest" || arg == "-r" {
            resume_latest = true;
        } else {
            remaining.push(arg.clone());
        }
        i += 1;
    }

    if resume_latest && session_id.is_none() {
        // Find the most recent session by transcript mtime.
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let manager = helen_runtime::SessionManager::new(Some(
            Path::new(&home).join(".helen").join("sessions").as_path(),
        ));
        let sessions = manager.list_sessions();
        if let Some(latest) = sessions.iter().max_by(|a, b| {
            a.modified_at
                .partial_cmp(&b.modified_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            session_id = Some(latest.session_id.clone());
        }
    }

    (session_id, remaining)
}

// ---------------------------------------------------------------------------
// `run_command` — shared by `helen <file>` and `helen watch`
// ---------------------------------------------------------------------------

/// Run a Helen program. Exit codes: 0 success, 1 file-not-found/syntax,
/// 2 semantic, 3 runtime.
pub fn run_command(file: &str, session_id: Option<&str>) -> i32 {
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
            eprintln!(
                "{}",
                crate::formatter::format_error(&diag, Some(&source_lines))
            );
        }
        return 1;
    }

    // Analyze
    let mut analyzer =
        helen_semantic::SemanticAnalyzer::new(helen_semantic::ErrorReporter::new(), ".");
    analyzer.analyze(&program);
    if analyzer.errors.has_errors() {
        for d in analyzer.errors.errors() {
            eprintln!("{}", crate::formatter::format_error(d, Some(&source_lines)));
        }
        return 2;
    }

    // Interpret
    let mut interp = Interpreter::new();
    if let Some(sid) = session_id {
        interp.session_id = sid.to_string();
    }
    interp.set_source_file(file);

    // Set up LLM runtime from config (so `llm act` works)
    let runtime = helen_runtime::http_llm::HttpLLMRuntime::new(None, None, None);
    if !runtime.api_key.is_empty() && runtime.api_key != "sk-placeholder" {
        let adapter = crate::llm_adapter::HttpLlmAdapter::new(runtime);
        #[allow(clippy::arc_with_non_send_sync)]
        interp.set_llm_runtime(std::sync::Arc::new(adapter));
    }

    let result = interp.interpret(&program);
    let stdout = interp.stdout.lock().expect("mutex poisoned").clone();
    // Only print buffer if stdout is not a TTY (piped/captured output)
    // For TTY, output was already printed incrementally by builtin_print
    if !std::io::stdout().is_terminal() {
        print!("{stdout}");
    }
    match result {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("RuntimeError: {}", e.to_display_string());
            3
        }
    }
}
