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

// Phase 3: Additional lexer tests for comprehensive coverage

#[test]
fn scan_all_keywords() {
    let keywords = "let fn if else while for return break continue agent spawn import as try catch throw match case default true false null";
    let (toks, errs) = scan(keywords);
    assert!(errs.is_empty());
    // Just verify we got the right number of tokens (excluding EOF)
    assert!(toks.len() >= 22);
    assert_eq!(toks[0].0, TokenType::Let);
    assert_eq!(toks[1].0, TokenType::Fn);
    assert_eq!(toks[2].0, TokenType::If);
}

#[test]
fn scan_chinese_keywords() {
    let (toks, errs) = scan("设 函数 如果 否则 当 循环 返回 中断 继续 智能体 分生 导入 作为 尝试 捕获 抛出 匹配 情况 默认 真 假 空");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Let);
    assert_eq!(toks[0].1, "设");
    assert_eq!(toks[1].0, TokenType::Fn);
    assert_eq!(toks[2].0, TokenType::If);
    assert_eq!(toks[3].0, TokenType::Else);
    assert_eq!(toks[9].0, TokenType::Agent);
    assert_eq!(toks[10].0, TokenType::Spawn);
}

#[test]
fn scan_punctuation() {
    let (toks, errs) = scan("( ) [ ] { } , . : ; @");
    assert!(errs.is_empty());
    let kinds: Vec<TokenType> = toks.iter().take(11).map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![
            TokenType::LeftParen,
            TokenType::RightParen,
            TokenType::LeftBracket,
            TokenType::RightBracket,
            TokenType::LeftBrace,
            TokenType::RightBrace,
            TokenType::Comma,
            TokenType::Dot,
            TokenType::Colon,
            TokenType::Semicolon,
            TokenType::At,
        ]
    );
}

#[test]
fn scan_comparison_operators() {
    let (toks, errs) = scan("< > <= >= == !=");
    assert!(errs.is_empty());
    let kinds: Vec<TokenType> = toks.iter().take(6).map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![
            TokenType::Less,
            TokenType::Greater,
            TokenType::LessEqual,
            TokenType::GreaterEqual,
            TokenType::EqualEqual,
            TokenType::BangEqual,
        ]
    );
}

#[test]
fn scan_arithmetic_operators() {
    let (toks, errs) = scan("+ - * / %");
    assert!(errs.is_empty());
    let kinds: Vec<TokenType> = toks.iter().take(5).map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![
            TokenType::Plus,
            TokenType::Minus,
            TokenType::Star,
            TokenType::Slash,
            TokenType::Percent,
        ]
    );
}

#[test]
fn scan_logical_operators() {
    let (toks, errs) = scan("&& || !");
    assert!(errs.is_empty());
    let kinds: Vec<TokenType> = toks.iter().take(3).map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![TokenType::And, TokenType::Or, TokenType::Bang]
    );
}

#[test]
fn scan_pipe_and_arrow() {
    let (toks, errs) = scan("|> ->");
    assert!(errs.is_empty());
    let kinds: Vec<TokenType> = toks.iter().take(2).map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![
            TokenType::PipeRight,
            TokenType::Arrow,
        ]
    );
}

#[test]
fn scan_range_operator() {
    let (toks, errs) = scan("..");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::DotDot);
}

