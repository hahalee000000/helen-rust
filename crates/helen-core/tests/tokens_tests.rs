//! Contract tests for `tokens.rs` (M1 Task 1.1).
//!
//! Pins the 88-variant `TokenType`, the Python-faithful `name()` strings,
//! the `Token` struct, and the 99-entry bilingual keyword map.

use helen_core::tokens::{keyword_type, keyword_type_count, LiteralValue, Token, TokenType};

#[test]
fn token_type_names_match_python_enum() {
    // Spot-check every category of the 88-variant enum.
    assert_eq!(TokenType::LeftParen.name(), "LEFT_PAREN");
    assert_eq!(TokenType::PipeRight.name(), "PIPE_RIGHT");
    assert_eq!(TokenType::Arrow.name(), "ARROW");
    assert_eq!(TokenType::BangEqual.name(), "BANG_EQUAL");
    assert_eq!(TokenType::TemplateOpen.name(), "TEMPLATE_OPEN");
    assert_eq!(TokenType::TripleQuoteString.name(), "TRIPLE_QUOTE_STRING");
    assert_eq!(TokenType::ReasoningEffort.name(), "REASONING_EFFORT");
    assert_eq!(TokenType::Eof.name(), "EOF");
}

#[test]
fn keyword_map_resolves_english_keywords() {
    assert_eq!(keyword_type("agent"), Some(TokenType::Agent));
    assert_eq!(keyword_type("let"), Some(TokenType::Let));
    assert_eq!(keyword_type("const"), Some(TokenType::Const));
    assert_eq!(keyword_type("spawn"), Some(TokenType::Spawn));
    assert_eq!(keyword_type("true"), Some(TokenType::True));
    assert_eq!(keyword_type("false"), Some(TokenType::False));
    assert_eq!(keyword_type("null"), Some(TokenType::NullKw));
    assert_eq!(keyword_type("shared"), Some(TokenType::Shared));
    assert_eq!(keyword_type("alias"), Some(TokenType::Alias));
}

#[test]
fn keyword_map_resolves_hyphenated_keywords() {
    assert_eq!(keyword_type("max-turns"), Some(TokenType::MaxTurns));
    assert_eq!(keyword_type("max-tokens"), Some(TokenType::MaxTokens));
    assert_eq!(keyword_type("thinking-mode"), Some(TokenType::ThinkingMode));
    assert_eq!(
        keyword_type("reasoning-effort"),
        Some(TokenType::ReasoningEffort)
    );
}

#[test]
fn keyword_map_resolves_chinese_keywords() {
    assert_eq!(keyword_type("设"), Some(TokenType::Let));
    assert_eq!(keyword_type("定义"), Some(TokenType::Let));
    assert_eq!(keyword_type("智能体"), Some(TokenType::Agent));
    assert_eq!(keyword_type("分生"), Some(TokenType::Spawn));
    assert_eq!(keyword_type("如果"), Some(TokenType::If));
    assert_eq!(keyword_type("返回"), Some(TokenType::Return));
    assert_eq!(keyword_type("真"), Some(TokenType::True));
    assert_eq!(keyword_type("假"), Some(TokenType::False));
    assert_eq!(keyword_type("空"), Some(TokenType::NullKw));
    assert_eq!(keyword_type("且"), Some(TokenType::And));
    assert_eq!(keyword_type("或"), Some(TokenType::Or));
    assert_eq!(keyword_type("仓库"), Some(TokenType::Store));
    assert_eq!(keyword_type("思考模式"), Some(TokenType::ThinkingMode));
    assert_eq!(keyword_type("推理强度"), Some(TokenType::ReasoningEffort));
}

#[test]
fn keyword_map_size_is_99() {
    // Verified against helen/core/tokens.py `_KEYWORD_MAP` (v1.44.0).
    // The Python source comment says "97" but the dict actually has 99 entries.
    assert_eq!(keyword_type_count(), 99);
}

#[test]
fn keyword_map_rejects_non_keywords() {
    assert_eq!(keyword_type("identifier"), None);
    assert_eq!(keyword_type("foo"), None);
    assert_eq!(keyword_type("MEMORY"), None); // context keyword, not in map
    assert_eq!(keyword_type(""), None);
}

#[test]
fn token_span_derived_from_position() {
    let tok = Token {
        kind: TokenType::Number,
        lexeme: "42".to_string(),
        literal: LiteralValue::Int(42.into()),
        line: 1,
        col: 5,
        end_line: 1,
        end_col: 7,
        file: "x.helen".to_string(),
    };
    let sp = tok.span();
    assert_eq!(sp.file, "x.helen");
    assert_eq!(
        (sp.start_line, sp.start_col, sp.end_line, sp.end_col),
        (1, 5, 1, 7)
    );
}
