//! Contract tests for helen-core primitives (M0 Task 0.2, M1-faithful).
//!
//! Pins the Python-faithful shapes: `SourceSpan{file, start_line,
//! start_col, end_line, end_col}` (1-based, end_col exclusive) and the
//! `ErrorCode` enum (E0300..E0357). Display formats are byte-identical to
//! the Python `SourceSpan.__str__` / `HelenError.__str__`.

use helen_core::errors::{ErrorCode, LexError, ParseError, SemanticError};
use helen_core::source::SourceSpan;

fn span() -> SourceSpan {
    SourceSpan::new("test.helen", 1, 1, 1, 5)
}

#[test]
fn source_span_single_line_display() {
    assert_eq!(span().to_string(), "test.helen:1:1-5");
}

#[test]
fn source_span_multi_line_display() {
    let s = SourceSpan::new("main.hl", 1, 1, 3, 2);
    assert_eq!(s.to_string(), "main.hl:1:1-3:2");
}

#[test]
fn source_span_contains() {
    let s = span();
    assert!(s.contains(1, 3));
    assert!(!s.contains(1, 5)); // end_col exclusive
    assert!(!s.contains(2, 1)); // past end_line
    assert!(!s.contains(1, 0)); // before start_col
}

#[test]
fn error_code_values_match_python() {
    assert_eq!(ErrorCode::ScannerError.value(), 300);
    assert_eq!(ErrorCode::UnterminatedString.value(), 306);
    assert_eq!(ErrorCode::UndeclaredVariable.value(), 332);
    assert_eq!(ErrorCode::AgentFunctionArgMismatch.value(), 357);
}

#[test]
fn lex_error_display_matches_python() {
    let e = LexError::new(
        ErrorCode::UnterminatedString,
        "Unterminated string literal".to_string(),
        span(),
    );
    assert_eq!(
        e.to_string(),
        "E0306 at test.helen:1:1-5: Unterminated string literal"
    );
    assert_eq!(e.code(), ErrorCode::UnterminatedString);
    assert_eq!(e.message(), "Unterminated string literal");
    assert_eq!(e.span(), span());
}

#[test]
fn parse_error_display_matches_python() {
    let e = ParseError::new(
        ErrorCode::UnexpectedToken,
        "Expected expression, got EOF".to_string(),
        span(),
    );
    assert_eq!(
        e.to_string(),
        "E0302 at test.helen:1:1-5: Expected expression, got EOF"
    );
}

#[test]
fn semantic_error_display_matches_python() {
    let e = SemanticError::new(
        ErrorCode::UndeclaredVariable,
        "undeclared variable 'x'".to_string(),
        span(),
    );
    assert_eq!(
        e.to_string(),
        "E0332 at test.helen:1:1-5: undeclared variable 'x'"
    );
}