#[test]
fn scan_string_with_newline_escape() {
    let mut s = Scanner::new(r#""line1\nline2""#, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "line1\nline2"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_string_with_tab_escape() {
    let mut s = Scanner::new(r#""col1\tcol2""#, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "col1\tcol2"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_string_with_quote_escape() {
    let mut s = Scanner::new(r#""say \"hello\"""#, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "say \"hello\""),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_string_with_backslash_escape() {
    let mut s = Scanner::new(r#""path\\to\\file""#, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "path\\to\\file"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_number_with_underscores() {
    let mut s = Scanner::new("1_000_000 1_2_3", "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Int(v) => assert_eq!(v.to_string(), "1000000"),
        other => panic!("expected Int, got {other:?}"),
    }
    match &toks[1].literal {
        LiteralValue::Int(v) => assert_eq!(v.to_string(), "123"),
        other => panic!("expected Int, got {other:?}"),
    }
}

#[test]
fn scan_number_with_exponent() {
    let mut s = Scanner::new("1e10 2.5e-3 1E+5", "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Float(v) => assert!((v - 1e10).abs() < 1e5),
        other => panic!("expected Float, got {other:?}"),
    }
    match &toks[1].literal {
        LiteralValue::Float(v) => assert!((v - 0.0025).abs() < 1e-6),
        other => panic!("expected Float, got {other:?}"),
    }
    match &toks[2].literal {
        LiteralValue::Float(v) => assert!((v - 100000.0).abs() < 1e-2),
        other => panic!("expected Float, got {other:?}"),
    }
}

#[test]
fn scan_identifier_with_underscore() {
    let (toks, errs) = scan("_var my_var __private");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Identifier);
    assert_eq!(toks[0].1, "_var");
    assert_eq!(toks[1].0, TokenType::Identifier);
    assert_eq!(toks[1].1, "my_var");
    assert_eq!(toks[2].0, TokenType::Identifier);
    assert_eq!(toks[2].1, "__private");
}

#[test]
fn scan_identifier_with_numbers() {
    let (toks, errs) = scan("var1 x2y3 test123");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Identifier);
    assert_eq!(toks[0].1, "var1");
    assert_eq!(toks[1].0, TokenType::Identifier);
    assert_eq!(toks[1].1, "x2y3");
    assert_eq!(toks[2].0, TokenType::Identifier);
    assert_eq!(toks[2].1, "test123");
}

#[test]
fn scan_chinese_identifier() {
    let (toks, errs) = scan("变量 函数名 测试123");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Identifier);
    assert_eq!(toks[0].1, "变量");
    assert_eq!(toks[1].0, TokenType::Identifier);
    assert_eq!(toks[1].1, "函数名");
    assert_eq!(toks[2].0, TokenType::Identifier);
    assert_eq!(toks[2].1, "测试123");
}

#[test]
fn scan_whitespace_handling() {
    let (toks, errs) = scan("a  b\t\tc\n\nd");
    assert!(errs.is_empty());
    assert_eq!(toks.len(), 5); // a, b, c, d, EOF
    assert_eq!(toks[0].1, "a");
    assert_eq!(toks[1].1, "b");
    assert_eq!(toks[2].1, "c");
    assert_eq!(toks[3].1, "d");
}

#[test]
fn scan_empty_string() {
    let mut s = Scanner::new(r#""""#, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, ""),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_multiline_string() {
    // Multiline strings in regular quotes may not be supported
    // Use triple quotes instead
    let src = "\"\"\"line1\nline2\nline3\"\"\"";
    let mut s = Scanner::new(src, "<test>");
    let toks = s.scan_all();
    assert!(s.errors().is_empty());
    match &toks[0].literal {
        LiteralValue::Str(v) => assert_eq!(v, "line1\nline2\nline3"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn scan_nested_block_comments() {
    let (toks, errs) = scan("a /* outer /* inner */ still outer */ b");
    assert!(errs.is_empty());
    assert_eq!(toks.len(), 3); // a, b, EOF
    assert_eq!(toks[0].1, "a");
    assert_eq!(toks[1].1, "b");
}

#[test]
fn scan_line_comment_to_eof() {
    let (toks, errs) = scan("a // comment");
    assert!(errs.is_empty());
    assert_eq!(toks.len(), 2); // a, EOF
    assert_eq!(toks[0].1, "a");
}

#[test]
fn scan_complex_expression() {
    let (toks, errs) = scan("if (x > 0 && y < 10) { return x + y; }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::If);
    assert_eq!(toks[1].0, TokenType::LeftParen);
    assert_eq!(toks[2].0, TokenType::Identifier);
    assert_eq!(toks[3].0, TokenType::Greater);
    assert_eq!(toks[4].0, TokenType::Number);
    assert_eq!(toks[5].0, TokenType::And);
}

#[test]
fn scan_function_definition() {
    let (toks, errs) = scan("fn add(a: int, b: int): int { return a + b; }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Fn);
    assert_eq!(toks[1].0, TokenType::Identifier);
    assert_eq!(toks[1].1, "add");
}

#[test]
fn scan_agent_definition() {
    let (toks, errs) = scan("agent MyAgent(input: str) { prompt \"Task: {{input}}\" }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Agent);
    assert_eq!(toks[1].0, TokenType::Identifier);
    assert_eq!(toks[1].1, "MyAgent");
}

#[test]
fn scan_import_statement() {
    let (toks, errs) = scan("import std.core.*");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Import);
    assert_eq!(toks[1].0, TokenType::Identifier);
}

#[test]
fn scan_try_catch() {
    let (toks, errs) = scan("try { risky() } catch (e) { handle(e) }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Try);
    // Find the catch token
    let catch_idx = toks.iter().position(|t| t.0 == TokenType::Catch);
    assert!(catch_idx.is_some());
}

#[test]
fn scan_match_expression() {
    let (toks, errs) = scan("match x { case 1 => \"one\", case 2 => \"two\", default => \"other\" }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Match);
    // Find case tokens
    let case_count = toks.iter().filter(|t| t.0 == TokenType::Case).count();
    assert_eq!(case_count, 2);
}

#[test]
fn scan_spawn_expression() {
    let (toks, errs) = scan("spawn agent task()");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Spawn);
    assert_eq!(toks[1].0, TokenType::Agent);
}

#[test]
fn scan_pipe_operator() {
    let (toks, errs) = scan("x |> f |> g");
    assert!(errs.is_empty());
    assert_eq!(toks[1].0, TokenType::PipeRight);
    assert_eq!(toks[3].0, TokenType::PipeRight);
}

#[test]
fn scan_type_annotation() {
    let (toks, errs) = scan("let x: int = 42");
    assert!(errs.is_empty());
    assert_eq!(toks[2].0, TokenType::Colon);
}

#[test]
fn scan_decorator() {
    let (toks, errs) = scan("@decorator fn test() {}");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::At);
}

#[test]
fn scan_wildcard_pattern() {
    let (toks, errs) = scan("match x { case _ => default }");
    assert!(errs.is_empty());
    // Find the wildcard token
    let wildcard_idx = toks.iter().position(|t| t.0 == TokenType::Wildcard);
    assert!(wildcard_idx.is_some());
}

#[test]
fn scan_boolean_literals() {
    let (toks, errs) = scan("true false");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::True);
    assert_eq!(toks[1].0, TokenType::False);
}

#[test]
fn scan_null_literal() {
    let (toks, errs) = scan("null");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::NullKw);
}

#[test]
fn scan_for_loop() {
    let (toks, errs) = scan("for i in 0..10 { print(i) }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::For);
    assert_eq!(toks[2].0, TokenType::In);
    assert_eq!(toks[4].0, TokenType::DotDot);
}

#[test]
fn scan_while_loop() {
    let (toks, errs) = scan("while x > 0 { x = x - 1 }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::While);
}

#[test]
fn scan_break_continue() {
    let (toks, errs) = scan("break continue");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Break);
    assert_eq!(toks[1].0, TokenType::Continue);
}

#[test]
fn scan_throw_exception() {
    let (toks, errs) = scan("throw Error(\"message\")");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Throw);
}

#[test]
fn scan_as_keyword() {
    let (toks, errs) = scan("import x as y");
    assert!(errs.is_empty());
    assert_eq!(toks[2].0, TokenType::As);
}

#[test]
fn scan_default_keyword() {
    let (toks, errs) = scan("match x { default => 0 }");
    assert!(errs.is_empty());
    // Find the default token
    let default_idx = toks.iter().position(|t| t.0 == TokenType::Default);
    assert!(default_idx.is_some());
}

#[test]
fn scan_complex_nested_expression() {
    let (toks, errs) = scan("if (a && (b || c)) && !(d == e) { return true; }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::If);
    assert_eq!(toks[1].0, TokenType::LeftParen);
}

#[test]
fn scan_multiple_statements() {
    let (toks, errs) = scan("let x = 1; let y = 2; let z = x + y;");
    assert!(errs.is_empty());
    assert_eq!(toks.iter().filter(|t| t.0 == TokenType::Semicolon).count(), 3);
}

#[test]
fn scan_array_literal() {
    let (toks, errs) = scan("[1, 2, 3, 4, 5]");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::LeftBracket);
    assert_eq!(toks.iter().filter(|t| t.0 == TokenType::Comma).count(), 4);
    assert_eq!(toks[10].0, TokenType::RightBracket);
}

#[test]
fn scan_dict_literal() {
    let (toks, errs) = scan("{\"key\": \"value\", \"num\": 42}");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::LeftBrace);
    assert_eq!(toks[1].0, TokenType::String);
    assert_eq!(toks[2].0, TokenType::Colon);
}

#[test]
fn scan_tuple_literal() {
    let (toks, errs) = scan("(1, 2, 3)");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::LeftParen);
    assert_eq!(toks.iter().filter(|t| t.0 == TokenType::Comma).count(), 2);
}

#[test]
fn scan_member_access() {
    let (toks, errs) = scan("obj.field.method()");
    assert!(errs.is_empty());
    assert_eq!(toks.iter().filter(|t| t.0 == TokenType::Dot).count(), 2);
}

#[test]
fn scan_index_access() {
    let (toks, errs) = scan("arr[0]");
    assert!(errs.is_empty());
    assert_eq!(toks[1].0, TokenType::LeftBracket);
    assert_eq!(toks[3].0, TokenType::RightBracket);
}

#[test]
fn scan_function_call() {
    let (toks, errs) = scan("func(arg1, arg2, arg3)");
    assert!(errs.is_empty());
    assert_eq!(toks[1].0, TokenType::LeftParen);
    assert_eq!(toks.iter().filter(|t| t.0 == TokenType::Comma).count(), 2);
    assert_eq!(toks[7].0, TokenType::RightParen);
}

#[test]
fn scan_generic_type() {
    let (toks, errs) = scan("List<int>");
    assert!(errs.is_empty());
    assert_eq!(toks[1].0, TokenType::Less);
    assert_eq!(toks[3].0, TokenType::Greater);
}

#[test]
fn scan_optional_type() {
    let (toks, errs) = scan("int?");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Identifier);
    assert_eq!(toks[1].0, TokenType::Question);
}

#[test]
fn scan_union_type() {
    let (toks, errs) = scan("int | str");
    assert!(errs.is_empty());
    assert_eq!(toks[1].0, TokenType::Pipe);
}

#[test]
fn scan_in_operator() {
    let (toks, errs) = scan("x in array");
    assert!(errs.is_empty());
    assert_eq!(toks[1].0, TokenType::In);
}

#[test]
fn scan_const_keyword() {
    let (toks, errs) = scan("const x = 42");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Const);
}

#[test]
fn scan_finally_keyword() {
    let (toks, errs) = scan("try { } catch { } finally { }");
    assert!(errs.is_empty());
    assert_eq!(toks[6].0, TokenType::Finally);
}

#[test]
fn scan_assert_keyword() {
    let (toks, errs) = scan("assert x > 0");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Assert);
}

