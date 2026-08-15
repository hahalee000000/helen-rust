//! Scanner (lexer) for the Helen language.
//!
//! Byte-faithful port of `helen/core/lexer.py`: maximal-munch scanning over
//! `&str` with char-based line/column tracking (CJK-safe). Handles bilingual
//! keywords, fullwidth operators, ASCII/Chinese/triple-quoted strings,
//! escapes, numbers with underscores/exponents, templates, and comments.

use crate::errors::{ErrorCode, LexError};
use crate::source::SourceSpan;
use crate::tokens::{keyword_type, LiteralValue, Token, TokenType};
use num_bigint::BigInt;

/// CJK Unified Ideographs ranges (same as Python `_CJK_RANGES`).
const CJK_RANGES: [(u32, u32); 4] = [
    (0x4E00, 0x9FFF),
    (0x3400, 0x4DBF),
    (0x20000, 0x2A6DF),
    (0xF900, 0xFAFF),
];

fn is_cjk(c: char) -> bool {
    let cp = c as u32;
    CJK_RANGES.iter().any(|&(lo, hi)| lo <= cp && cp <= hi)
}

fn is_alpha_char(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || is_cjk(c)
}

fn is_alnum_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || is_cjk(c)
}

/// Chinese quote pairs: opening → closing.
const CHINESE_QUOTE_PAIRS: [(char, char); 4] = [
    ('\u{201c}', '\u{201d}'), // " → "
    ('\u{2018}', '\u{2019}'), // ' → '
    ('\u{300c}', '\u{300d}'), // 「 → 」
    ('\u{300e}', '\u{300f}'), // 『 → 』
];

fn chinese_close(open: char) -> Option<char> {
    CHINESE_QUOTE_PAIRS
        .iter()
        .find(|(o, _)| *o == open)
        .map(|(_, c)| *c)
}

const FULLWIDTH_QUOTE: char = '\u{ff02}'; // ＂

/// Characters that may start a two-character operator (ASCII + fullwidth).
fn is_two_char_op(c: char) -> bool {
    matches!(
        c,
        '!' | '=' | '>' | '<' | '&' | '|' | '-' | '.'
            | '\u{ff01}' // ！
            | '\u{ff1d}' // ＝
            | '\u{ff1e}' // ＞
            | '\u{ff1c}' // ＜
            | '\u{ff06}' // ＆
            | '\u{ff5c}' // ｜
            | '\u{ff0d}' // －
            | '\u{ff0e}' // ．
    )
}

/// Single-character token dispatch (ASCII + fullwidth).
fn single_char_token(c: char) -> Option<TokenType> {
    Some(match c {
        '(' => TokenType::LeftParen,
        ')' => TokenType::RightParen,
        '{' => TokenType::LeftBrace,
        '}' => TokenType::RightBrace,
        '[' => TokenType::LeftBracket,
        ']' => TokenType::RightBracket,
        ',' => TokenType::Comma,
        '.' => TokenType::Dot,
        ':' => TokenType::Colon,
        ';' => TokenType::Semicolon,
        '?' => TokenType::Question,
        '+' => TokenType::Plus,
        '-' => TokenType::Minus,
        '*' => TokenType::Star,
        '/' => TokenType::Slash,
        '%' => TokenType::Percent,
        '!' => TokenType::Bang,
        '=' => TokenType::Assign,
        '>' => TokenType::Greater,
        '<' => TokenType::Less,
        '|' => TokenType::Pipe,
        '@' => TokenType::At,
        // Fullwidth punctuation
        '\u{ff08}' => TokenType::LeftParen,    // （
        '\u{ff09}' => TokenType::RightParen,   // ）
        '\u{ff3b}' => TokenType::LeftBracket,  // ［
        '\u{ff3d}' => TokenType::RightBracket, // ］
        '\u{ff5b}' => TokenType::LeftBrace,    // ｛
        '\u{ff5d}' => TokenType::RightBrace,   // ｝
        '\u{ff0c}' => TokenType::Comma,        // ，
        '\u{ff0e}' => TokenType::Dot,          // ．
        '\u{ff1a}' => TokenType::Colon,        // ：
        '\u{ff1b}' => TokenType::Semicolon,    // ；
        '\u{ff1f}' => TokenType::Question,     // ？
        '\u{ff0b}' => TokenType::Plus,         // ＋
        '\u{ff0d}' => TokenType::Minus,        // －
        '\u{ff0a}' => TokenType::Star,         // ＊
        '\u{ff0f}' => TokenType::Slash,        // ／
        '\u{ff05}' => TokenType::Percent,      // ％
        '\u{ff01}' => TokenType::Bang,         // ！
        '\u{ff1d}' => TokenType::Assign,       // ＝
        '\u{ff1e}' => TokenType::Greater,      // ＞
        '\u{ff1c}' => TokenType::Less,         // ＜
        '\u{ff5c}' => TokenType::Pipe,         // ｜
        _ => return None,
    })
}

