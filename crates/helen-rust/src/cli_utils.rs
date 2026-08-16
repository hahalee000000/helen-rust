//! CLI utility functions — pure functions extracted from main.rs for testing.

use helen_interpreter::exceptions::ExceptionValue;

/// The Helen version string (matches Python `helen.__version__`).
pub const HELEN_VERSION: &str = "1.45.0";

/// Python `reference.py` strips " at <path>:<line>:<col>-<col>" suffixes via
/// `_SPAN_RE = re.compile(r" at \S+:\d+:\d+-\d+")`. Mirror that regex exactly
/// so stderr compares equal.
pub fn normalize_stderr(s: &str) -> String {
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
pub fn last_span_end(b: &[u8]) -> Option<usize> {
    let n = b.len();
    // Scan for each `:` then try to match `:\d+:\d+-\d+`; keep the rightmost.
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
pub fn render_uncaught(e: &ExceptionValue) -> String {
    format!("RuntimeError: {}\n", e.to_display_string())
}

/// Emit the run-result JSON in reference.py's exact layout:
/// `{"stdout": "...", "stderr": "...", "exit_code": N, "error_classes": [...]}`
/// (spaces after separators, stdout/stderr/exit_code/error_classes order).
pub fn run_json(stdout: &str, stderr: &str, exit_code: i64, error_classes: &[String]) -> String {
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

pub fn print_help() {
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

pub fn print_version() {
    println!("{HELEN_VERSION}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_stderr_simple_span() {
        let input = "Error: undefined variable at test.helen:10:5-10";
        let expected = "Error: undefined variable";
        assert_eq!(normalize_stderr(input), expected);
    }

    #[test]
    fn test_normalize_stderr_multiple_spans() {
        let input = "Error at a.helen:1:1-2 and at b.helen:5:3-7";
        let expected = "Error and";
        assert_eq!(normalize_stderr(input), expected);
    }

    #[test]
    fn test_normalize_stderr_no_span() {
        let input = "Error: something went wrong";
        assert_eq!(normalize_stderr(input), input);
    }

    #[test]
    fn test_normalize_stderr_span_with_trailing_colon() {
        let input = "Error at file.helen:10:5-10: extra info";
        let expected = "Error: extra info";
        assert_eq!(normalize_stderr(input), expected);
    }

    #[test]
    fn test_normalize_stderr_empty_string() {
        assert_eq!(normalize_stderr(""), "");
    }

    #[test]
    fn test_normalize_stderr_at_without_span() {
        let input = "Error at some location";
        let expected = "Error at some location";
        assert_eq!(normalize_stderr(input), expected);
    }

    #[test]
    fn test_last_span_end_basic() {
        let input = b":10:5-10";
        assert_eq!(last_span_end(input), Some(8));
    }

    #[test]
    fn test_last_span_end_multiple() {
        let input = b":1:1-2:5:3-7";
        assert_eq!(last_span_end(input), Some(12));
    }

    #[test]
    fn test_last_span_end_no_match() {
        let input = b":not:a:span";
        assert_eq!(last_span_end(input), None);
    }

    #[test]
    fn test_last_span_end_incomplete() {
        let input = b":10:5";
        assert_eq!(last_span_end(input), None);
    }

    #[test]
    fn test_last_span_end_empty() {
        let input = b"";
        assert_eq!(last_span_end(input), None);
    }

    #[test]
    fn test_run_json_success() {
        let result = run_json("hello\n", "", 0, &[]);
        assert_eq!(
            result,
            r#"{"stdout": "hello\n", "stderr": "", "exit_code": 0, "error_classes": []}"#
        );
    }

    #[test]
    fn test_run_json_with_error() {
        let result = run_json("", "Error: something", 1, &["RuntimeError".to_string()]);
        assert_eq!(
            result,
            r#"{"stdout": "", "stderr": "Error: something", "exit_code": 1, "error_classes": ["RuntimeError"]}"#
        );
    }

    #[test]
    fn test_run_json_multiple_errors() {
        let result = run_json(
            "",
            "Error",
            2,
            &["TypeError".to_string(), "ValueError".to_string()],
        );
        assert_eq!(
            result,
            r#"{"stdout": "", "stderr": "Error", "exit_code": 2, "error_classes": ["TypeError", "ValueError"]}"#
        );
    }

    #[test]
    fn test_run_json_special_characters() {
        let result = run_json("line1\nline2\ttab", "quote\"backslash\\", 0, &[]);
        assert!(result.contains(r#""line1\nline2\ttab""#));
        assert!(result.contains(r#""quote\"backslash\\""#));
    }

    #[test]
    fn test_helen_version_constant() {
        assert_eq!(HELEN_VERSION, "1.45.0");
    }
}
