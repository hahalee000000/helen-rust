//! Error/Warning report formatter (HLD 3.11.2) — port of `cli/formatter.py`.
//!
//! Format:
//! ```text
//! Error: [ERR_CODE] description
//!   --> file:line:column
//!    |
//! line |     source line
//!    |     ^^^^ position indicator
//!    |
//!   = detail and suggestion
//! ```

use helen_semantic::Diagnostic;

/// Format a semantic/parse diagnostic as an `Error:` report.
pub fn format_error(error: &Diagnostic, source_lines: Option<&[String]>) -> String {
    format_diagnostic("Error", error, source_lines)
}

/// Format a diagnostic as a `Warning:` report.
#[allow(dead_code)] // used by LSP diagnostics in the helen-lsp crate
pub fn format_warning(warning: &Diagnostic, source_lines: Option<&[String]>) -> String {
    format_diagnostic("Warning", warning, source_lines)
}

/// Port of Python `_format_diagnostic(label, diagnostic, source_lines)`.
fn format_diagnostic(
    label: &str,
    diagnostic: &Diagnostic,
    source_lines: Option<&[String]>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Header: `Error: [E0301] first line of message` (label[0] = E|W).
    let code = diagnostic.code.value();
    let label_char = if label == "Warning" { 'W' } else { 'E' };
    let first_line = diagnostic.message.split('\n').next().unwrap_or("").trim();
    parts.push(format!("{label}: [{label_char}{code:04}] {first_line}"));

    if let Some(span) = &diagnostic.span {
        // Location: `  --> file:line:column`
        parts.push(format!(
            "  --> {}:{}:{}",
            span.file, span.start_line, span.start_col
        ));

        // Source context
        if let Some(lines) = source_lines {
            let sl = span.start_line as usize;
            if sl >= 1 && sl <= lines.len() {
                let line_text = &lines[sl - 1];
                parts.push("   |".to_string());
                parts.push(format!("{} | {line_text}", span.start_line));

                // Position indicator: caret_start = col-1 (0-based);
                // caret_end = end_col-1, or start_col when end==start.
                let caret_start = span.start_col.saturating_sub(1) as usize;
                let caret_end = if span.end_col > span.start_col {
                    (span.end_col - 1) as usize
                } else {
                    caret_start + 1
                };
                let width = (caret_end - caret_start).max(1);
                parts.push(format!(
                    "   | {}{}",
                    " ".repeat(caret_start),
                    "^".repeat(width)
                ));
                parts.push("   |".to_string());
            }
        }
    }

    // Detail: multi-line messages show the remaining lines; single-line
    // messages get the `  = message` prefix.
    if diagnostic.message.contains('\n') {
        let lines: Vec<&str> = diagnostic.message.split('\n').collect();
        let remaining = lines[1..].join("\n");
        let remaining = remaining.trim_start_matches('\n');
        if !remaining.trim().is_empty() {
            parts.push(remaining.to_string());
        }
    } else {
        parts.push(format!("  = {}", diagnostic.message));
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use helen_core::errors::ErrorCode;
    use helen_core::source::SourceSpan;

    fn diag(code: ErrorCode, message: &str, span: Option<SourceSpan>) -> Diagnostic {
        Diagnostic::new(code, message.to_string(), span)
    }

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_error_with_span() {
        let span = SourceSpan::new("test.helen", 5, 10, 5, 15);
        let error = diag(ErrorCode::ParserError, "unexpected token", Some(span));
        let source = lines(&[
            "agent Test {",
            "  main {",
            "    let x = 1;",
            "    let y = ;",
            "    let z = ;",
        ]);
        let result = format_error(&error, Some(&source));
        assert!(result.contains("Error:"), "{result}");
        assert!(result.contains("E0301"), "{result}");
        assert!(result.contains("unexpected token"), "{result}");
        assert!(result.contains("test.helen:5:10"), "{result}");
        assert!(result.contains("let z = ;"), "{result}");
        assert!(result.contains('^'), "{result}");
    }

    #[test]
    fn test_error_without_span() {
        let error = diag(ErrorCode::ScannerError, "illegal character", None);
        let result = format_error(&error, None);
        assert!(result.contains("Error:"), "{result}");
        assert!(result.contains("E0300"), "{result}");
        assert!(result.contains("illegal character"), "{result}");
        // No location line
        assert!(!result.contains("-->"), "{result}");
    }

    #[test]
    fn test_error_includes_detail_line() {
        let span = SourceSpan::new("test.helen", 1, 1, 1, 5);
        let error = diag(
            ErrorCode::UnexpectedToken,
            "expected identifier",
            Some(span),
        );
        let result = format_error(&error, None);
        assert!(result.contains("= expected identifier"), "{result}");
    }

    #[test]
    fn test_warning_with_span() {
        let span = SourceSpan::new("test.helen", 3, 5, 3, 10);
        let warning = diag(ErrorCode::DeprecatedSyntax, "deprecated syntax", Some(span));
        let source = lines(&["agent Test {", "  main {", "    let old = 1;"]);
        let result = format_warning(&warning, Some(&source));
        assert!(result.contains("Warning:"), "{result}");
        assert!(result.contains("W0308"), "{result}");
        assert!(result.contains("deprecated syntax"), "{result}");
        assert!(result.contains("test.helen:3:5"), "{result}");
        assert!(result.contains("let old = 1;"), "{result}");
    }

    #[test]
    fn test_caret_underlines_error_position() {
        let span = SourceSpan::new("test.helen", 1, 5, 1, 10);
        let error = diag(ErrorCode::ParserError, "bad syntax", Some(span));
        let source = lines(&["let abc = def;"]);
        let result = format_error(&error, Some(&source));
        assert!(result.contains('^'), "{result}");
        // Caret width = end_col-1 - (start_col-1) = 9-4 = 5
        let caret_line = result.lines().find(|l| l.contains('^')).unwrap();
        assert_eq!(caret_line.chars().filter(|c| *c == '^').count(), 5);
    }
}