/// The Helen scanner.
pub struct Scanner<'a> {
    source: &'a str,
    file: &'a str,
    pos: usize,
    line: u32,
    col: u32,
    start_line: u32,
    start_col: u32,
    token_start_pos: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str, file: &'a str) -> Self {
        Scanner {
            source,
            file,
            pos: 0,
            line: 1,
            col: 1,
            start_line: 1,
            start_col: 1,
            token_start_pos: 0,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Scan the entire source, returning tokens ending with an EOF token.
    pub fn scan_all(&mut self) -> Vec<Token> {
        self.tokens.clear();
        self.errors.clear();
        self.pos = 0;
        self.line = 1;
        self.col = 1;
        self.token_start_pos = 0;
        while !self.at_end() {
            self.start_line = self.line;
            self.start_col = self.col;
            self.token_start_pos = self.pos;
            self.scan_token();
        }
        self.tokens.push(Token {
            kind: TokenType::Eof,
            lexeme: String::new(),
            literal: LiteralValue::Null,
            line: self.line,
            col: self.col,
            end_line: self.line,
            end_col: self.col,
            file: self.file.to_string(),
        });
        self.tokens.clone()
    }

    /// Collected lexical errors (a copy).
    pub fn errors(&self) -> Vec<LexError> {
        self.errors.clone()
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    // ── core scanning logic ─────────────────────────────────────────

    fn scan_token(&mut self) {
        let c = self.peek();

        // 1. Multi-character tokens first (maximal munch).
        if self.try_multi_char_token(c) {
            return;
        }

        // 2. Two-character operators.
        if is_two_char_op(c) {
            self.handle_two_char_op(c);
            return;
        }

        // 3. Single-character tokens.
        if let Some(tt) = single_char_token(c) {
            self.advance();
            self.consume_one(tt);
            return;
        }

        // 4. Unknown character — report and skip.
        self.error_char(c);
    }

    fn try_multi_char_token(&mut self, c: char) -> bool {
        // Whitespace (skip silently)
        if c == ' ' || c == '\t' || c == '\r' {
            self.whitespace();
            return true;
        }

        // Newline (skip silently)
        if c == '\n' {
            self.advance();
            return true;
        }

        // Comments
        if c == '/' {
            if self.peek_next() == '/' {
                self.line_comment();
                return true;
            }
            if self.peek_next() == '*' {
                self.block_comment();
                return true;
            }
        }

        // Strings (ASCII)
        if c == '"' {
            if self.peek_ahead(1) == '"' && self.peek_ahead(2) == '"' {
                self.triple_quoted_string();
            } else {
                self.string();
            }
            return true;
        }

        // Strings (Chinese quotes)
        if chinese_close(c).is_some() || c == FULLWIDTH_QUOTE {
            self.chinese_string(c);
            return true;
        }

        // Numbers (digit or dot followed by digit)
        if c.is_ascii_digit() || (c == '.' && self.peek_next().is_ascii_digit()) {
            self.number();
            return true;
        }

        // Template delimiters
        if c == '{' && self.peek_next() == '{' {
            self.template_open();
            return true;
        }
        if c == '}' && self.peek_next() == '}' {
            self.template_close();
            return true;
        }

        // Identifiers / keywords
        if is_alpha_char(c) {
            self.identifier_or_keyword();
            return true;
        }

        false
    }

    fn handle_two_char_op(&mut self, c: char) {
        let second = self.peek_next();
        let two: [char; 2] = [c, second];

        // ASCII two-character operators
        let tt = match two {
            ['!', '='] => Some(TokenType::BangEqual),
            ['=', '='] => Some(TokenType::EqualEqual),
            ['>', '='] => Some(TokenType::GreaterEqual),
            ['<', '='] => Some(TokenType::LessEqual),
            ['&', '&'] => Some(TokenType::And),
            ['|', '|'] => Some(TokenType::Or),
            ['|', '>'] => Some(TokenType::PipeRight),
            ['-', '>'] => Some(TokenType::Arrow),
            ['.', '.'] => Some(TokenType::DotDot),
            // Fullwidth two-character operators
            ['\u{ff01}', '\u{ff1d}'] => Some(TokenType::BangEqual), // ！＝
            ['\u{ff1d}', '\u{ff1d}'] => Some(TokenType::EqualEqual), // ＝＝
            ['\u{ff1e}', '\u{ff1d}'] => Some(TokenType::GreaterEqual), // ＞＝
            ['\u{ff1c}', '\u{ff1d}'] => Some(TokenType::LessEqual), // ＜＝
            ['\u{ff06}', '\u{ff06}'] => Some(TokenType::And),       // ＆＆
            ['\u{ff5c}', '\u{ff5c}'] => Some(TokenType::Or),        // ｜｜
            ['\u{ff5c}', '\u{ff1e}'] => Some(TokenType::PipeRight), // ｜＞
            ['\u{ff0d}', '\u{ff1e}'] => Some(TokenType::Arrow),     // －＞
            ['\u{ff0e}', '\u{ff0e}'] => Some(TokenType::DotDot),    // ．．
            _ => None,
        };
        if let Some(tt) = tt {
            self.consume_two(tt);
            return;
        }

        // Fall back to single-char token (Python advances first).
        self.advance();
        match c {
            '!' | '\u{ff01}' => self.consume_one(TokenType::Bang),
            '=' | '\u{ff1d}' => self.consume_one(TokenType::Assign),
            '>' | '\u{ff1e}' => self.consume_one(TokenType::Greater),
            '<' | '\u{ff1c}' => self.consume_one(TokenType::Less),
            '&' => self.error(
                ErrorCode::ScannerError,
                "Unexpected character: '&'. Did you mean '&&'?".to_string(),
            ),
            '\u{ff06}' => self.error(
                ErrorCode::ScannerError,
                "Unexpected character: '＆'. Did you mean '＆＆'?".to_string(),
            ),
            '|' | '\u{ff5c}' => self.consume_one(TokenType::Pipe),
            '-' | '\u{ff0d}' => self.consume_one(TokenType::Minus),
            '.' | '\u{ff0e}' => self.consume_one(TokenType::Dot),
            _ => {}
        }
    }

    // ── whitespace & comments ───────────────────────────────────────

    fn whitespace(&mut self) {
        while matches!(self.peek(), ' ' | '\t' | '\r') && !self.at_end() {
            self.advance();
        }
    }

    fn line_comment(&mut self) {
        self.advance(); // /
        self.advance(); // /
        while !self.at_end() && self.peek() != '\n' {
            self.advance();
        }
    }

    fn block_comment(&mut self) {
        self.advance(); // /
        self.advance(); // *
        let mut depth: u32 = 1;
        while !self.at_end() && depth > 0 {
            let c = self.advance();
            if c == '/' && self.peek() == '*' {
                self.advance();
                depth += 1;
            } else if c == '*' && self.peek() == '/' {
                self.advance();
                depth -= 1;
            }
        }
        if depth > 0 {
            self.error(
                ErrorCode::ScannerError,
                "Unterminated block comment".to_string(),
            );
        }
    }

    // ── strings ─────────────────────────────────────────────────────

    fn string(&mut self) {
        self.advance(); // opening "
        let mut buffer = String::new();
        while !self.at_end() && self.peek() != '"' && self.peek() != '\n' {
            let c = self.advance();
            if c == '\\' {
                buffer.push_str(&self.parse_escape());
            } else {
                buffer.push(c);
            }
        }

        if self.at_end() || self.peek() == '\n' {
            self.error(
                ErrorCode::UnterminatedString,
                "Unterminated string literal".to_string(),
            );
        } else {
            self.advance(); // closing "
        }

        self.push_token(TokenType::String, LiteralValue::Str(buffer));
    }

    fn chinese_string(&mut self, open_quote: char) {
        self.advance(); // opening quote
        let close_quote = if open_quote == FULLWIDTH_QUOTE {
            FULLWIDTH_QUOTE
        } else {
            chinese_close(open_quote).unwrap_or(FULLWIDTH_QUOTE)
        };

        let mut buffer = String::new();
        while !self.at_end() && self.peek() != close_quote && self.peek() != '\n' {
            let c = self.advance();
            if c == '\\' {
                buffer.push_str(&self.parse_escape());
            } else {
                buffer.push(c);
            }
        }

        if self.at_end() || self.peek() == '\n' {
            self.error(
                ErrorCode::UnterminatedString,
                "Unterminated string literal".to_string(),
            );
        } else {
            self.advance(); // closing quote
        }

        self.push_token(TokenType::String, LiteralValue::Str(buffer));
    }

    fn triple_quoted_string(&mut self) {
        self.advance(); // "
        self.advance(); // "
        self.advance(); // "
        let mut buffer = String::new();
        let mut terminated = false;
        while !self.at_end() {
            if self.peek() == '"' && self.peek_next() == '"' && self.peek_ahead(2) == '"' {
                self.advance();
                self.advance();
                self.advance();
                terminated = true;
                break;
            }
            let c = self.advance();
            if c == '\\' {
                buffer.push_str(&self.parse_escape());
            } else {
                buffer.push(c);
            }
        }
        if !terminated {
            self.error(
                ErrorCode::UnterminatedString,
                "Unterminated triple-quoted string literal".to_string(),
            );
        }

        let literal = self.dedent_string(&buffer);
        self.push_token(TokenType::TripleQuoteString, LiteralValue::Str(literal));
    }

    fn dedent_string(&self, text: &str) -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.is_empty() {
            return text.to_string();
        }

        // Minimum indentation, ignoring empty (whitespace-only) lines.
        let mut min_indent: Option<usize> = None;
        for line in &lines {
            if !line.trim().is_empty() {
                let indent = line.chars().count() - line.trim_start().chars().count();
                if min_indent.is_none() || indent < min_indent.unwrap() {
                    min_indent = Some(indent);
                }
            }
        }

        let Some(min_indent) = min_indent else {
            return text.to_string();
        };
        if min_indent == 0 {
            return text.to_string();
        }

        let mut dedented = Vec::with_capacity(lines.len());
        for line in &lines {
            if line.trim().is_empty() {
                dedented.push(String::new());
            } else {
                dedented.push(line.chars().skip(min_indent).collect());
            }
        }
        dedented.join("\n")
    }

    // ── numbers ─────────────────────────────────────────────────────

    fn number(&mut self) {
        let mut has_dot = false;
        if self.peek() == '.' {
            if self.peek_next() == '.' {
                // part of the range operator `..`
            } else {
                has_dot = true;
                self.advance();
                self.scan_integer_part();
            }
        } else {
            self.scan_integer_part();
            if self.peek() == '.' {
                if self.peek_next() == '.' {
                    // part of `..`
                } else {
                    has_dot = true;
                    self.advance();
                    self.scan_integer_part();
                }
            }
        }

        let mut has_exp = false;
        if self.peek() == 'e' || self.peek() == 'E' {
            has_exp = true;
            self.scan_optional_exponent();
        }

        let lexeme = self.current_lexeme().to_string();
        let mut clean = lexeme.replace('_', "");
        if clean.ends_with('.') {
            clean.pop();
        }

        let literal = if has_dot || has_exp {
            LiteralValue::Float(self.parse_float_value(&clean, &lexeme))
        } else {
            LiteralValue::Int(self.parse_int_value(&clean, &lexeme))
        };
        self.push_token(TokenType::Number, literal);
    }

    fn scan_integer_part(&mut self) {
        while self.peek().is_ascii_digit() || self.peek() == '_' {
            self.advance();
        }
    }

    fn scan_optional_exponent(&mut self) -> bool {
        if self.peek() != 'e' && self.peek() != 'E' {
            return false;
        }
        self.advance(); // e/E
        if self.peek() == '+' || self.peek() == '-' {
            self.advance();
        }
        if !self.peek().is_ascii_digit() {
            self.error(
                ErrorCode::ScannerError,
                "Expected digit after exponent marker".to_string(),
            );
            return false;
        }
        self.scan_integer_part();
        true
    }

    fn parse_float_value(&mut self, clean: &str, lexeme: &str) -> f64 {
        match clean.parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                self.error(
                    ErrorCode::InvalidLiteral,
                    format!("Invalid numeric literal: '{lexeme}'"),
                );
                0.0
            }
        }
    }

    fn parse_int_value(&mut self, clean: &str, lexeme: &str) -> BigInt {
        match clean.parse::<BigInt>() {
            Ok(v) => v,
            Err(_) => {
                self.error(
                    ErrorCode::InvalidLiteral,
                    format!("Invalid integer literal: '{lexeme}'"),
                );
                BigInt::from(0)
            }
        }
    }

    // ── identifiers & keywords ──────────────────────────────────────

    fn identifier_or_keyword(&mut self) {
        while is_alnum_char(self.peek()) {
            self.advance();
        }

        // Hyphenated keyword disambiguation: `max-turns`, `thinking-mode`, ...
        if self.peek() == '-' && is_alpha_char(self.peek_next()) {
            self.advance(); // '-'
            while is_alnum_char(self.peek()) {
                self.advance();
            }
        }

        let lexeme = self.current_lexeme().to_string();
        let tt = if lexeme == "_" {
            TokenType::Wildcard
        } else {
            keyword_type(&lexeme).unwrap_or(TokenType::Identifier)
        };

        let literal = match tt {
            TokenType::True => LiteralValue::Bool(true),
            TokenType::False => LiteralValue::Bool(false),
            _ => LiteralValue::Null,
        };

        self.push_token(tt, literal);
    }

    // ── templates ───────────────────────────────────────────────────

    fn template_open(&mut self) {
        self.advance(); // {
        self.advance(); // {
        self.consume_one(TokenType::TemplateOpen);
    }

    fn template_close(&mut self) {
        self.advance(); // }
        self.advance(); // }
        self.consume_one(TokenType::TemplateClose);
    }

    // ── escape sequences ────────────────────────────────────────────

    fn parse_escape(&mut self) -> String {
        if self.at_end() {
            self.error(
                ErrorCode::InvalidEscape,
                "Unterminated escape sequence".to_string(),
            );
            return String::new();
        }

        let c = self.advance();
        match c {
            'n' => "\n".to_string(),
            't' => "\t".to_string(),
            'r' => "\r".to_string(),
            '\\' => "\\".to_string(),
            '"' => "\"".to_string(),
            '\'' => "'".to_string(),
            '0' => "\0".to_string(),
            'x' => {
                let mut hex_chars = String::new();
                for _ in 0..2 {
                    if self.at_end() {
                        self.error(
                            ErrorCode::InvalidEscape,
                            "Unterminated \\x escape sequence".to_string(),
                        );
                        return String::new();
                    }
                    let h = self.advance();
                    if !h.is_ascii_hexdigit() {
                        self.error(
                            ErrorCode::InvalidEscape,
                            format!("Invalid hex digit in \\x escape: '{h}'"),
                        );
                        return String::new();
                    }
                    hex_chars.push(h);
                }
                let code = u32::from_str_radix(&hex_chars, 16).unwrap_or(0);
                char::from_u32(code).unwrap_or('\u{fffd}').to_string()
            }
            'u' => {
                let mut hex_chars = String::new();
                for _ in 0..4 {
                    if self.at_end() {
                        self.error(
                            ErrorCode::InvalidEscape,
                            "Unterminated \\u escape sequence".to_string(),
                        );
                        return String::new();
                    }
                    let h = self.advance();
                    if !h.is_ascii_hexdigit() {
                        self.error(
                            ErrorCode::InvalidEscape,
                            format!("Invalid hex digit in \\u escape: '{h}'"),
                        );
                        return String::new();
                    }
                    hex_chars.push(h);
                }
                let code = u32::from_str_radix(&hex_chars, 16).unwrap_or(0);
                char::from_u32(code).unwrap_or('\u{fffd}').to_string()
            }
            _ => {
                self.error(
                    ErrorCode::InvalidEscape,
                    format!("Invalid escape sequence: '\\{c}'"),
                );
                String::new()
            }
        }
    }

    // ── error helpers ───────────────────────────────────────────────

    fn error_char(&mut self, c: char) {
        self.error(
            ErrorCode::ScannerError,
            format!("Unexpected character: '{c}'"),
        );
        self.advance();
    }

    fn error(&mut self, code: ErrorCode, message: String) {
        let span = SourceSpan::new(
            self.file.to_string(),
            self.start_line,
            self.start_col,
            self.line,
            self.col,
        );
        self.errors.push(LexError::new(code, message, span));
    }

    // ── cursor helpers ──────────────────────────────────────────────

    fn peek(&self) -> char {
        self.source[self.pos..].chars().next().unwrap_or('\0')
    }

    fn peek_next(&self) -> char {
        let mut it = self.source[self.pos..].chars();
        it.next();
        it.next().unwrap_or('\0')
    }

    fn peek_ahead(&self, n: usize) -> char {
        let mut it = self.source[self.pos..].chars();
        for _ in 0..n {
            it.next();
        }
        it.next().unwrap_or('\0')
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.pos..].chars().next().unwrap_or('\0');
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn current_lexeme(&self) -> &str {
        &self.source[self.token_start_pos..self.pos]
    }

    fn push_token(&mut self, tt: TokenType, literal: LiteralValue) {
        self.tokens.push(Token {
            kind: tt,
            lexeme: self.current_lexeme().to_string(),
            literal,
            line: self.start_line,
            col: self.start_col,
            end_line: self.line,
            end_col: self.col,
            file: self.file.to_string(),
        });
    }

    fn consume_one(&mut self, tt: TokenType) {
        self.push_token(tt, LiteralValue::Null);
    }

    fn consume_two(&mut self, tt: TokenType) {
        self.advance();
        self.advance();
        self.push_token(tt, LiteralValue::Null);
    }
}
