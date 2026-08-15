//! Contract tests for `lexer.rs` (M1 Task 1.2).
//!
//! Mirrors helen/core/lexer.py behavior: maximal munch, bilingual keywords,
//! fullwidth operators, strings (ASCII / Chinese quotes / triple-quoted),
//! numbers with underscores/exponents, templates, comments, and error codes.

use helen_core::errors::ErrorCode;
use helen_core::lexer::Scanner;
use helen_core::tokens::{LiteralValue, TokenType};

type TokPair = (TokenType, String);
type ErrPair = (ErrorCode, String);

fn scan(src: &str) -> (Vec<TokPair>, Vec<ErrPair>) {
    let mut s = Scanner::new(src, "<test>");
    let tokens = s.scan_all();
    let pairs: Vec<TokPair> = tokens.iter().map(|t| (t.kind, t.lexeme.clone())).collect();
    let errs: Vec<ErrPair> = s
        .errors()
        .iter()
        .map(|e| (e.code(), e.message().to_string()))
        .collect();
    (pairs, errs)
}

#[test]
fn scan_basic_let() {
    let (toks, errs) = scan("let x = 42");
    assert!(errs.is_empty());
    assert_eq!(
        toks,
        vec![
            (TokenType::Let, "let".into()),
            (TokenType::Identifier, "x".into()),
            (TokenType::Assign, "=".into()),
            (TokenType::Number, "42".into()),
            (TokenType::Eof, "".into()),
        ]
    );
}

#[test]
fn scan_chinese_keyword_and_identifier() {
    let (toks, errs) = scan("设 变量 = 1");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Let);
    assert_eq!(toks[0].1, "设");
    assert_eq!(toks[1].0, TokenType::Identifier);
    assert_eq!(toks[1].1, "变量");
}

#[test]
fn scan_bilingual_agent_keywords() {
    let (toks, _) = scan("agent 智能体 分生");
    assert_eq!(toks[0].0, TokenType::Agent);
    assert_eq!(toks[1].0, TokenType::Agent);
    assert_eq!(toks[2].0, TokenType::Spawn);
}

#[test]
fn scan_hyphenated_keywords() {
    let (toks, _) = scan("max-turns thinking-mode max-tokens");
    assert_eq!(toks[0].0, TokenType::MaxTurns);
    assert_eq!(toks[1].0, TokenType::ThinkingMode);
    assert_eq!(toks[2].0, TokenType::MaxTokens);
    // "max-" followed by non-alpha is NOT a keyword
    let (toks2, _) = scan("max-1");
    assert_eq!(toks2[0].0, TokenType::Identifier);
}

#[test]
fn scan_wildcard_underscore() {
    let (toks, _) = scan("_");
    assert_eq!(toks[0].0, TokenType::Wildcard);
}

#[test]
fn scan_operators() {
    let (toks, errs) = scan("== != >= <= && || |> -> .. ! - + * / %");
    assert!(errs.is_empty());
    let kinds: Vec<TokenType> = toks.iter().map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![
            TokenType::EqualEqual,
            TokenType::BangEqual,
            TokenType::GreaterEqual,
            TokenType::LessEqual,
            TokenType::And,
            TokenType::Or,
            TokenType::PipeRight,
            TokenType::Arrow,
            TokenType::DotDot,
            TokenType::Bang,
            TokenType::Minus,
            TokenType::Plus,
            TokenType::Star,
            TokenType::Slash,
            TokenType::Percent,
            TokenType::Eof,
        ]
    );
}

#[test]
fn scan_fullwidth_operators() {
    let (toks, errs) = scan("！＝ ＝＝ ＞＝ ＜＝ ｜＞ －＞ ．．");
    assert!(errs.is_empty());
    let kinds: Vec<TokenType> = toks.iter().map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![
            TokenType::BangEqual,
            TokenType::EqualEqual,
            TokenType::GreaterEqual,
            TokenType::LessEqual,
            TokenType::PipeRight,
            TokenType::Arrow,
            TokenType::DotDot,
            TokenType::Eof,
        ]
    );
}

