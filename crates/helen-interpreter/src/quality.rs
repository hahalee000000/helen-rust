//! Quality analysis stdlib functions.
//!
//! Byte-faithful port of `helen/stdlib/quality.py` (v1.44.0): provides
//! code quality analysis and scoring functions.
//!
//! This is a simplified implementation that provides basic metrics and scoring.
//! The full Python version has 1471 lines with comprehensive static analysis.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

/// Analyze code quality — returns basic metrics.
/// Python: `analyze_code(source: str, filename: str = "<unknown>") -> dict`.
pub fn quality_analyze_code(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let source = arg_str_or(args, 0, "");
    let filename = arg_str_or(args, 1, "<unknown>");

    let lines: Vec<&str> = source.lines().collect();
    let total_lines = lines.len();
    let code_lines = lines
        .iter()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//"))
        .count();
    let comment_lines = lines.iter().filter(|l| l.trim().starts_with("//")).count();
    let blank_lines = lines.iter().filter(|l| l.trim().is_empty()).count();
    let comment_ratio = if total_lines > 0 {
        comment_lines as f64 / total_lines as f64
    } else {
        0.0
    };

    // Count functions and agents (simple regex-like matching)
    let function_count = lines.iter().filter(|l| l.trim().starts_with("fn ")).count();
    let agent_count = lines
        .iter()
        .filter(|l| l.trim().starts_with("agent "))
        .count();

    // Estimate average function length (rough heuristic)
    let avg_function_length = if function_count > 0 {
        code_lines as f64 / function_count as f64
    } else {
        0.0
    };
    let max_function_length = if function_count > 0 { code_lines } else { 0 };

    let mut metrics = indexmap::IndexMap::new();
    metrics.insert(
        Value::Str(Rc::from("total_lines")),
        Value::Int(BigInt::from(total_lines as i64)),
    );
    metrics.insert(
        Value::Str(Rc::from("code_lines")),
        Value::Int(BigInt::from(code_lines as i64)),
    );
    metrics.insert(
        Value::Str(Rc::from("comment_lines")),
        Value::Int(BigInt::from(comment_lines as i64)),
    );
    metrics.insert(
        Value::Str(Rc::from("blank_lines")),
        Value::Int(BigInt::from(blank_lines as i64)),
    );
    metrics.insert(
        Value::Str(Rc::from("comment_ratio")),
        Value::Float(comment_ratio),
    );
    metrics.insert(
        Value::Str(Rc::from("function_count")),
        Value::Int(BigInt::from(function_count as i64)),
    );
    metrics.insert(
        Value::Str(Rc::from("agent_count")),
        Value::Int(BigInt::from(agent_count as i64)),
    );
    metrics.insert(
        Value::Str(Rc::from("avg_function_length")),
        Value::Float(avg_function_length),
    );
    metrics.insert(
        Value::Str(Rc::from("max_function_length")),
        Value::Int(BigInt::from(max_function_length as i64)),
    );
    metrics.insert(
        Value::Str(Rc::from("filename")),
        Value::Str(Rc::from(filename.as_str())),
    );

    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
    result.insert(
        Value::Str(Rc::from("metrics")),
        Value::Map(Rc::new(RefCell::new(metrics))),
    );
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Check code security — returns list of issues.
/// Python: `check_security(source: str) -> list[dict]` via SecurityAnalyzer.
/// Port of helen/stdlib/quality.py SecurityAnalyzer (v1.44.0).
pub fn quality_check_security(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let source = arg_str_or(args, 0, "");
    let issues = analyze_security(&source);
    Ok(Value::List(Rc::new(RefCell::new(issues))))
}

// ── SecurityAnalyzer port (helen/stdlib/quality.py) ──────────────────────

struct SecurityIssue {
    line: usize,
    severity: &'static str,
    pattern: &'static str,
    message: String,
}

/// Dangerous patterns: (regex, severity, name, message).
/// Mirrors `SecurityAnalyzer.DANGEROUS_PATTERNS`.
const DANGEROUS_PATTERNS: &[(&str, &str, &str, &str)] = &[
    // High severity
    (
        r"\beval\s*\(",
        "high",
        "eval()",
        "eval() can execute arbitrary code",
    ),
    (
        r"\bexec\s*\(",
        "high",
        "exec()",
        "exec() can execute arbitrary code",
    ),
    (
        r"shell\s*=\s*true",
        "high",
        "shell=true",
        "shell=true enables command injection",
    ),
    (
        r#"\bimport\s+["']os["']"#,
        "high",
        "FFI os import",
        "FFI import of os module enables system access",
    ),
    (
        r#"\bimport\s+["']subprocess["']"#,
        "high",
        "FFI subprocess import",
        "FFI import of subprocess enables command execution",
    ),
    // Medium severity
    (
        r"shell_exec\s*\([^)]*\+",
        "medium",
        "shell_exec concat",
        "shell_exec with concatenated input — validate arguments to prevent command injection",
    ),
    (
        r"shell_exec\s*\(",
        "medium",
        "shell_exec()",
        "shell_exec can execute system commands",
    ),
    (
        r#"\bopen\s*\([^)]*["']w"#,
        "medium",
        "file write",
        "file write without path validation",
    ),
    (
        r"http_get\s*\([^)]*\+",
        "medium",
        "URL concatenation",
        "URL built from user input may allow SSRF",
    ),
    (
        r"http_post\s*\([^)]*\+",
        "medium",
        "URL concatenation",
        "URL built from user input may allow SSRF",
    ),
    (
        r"read_file\s*\([^)]*\+",
        "medium",
        "path concatenation",
        "file path from user input may allow traversal",
    ),
    (
        r"write_file\s*\([^)]*\+",
        "medium",
        "path concatenation",
        "file path from user input may allow traversal",
    ),
    // Low severity
    (
        r"\binput\s*\(",
        "low",
        "user input",
        "user input should be validated before use",
    ),
    (
        r"llm\s+act\b",
        "low",
        "LLM act",
        "LLM output should be validated before use in critical operations",
    ),
];

/// Patterns whose severity is downgraded when safety measures are nearby.
const DOWNGRADABLE: &[&str] = &[
    "shell_exec concat",
    "shell_exec()",
    "file write",
    "URL concatenation",
    "path concatenation",
];

/// Safety-measure detection regexes (surrounding context).
const SAFETY_PATTERNS: &[&str] = &[
    r"\b(is_file|is_dir|exists|file_exists|dir_exists)\s*\(",
    r"\b(validate_path|path_validate|safe_path|allowed_path|check_path)\s*\(",
    r"\b(resolve|realpath|normalize|canonicalize)\s*\(",
    r"\b(allowed_dir|base_dir|sandbox|chroot|allowed_root)\b",
    r"\b(starts_with|startswith|endswith|contains)\s*\([^)]*(dir|path|root|base)",
    r"\b(sanitize|escape|shlex\.quote|shlex_quote|shell_quote)\s*\(",
    r"\b(validate|check|verify|assert_safe)\s*\([^)]*(input|arg|param|cmd|command|path)",
    r"\b(whitelist|allowlist|allowed_commands|safe_commands|permitted)\b",
    r"\btry\s*\{",
];

/// Strip a trailing `// ...` comment (aware of quotes). Simplified port.
fn strip_inline_comment(line: &str) -> String {
    let mut in_str = false;
    let mut prev = '\0';
    for (i, ch) in line.char_indices() {
        if ch == '"' && prev != '\\' {
            in_str = !in_str;
        }
        if ch == '/' && prev == '/' && !in_str {
            return line[..i - 1].to_string();
        }
        prev = ch;
    }
    line.to_string()
}

/// Find the enclosing block start line for `line_idx` (fn/agent) — simplified
/// heuristic: scan upward for a line whose stripped form starts with `fn `,
/// `agent `, or is a block opener.
fn find_enclosing_block_start(lines: &[&str], line_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    for i in (0..line_idx).rev() {
        let s = lines[i].trim();
        if s.starts_with("fn ") || s.starts_with("agent ") {
            return Some(i);
        }
        if s.ends_with('}') {
            depth += 1;
        }
        if s.ends_with('{') {
            if depth > 0 {
                depth -= 1;
            } else {
                return Some(i);
            }
        }
    }
    None
}

fn has_safety_context(lines: &[&str], line_idx: usize) -> bool {
    // Scan enclosing block + up to 15 lines before (simplified: just the
    // preceding 15 lines within the same block).
    let block_start = find_enclosing_block_start(lines, line_idx);
    let start = match block_start {
        Some(b) => b,
        None => line_idx.saturating_sub(15),
    };
    for (_, line) in lines.iter().enumerate().take(line_idx).skip(start) {
        for pat in SAFETY_PATTERNS {
            if let Ok(re) = regex::Regex::new(pat) {
                if re.is_match(line) {
                    return true;
                }
            }
        }
    }
    false
}

fn analyze_security(source: &str) -> Vec<Value> {
    let lines: Vec<&str> = source.lines().collect();
    let mut issues: Vec<SecurityIssue> = Vec::new();
    let mut in_multiline_string = false;
    let mut in_block_comment = false;

    for (i, raw_line) in lines.iter().enumerate() {
        let mut line = raw_line.to_string();
        let stripped = line.trim().to_string();

        // Block comments /* ... */
        if in_block_comment {
            if line.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if stripped.starts_with("/*") {
            if !line[2..].contains("*/") {
                in_block_comment = true;
            }
            continue;
        }

        // Multi-line strings """ ... """
        if in_multiline_string {
            if let Some(close_idx) = line.find("\"\"\"") {
                in_multiline_string = false;
                line = line[close_idx + 3..].to_string();
            } else {
                continue;
            }
        }
        let triple_count = line.matches("\"\"\"").count();
        if triple_count % 2 == 1 {
            if let Some(open_idx) = line.find("\"\"\"") {
                if let Some(close_idx) = line[open_idx + 3..].find("\"\"\"") {
                    // open+close on same line — strip the quoted region
                    line = format!(
                        "{}{}",
                        &line[..open_idx],
                        &line[open_idx + 3 + close_idx + 3..]
                    );
                } else {
                    in_multiline_string = true;
                    line = line[..open_idx].to_string();
                }
            }
        }

        if line.trim().starts_with("//") {
            continue;
        }
        let code_part = strip_inline_comment(&line);

        for &(pattern, severity, name, message) in DANGEROUS_PATTERNS {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(&code_part) {
                    let mut effective_severity = severity;
                    let mut effective_message = message.to_string();
                    if severity == "medium"
                        && DOWNGRADABLE.contains(&name)
                        && has_safety_context(&lines, i)
                    {
                        effective_severity = "low";
                        effective_message
                            .push_str(" (downgraded: safety measures detected nearby)");
                    }
                    issues.push(SecurityIssue {
                        line: i + 1,
                        severity: effective_severity,
                        pattern: name,
                        message: effective_message,
                    });
                }
            }
        }
    }

    issues
        .into_iter()
        .map(|iss| {
            let mut m = indexmap::IndexMap::new();
            m.insert(
                Value::Str(Rc::from("line")),
                Value::Int(BigInt::from(iss.line as i64)),
            );
            m.insert(
                Value::Str(Rc::from("severity")),
                Value::Str(Rc::from(iss.severity)),
            );
            m.insert(
                Value::Str(Rc::from("pattern")),
                Value::Str(Rc::from(iss.pattern)),
            );
            m.insert(
                Value::Str(Rc::from("message")),
                Value::Str(Rc::from(iss.message)),
            );
            Value::Map(Rc::new(RefCell::new(m)))
        })
        .collect()
}

/// Get quality score — returns 0-10 score based on basic heuristics.
/// Python: `quality_score(source: str) -> float`.
pub fn quality_quality_score(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let source = arg_str_or(args, 0, "");
    let lines: Vec<&str> = source.lines().collect();
    let total_lines = lines.len();

    if total_lines == 0 {
        return Ok(Value::Float(0.0));
    }

    let comment_lines = lines.iter().filter(|l| l.trim().starts_with("//")).count();
    let comment_ratio = comment_lines as f64 / total_lines as f64;
    let function_count = lines.iter().filter(|l| l.trim().starts_with("fn ")).count();

    // Simple scoring heuristic:
    // - Base score: 5.0
    // - +2.0 if has comments (ratio > 0.1)
    // - +1.5 if has functions
    // - +1.5 if code is reasonably sized (>10 lines)
    let mut score: f64 = 5.0;
    if comment_ratio > 0.1 {
        score += 2.0;
    }
    if function_count > 0 {
        score += 1.5;
    }
    if total_lines > 10 {
        score += 1.5;
    }

    Ok(Value::Float(score.min(10.0)))
}

/// Compute per-dimension quality scores.
/// Returns a map of dimension name -> score (0.0-10.0).
/// Dimensions: architecture, code_quality, security, test_coverage, documentation, maintainability, engineering.
pub fn quality_dimension_scores(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let source = arg_str_or(args, 0, "");
    let lines: Vec<&str> = source.lines().collect();
    let total_lines = lines.len();

    if total_lines == 0 {
        let mut scores = indexmap::IndexMap::new();
        for dim in &[
            "architecture",
            "code_quality",
            "security",
            "test_coverage",
            "documentation",
            "maintainability",
            "engineering",
        ] {
            scores.insert(Value::Str(Rc::from(*dim)), Value::Float(0.0));
        }
        return Ok(Value::Map(Rc::new(RefCell::new(scores))));
    }

    let comment_lines = lines.iter().filter(|l| l.trim().starts_with("//")).count();
    let comment_ratio = comment_lines as f64 / total_lines as f64;
    let function_count = lines.iter().filter(|l| l.trim().starts_with("fn ")).count();
    let agent_count = lines
        .iter()
        .filter(|l| l.trim().starts_with("agent "))
        .count();
    let test_count = lines
        .iter()
        .filter(|l| l.trim().starts_with("fn test_"))
        .count();

    // Architecture: based on agent usage and function organization
    let mut architecture: f64 = 5.0;
    if agent_count > 0 {
        architecture += 2.0;
    }
    if function_count > 2 {
        architecture += 1.5;
    }
    if total_lines > 20 {
        architecture += 1.5;
    }

    // Code quality: based on function structure and size
    let mut code_quality: f64 = 5.0;
    if function_count > 0 {
        code_quality += 2.0;
    }
    let avg_fn_len = if function_count > 0 {
        total_lines as f64 / function_count as f64
    } else {
        0.0
    };
    if avg_fn_len > 5.0 && avg_fn_len < 50.0 {
        code_quality += 2.0;
    }
    if total_lines > 10 {
        code_quality += 1.0;
    }

    // Security: basic heuristic (no sensitive patterns)
    let mut security: f64 = 7.0; // Start high, deduct for risks
    let has_hardcoded_keys = lines
        .iter()
        .any(|l| l.contains("api_key") && l.contains("=") && !l.contains("config"));
    if has_hardcoded_keys {
        security -= 3.0;
    }
    if lines
        .iter()
        .any(|l| l.contains("eval(") || l.contains("exec("))
    {
        security -= 2.0;
    }

    // Test coverage: based on test function presence
    let mut test_coverage: f64 = 3.0;
    if test_count > 0 {
        test_coverage += 3.0;
    }
    if test_count >= function_count / 2 {
        test_coverage += 2.0;
    }
    if test_count >= function_count && function_count > 0 {
        test_coverage += 2.0;
    }

    // Documentation: based on comment ratio
    let mut documentation: f64 = 4.0;
    if comment_ratio > 0.05 {
        documentation += 2.0;
    }
    if comment_ratio > 0.1 {
        documentation += 2.0;
    }
    if comment_ratio > 0.2 {
        documentation += 2.0;
    }

    // Maintainability: based on function size and modularity
    let mut maintainability: f64 = 5.0;
    if function_count > 3 {
        maintainability += 2.0;
    }
    if avg_fn_len < 30.0 {
        maintainability += 2.0;
    }
    if agent_count > 0 {
        maintainability += 1.0;
    }

    // Engineering: overall code organization
    let mut engineering: f64 = 5.0;
    if function_count > 0 {
        engineering += 1.5;
    }
    if agent_count > 0 {
        engineering += 1.5;
    }
    if comment_ratio > 0.1 {
        engineering += 1.0;
    }
    if total_lines > 15 {
        engineering += 1.0;
    }

    let mut scores = indexmap::IndexMap::new();
    scores.insert(
        Value::Str(Rc::from("architecture")),
        Value::Float(architecture.min(10.0)),
    );
    scores.insert(
        Value::Str(Rc::from("code_quality")),
        Value::Float(code_quality.min(10.0)),
    );
    scores.insert(
        Value::Str(Rc::from("security")),
        Value::Float(security.clamp(0.0, 10.0)),
    );
    scores.insert(
        Value::Str(Rc::from("test_coverage")),
        Value::Float(test_coverage.min(10.0)),
    );
    scores.insert(
        Value::Str(Rc::from("documentation")),
        Value::Float(documentation.min(10.0)),
    );
    scores.insert(
        Value::Str(Rc::from("maintainability")),
        Value::Float(maintainability.min(10.0)),
    );
    scores.insert(
        Value::Str(Rc::from("engineering")),
        Value::Float(engineering.min(10.0)),
    );

    Ok(Value::Map(Rc::new(RefCell::new(scores))))
}

/// Get quality report — returns formatted text report.
/// Python: `quality_report(source: str, filename: str = "<unknown>") -> str`.
pub fn quality_quality_report(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let source = arg_str_or(args, 0, "");
    let filename = arg_str_or(args, 1, "<unknown>");

    let lines: Vec<&str> = source.lines().collect();
    let total_lines = lines.len();
    let code_lines = lines
        .iter()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//"))
        .count();
    let comment_lines = lines.iter().filter(|l| l.trim().starts_with("//")).count();
    let comment_ratio = if total_lines > 0 {
        comment_lines as f64 / total_lines as f64
    } else {
        0.0
    };
    let function_count = lines.iter().filter(|l| l.trim().starts_with("fn ")).count();
    let agent_count = lines
        .iter()
        .filter(|l| l.trim().starts_with("agent "))
        .count();

    // Calculate score
    let mut score: f64 = 5.0;
    if comment_ratio > 0.1 {
        score += 2.0;
    }
    if function_count > 0 {
        score += 1.5;
    }
    if total_lines > 10 {
        score += 1.5;
    }
    score = score.min(10.0);

    let grade = if score >= 9.0 {
        "A"
    } else if score >= 8.0 {
        "B"
    } else if score >= 7.0 {
        "C"
    } else if score >= 6.0 {
        "D"
    } else {
        "F"
    };

    let mut report = String::new();
    report.push('\n');
    report.push_str("============================================================\n");
    report.push_str("  HELEN QUALITY REPORT\n");
    report.push_str("============================================================\n");
    report.push_str(&format!("  File: {}\n\n", filename));

    report.push_str("  Code Metrics:\n");
    report.push_str(&format!("    Total lines: {}\n", total_lines));
    report.push_str(&format!("    Code lines: {}\n", code_lines));
    report.push_str(&format!(
        "    Comment lines: {} ({:.0}%)\n",
        comment_lines,
        comment_ratio * 100.0
    ));
    report.push_str(&format!("    Functions: {}\n", function_count));
    report.push_str(&format!("    Agents: {}\n", agent_count));
    report.push('\n');

    report.push_str("  Quality Score (0-10):\n");
    report.push_str(&format!("    TOTAL: {:.2}\n", score));
    report.push_str(&format!("    GRADE: {}\n", grade));
    report.push('\n');

    report.push_str("============================================================\n");
    report.push('\n');

    Ok(Value::Str(Rc::from(report.as_str())))
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn arg_str_or(args: &[Value], i: usize, default: &str) -> String {
    match args.get(i) {
        Some(Value::Str(s)) => s.to_string(),
        _ => default.to_string(),
    }
}