#[test]
fn scan_functions_keyword() {
    let (toks, errs) = scan("functions { fn helper() {} }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Functions);
}

#[test]
fn scan_main_keyword() {
    let (toks, errs) = scan("main { return 0; }");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Main);
}

#[test]
fn scan_store_keyword() {
    let (toks, errs) = scan("store data = {}");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Store);
}

#[test]
fn scan_protocol_keyword() {
    let (toks, errs) = scan("protocol MyProtocol {}");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Protocol);
}

#[test]
fn scan_impl_keyword() {
    let (toks, errs) = scan("impl MyProtocol for MyClass {}");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Impl);
}

#[test]
fn scan_is_keyword() {
    let (toks, errs) = scan("x is int");
    assert!(errs.is_empty());
    assert_eq!(toks[1].0, TokenType::Is);
}

#[test]
fn scan_shared_keyword() {
    let (toks, errs) = scan("shared store data = {}");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Shared);
}

#[test]
fn scan_alias_keyword() {
    let (toks, errs) = scan("alias MyType = int");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Alias);
}

#[test]
fn scan_transcript_keyword() {
    let (toks, errs) = scan("transcript log = []");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Transcript);
}

#[test]
fn scan_thinking_mode_keyword() {
    let (toks, errs) = scan("thinking-mode enabled");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::ThinkingMode);
}

