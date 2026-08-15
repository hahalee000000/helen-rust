//! Compile-time error types (lex / parse / semantic).

use crate::source::SourceSpan;
use std::fmt;

/// A lexical diagnostic. `code` is the Helen error code (e.g. 300 =
/// `SCANNER_ERROR`), matching the reference interpreter's `ErrorCode` enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    code: u32,
    message: String,
    span: SourceSpan,
}

impl LexError {
    pub fn new(code: u32, message: String, span: SourceSpan) -> Self {
        LexError {
            code,
            message,
            span,
        }
    }

    pub fn code(&self) -> u32 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E{}: {}", self.code, self.message)
    }
}

/// A parsing diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    code: u32,
    message: String,
    span: SourceSpan,
}

impl ParseError {
    pub fn new(code: u32, message: String, span: SourceSpan) -> Self {
        ParseError {
            code,
            message,
            span,
        }
    }

    pub fn code(&self) -> u32 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E{}: {}", self.code, self.message)
    }
}

/// A semantic-analysis diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    code: u32,
    message: String,
    span: SourceSpan,
}

impl SemanticError {
    pub fn new(code: u32, message: String, span: SourceSpan) -> Self {
        SemanticError {
            code,
            message,
            span,
        }
    }

    pub fn code(&self) -> u32 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E{}: {}", self.code, self.message)
    }
}

/// Envelope for any compile-phase diagnostic, tagged by phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelenCompileError {
    Lex(LexError),
    Parse(ParseError),
    Semantic(SemanticError),
}

impl HelenCompileError {
    pub fn code(&self) -> u32 {
        match self {
            HelenCompileError::Lex(e) => e.code(),
            HelenCompileError::Parse(e) => e.code(),
            HelenCompileError::Semantic(e) => e.code(),
        }
    }

    pub fn message(&self) -> &str {
        match self {
            HelenCompileError::Lex(e) => e.message(),
            HelenCompileError::Parse(e) => e.message(),
            HelenCompileError::Semantic(e) => e.message(),
        }
    }

    pub fn span(&self) -> SourceSpan {
        match self {
            HelenCompileError::Lex(e) => e.span(),
            HelenCompileError::Parse(e) => e.span(),
            HelenCompileError::Semantic(e) => e.span(),
        }
    }
}

impl fmt::Display for HelenCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HelenCompileError::Lex(e) => write!(f, "{e}"),
            HelenCompileError::Parse(e) => write!(f, "{e}"),
            HelenCompileError::Semantic(e) => write!(f, "{e}"),
        }
    }
}
