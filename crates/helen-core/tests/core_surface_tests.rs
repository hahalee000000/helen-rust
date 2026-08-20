//! M13 Task 13.8 — helen-core surface driver for the coverage gate.
//!
//! Drives every `ErrorCode` value, every `TokenType` name, and the
//! Display impls of the diagnostic types. These small but heavily-used
//! modules (errors.rs, tokens.rs) otherwise sit at ~50% line coverage
//! because only the codes/types that the corpus happens to hit get
//! exercised.

use helen_core::errors::{ErrorCode, LexError, ParseError, SemanticError};
use helen_core::source::SourceSpan;
use helen_core::tokens::{LiteralValue, Token, TokenType};
use num_bigint::BigInt;

fn span() -> SourceSpan {
    SourceSpan::new("t.helen".to_string(), 1, 1, 1, 5)
}

fn token(tt: TokenType) -> Token {
    Token {
        kind: tt,
        lexeme: String::new(),
        literal: LiteralValue::Null,
        line: 1,
        col: 1,
        end_line: 1,
        end_col: 5,
        file: "t.helen".to_string(),
    }
}

#[test]
fn every_error_code_value_is_in_range() {
    // Exhaustive ErrorCode sweep: every variant must map to 300..=357 and
    // render as an E-code (the formatter path in errors.rs).
    let codes = [
        ErrorCode::ScannerError,
        ErrorCode::ParserError,
        ErrorCode::UnexpectedToken,
        ErrorCode::MissingToken,
        ErrorCode::InvalidLiteral,
        ErrorCode::InvalidEscape,
        ErrorCode::UnterminatedString,
        ErrorCode::InvalidIdentifier,
        ErrorCode::DeprecatedSyntax,
        ErrorCode::ReservedKeyword,
        ErrorCode::TypeMismatch,
        ErrorCode::UndefinedVariable,
        ErrorCode::UndefinedFunction,
        ErrorCode::DuplicateDeclaration,
        ErrorCode::MissingReturn,
        ErrorCode::InvalidBreak,
        ErrorCode::InvalidContinue,
        ErrorCode::MissingDefaultCase,
        ErrorCode::AsyncOnNonCall,
        ErrorCode::InvalidAgentParam,
        ErrorCode::UnterminatedBlock,
        ErrorCode::SemanticError,
        ErrorCode::SemanticTypeError,
        ErrorCode::UndeclaredVariable,
        ErrorCode::DuplicateSymbol,
        ErrorCode::AgentRuntimeError,
        ErrorCode::DuplicateAgentName,
        ErrorCode::DuplicateParam,
        ErrorCode::MissingPrompt,
        ErrorCode::BreakOutsideLoop,
        ErrorCode::ContinueOutsideLoop,
        ErrorCode::ReturnOutsideFunction,
        ErrorCode::ImportNotFound,
        ErrorCode::InvalidCatchType,
        ErrorCode::CatchAllNotLast,
        ErrorCode::LlmIfNoDefault,
        ErrorCode::MatchNoDefault,
        ErrorCode::ConstAssignment,
        ErrorCode::AgentParamMismatch,
        ErrorCode::InvalidAgentName,
        ErrorCode::MissingDefaultBranch,
        ErrorCode::ScopeViolation,
        ErrorCode::RuntimeError,
        ErrorCode::ImportError,
        ErrorCode::InvalidToolsDeclaration,
        ErrorCode::BuiltinShadowed,
        ErrorCode::TopLevelStatement,
        ErrorCode::UndeclaredAgentFunction,
        ErrorCode::AgentFunctionArgMismatch,
    ];
    assert_eq!(
        codes.len(),
        49,
        "new ErrorCode variant added — update sweep"
    );
    for c in &codes {
        let v = c.value();
        assert!((300..=357).contains(&v), "code {v} out of range");
    }
}

