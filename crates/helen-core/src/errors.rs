//! Error codes and diagnostics.
//!
//! `ErrorCode` mirrors the Python `helen.core.errors.ErrorCode` enum
//! (E0300–E0357). Phase structs `LexError` / `ParseError` / `SemanticError`
//! render byte-identically to Python's `HelenError.__str__`:
//! `E{value:04d} at {span}: {message}`.

use crate::source::SourceSpan;
use std::fmt;

/// Numeric codes for Helen compilation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // 300-309: lexical/syntax
    ScannerError = 300,
    ParserError = 301,
    UnexpectedToken = 302,
    MissingToken = 303,
    InvalidLiteral = 304,
    InvalidEscape = 305,
    UnterminatedString = 306,
    InvalidIdentifier = 307,
    DeprecatedSyntax = 308,
    ReservedKeyword = 309,
    // 310-320: parser/semantic (Phase 0/1)
    TypeMismatch = 310,
    UndefinedVariable = 311,
    UndefinedFunction = 312,
    DuplicateDeclaration = 313,
    MissingReturn = 314,
    InvalidBreak = 315,
    InvalidContinue = 316,
    MissingDefaultCase = 317,
    AsyncOnNonCall = 318,
    InvalidAgentParam = 319,
    UnterminatedBlock = 320,
    // 330-350: semantic analysis (Phase 2)
    SemanticError = 330,
    SemanticTypeError = 331,
    UndeclaredVariable = 332,
    DuplicateSymbol = 333,
    AgentRuntimeError = 334,
    DuplicateAgentName = 335,
    DuplicateParam = 336,
    MissingPrompt = 337,
    BreakOutsideLoop = 338,
    ContinueOutsideLoop = 339,
    ReturnOutsideFunction = 340,
    ImportNotFound = 341,
    InvalidCatchType = 342,
    CatchAllNotLast = 343,
    LlmIfNoDefault = 344,
    MatchNoDefault = 345,
    ConstAssignment = 346,
    AgentParamMismatch = 347,
    InvalidAgentName = 348,
    MissingDefaultBranch = 349,
    ScopeViolation = 350,
    RuntimeError = 351,
    ImportError = 352,
    InvalidToolsDeclaration = 353,
    BuiltinShadowed = 354,
    TopLevelStatement = 355,
    UndeclaredAgentFunction = 356,
    AgentFunctionArgMismatch = 357,
}

impl ErrorCode {
    /// The numeric value (300..=357), as printed in `E0306` style codes.
    pub fn value(&self) -> u32 {
        *self as u32
    }
}

/// Renders `E{value:04d} at {loc}: {message}` like Python's `HelenError`.
fn fmt_error(
    code: ErrorCode,
    span: &SourceSpan,
    message: &str,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    write!(f, "E{:04} at {}: {}", code.value(), span, message)
}

/// A lexical diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    code: ErrorCode,
    message: String,
    span: SourceSpan,
}

impl LexError {
    pub fn new(code: ErrorCode, message: String, span: SourceSpan) -> Self {
        LexError {
            code,
            message,
            span,
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_error(self.code, &self.span, &self.message, f)
    }
}

/// A parsing diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    code: ErrorCode,
    message: String,
    span: SourceSpan,
}

impl ParseError {
    pub fn new(code: ErrorCode, message: String, span: SourceSpan) -> Self {
        ParseError {
            code,
            message,
            span,
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_error(self.code, &self.span, &self.message, f)
    }
}

/// A semantic-analysis diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    code: ErrorCode,
    message: String,
    span: SourceSpan,
}

impl SemanticError {
    pub fn new(code: ErrorCode, message: String, span: SourceSpan) -> Self {
        SemanticError {
            code,
            message,
            span,
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_error(self.code, &self.span, &self.message, f)
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
    pub fn code(&self) -> ErrorCode {
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

impl std::error::Error for HelenCompileError {}
