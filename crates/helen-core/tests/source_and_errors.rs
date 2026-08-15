//! Contract tests for helen-core primitives (M0 Task 0.2).
//!
//! These tests pin the shape of the two types every other crate depends on:
//! `SourceSpan` (positioning) and `HelenCompileError` (diagnostic envelope).
//! The display format mirrors the Python CLI's `E<code>: message` style and
//! `line:col` spans so the differential harness can normalize both sides.

use helen_core::errors::{HelenCompileError, LexError, ParseError, SemanticError};
use helen_core::source::SourceSpan;

fn span() -> SourceSpan {
    SourceSpan::new(0, 5, 12, 34)
}

#[test]
fn source_span_displays_as_line_col() {
    assert_eq!(span().to_string(), "12:34");
}

#[test]
fn source_span_fields_are_accessible() {
    let s = span();
    assert_eq!((s.start, s.end, s.line, s.col), (0, 5, 12, 34));
}

#[test]
fn lex_error_displays_ecode_and_message() {
    let e = LexError::new(300, "unexpected character '#'".to_string(), span());
    assert_eq!(e.to_string(), "E300: unexpected character '#'");
    assert_eq!(e.code(), 300);
    assert_eq!(e.message(), "unexpected character '#'");
    assert_eq!(e.span(), span());
}

#[test]
fn parse_error_displays_ecode_and_message() {
    let e = ParseError::new(302, "expected expression".to_string(), span());
    assert_eq!(e.to_string(), "E302: expected expression");
}

#[test]
fn semantic_error_displays_ecode_and_message() {
    let e = SemanticError::new(332, "undeclared variable 'x'".to_string(), span());
    assert_eq!(e.to_string(), "E332: undeclared variable 'x'");
}

#[test]
fn compile_error_wraps_and_delegates() {
    let e = HelenCompileError::Lex(LexError::new(305, "invalid escape".to_string(), span()));
    assert_eq!(e.code(), 305);
    assert_eq!(e.message(), "invalid escape");
    assert_eq!(e.span(), span());
    assert!(e.to_string().contains("E305"));
}

#[test]
fn compile_error_variants_cover_all_phases() {
    let lex = HelenCompileError::Lex(LexError::new(300, "a".into(), span()));
    let parse = HelenCompileError::Parse(ParseError::new(301, "b".into(), span()));
    let sem = HelenCompileError::Semantic(SemanticError::new(333, "c".into(), span()));
    for (e, expected_code) in [(lex, 300), (parse, 301), (sem, 333)] {
        assert_eq!(e.code(), expected_code);
    }
}
