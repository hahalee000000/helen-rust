//! `helen test <file...>` — port of the CLI test runner.
//!
//! Interprets each test file, auto-discovers `fn test_*` functions, runs
//! them, and prints the report in Python's `_format_report` layout.
//! Supports `--json`, `--only <name>`, `--suite <name>`, `--filter <pat>`,
//! `--verbose`.

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use helen_semantic::SemanticAnalyzer;
use std::time::Instant;

pub struct TestOpts {
    pub json_output: bool,
    #[allow(dead_code)] // --verbose flag parsed, output detail reserved
    pub verbose: bool,
    pub only: Option<String>,
    pub suite: Option<String>,
    pub filter: Option<String>,
}

struct TestResult {
    name: String,
    suite: String,
    passed: bool,
    error: Option<String>,
    duration_ms: f64,
}

pub struct TestReport {
    results: Vec<TestResult>,
    duration_ms: f64,
    warnings: Vec<String>,
}

impl TestReport {
    fn totals(&self) -> (usize, usize, usize) {
        let passed = self.results.iter().filter(|r| r.passed).count();
        let failed = self.results.iter().filter(|r| !r.passed).count();
        (passed, failed, 0)
    }
}

/// Parse CLI args (port of the Python arg loop).
pub fn parse_args(argv: &[String]) -> Result<(Vec<String>, TestOpts), String> {
    let mut files: Vec<String> = Vec::new();
    let mut json_output = false;
    let mut verbose = false;
    let mut only: Option<String> = None;
    let mut suite: Option<String> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "--json" => json_output = true,
            "--verbose" | "-v" => verbose = true,
            "--only" => {
                i += 1;
                if i >= argv.len() {
                    return Err("Error: --only requires a test name argument".into());
                }
                only = Some(argv[i].clone());
            }
            "--suite" => {
                i += 1;
                if i >= argv.len() {
                    return Err("Error: --suite requires a suite name argument".into());
                }
                suite = Some(argv[i].clone());
            }
            "--filter" => {
                i += 1;
                if i >= argv.len() {
                    return Err("Error: --filter requires a pattern argument".into());
                }
                filter = Some(argv[i].clone());
            }
            _ if !arg.starts_with('-') => files.push(arg.clone()),
            _ => return Err(format!("Unknown option: {arg}")),
        }
        i += 1;
    }

    if files.is_empty() {
        return Err("Error: 'test' requires at least one file argument".into());
    }

    Ok((
        files,
        TestOpts {
            json_output,
            verbose,
            only,
            suite,
            filter,
        },
    ))
}

/// Run the `helen test` command. Returns the process exit code.
pub fn test_command(argv: &[String]) -> i32 {
    let (files, opts) = match parse_args(argv) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    let total_start = Instant::now();

    // Shared interpreter so functions are visible across files.
    let mut shared_interp = Interpreter::new();

    // First pass: interpret each test file (registers fn test_* functions).
    for file in &files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: cannot read {file}: {e}");
                return 2;
            }
        };

        let mut scanner = Scanner::new(&source, file);
        let tokens = scanner.scan_all();
        let mut parser = Parser::new(tokens);
        let program = parser.parse();
        let parse_errors = parser.errors();
        if !parse_errors.is_empty() {
            for e in parse_errors {
                eprintln!("{e}");
            }
            return 2;
        }

        let mut analyzer = SemanticAnalyzer::new(helen_semantic::ErrorReporter::new(), ".");
        analyzer.analyze(&program);
        if analyzer.errors.has_errors() {
            for d in analyzer.errors.errors() {
                eprintln!("{d}");
            }
            return 2;
        }

        if let Err(e) = shared_interp.interpret(&program) {
            eprintln!("RuntimeError: {}", e.to_display_string());
            return 2;
        }
    }

    // Auto-discover fn test_* functions.
    let mut test_names: Vec<String> = shared_interp
        .functions
        .keys()
        .filter(|n| n.starts_with("test_"))
        .cloned()
        .collect();
    test_names.sort();

    // Apply filters.
    if let Some(only) = &opts.only {
        test_names.retain(|n| n == only);
    }
    if let Some(suite) = &opts.suite {
        // The Rust runner treats each file as a suite; filter by suite name
        // matches test names containing the suite (best-effort parity).
        test_names.retain(|n| n.contains(suite));
    }
    if let Some(pattern) = &opts.filter {
        test_names.retain(|n| n.contains(pattern));
    }

    // Show filter info.
    if opts.only.is_some() || opts.suite.is_some() || opts.filter.is_some() {
        let mut filters = Vec::new();
        if let Some(o) = &opts.only {
            filters.push(format!("test='{o}'"));
        }
        if let Some(s) = &opts.suite {
            filters.push(format!("suite='{s}'"));
        }
        if let Some(f) = &opts.filter {
            filters.push(format!("pattern='{f}'"));
        }
        println!("🔍 Filtered by: {}", filters.join(", "));
        println!();
    }

    // Run each test.
    let mut results: Vec<TestResult> = Vec::new();
    for name in &test_names {
        let node = match shared_interp.functions.get(name) {
            Some(n) => n.clone(),
            None => continue,
        };
        let start = Instant::now();
        let span = helen_core::source::SourceSpan::new("", 0, 0, 0, 0);
        let outcome = shared_interp.call_function(&node, vec![], None, &span);
        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        let (passed, error) = match outcome {
            Ok(_) => (true, None),
            Err(e) => (false, Some(e.to_display_string())),
        };
        results.push(TestResult {
            name: name.clone(),
            suite: "(default)".to_string(),
            passed,
            error,
            duration_ms,
        });
    }

    let total_elapsed = total_start.elapsed().as_secs_f64() * 1000.0;
    let report = TestReport {
        results,
        duration_ms: total_elapsed,
        warnings: Vec::new(),
    };

    if opts.json_output {
        let (passed, failed, skipped) = report.totals();
        let results_json: Vec<serde_json::Value> = report
            .results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "suite": r.suite,
                    "passed": r.passed,
                    "error": r.error,
                    "duration_ms": (r.duration_ms * 100.0).round() / 100.0,
                })
            })
            .collect();
        let data = serde_json::json!({
            "total": report.results.len(),
            "passed": passed,
            "failed": failed,
            "skipped": skipped,
            "duration_ms": (total_elapsed * 100.0).round() / 100.0,
            "suites": [{"name": "(default)", "tests": report.results.len()}],
            "results": results_json,
        });
        println!("{}", serde_json::to_string_pretty(&data).unwrap());
    } else {
        println!("{}", format_report(&report));
    }

    let (_, failed, _) = report.totals();
    if failed == 0 {
        0
    } else {
        1
    }
}

