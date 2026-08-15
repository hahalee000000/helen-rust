//! Diagnostic collection for semantic analysis.
//!
//! Mirrors Python's `ErrorReporter` (`helen/core/errors.py`): collects
//! `error()` / `warning()` diagnostics with optional spans, and exposes
//! `errors`, `warnings`, `has_errors`.

use helen_core::errors::ErrorCode;
use helen_core::source::SourceSpan;

/// A single diagnostic (error or warning).
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub code: ErrorCode,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl Diagnostic {
    pub fn new(code: ErrorCode, message: String, span: Option<SourceSpan>) -> Self {
        Diagnostic {
            code,
            message,
            span,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.span {
            Some(sp) => write!(f, "E{:04} at {}: {}", self.code.value(), sp, self.message),
            None => write!(
                f,
                "E{:04} at <unknown>: {}",
                self.code.value(),
                self.message
            ),
        }
    }
}

/// Collector for errors and warnings across compilation phases.
#[derive(Debug, Default, Clone)]
pub struct ErrorReporter {
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
}

impl ErrorReporter {
    pub fn new() -> Self {
        ErrorReporter::default()
    }

    /// Record a new error (Python `ErrorReporter.error`).
    pub fn error(&mut self, code: ErrorCode, message: impl Into<String>, span: SourceSpan) {
        self.errors
            .push(Diagnostic::new(code, message.into(), Some(span)));
    }

    /// Record a new error without a span (Python default `span=None`).
    pub fn error_no_span(&mut self, code: ErrorCode, message: impl Into<String>) {
        self.errors
            .push(Diagnostic::new(code, message.into(), None));
    }

    /// Record a new warning (Python `ErrorReporter.warning`).
    pub fn warning(&mut self, code: ErrorCode, message: impl Into<String>, span: SourceSpan) {
        self.warnings
            .push(Diagnostic::new(code, message.into(), Some(span)));
    }

    /// Snapshot of all errors collected so far (Python `errors` property).
    pub fn errors(&self) -> &[Diagnostic] {
        &self.errors
    }

    /// Snapshot of all warnings collected so far (Python `warnings` property).
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }

    /// Number of errors recorded (Python `len(reporter.errors)`).
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Whether any errors have been recorded (Python `has_errors`).
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}
