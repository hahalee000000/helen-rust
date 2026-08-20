//! Tests for errors module — ErrorCode, LexError, ParseError, SemanticError.

use helen_core::errors::*;
use helen_core::source::SourceSpan;

fn dummy_span() -> SourceSpan {
    SourceSpan::new("test.helen", 1, 1, 1, 10)
}

// ── ErrorCode tests ─────────────────────────────────────────────────────

#[test]
fn error_code_value_scanner() {
    assert_eq!(ErrorCode::ScannerError.value(), 300);
}

#[test]
fn error_code_value_parser() {
    assert_eq!(ErrorCode::ParserError.value(), 301);
}

#[test]
fn error_code_value_unexpected_token() {
    assert_eq!(ErrorCode::UnexpectedToken.value(), 302);
}

#[test]
fn error_code_value_missing_token() {
    assert_eq!(ErrorCode::MissingToken.value(), 303);
}

#[test]
fn error_code_value_invalid_literal() {
    assert_eq!(ErrorCode::InvalidLiteral.value(), 304);
}

#[test]
fn error_code_value_invalid_escape() {
    assert_eq!(ErrorCode::InvalidEscape.value(), 305);
}

#[test]
fn error_code_value_unterminated_string() {
    assert_eq!(ErrorCode::UnterminatedString.value(), 306);
}

#[test]
fn error_code_value_type_mismatch() {
    assert_eq!(ErrorCode::TypeMismatch.value(), 310);
}

#[test]
fn error_code_value_undefined_variable() {
    assert_eq!(ErrorCode::UndefinedVariable.value(), 311);
}

#[test]
fn error_code_value_undefined_function() {
    assert_eq!(ErrorCode::UndefinedFunction.value(), 312);
}

#[test]
fn error_code_value_duplicate_declaration() {
    assert_eq!(ErrorCode::DuplicateDeclaration.value(), 313);
}

#[test]
fn error_code_value_semantic_error() {
    assert_eq!(ErrorCode::SemanticError.value(), 330);
}

#[test]
fn error_code_value_runtime_error() {
    assert_eq!(ErrorCode::RuntimeError.value(), 351);
}

#[test]
fn error_code_value_import_error() {
    assert_eq!(ErrorCode::ImportError.value(), 352);
}

#[test]
fn error_code_clone() {
    let code = ErrorCode::ScannerError;
    let cloned = code;
    assert_eq!(code, cloned);
}

#[test]
fn error_code_debug() {
    let code = ErrorCode::ParserError;
    let debug = format!("{:?}", code);
    assert!(debug.contains("ParserError"));
}

// ── LexError tests ──────────────────────────────────────────────────────

#[test]
fn lex_error_new() {
    let err = LexError::new(ErrorCode::ScannerError, "bad token".into(), dummy_span());
    assert_eq!(err.code(), ErrorCode::ScannerError);
    assert_eq!(err.message(), "bad token");
}

#[test]
fn lex_error_display() {
    let err = LexError::new(
        ErrorCode::UnterminatedString,
        "unterminated".into(),
        dummy_span(),
    );
    let display = format!("{err}");
    assert!(display.contains("E0306"));
    assert!(display.contains("unterminated"));
}

#[test]
fn lex_error_span() {
    let span = dummy_span();
    let err = LexError::new(ErrorCode::ScannerError, "msg".into(), span.clone());
    assert_eq!(err.span(), span);
}

// ── ParseError tests ────────────────────────────────────────────────────

#[test]
fn parse_error_new() {
    let err = ParseError::new(
        ErrorCode::ParserError,
        "unexpected token".into(),
        dummy_span(),
    );
    assert_eq!(err.code(), ErrorCode::ParserError);
    assert_eq!(err.message(), "unexpected token");
}

#[test]
fn parse_error_display() {
    let err = ParseError::new(ErrorCode::MissingToken, "expected ;".into(), dummy_span());
    let display = format!("{err}");
    assert!(display.contains("E0303"));
    assert!(display.contains("expected ;"));
}

#[test]
fn parse_error_span() {
    let span = dummy_span();
    let err = ParseError::new(ErrorCode::ParserError, "msg".into(), span.clone());
    assert_eq!(err.span(), span);
}

// ── SemanticError tests ─────────────────────────────────────────────────

#[test]
fn semantic_error_new() {
    let err = SemanticError::new(
        ErrorCode::SemanticError,
        "type mismatch".into(),
        dummy_span(),
    );
    assert_eq!(err.code(), ErrorCode::SemanticError);
    assert_eq!(err.message(), "type mismatch");
}

#[test]
fn semantic_error_display() {
    let err = SemanticError::new(
        ErrorCode::UndeclaredVariable,
        "x not defined".into(),
        dummy_span(),
    );
    let display = format!("{err}");
    assert!(display.contains("E0332"));
    assert!(display.contains("x not defined"));
}

#[test]
fn semantic_error_span() {
    let span = dummy_span();
    let err = SemanticError::new(ErrorCode::SemanticError, "msg".into(), span.clone());
    assert_eq!(err.span(), span);
}

// ── Error formatting tests ──────────────────────────────────────────────

#[test]
fn error_format_includes_code() {
    let err = LexError::new(ErrorCode::InvalidEscape, "bad escape".into(), dummy_span());
    let display = format!("{err}");
    assert!(display.contains("E0305"));
}

#[test]
fn error_format_includes_message() {
    let err = ParseError::new(ErrorCode::UnexpectedToken, "got +".into(), dummy_span());
    let display = format!("{err}");
    assert!(display.contains("got +"));
}

#[test]
fn error_format_includes_location() {
    let err = SemanticError::new(ErrorCode::TypeMismatch, "wrong type".into(), dummy_span());
    let display = format!("{err}");
    assert!(display.contains("test.helen"));
}

// ── All error codes have values ─────────────────────────────────────────

#[test]
fn all_error_codes_have_values() {
    let codes = vec![
        ErrorCode::ScannerError,
        ErrorCode::ParserError,
        ErrorCode::UnexpectedToken,
        ErrorCode::MissingToken,
        ErrorCode::InvalidLiteral,
        ErrorCode::InvalidEscape,
        ErrorCode::UnterminatedString,
        ErrorCode::TypeMismatch,
        ErrorCode::UndefinedVariable,
        ErrorCode::UndefinedFunction,
        ErrorCode::DuplicateDeclaration,
        ErrorCode::SemanticError,
        ErrorCode::RuntimeError,
    ];
    for code in codes {
        assert!(code.value() >= 300);
        assert!(code.value() <= 400);
    }
}