#[test]
fn scan_string_with_escapes() {
    // Python reference src: '"a\\nb\\tc\\\\d\\""' (14 chars, real closing quote)
    let src = "\"a\\nb\\tc\\\\d\\\"\"";
    let mut s = Scanner::new(src, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty(), "{:?}", s.errors());
    assert_eq!(toks[0].kind, TokenType::String);
    assert_eq!(toks[0].lexeme, "\"a\\nb\\tc\\\\d\\\"\"");
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "a\nb\tc\\d\""),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_hex_and_unicode_escapes() {
    let mut s = Scanner::new(r#""\x1b\u4e00""#, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "\u{1b}\u{4e00}"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_chinese_quoted_string() {
    let mut s = Scanner::new("「你好」", "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    assert_eq!(toks[0].kind, TokenType::String);
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "你好"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_triple_quoted_dedent() {
    // Python: '"""\n    line1\n    line2\n    """' → literal '\nline1\nline2\n'
    let src = "\"\"\"\n    line1\n    line2\n    \"\"\"";
    let mut s = Scanner::new(src, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty(), "{:?}", s.errors());
    assert_eq!(toks[0].kind, TokenType::TripleQuoteString);
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "\nline1\nline2\n"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_numbers() {
    let mut s = Scanner::new("42 2.5 1e20 1_000_000 .5 42.", "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    let nums: Vec<&LiteralValue> = toks.iter().take(6).map(|t| &t.literal).collect();
    assert!(matches!(nums[0], LiteralValue::Int(v) if v.to_string() == "42"));
    assert!(matches!(nums[1], LiteralValue::Float(v) if (*v - 2.5).abs() < 1e-12));
    assert!(matches!(nums[2], LiteralValue::Float(v) if (*v - 1e20).abs() < 1e5));
    assert!(matches!(nums[3], LiteralValue::Int(v) if v.to_string() == "1000000"));
    assert!(matches!(nums[4], LiteralValue::Float(v) if (*v - 0.5).abs() < 1e-12));
    assert!(matches!(nums[5], LiteralValue::Float(v) if (*v - 42.0).abs() < 1e-12));
}

#[test]
fn scan_big_int_literal() {
    let mut s = Scanner::new("99999999999999999999999999", "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Int(v) => assert_eq!(v.to_string(), "99999999999999999999999999"),
        other => panic!("expected Int, got {other:?}"),
    }
}

#[test]
fn scan_templates() {
    let (toks, errs) = scan("{{ x }}");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::TemplateOpen);
    assert_eq!(toks[0].1, "{{");
    assert_eq!(toks[1].0, TokenType::Identifier);
    assert_eq!(toks[2].0, TokenType::TemplateClose);
    assert_eq!(toks[2].1, "}}");
}

#[test]
fn scan_comments() {
    let (toks, errs) = scan("a // line comment\nb /* block /* nested */ */ c");
    assert!(errs.is_empty());
    let kinds: Vec<TokenType> = toks.iter().map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![
            TokenType::Identifier,
            TokenType::Identifier,
            TokenType::Identifier,
            TokenType::Eof,
        ]
    );
}

#[test]
fn scan_unterminated_string_reports_e0306() {
    let (_, errs) = scan("\"abc");
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].0, ErrorCode::UnterminatedString);
    assert_eq!(errs[0].1, "Unterminated string literal");
}

#[test]
fn scan_unterminated_block_comment_reports_e0300() {
    let (_, errs) = scan("/* never closed");
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].0, ErrorCode::ScannerError);
    assert_eq!(errs[0].1, "Unterminated block comment");
}

#[test]
fn scan_lone_ampersand_reports_e0300() {
    let (_, errs) = scan("a & b");
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].0, ErrorCode::ScannerError);
    assert_eq!(errs[0].1, "Unexpected character: '&'. Did you mean '&&'?");
}

#[test]
fn scan_invalid_escape_reports_e0305() {
    let (_, errs) = scan(r#""\q""#);
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].0, ErrorCode::InvalidEscape);
    assert_eq!(errs[0].1, "Invalid escape sequence: '\\q'");
}

#[test]
fn scan_line_col_tracking() {
    let mut s = Scanner::new("a\nbb", "<test>");
    let toks = s.scan_all();
    assert_eq!((toks[0].line, toks[0].col), (1, 1));
    assert_eq!((toks[1].line, toks[1].col), (2, 1));
    assert_eq!((toks[1].end_line, toks[1].end_col), (2, 3));
}

#[test]
fn scan_eof_token_has_empty_lexeme() {
    let mut s = Scanner::new("", "<test>");
    let toks = s.scan_all();
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, TokenType::Eof);
    assert_eq!(toks[0].lexeme, "");
}