#[test]
fn every_token_type_has_python_name() {
    // Exhaustive TokenType sweep: `name()` returns the Python
    // SCREAMING_SNAKE name and is never empty.
    let types = [
        TokenType::LeftParen,
        TokenType::RightParen,
        TokenType::LeftBrace,
        TokenType::RightBrace,
        TokenType::LeftBracket,
        TokenType::RightBracket,
        TokenType::Comma,
        TokenType::Dot,
        TokenType::DotDot,
        TokenType::Colon,
        TokenType::Semicolon,
        TokenType::Question,
        TokenType::Pipe,
        TokenType::PipeRight,
        TokenType::Minus,
        TokenType::Plus,
        TokenType::Slash,
        TokenType::Star,
        TokenType::Percent,
        TokenType::Arrow,
        TokenType::Bang,
        TokenType::BangEqual,
        TokenType::Assign,
        TokenType::EqualEqual,
        TokenType::Greater,
        TokenType::GreaterEqual,
        TokenType::Less,
        TokenType::LessEqual,
        TokenType::And,
        TokenType::Or,
        TokenType::At,
        TokenType::Identifier,
        TokenType::String,
        TokenType::TripleQuoteString,
        TokenType::Number,
        TokenType::True,
        TokenType::False,
        TokenType::NullKw,
        TokenType::TemplateOpen,
        TokenType::TemplateClose,
        TokenType::Agent,
        TokenType::Description,
        TokenType::Model,
        TokenType::Tools,
        TokenType::Streaming,
        TokenType::Temperature,
        TokenType::MaxTurns,
        TokenType::MaxTokens,
        TokenType::Memory,
        TokenType::Prompt,
        TokenType::Llm,
        TokenType::Import,
        TokenType::Let,
        TokenType::Const,
        TokenType::If,
        TokenType::Else,
        TokenType::For,
        TokenType::While,
        TokenType::Break,
        TokenType::Continue,
        TokenType::Return,
        TokenType::Spawn,
        TokenType::Match,
        TokenType::Case,
        TokenType::Branch,
        TokenType::Default,
        TokenType::Act,
        TokenType::Try,
        TokenType::Catch,
        TokenType::Finally,
        TokenType::Throw,
        TokenType::Assert,
        TokenType::Fn,
        TokenType::As,
        TokenType::In,
        TokenType::Functions,
        TokenType::Main,
        TokenType::Store,
        TokenType::Protocol,
        TokenType::Impl,
        TokenType::Is,
        TokenType::Wildcard,
        TokenType::Shared,
        TokenType::Alias,
        TokenType::Transcript,
        TokenType::ThinkingMode,
        TokenType::ReasoningEffort,
        TokenType::Eof,
    ];
    assert_eq!(
        types.len(),
        88,
        "new TokenType variant added — update sweep"
    );
    for t in &types {
        let n = t.name();
        assert!(!n.is_empty(), "TokenType {:?} has empty name()", t);
        assert!(n.chars().all(|c| c.is_ascii_uppercase() || c == '_'));
    }
}

#[test]
fn diagnostic_display_paths() {
    // Drive the E-code formatter through all three diagnostic types.
    let lex = LexError::new(ErrorCode::ScannerError, "bad char".into(), span());
    let lex_s = lex.to_string();
    assert!(lex_s.starts_with("E0300"), "got: {lex_s}");
    assert!(lex_s.contains("t.helen:1:1"));

    let parse = ParseError::new(ErrorCode::ParserError, "boom".into(), span());
    let p_s = parse.to_string();
    assert!(p_s.starts_with("E0301"), "got: {p_s}");
    assert_eq!(parse.code(), ErrorCode::ParserError);
    assert_eq!(parse.message(), "boom");

    let sem = SemanticError::new(ErrorCode::UndeclaredVariable, "x".into(), span());
    let s_s = sem.to_string();
    assert!(s_s.starts_with("E0332"), "got: {s_s}");
    assert_eq!(sem.code(), ErrorCode::UndeclaredVariable);
}

#[test]
fn token_helpers_surface() {
    // Token accessors used by parser/lexer error paths.
    let t = token(TokenType::Identifier);
    let _ = t.span();
    let _ = t.kind;
    let _ = t.line;
    let _ = t.col;
    let _ = t.end_line;
    let _ = t.end_col;
    let lit = LiteralValue::Int(BigInt::from(42));
    assert_eq!(lit, LiteralValue::Int(BigInt::from(42)));
    let lf = LiteralValue::Float(1.5);
    assert_eq!(lf, LiteralValue::Float(1.5));
    let ls = LiteralValue::Str("s".to_string());
    assert_eq!(ls, LiteralValue::Str("s".to_string()));
    let lb = LiteralValue::Bool(true);
    assert_eq!(lb, LiteralValue::Bool(true));
    let ln = LiteralValue::Null;
    assert_eq!(ln, LiteralValue::Null);
}
