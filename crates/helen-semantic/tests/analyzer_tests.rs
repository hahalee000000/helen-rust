//! Tests for analyzer module — helper functions and utilities.

use helen_semantic::analyzer::{SemanticAnalyzer, analyze_codes, analyze_messages};
use helen_core::ast::*;
use helen_core::source::SourceSpan;

fn dummy_span() -> SourceSpan {
    SourceSpan::new("test.helen", 1, 1, 1, 10)
}

// ── SemanticAnalyzer basic tests ────────────────────────────────────────

#[test]
fn analyzer_new() {
    use helen_semantic::diagnostics::ErrorReporter;
    let reporter = ErrorReporter::new();
    let analyzer = SemanticAnalyzer::new(reporter, ".");
    assert_eq!(analyzer.base_dir, ".");
}

#[test]
fn analyzer_reset() {
    use helen_semantic::diagnostics::ErrorReporter;
    let reporter = ErrorReporter::new();
    let mut analyzer = SemanticAnalyzer::new(reporter, ".");
    analyzer.reset();
    // Should not panic
}

#[test]
fn analyzer_analyze_empty() {
    use helen_semantic::diagnostics::ErrorReporter;
    let reporter = ErrorReporter::new();
    let mut analyzer = SemanticAnalyzer::new(reporter, ".");
    let program = Program {
        statements: vec![],
        span: dummy_span(),
    };
    analyzer.analyze(&program);
    // Should not panic
}

// ── analyze_codes tests ─────────────────────────────────────────────────

#[test]
fn analyze_codes_empty_program() {
    let program = Program {
        statements: vec![],
        span: dummy_span(),
    };
    let codes = analyze_codes(&program);
    assert!(codes.is_empty());
}

// ── analyze_messages tests ──────────────────────────────────────────────

#[test]
fn analyze_messages_empty_program() {
    let program = Program {
        statements: vec![],
        span: dummy_span(),
    };
    let messages = analyze_messages(&program);
    assert!(messages.is_empty());
}