#[test]
fn scan_reasoning_effort_keyword() {
    let (toks, errs) = scan("reasoning-effort high");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::ReasoningEffort);
}

#[test]
fn scan_description_keyword() {
    let (toks, errs) = scan("description \"My agent\"");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Description);
}

#[test]
fn scan_model_keyword() {
    let (toks, errs) = scan("model gpt-4");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Model);
}

#[test]
fn scan_tools_keyword() {
    let (toks, errs) = scan("tools [\"read_file\", \"write_file\"]");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Tools);
}

#[test]
fn scan_streaming_keyword() {
    let (toks, errs) = scan("streaming true");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Streaming);
}

#[test]
fn scan_temperature_keyword() {
    let (toks, errs) = scan("temperature 0.7");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Temperature);
}

#[test]
fn scan_max_turns_keyword() {
    let (toks, errs) = scan("max-turns 10");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::MaxTurns);
}

#[test]
fn scan_max_tokens_keyword() {
    let (toks, errs) = scan("max-tokens 1000");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::MaxTokens);
}

#[test]
fn scan_memory_keyword() {
    // "memory" is not a keyword in Helen, it's an identifier
    let (toks, errs) = scan("memory enabled");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Identifier);
    assert_eq!(toks[0].1, "memory");
}

#[test]
fn scan_prompt_keyword() {
    let (toks, errs) = scan("prompt \"Task: {{input}}\"");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Prompt);
}

#[test]
fn scan_llm_keyword() {
    let (toks, errs) = scan("llm act");
    assert!(errs.is_empty());
    assert_eq!(toks[0].0, TokenType::Llm);
}

#[test]
fn scan_act_keyword() {
    let (toks, errs) = scan("llm act");
    assert!(errs.is_empty());
    assert_eq!(toks[1].0, TokenType::Act);
}

#[test]
fn scan_branch_keyword() {
    let (toks, errs) = scan("match x { branch y => y }");
    assert!(errs.is_empty());
    // Find the branch token
    let branch_idx = toks.iter().position(|t| t.0 == TokenType::Branch);
    assert!(branch_idx.is_some());
}

#[test]
fn scan_complex_real_world_code() {
    let src = r#"
        agent DataProcessor(input: str) {
            description "Process data"
            prompt "Task: {{input}}"
            tools ["read_file", "write_file"]
            
            functions {
                fn helper(x: int): int {
                    return x * 2;
                }
            }
            
            main {
                let result = llm act;
                return result;
            }
        }
    "#;
    let (toks, errs) = scan(src);
    assert!(errs.is_empty(), "Errors: {:?}", errs);
    assert!(toks.len() > 50);
}