/// `_format_report(report)` — port of the Python test report layout.
pub fn format_report(report: &TestReport) -> String {
    let mut lines: Vec<String> = vec![String::new()];
    lines.push("=".repeat(60));
    lines.push("  HELEN TEST RESULTS".to_string());
    lines.push("=".repeat(60));
    lines.push(String::new());

    if !report.warnings.is_empty() {
        for w in &report.warnings {
            lines.push(format!("  ⚠ {w}"));
        }
        lines.push(String::new());
    }

    let mut suite_results: Vec<&TestResult> = report
        .results
        .iter()
        .filter(|r| r.suite == "(default)")
        .collect();
    suite_results.sort_by(|a, b| a.name.cmp(&b.name));
    if !suite_results.is_empty() {
        lines.push("  (default)".to_string());
        for r in suite_results {
            if r.passed {
                lines.push(format!("    ✓ {} ({:.1}ms)", r.name, r.duration_ms));
            } else {
                lines.push(format!("    ✗ {}", r.name));
                let error_line = r
                    .error
                    .as_deref()
                    .unwrap_or("")
                    .split('\n')
                    .next()
                    .unwrap_or("");
                lines.push(format!("      → {error_line}"));
            }
        }
        lines.push(String::new());
    }

    let (passed, failed, skipped) = report.totals();
    let total = report.results.len();
    lines.push("-".repeat(60));
    lines.push(format!(
        "  {passed} passed, {failed} failed, {skipped} skipped ({total} total)"
    ));
    lines.push(format!("  Duration: {:.1}ms", report.duration_ms));
    lines.push("=".repeat(60));
    if failed > 0 {
        lines.push("  ✗ TESTS FAILED".to_string());
    } else {
        lines.push("  ✓ ALL TESTS PASSED".to_string());
    }
    lines.push("=".repeat(60));
    lines.push(String::new());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_simple() {
        let (files, opts) = parse_args(&["test_a.helen".to_string()]).unwrap();
        assert_eq!(files, vec!["test_a.helen"]);
        assert!(!opts.json_output);
    }

    #[test]
    fn test_parse_args_flags() {
        let (files, opts) = parse_args(&[
            "--json".to_string(),
            "a.helen".to_string(),
            "b.helen".to_string(),
            "--only".to_string(),
            "test_x".to_string(),
        ])
        .unwrap();
        assert_eq!(files.len(), 2);
        assert!(opts.json_output);
        assert_eq!(opts.only.as_deref(), Some("test_x"));
    }

    #[test]
    fn test_parse_args_missing_only() {
        let r = parse_args(&["--only".to_string()]);
        assert!(r.is_err());
    }

    #[test]
    fn test_parse_args_no_files() {
        let r = parse_args(&[]);
        assert!(r.is_err());
    }

    #[test]
    fn test_parse_args_unknown() {
        let r = parse_args(&["--bogus".to_string()]);
        assert!(r.is_err());
    }

    #[test]
    fn test_format_report_layout() {
        let report = TestReport {
            results: vec![TestResult {
                name: "test_add".to_string(),
                suite: "(default)".to_string(),
                passed: true,
                error: None,
                duration_ms: 1.5,
            }],
            duration_ms: 12.3,
            warnings: Vec::new(),
        };
        let out = format_report(&report);
        assert!(out.contains("HELEN TEST RESULTS"), "{out}");
        assert!(out.contains("1 passed"), "{out}");
        assert!(out.contains("ALL TESTS PASSED"), "{out}");
    }

    #[test]
    fn test_format_report_failure() {
        let report = TestReport {
            results: vec![TestResult {
                name: "test_bad".to_string(),
                suite: "(default)".to_string(),
                passed: false,
                error: Some("RuntimeError: boom".to_string()),
                duration_ms: 0.5,
            }],
            duration_ms: 5.0,
            warnings: Vec::new(),
        };
        let out = format_report(&report);
        assert!(out.contains("✗ test_bad"), "{out}");
        assert!(out.contains("0 passed, 1 failed"), "{out}");
        assert!(out.contains("TESTS FAILED"), "{out}");
    }
}
