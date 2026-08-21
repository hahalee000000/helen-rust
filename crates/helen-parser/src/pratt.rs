//! Helen Pratt precedence parser + recursive descent.
//!
//! Byte-faithful port of `helen/core/parser.py` (v1.44.0). Mirrors the
//! Python implementation exactly: precedence levels, rule registration,
//! error messages, span computation, and panic-mode synchronization.

use helen_core::ast::*;
use helen_core::errors::{ErrorCode, ParseError};
use helen_core::source::SourceSpan;
use helen_core::tokens::{LiteralValue, Token, TokenType};

/// Pratt parsing precedence levels (higher value = tighter binding).
pub mod precedence {
    pub const NONE: u8 = 0;
    pub const ASSIGNMENT: u8 = 1;
    pub const PIPE: u8 = 2; // |> pipe operator
    pub const OR: u8 = 3;
    pub const AND: u8 = 4;
    pub const EQUALITY: u8 = 5;
    pub const COMPARISON: u8 = 6;
    pub const TERM: u8 = 7;
    pub const FACTOR: u8 = 8;
    pub const UNARY: u8 = 9;
    pub const CALL: u8 = 11;
}

/// Tokens that indicate the end of an expression (for bare form detection)
/// in `llm act` / `llm if`.
fn is_bare_form_token(tt: TokenType) -> bool {
    matches!(
        tt,
        TokenType::RightBrace
            | TokenType::Semicolon
            | TokenType::Eof
            | TokenType::Return
            | TokenType::Let
            | TokenType::Const
            | TokenType::If
            | TokenType::For
            | TokenType::While
            | TokenType::Break
            | TokenType::Continue
            | TokenType::Match
            | TokenType::Try
            | TokenType::Throw
            | TokenType::Llm
    )
}

/// Prefix parse rule kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrefixKind {
    LiteralNumber,
    LiteralString,
    LiteralBool,
    LiteralNull,
    Identifier,
    Grouping,
    Unary,
    ListLiteral,
    MapLiteral,
    TemplateRef,
    LlmExpr,
    SpawnExpr,
    LambdaExpr,
    MatchExpr,
}

/// Infix parse rule kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfixKind {
    Binary,
    Pipe,
    Call,
    Index,
    Access,
}

/// The Pratt parser.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    /// Initialize the parser with a token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    /// The collected parse errors.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    // ------------------------------------------------------------------
    // Rule registration (mirrors `_register_pratt_rules`)
    // ------------------------------------------------------------------

    fn prefix_rule(&self, tt: TokenType) -> Option<PrefixKind> {
        match tt {
            TokenType::Number => Some(PrefixKind::LiteralNumber),
            TokenType::String | TokenType::TripleQuoteString => Some(PrefixKind::LiteralString),
            TokenType::True | TokenType::False => Some(PrefixKind::LiteralBool),
            TokenType::NullKw => Some(PrefixKind::LiteralNull),
            TokenType::Identifier => Some(PrefixKind::Identifier),
            TokenType::LeftParen => Some(PrefixKind::Grouping),
            TokenType::Bang | TokenType::Minus => Some(PrefixKind::Unary),
            TokenType::LeftBracket => Some(PrefixKind::ListLiteral),
            TokenType::LeftBrace => Some(PrefixKind::MapLiteral),
            TokenType::TemplateOpen => Some(PrefixKind::TemplateRef),
            TokenType::Llm => Some(PrefixKind::LlmExpr),
            TokenType::Spawn => Some(PrefixKind::SpawnExpr),
            TokenType::Fn => Some(PrefixKind::LambdaExpr),
            TokenType::Match => Some(PrefixKind::MatchExpr),
            _ => None,
        }
    }

    fn infix_rule(&self, tt: TokenType) -> Option<(InfixKind, u8)> {
        let prec = match tt {
            TokenType::Plus | TokenType::Minus => precedence::TERM,
            TokenType::Star | TokenType::Slash | TokenType::Percent => precedence::FACTOR,
            TokenType::BangEqual | TokenType::EqualEqual => precedence::EQUALITY,
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => precedence::COMPARISON,
            TokenType::And => precedence::AND,
            TokenType::Or => precedence::OR,
            TokenType::Assign => precedence::ASSIGNMENT,
            TokenType::LeftParen | TokenType::LeftBracket | TokenType::Dot => precedence::CALL,
            TokenType::PipeRight => precedence::PIPE,
            _ => return None,
        };
        let kind = match tt {
            TokenType::Plus
            | TokenType::Minus
            | TokenType::Star
            | TokenType::Slash
            | TokenType::Percent
            | TokenType::BangEqual
            | TokenType::EqualEqual
            | TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual
            | TokenType::And
            | TokenType::Or
            | TokenType::Assign => InfixKind::Binary,
            TokenType::PipeRight => InfixKind::Pipe,
            TokenType::LeftParen => InfixKind::Call,
            TokenType::LeftBracket => InfixKind::Index,
            TokenType::Dot => InfixKind::Access,
            _ => unreachable!("infix kind for {tt:?}"),
        };
        Some((kind, prec))
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Parse the token stream and return a `Program`.
    pub fn parse(&mut self) -> Program {
        let mut statements: Vec<Stmt> = Vec::new();
        while !self.at_end() {
            if let Some(stmt) = self.declaration() {
                statements.push(stmt);
            }
        }
        let span = if !self.tokens.is_empty() {
            let first = &self.tokens[0];
            let last = self.tokens.last().unwrap();
            SourceSpan::new(
                first.file.clone(),
                first.line,
                first.col,
                last.end_line,
                last.end_col,
            )
        } else {
            SourceSpan::new("<unknown>".to_string(), 1, 1, 1, 1)
        };
        Program { statements, span }
    }

    // ------------------------------------------------------------------
    // Pratt core
    // ------------------------------------------------------------------

    fn expression(&mut self, prec: u8) -> Expr {
        if self.at_end() {
            self.error("Expected expression.");
            let t = self.peek();
            return Expr::Literal(Lit {
                value: LiteralValue::Null,
                span: self.make_span_from_token(t, t),
            });
        }

        let token = self.current().clone();
        let rule = self.prefix_rule(token.kind);
        let Some(prefix) = rule else {
            self.error(&format!("Expected expression, got {}", token.kind.name()));
            self.advance(); // Consume the unexpected token to avoid infinite loops
            return Expr::Literal(Lit {
                value: LiteralValue::Null,
                span: self.make_span_from_token(&token, &token),
            });
        };

        self.advance();
        let mut left = self.run_prefix(prefix);

        loop {
            if self.at_end() {
                break;
            }
            let cur = self.current().kind;
            let Some((infix, iprec)) = self.infix_rule(cur) else {
                break;
            };
            if iprec < prec {
                break;
            }
            self.advance();
            left = self.run_infix(infix, left);
        }

        left
    }

    fn run_prefix(&mut self, kind: PrefixKind) -> Expr {
        match kind {
            PrefixKind::LiteralNumber => self.literal_number(),
            PrefixKind::LiteralString => self.literal_string(),
            PrefixKind::LiteralBool => self.literal_bool(),
            PrefixKind::LiteralNull => self.literal_null(),
            PrefixKind::Identifier => self.identifier(),
            PrefixKind::Grouping => self.grouping(),
            PrefixKind::Unary => self.unary(),
            PrefixKind::ListLiteral => self.list_literal(),
            PrefixKind::MapLiteral => self.map_literal(),
            PrefixKind::TemplateRef => self.template_ref(),
            PrefixKind::LlmExpr => self.llm_expr(),
            PrefixKind::SpawnExpr => self.spawn_expr(),
            PrefixKind::LambdaExpr => self.lambda_expr(),
            PrefixKind::MatchExpr => self.match_expr(),
        }
    }

    fn run_infix(&mut self, kind: InfixKind, left: Expr) -> Expr {
        match kind {
            InfixKind::Binary => self.binary(left),
            InfixKind::Pipe => self.pipe(left),
            InfixKind::Call => self.call(left),
            InfixKind::Index => self.index(left),
            InfixKind::Access => self.access(left),
        }
    }

    // -- prefix implementations -------------------------------------------

    fn literal_number(&mut self) -> Expr {
        let prev = self.previous().clone();
        Expr::Literal(Lit {
            value: prev.literal.clone(),
            span: prev.span(),
        })
    }

    fn literal_string(&mut self) -> Expr {
        let prev = self.previous().clone();
        Expr::Literal(Lit {
            value: prev.literal.clone(),
            span: prev.span(),
        })
    }

    fn literal_bool(&mut self) -> Expr {
        let prev = self.previous().clone();
        Expr::Literal(Lit {
            value: prev.literal.clone(),
            span: prev.span(),
        })
    }

    fn literal_null(&mut self) -> Expr {
        let prev = self.previous().clone();
        Expr::Literal(Lit {
            value: LiteralValue::Null,
            span: prev.span(),
        })
    }

    fn identifier(&mut self) -> Expr {
        let prev = self.previous().clone();
        Expr::Variable(Variable {
            name: prev.lexeme.clone(),
            span: prev.span(),
        })
    }

    fn llm_expr(&mut self) -> Expr {
        // start = previous (LLM token)
        if self.check(TokenType::Act) {
            self.llm_act_expr()
        } else if self.check(TokenType::If) {
            self.error("'llm if' is not allowed in expression context; use as a statement.");
            self.synchronize();
            let start = self.previous().clone();
            Expr::Literal(Lit {
                value: LiteralValue::Null,
                span: self.make_span_from_token(&start, self.previous()),
            })
        } else {
            self.error("Expected 'act' after 'llm'.");
            let start = self.previous().clone();
            Expr::Literal(Lit {
                value: LiteralValue::Null,
                span: self.make_span_from_token(&start, self.previous()),
            })
        }
    }

    fn spawn_expr(&mut self) -> Expr {
        let start = self.previous().clone();
        let call_expr = self.expression(precedence::NONE);
        let call = match call_expr {
            Expr::Call(c) => c,
            other => {
                self.error("'spawn' must be followed by a function call.");
                Call {
                    callee: Box::new(other),
                    arguments: Vec::new(),
                    span: self.make_span_from_token(&start, self.previous()),
                }
            }
        };
        // v1.27: optional resume("...") clause
        let mut resume_expr: Option<Box<Expr>> = None;
        if self.check(TokenType::Identifier)
            && matches!(self.current().lexeme.as_str(), "resume" | "恢复会话")
        {
            self.advance();
            resume_expr = Some(Box::new(self.expression(precedence::NONE)));
        }
        Expr::Spawn(Spawn {
            call: Box::new(call),
            span: self.make_span_from_token(&start, self.previous()),
            resume_session: resume_expr,
        })
    }

    #[allow(clippy::if_same_then_else)]
    fn llm_act_expr(&mut self) -> Expr {
        let start = self.previous().clone(); // LLM token
        self.consume(TokenType::Act, "Expected 'act' after 'llm'.");
        let act_token = self.previous().clone();

        let clause_keywords: &[&str] = &[
            "on_chunk",
            "on_complete",
            "on_tool_end",
            "on_media",
            "on_generate",
            "provider",
            "逐块处理",
            "完成",
            "工具结束",
            "处理媒体",
            "生成",
        ];

        // Deprecated-form check: llm act Agent(args) "desc"
        if self.check(TokenType::Identifier) {
            let saved_pos = self.pos;
            let ident_tok = self.advance().clone();
            if (self.check(TokenType::LeftParen) || self.check(TokenType::String))
                && !matches!(ident_tok.lexeme.as_str(), "media" | "媒体")
            {
                self.error(&format!(
                    "'llm act {}(...)' is deprecated. Use 'call {}(...)' instead.",
                    ident_tok.lexeme, ident_tok.lexeme
                ));
            }
            self.pos = saved_pos;
        }

        // Bare-form detection
        let prompt_expr: Option<Box<Expr>> = if self.check_many_bare() {
            None
        } else if self.current().line > act_token.line {
            None
        } else if self.check(TokenType::Identifier)
            && clause_keywords.contains(&self.current().lexeme.as_str())
        {
            None
        } else if self.check(TokenType::Identifier)
            && matches!(self.current().lexeme.as_str(), "media" | "媒体")
            && self.peek_next().kind == TokenType::LeftParen
        {
            None
        } else {
            Some(Box::new(self.expression(precedence::NONE)))
        };

        let mut media: Vec<Expr> = Vec::new();
        let mut on_chunk: Option<Box<Expr>> = None;
        let mut on_complete: Option<Box<Expr>> = None;
        let mut on_tool_end: Option<Box<Expr>> = None;
        let mut on_media: Option<Box<Expr>> = None;
        let mut on_generate: Vec<Expr> = Vec::new();
        let mut provider: Option<Box<Expr>> = None;

        while !self.at_end() && !self.check_many_bare() {
            if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "media" | "媒体")
                && self.peek_next().kind == TokenType::LeftParen
            {
                media.push(self.expression(precedence::NONE));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_chunk" | "逐块处理")
            {
                self.advance();
                on_chunk = Some(Box::new(self.expression(precedence::NONE)));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_complete" | "完成")
            {
                self.advance();
                on_complete = Some(Box::new(self.expression(precedence::NONE)));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_tool_end" | "工具结束")
            {
                self.advance();
                on_tool_end = Some(Box::new(self.expression(precedence::NONE)));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_media" | "处理媒体")
            {
                self.advance();
                on_media = Some(Box::new(self.expression(precedence::NONE)));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_generate" | "生成")
            {
                self.advance();
                on_generate.push(self.expression(precedence::NONE));
            } else if self.check(TokenType::Identifier) && self.current().lexeme == "provider" {
                self.advance();
                provider = Some(Box::new(self.expression(precedence::NONE)));
            } else {
                break;
            }
        }

        Expr::LlmAct(LlmAct {
            prompt: prompt_expr,
            media,
            on_chunk,
            on_complete,
            on_tool_end,
            on_media,
            on_generate,
            provider,
            span: self.make_span_from_token(&start, self.previous()),
        })
    }

    fn grouping(&mut self) -> Expr {
        let expr = self.expression(precedence::NONE);
        let end = self.consume(TokenType::RightParen, "Expected ')' after expression.");
        let span = self.make_span_from_expr(&expr, &end);
        Expr::Grouping(Grouping {
            expression: Box::new(expr),
            span,
        })
    }

    fn unary(&mut self) -> Expr {
        let operator = self.previous().clone();
        let right = self.expression(precedence::UNARY);
        Expr::Unary(Unary {
            operator,
            operand: Box::new(right),
            span: self.make_span_from_token(self.previous(), self.previous()),
        })
    }

    // -- infix implementations --------------------------------------------

    fn binary(&mut self, left: Expr) -> Expr {
        let operator = self.previous().clone();
        let (_, iprec) = self.infix_rule(operator.kind).unwrap();
        let right = self.expression(iprec + 1);
        Expr::Binary(Binary {
            left: Box::new(left),
            operator: operator.clone(),
            right: Box::new(right),
            // Python `_binary`: span = _make_span(operator, previous) — the
            // span starts at the OPERATOR token, not the left operand.
            span: self.make_span_from_token(&operator, self.previous()),
        })
    }

    fn pipe(&mut self, left: Expr) -> Expr {
        let operator = self.previous().clone();
        let (_, iprec) = self.infix_rule(TokenType::PipeRight).unwrap();
        let right = self.expression(iprec + 1); // left-associative
        Expr::Pipe(Pipe {
            value: Box::new(left),
            function: Box::new(right),
            span: self.make_span_from_token(&operator, self.previous()),
        })
    }

    fn call(&mut self, callee: Expr) -> Expr {
        let mut args: Vec<CallArg> = Vec::new();
        if !self.check(TokenType::RightParen) {
            if self.check(TokenType::Identifier) {
                let saved_pos = self.pos;
                self.advance();
                if self.check(TokenType::Assign) {
                    let name = self.previous().lexeme.clone();
                    self.advance(); // consume =
                    let value = self.expression(precedence::NONE);
                    args.push(CallArg {
                        name: Some(name),
                        value,
                    });
                } else {
                    self.pos = saved_pos;
                    let value = self.expression(precedence::NONE);
                    args.push(CallArg { name: None, value });
                }
            } else {
                let value = self.expression(precedence::NONE);
                args.push(CallArg { name: None, value });
            }
            while self.match_tokens(&[TokenType::Comma]) {
                if self.check(TokenType::RightParen) {
                    break;
                }
                if self.check(TokenType::Identifier) {
                    let saved_pos = self.pos;
                    self.advance();
                    if self.check(TokenType::Assign) {
                        let name = self.previous().lexeme.clone();
                        self.advance();
                        let value = self.expression(precedence::NONE);
                        args.push(CallArg {
                            name: Some(name),
                            value,
                        });
                    } else {
                        self.pos = saved_pos;
                        let value = self.expression(precedence::NONE);
                        args.push(CallArg { name: None, value });
                    }
                } else {
                    let value = self.expression(precedence::NONE);
                    args.push(CallArg { name: None, value });
                }
            }
        }
        let paren = self.consume(TokenType::RightParen, "Expected ')' after arguments.");
        let span = self.make_span_from_expr(&callee, &paren);
        Expr::Call(Call {
            callee: Box::new(callee),
            arguments: args,
            span,
        })
    }

    fn index(&mut self, target: Expr) -> Expr {
        let index = self.expression(precedence::NONE);
        let bracket = self.consume(TokenType::RightBracket, "Expected ']' after index.");
        let span = self.make_span_from_expr(&target, &bracket);
        Expr::Index(Index {
            target: Box::new(target),
            index: Box::new(index),
            span,
        })
    }

    fn access(&mut self, target: Expr) -> Expr {
        let prop = self.consume(TokenType::Identifier, "Expected property name after '.'.");
        let span = self.make_span_from_expr(&target, &prop);
        Expr::Access(Access {
            target: Box::new(target),
            property: prop.lexeme.clone(),
            span,
        })
    }

    // -- literals ----------------------------------------------------------

    fn list_literal(&mut self) -> Expr {
        let start = self.previous().clone();
        let mut elements: Vec<Expr> = Vec::new();
        if !self.check(TokenType::RightBracket) {
            elements.push(self.expression(precedence::NONE));
            while self.match_tokens(&[TokenType::Comma]) {
                if self.check(TokenType::RightBracket) {
                    break;
                }
                elements.push(self.expression(precedence::NONE));
            }
        }
        let end = self.consume(TokenType::RightBracket, "Expected ']' after list elements.");
        Expr::List(ListLit {
            elements,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn map_literal(&mut self) -> Expr {
        let start = self.previous().clone();
        let mut entries: Vec<MapEntry> = Vec::new();
        if !self.check(TokenType::RightBrace) {
            if let Some(entry) = self.map_entry() {
                entries.push(entry);
            }
            while self.match_tokens(&[TokenType::Comma]) {
                if self.check(TokenType::RightBrace) {
                    break;
                }
                if let Some(entry) = self.map_entry() {
                    entries.push(entry);
                }
            }
        }
        let end = self.consume(TokenType::RightBrace, "Expected '}' after map entries.");
        Expr::Map(MapLit {
            entries,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn map_entry(&mut self) -> Option<MapEntry> {
        let key = self.expression(precedence::NONE);
        self.consume(TokenType::Colon, "Expected ':' after map key.");
        let value = self.expression(precedence::NONE);
        let span = self.make_span_from_expr(&key, self.previous());
        Some(MapEntry { key, value, span })
    }

    fn template_ref(&mut self) -> Expr {
        let start = self.previous().clone();
        let expr = self.expression(precedence::NONE);
        let end = self.consume(TokenType::TemplateClose, "Expected '}}' to close template.");
        Expr::TemplateRef(TemplateRef {
            expression: Box::new(expr),
            span: self.make_span_from_token(&start, &end),
        })
    }

    // ------------------------------------------------------------------
    // Declarations / statements
    // ------------------------------------------------------------------

    fn declaration(&mut self) -> Option<Stmt> {
        if self.at_end() {
            return None;
        }
        // Skip stray closing braces at top level
        if self.check(TokenType::RightBrace) {
            self.advance();
            return None;
        }

        // v1.12: decorators
        let mut isolation_level = "standard".to_string();
        if self.check(TokenType::At) {
            isolation_level = self.parse_decorator();
        }

        if self.match_tokens(&[TokenType::Shared]) {
            if isolation_level != "standard" {
                self.error(&format!(
                    "Decorator '@{isolation_level}' can only be applied to agent declarations."
                ));
            }
            if self.match_tokens(&[TokenType::Let, TokenType::Const]) {
                return Some(self.var_decl(true));
            }
            if self.match_tokens(&[TokenType::Store]) {
                return Some(self.shared_store_decl());
            }
            self.error("Expected 'let', 'const', or 'store' after 'shared'.");
            return None;
        }
        if self.match_tokens(&[TokenType::Let, TokenType::Const]) {
            if isolation_level != "standard" {
                self.error(&format!(
                    "Decorator '@{isolation_level}' can only be applied to agent declarations."
                ));
            }
            return Some(self.var_decl(false));
        }
        if self.match_tokens(&[TokenType::If]) {
            return Some(self.if_stmt());
        }
        if self.match_tokens(&[TokenType::For]) {
            return Some(self.for_stmt());
        }
        if self.match_tokens(&[TokenType::While]) {
            return Some(self.while_stmt());
        }
        if self.match_tokens(&[TokenType::Break]) {
            return Some(self.break_stmt());
        }
        if self.match_tokens(&[TokenType::Continue]) {
            return Some(self.continue_stmt());
        }
        if self.match_tokens(&[TokenType::Return]) {
            return Some(self.return_stmt());
        }
        if self.match_tokens(&[TokenType::Fn]) {
            if isolation_level != "standard" {
                self.error(&format!(
                    "Decorator '@{isolation_level}' can only be applied to agent declarations, not functions."
                ));
            }
            return Some(Stmt::FunctionDecl(self.function_decl()));
        }
        if self.match_tokens(&[TokenType::Main]) {
            return Some(self.main_block());
        }
        if self.match_tokens(&[TokenType::Import]) {
            return Some(self.import_stmt());
        }
        if self.match_tokens(&[TokenType::Alias]) {
            return Some(self.alias_stmt());
        }
        if self.match_tokens(&[TokenType::Agent]) {
            let mut agent_node = self.agent_decl();
            if isolation_level != "standard" {
                agent_node.isolation_level = isolation_level.clone();
            }
            return Some(Stmt::AgentDecl(agent_node));
        }
        if self.match_tokens(&[TokenType::Try]) {
            return Some(self.try_stmt());
        }
        if self.match_tokens(&[TokenType::Throw]) {
            return Some(self.throw_stmt());
        }
        if self.match_tokens(&[TokenType::Assert]) {
            return Some(self.assert_stmt());
        }
        if self.match_tokens(&[TokenType::Match]) {
            return Some(self.match_stmt());
        }
        if self.match_tokens(&[TokenType::Protocol]) {
            if isolation_level != "standard" {
                self.error(&format!(
                    "Decorator '@{isolation_level}' can only be applied to agent declarations."
                ));
            }
            return Some(self.protocol_decl());
        }
        if self.match_tokens(&[TokenType::Impl]) {
            if isolation_level != "standard" {
                self.error(&format!(
                    "Decorator '@{isolation_level}' can only be applied to agent declarations."
                ));
            }
            return Some(self.impl_decl());
        }

        // LLM keyword disambiguation
        if self.check(TokenType::Llm) {
            return self.llm_stmt();
        }

        if self.at_end() {
            return None;
        }
        Some(self.expr_stmt())
    }

    fn parse_decorator(&mut self) -> String {
        self.advance(); // consume @
        if self.check(TokenType::Identifier) {
            let decorator_name = self.current().lexeme.clone();
            let mapped = match decorator_name.as_str() {
                "开放" => "open".to_string(),
                "严格" => "strict".to_string(),
                "沙箱" => "sandbox".to_string(),
                other => other.to_string(),
            };
            if matches!(mapped.as_str(), "sandbox" | "open" | "strict") {
                self.advance();
                return mapped;
            }
            self.error(&format!(
                "Unknown decorator '@{decorator_name}'. Expected @sandbox, @open, or @strict."
            ));
        } else {
            self.error("Expected decorator name after '@'.");
        }
        "standard".to_string()
    }

    fn llm_stmt(&mut self) -> Option<Stmt> {
        self.advance(); // consume LLM
        if self.check(TokenType::If) {
            Some(self.llm_if_stmt())
        } else if self.check(TokenType::Act) {
            Some(self.llm_act_stmt())
        } else {
            self.error("Expected 'if' or 'act' after 'llm'.");
            self.synchronize();
            None
        }
    }

    fn var_decl(&mut self, shared: bool) -> Stmt {
        let mutable = self.previous().kind == TokenType::Let;
        let name_tok = self.consume(
            TokenType::Identifier,
            "Expected variable name after 'let'/'const'.",
        );
        let mut type_annotation: Option<Box<TypeRef>> = None;
        if self.match_tokens(&[TokenType::Colon]) {
            type_annotation = Some(Box::new(self.parse_type()));
        }
        let mut initializer: Option<Box<Expr>> = None;
        if self.match_tokens(&[TokenType::Assign]) {
            initializer = Some(Box::new(self.expression(precedence::NONE)));
        }
        let end = self.previous().clone();
        Stmt::VarDecl(VarDecl {
            name: name_tok.lexeme.clone(),
            type_annotation,
            initializer,
            mutable,
            span: self.make_span_from_token(&name_tok, &end),
            shared,
        })
    }

    fn if_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        let condition;
        if self.match_tokens(&[TokenType::LeftParen]) {
            condition = self.expression(precedence::NONE);
            self.consume(TokenType::RightParen, "Expected ')' after if condition.");
            if self.check(TokenType::And) || self.check(TokenType::Or) {
                let op_lexeme = self.current().lexeme.clone();
                self.error(&format!(
                    "Unexpected '{op_lexeme}' after if condition. The parentheses after 'if' are consumed as the condition delimiters. Wrap the entire condition in double parentheses: 'if ((...){op_lexeme} ...) {{ }}'."
                ));
            }
        } else {
            condition = self.expression(precedence::NONE);
        }
        self.consume(TokenType::LeftBrace, "Expected '{' before if body.");
        let then_body = self.block_body();
        let mut else_branch: Option<Box<Stmt>> = None;
        if self.match_tokens(&[TokenType::Else]) {
            if self.check(TokenType::LeftBrace) {
                self.advance();
                else_branch = Some(Box::new(self.block_body()));
            } else if self.check(TokenType::If) {
                self.advance();
                else_branch = Some(Box::new(self.if_stmt()));
            }
        }
        let end = self.previous().clone();
        Stmt::If(IfStmt {
            condition: Box::new(condition),
            then_branch: Box::new(then_body),
            else_branch,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn for_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        let iter_tok = self.consume(TokenType::Identifier, "Expected iterator after 'for'.");
        self.consume(TokenType::In, "Expected 'in' after iterator.");
        let iterable = self.expression(precedence::NONE);
        self.consume(TokenType::LeftBrace, "Expected '{' before for body.");
        let body = self.block_body();
        let end = self.previous().clone();
        Stmt::For(ForStmt {
            iterator: Some(Variable {
                name: iter_tok.lexeme.clone(),
                span: iter_tok.span(),
            }),
            iterable: Box::new(iterable),
            body: Box::new(body),
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn while_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        let condition;
        if self.match_tokens(&[TokenType::LeftParen]) {
            condition = self.expression(precedence::NONE);
            self.consume(TokenType::RightParen, "Expected ')' after while condition.");
            if self.check(TokenType::And) || self.check(TokenType::Or) {
                let op_lexeme = self.current().lexeme.clone();
                self.error(&format!(
                    "Unexpected '{op_lexeme}' after while condition. The parentheses after 'while' are consumed as the condition delimiters. Wrap the entire condition in double parentheses: 'while ((...){op_lexeme} ...) {{ }}'."
                ));
            }
        } else {
            condition = self.expression(precedence::NONE);
        }
        self.consume(TokenType::LeftBrace, "Expected '{' before while body.");
        let body = self.block_body();
        let end = self.previous().clone();
        Stmt::While(WhileStmt {
            condition: Box::new(condition),
            body: Box::new(body),
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn break_stmt(&mut self) -> Stmt {
        let prev = self.previous().clone();
        Stmt::Break(BreakStmt { span: prev.span() })
    }

    fn continue_stmt(&mut self) -> Stmt {
        let prev = self.previous().clone();
        Stmt::Continue(ContinueStmt { span: prev.span() })
    }

    fn return_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        let mut value: Option<Box<Expr>> = None;
        if !self.check(TokenType::Semicolon)
            && !self.check(TokenType::RightBrace)
            && !self.check(TokenType::Eof)
        {
            value = Some(Box::new(self.expression(precedence::NONE)));
        }
        self.match_tokens(&[TokenType::Semicolon]);
        Stmt::Return(ReturnStmt {
            value,
            span: start.span(),
        })
    }

    fn expr_stmt(&mut self) -> Stmt {
        let expr = self.expression(precedence::NONE);
        self.match_tokens(&[TokenType::Semicolon]);
        let span = match &expr {
            Expr::Literal(Lit { span, .. }) => span.clone(),
            Expr::Variable(Variable { span, .. }) => span.clone(),
            Expr::Binary(Binary { span, .. }) => span.clone(),
            Expr::Pipe(Pipe { span, .. }) => span.clone(),
            Expr::Unary(Unary { span, .. }) => span.clone(),
            Expr::Grouping(Grouping { span, .. }) => span.clone(),
            Expr::Call(Call { span, .. }) => span.clone(),
            Expr::Index(Index { span, .. }) => span.clone(),
            Expr::Access(Access { span, .. }) => span.clone(),
            Expr::List(ListLit { span, .. }) => span.clone(),
            Expr::Map(MapLit { span, .. }) => span.clone(),
            Expr::TemplateRef(TemplateRef { span, .. }) => span.clone(),
            Expr::Type(TypeRef { span, .. }) => span.clone(),
            Expr::OptionalType(OptionalType { span, .. }) => span.clone(),
            Expr::UnionType(UnionType { span, .. }) => span.clone(),
            Expr::LiteralType(LiteralType { span, .. }) => span.clone(),
            Expr::Lambda(Lambda { span, .. }) => span.clone(),
            Expr::Spawn(Spawn { span, .. }) => span.clone(),
            Expr::LlmAct(LlmAct { span, .. }) => span.clone(),
            Expr::MatchExpr(MatchExprNode { span, .. }) => span.clone(),
            Expr::RangePattern(RangePattern { span, .. }) => span.clone(),
            Expr::WildcardPattern(WildcardPattern { span }) => span.clone(),
            Expr::VariablePattern(VariablePattern { span, .. }) => span.clone(),
            Expr::TypePattern(TypePattern { span, .. }) => span.clone(),
        };
        Stmt::Expr(ExprStmt {
            expression: expr,
            span,
        })
    }

    // -- agent declaration ------------------------------------------------

    fn agent_decl(&mut self) -> AgentDecl {
        let name_tok = self.consume(TokenType::Identifier, "Expected agent name after 'agent'.");
        let mut params: Vec<AgentParam> = Vec::new();
        if self.check(TokenType::LeftParen) {
            self.advance();
            if !self.check(TokenType::RightParen) {
                params.push(self.agent_param());
                while self.match_tokens(&[TokenType::Comma]) {
                    if self.check(TokenType::RightParen) {
                        break;
                    }
                    params.push(self.agent_param());
                }
            }
            self.consume(
                TokenType::RightParen,
                "Expected ')' after agent parameters.",
            );
        }
        self.consume(TokenType::LeftBrace, "Expected '{' after agent name.");
        let mut declarations: Vec<Declaration> = Vec::new();
        let mut prompt: Option<PromptDef> = None;
        let mut logic: Option<Box<Stmt>> = None;
        let mut agent_functions: Vec<FunctionDecl> = Vec::new();
        let mut agent_function_vars: Vec<VarDecl> = Vec::new();
        let mut context_config: Option<ContextConfig> = None;
        let mut transcript_level = "none".to_string();
        let mut output_contract: Option<String> = None;

        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            if self.match_tokens(&[TokenType::Prompt]) {
                if self.match_tokens(&[TokenType::String, TokenType::TripleQuoteString]) {
                    let content = match &self.previous().literal {
                        LiteralValue::Str(s) => s.clone(),
                        _ => String::new(),
                    };
                    prompt = Some(PromptDef {
                        content,
                        span: self.previous().span(),
                    });
                } else {
                    self.error("Expected string after 'prompt'.");
                    prompt = Some(PromptDef {
                        content: String::new(),
                        span: self.previous().span(),
                    });
                }
            } else if self.match_tokens(&[TokenType::Main]) {
                logic = Some(Box::new(self.main_block()));
            } else if self.match_tokens(&[TokenType::Transcript]) {
                if self.match_tokens(&[TokenType::String]) {
                    let level = match &self.previous().literal {
                        LiteralValue::Str(s) => s.clone(),
                        _ => "none".to_string(),
                    };
                    if !matches!(level.as_str(), "none" | "memory" | "persistent") {
                        self.error(&format!(
                            "Invalid transcript level: {level}. Must be 'none', 'memory', or 'persistent'."
                        ));
                    }
                    transcript_level = level;
                } else {
                    self.error("Expected string after 'transcript'.");
                }
            } else if matches!(
                self.current().lexeme.as_str(),
                "output_contract" | "输出契约"
            ) {
                self.advance();
                self.consume(TokenType::Colon, "Expected ':' after 'output_contract'.");
                if self.match_tokens(&[TokenType::String]) {
                    let contract_value = match &self.previous().literal {
                        LiteralValue::Str(s) => s.clone(),
                        _ => String::new(),
                    };
                    if !matches!(contract_value.as_str(), "json" | "text") {
                        self.error(&format!(
                            "Invalid output_contract: {contract_value}. Must be 'json' or 'text'."
                        ));
                    }
                    output_contract = Some(contract_value);
                } else if self.match_tokens(&[TokenType::LeftBrace]) {
                    output_contract = Some(self.parse_output_contract_dict());
                } else {
                    self.error("Expected string or dict after 'output_contract:'.");
                }
            } else if self.match_tokens(&[TokenType::Fn]) {
                self.function_decl();
            } else if self.match_tokens(&[TokenType::Functions]) {
                self.consume(TokenType::LeftBrace, "Expected '{' after 'functions'.");
                while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
                    if self.match_tokens(&[TokenType::Fn]) {
                        agent_functions.push(self.function_decl());
                    } else if self.check(TokenType::Let) || self.check(TokenType::Const) {
                        self.advance();
                        if let Stmt::VarDecl(vd) = self.var_decl(false) {
                            agent_function_vars.push(vd);
                        }
                    } else {
                        self.error(&format!(
                            "Expected 'fn', 'let', or 'const' inside functions block, got {}",
                            self.current().kind.name()
                        ));
                        self.synchronize();
                    }
                }
                self.consume(TokenType::RightBrace, "Expected '}' after functions block.");
            } else if self.is_context_keyword(&["context", "上下文"]) {
                self.advance();
                self.consume(TokenType::LeftBrace, "Expected '{' after 'context'.");
                let mut compression = "graduated".to_string();
                let mut cache_aware = true;
                let mut working_memory = true;
                let mut working_memory_tokens: i64 = 5000;
                let context_span = self.previous().span();
                while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
                    let mut key = self.current().lexeme.clone();
                    self.advance();
                    while self.check(TokenType::Minus) {
                        self.advance();
                        key.push('-');
                        key.push_str(&self.current().lexeme);
                        self.advance();
                    }
                    match key.as_str() {
                        "compression" | "压缩" => {
                            compression = match &self
                                .consume(TokenType::String, "Expected string value for compression")
                                .literal
                            {
                                LiteralValue::Str(s) => s.clone(),
                                _ => "graduated".to_string(),
                            };
                        }
                        "cache-aware" | "缓存感知" => {
                            let val_tok = self.advance();
                            cache_aware = matches!(val_tok.lexeme.as_str(), "true" | "是");
                        }
                        "working-memory" | "工作记忆" => {
                            let val_tok = self.advance();
                            working_memory = matches!(val_tok.lexeme.as_str(), "true" | "是");
                        }
                        "working-memory-tokens" | "工作记忆词元" | "工作记忆令牌" => {
                            working_memory_tokens = match &self.advance().literal {
                                LiteralValue::Int(i) => {
                                    i.to_string().parse::<i64>().unwrap_or(5000)
                                }
                                _ => 5000,
                            };
                        }
                        _ => {
                            self.error(&format!("Unknown context option: {key}"));
                            self.synchronize();
                        }
                    }
                }
                self.consume(TokenType::RightBrace, "Expected '}' after context block.");
                context_config = Some(ContextConfig {
                    compression,
                    cache_aware,
                    working_memory,
                    working_memory_tokens,
                    span: Some(context_span),
                });
            } else if matches!(
                self.current().kind,
                TokenType::Description
                    | TokenType::Model
                    | TokenType::Tools
                    | TokenType::Memory
                    | TokenType::Temperature
                    | TokenType::MaxTurns
                    | TokenType::MaxTokens
                    | TokenType::Streaming
                    | TokenType::ThinkingMode
                    | TokenType::ReasoningEffort
            ) || self.is_context_keyword(&["memory", "提供商"])
                || self.current().lexeme == "记忆"
            {
                declarations.push(self.declaration_block());
            } else {
                self.error(&format!(
                    "Unexpected token in agent body: {}",
                    self.current().kind.name()
                ));
                self.synchronize();
            }
        }
        let end = self.consume(TokenType::RightBrace, "Expected '}' after agent body.");
        let has_streaming = declarations.iter().any(|d| d.streaming);
        AgentDecl {
            name: name_tok.lexeme.clone(),
            params,
            declarations,
            prompt,
            logic,
            span: self.make_span_from_token(&name_tok, &end),
            functions: agent_functions,
            function_vars: agent_function_vars,
            has_streaming,
            isolation_level: "standard".to_string(),
            context_config,
            transcript: transcript_level,
            output_contract,
        }
    }

    fn agent_param(&mut self) -> AgentParam {
        let name_tok = self.consume(TokenType::Identifier, "Expected parameter name.");
        let mut type_annotation: Option<Box<TypeRef>> = None;
        let mut default_value: Option<Box<Expr>> = None;
        if self.match_tokens(&[TokenType::Colon]) {
            type_annotation = Some(Box::new(self.parse_type()));
        }
        if self.match_tokens(&[TokenType::Assign]) {
            default_value = Some(Box::new(self.expression(precedence::NONE)));
        }
        let end = self.previous().clone();
        AgentParam {
            name: name_tok.lexeme.clone(),
            type_annotation,
            default_value,
            span: self.make_span_from_token(&name_tok, &end),
        }
    }

    fn declaration_block(&mut self) -> Declaration {
        let start = self.advance().clone(); // consume the config keyword
        let mut token_type = start.kind;

        if token_type == TokenType::Identifier && matches!(start.lexeme.as_str(), "memory" | "记忆")
        {
            token_type = TokenType::Memory;
        }

        self.match_tokens(&[TokenType::Assign]);

        let value = if self.check(TokenType::String)
            || self.check(TokenType::TripleQuoteString)
            || self.check(TokenType::Number)
        {
            // Python branches STRING and NUMBER both build `LiteralNode(value, span)`;
            // in Rust `LiteralValue` already carries the variant, so they collapse.
            self.advance();
            let value_tok = self.previous().clone();
            Expr::Literal(Lit {
                value: value_tok.literal.clone(),
                span: value_tok.span(),
            })
        } else if self.check(TokenType::True) || self.check(TokenType::False) {
            self.advance();
            let value_tok = self.previous().clone();
            Expr::Literal(Lit {
                value: LiteralValue::Bool(value_tok.kind == TokenType::True),
                span: value_tok.span(),
            })
        } else if self.check(TokenType::LeftBracket) {
            self.advance();
            let mut items: Vec<LiteralValue> = Vec::new();
            while !self.check(TokenType::RightBracket) && !self.check(TokenType::Eof) {
                if self.check(TokenType::String) || self.check(TokenType::TripleQuoteString) {
                    self.advance();
                    if let LiteralValue::Str(s) = &self.previous().literal {
                        items.push(LiteralValue::Str(s.clone()));
                    }
                }
                if self.check(TokenType::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            if self.check(TokenType::RightBracket) {
                self.advance();
            }
            let span = self.make_span_from_token(&start, self.previous());
            // Python stores a Python list; we store it as Str joined list marker
            // is not possible — Python's LiteralNode.value can be a list, but our
            // LiteralValue cannot. Store a Str with "\u{1f}joined\u{1f}" marker
            // so the AST printer produces `str(list)`-compatible output is not
            // needed (declaration tools only appear inside agent decls, which
            // the printer omits). Keep items as a marker list in Str.
            let joined = items
                .iter()
                .map(|v| match v {
                    LiteralValue::Str(s) => s.clone(),
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\u{1e}");
            Expr::Literal(Lit {
                value: LiteralValue::Str(format!("\u{1f}{joined}\u{1f}")),
                span,
            })
        } else if token_type == TokenType::Tools && self.check(TokenType::Identifier) {
            let name_tok = self.advance().clone();
            Expr::Variable(Variable {
                name: name_tok.lexeme.clone(),
                span: name_tok.span(),
            })
        } else {
            self.error(&format!("Expected value after '{}'.", start.lexeme));
            Expr::Literal(Lit {
                value: LiteralValue::Null,
                span: start.span(),
            })
        };

        let end = self.previous().clone();
        let span = self.make_span_from_token(&start, &end);

        let field_name = match token_type {
            TokenType::Description => Some("description"),
            TokenType::Model => Some("model"),
            TokenType::Tools => Some("tools"),
            TokenType::Memory => Some("memory"),
            TokenType::Temperature => Some("temperature"),
            TokenType::MaxTurns => Some("max_turns"),
            TokenType::MaxTokens => Some("max_tokens"),
            TokenType::Streaming => Some("streaming"),
            TokenType::ThinkingMode => Some("thinking_mode"),
            TokenType::ReasoningEffort => Some("reasoning_effort"),
            _ => None,
        };
        let field_name = if field_name.is_none() && start.lexeme == "提供商" {
            Some("provider")
        } else {
            field_name
        };

        let streaming_value = matches!(field_name, Some("streaming")) && {
            matches!(&value, Expr::Literal(Lit { value: LiteralValue::Bool(b), .. }) if *b)
        };

        let opt = |name: &str, value: &Expr| -> Option<Expr> {
            if field_name == Some(name) {
                Some(value.clone())
            } else {
                None
            }
        };

        Declaration {
            description: opt("description", &value),
            model: opt("model", &value),
            tools: opt("tools", &value),
            memory: opt("memory", &value),
            temperature: opt("temperature", &value),
            max_turns: opt("max_turns", &value),
            max_tokens: opt("max_tokens", &value),
            span,
            streaming: streaming_value,
            thinking_mode: opt("thinking_mode", &value),
            reasoning_effort: opt("reasoning_effort", &value),
            provider: opt("provider", &value),
        }
    }

    fn parse_output_contract_dict(&mut self) -> String {
        // Python stores a dict; we flatten to a canonical JSON-like string.
        // Only used in agent declarations (omitted by the printer).
        let mut parts: Vec<String> = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            let start = self.current().clone();
            let key = match &self.current().literal {
                LiteralValue::Str(s) => s.clone(),
                _ => self.current().lexeme.clone(),
            };
            self.advance();
            self.consume(
                TokenType::Colon,
                "Expected ':' after key in output_contract dict.",
            );
            let mut val = String::new();
            if self.match_tokens(&[TokenType::String]) {
                if let LiteralValue::Str(s) = &self.previous().literal {
                    val = s.clone();
                }
            } else if self.match_tokens(&[TokenType::LeftBracket]) {
                let mut arr: Vec<String> = Vec::new();
                while !self.check(TokenType::RightBracket) && !self.check(TokenType::Eof) {
                    if self.match_tokens(&[TokenType::String]) {
                        if let LiteralValue::Str(s) = &self.previous().literal {
                            arr.push(s.clone());
                        }
                    } else if self.match_tokens(&[TokenType::Number]) {
                        if let LiteralValue::Int(i) = &self.previous().literal {
                            arr.push(i.to_string());
                        }
                    } else {
                        self.error("Expected string or number in array.");
                    }
                    if !self.check(TokenType::RightBracket) {
                        self.consume(TokenType::Comma, "Expected ',' in array.");
                    }
                }
                self.consume(TokenType::RightBracket, "Expected ']' after array.");
                val = format!("[{}]", arr.join(","));
            } else if self.match_tokens(&[TokenType::LeftBrace]) {
                val = self.parse_output_contract_dict();
            } else if self.match_tokens(&[TokenType::Number]) {
                if let LiteralValue::Int(i) = &self.previous().literal {
                    val = i.to_string();
                }
            } else if matches!(
                self.current().lexeme.as_str(),
                "true" | "false" | "是" | "否"
            ) {
                val = matches!(self.current().lexeme.as_str(), "true" | "是").to_string();
                self.advance();
            } else {
                self.error("Unexpected value type in output_contract dict.");
            }
            parts.push(format!("{key}:{val}"));
            if !self.check(TokenType::RightBrace) {
                self.consume(TokenType::Comma, "Expected ',' in output_contract dict.");
            }
            let _ = start;
        }
        self.consume(
            TokenType::RightBrace,
            "Expected '}' after output_contract dict.",
        );
        format!("{{{}}}", parts.join(","))
    }

    fn shared_store_decl(&mut self) -> Stmt {
        let label = "store";
        let name_tok = self.consume(
            TokenType::Identifier,
            &format!("Expected {label} name after 'shared {label}'."),
        );
        self.consume(
            TokenType::LeftBrace,
            &format!("Expected '{{' after {label} name."),
        );

        let mut fields: Vec<VarDecl> = Vec::new();
        let mut methods: Vec<FunctionDecl> = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            if self.match_tokens(&[TokenType::Fn]) {
                methods.push(self.function_decl());
            } else if self.check(TokenType::Let) || self.check(TokenType::Const) {
                self.advance();
                if let Stmt::VarDecl(vd) = self.var_decl(false) {
                    fields.push(vd);
                }
            } else {
                self.error(&format!(
                    "Expected 'fn', 'let', or 'const' inside shared {label}, got {}",
                    self.current().kind.name()
                ));
                self.synchronize();
            }
        }

        let end = self.consume(
            TokenType::RightBrace,
            &format!("Expected '}}' after shared {label} body."),
        );
        Stmt::SharedStoreDecl(SharedStoreDecl {
            name: name_tok.lexeme.clone(),
            fields,
            methods,
            span: self.make_span_from_token(&name_tok, &end),
        })
    }

    // -- blocks and functions ---------------------------------------------

    fn main_block(&mut self) -> Stmt {
        let start = self.previous().clone();
        self.consume(TokenType::LeftBrace, "Expected '{' after 'main'.");
        let mut body: Vec<Stmt> = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            let prev_pos = self.pos;
            if let Some(stmt) = self.statement() {
                body.push(stmt);
            }
            if self.pos == prev_pos {
                break;
            }
        }
        let end = self.consume(TokenType::RightBrace, "Expected '}' after main block.");
        Stmt::MainBlock(MainBlock {
            body,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn block_body(&mut self) -> Stmt {
        let start = self.previous().clone();
        let mut body: Vec<Stmt> = Vec::new();
        while !self.check(TokenType::RightBrace)
            && !self.check(TokenType::Eof)
            && !self.check(TokenType::Catch)
            && !self.check(TokenType::Finally)
            && !self.check(TokenType::Case)
            && !self.check(TokenType::Default)
            && !self.check(TokenType::Branch)
        {
            let prev_pos = self.pos;
            if let Some(stmt) = self.statement() {
                body.push(stmt);
            }
            if self.pos == prev_pos {
                break;
            }
        }
        let end = self.consume(TokenType::RightBrace, "Expected '}'");
        Stmt::MainBlock(MainBlock {
            body,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn block_body_list(&mut self) -> Vec<Stmt> {
        let mut body: Vec<Stmt> = Vec::new();
        while !self.check(TokenType::RightBrace)
            && !self.check(TokenType::Eof)
            && !self.check(TokenType::Catch)
            && !self.check(TokenType::Finally)
            && !self.check(TokenType::Case)
            && !self.check(TokenType::Default)
            && !self.check(TokenType::Branch)
        {
            let prev_pos = self.pos;
            if let Some(stmt) = self.statement() {
                body.push(stmt);
            }
            if self.pos == prev_pos {
                break;
            }
        }
        body
    }

    fn statement(&mut self) -> Option<Stmt> {
        self.declaration()
    }

    fn function_decl(&mut self) -> FunctionDecl {
        let start = self.previous().clone();
        let name_tok = self.consume(TokenType::Identifier, "Expected function name.");
        self.consume(TokenType::LeftParen, "Expected '(' after function name.");
        let mut params: Vec<AgentParam> = Vec::new();
        if !self.check(TokenType::RightParen) {
            params.push(self.agent_param());
            while self.match_tokens(&[TokenType::Comma]) {
                if self.check(TokenType::RightParen) {
                    break;
                }
                params.push(self.agent_param());
            }
        }
        self.consume(TokenType::RightParen, "Expected ')' after parameters.");
        let mut ret_type: Option<Box<TypeRef>> = None;
        if self.match_tokens(&[TokenType::Colon]) {
            ret_type = Some(Box::new(self.parse_type()));
        }
        self.consume(TokenType::LeftBrace, "Expected '{' before function body.");
        let mut body_stmts: Vec<Stmt> = Vec::new();
        while !self.check(TokenType::RightBrace)
            && !self.check(TokenType::Eof)
            && !self.check(TokenType::Prompt)
            && !self.check(TokenType::Main)
            && !self.check(TokenType::Fn)
            && !self.check(TokenType::Description)
            && !self.check(TokenType::Model)
            && !self.check(TokenType::Tools)
            && !self.check(TokenType::Memory)
            && !self.check(TokenType::Temperature)
            && !self.check(TokenType::MaxTurns)
        {
            if self.is_context_keyword(&["memory"]) || self.current().lexeme == "记忆" {
                break;
            }
            let prev_pos = self.pos;
            if let Some(s) = self.statement() {
                body_stmts.push(s);
            }
            if self.pos == prev_pos {
                break;
            }
        }
        let end = self.consume(TokenType::RightBrace, "Expected '}' after function body.");
        let fn_body = FnBlock {
            body: body_stmts,
            span: self.make_span_from_token(self.previous(), &end),
        };
        FunctionDecl {
            name: name_tok.lexeme.clone(),
            params,
            return_type: ret_type,
            body: fn_body,
            span: self.make_span_from_token(&start, &end),
        }
    }

    fn lambda_expr(&mut self) -> Expr {
        let start = self.previous().clone();
        self.consume(TokenType::LeftParen, "Expected '(' after 'fn'.");
        let mut params: Vec<AgentParam> = Vec::new();
        if !self.check(TokenType::RightParen) {
            params.push(self.agent_param());
            while self.match_tokens(&[TokenType::Comma]) {
                if self.check(TokenType::RightParen) {
                    break;
                }
                params.push(self.agent_param());
            }
        }
        self.consume(TokenType::RightParen, "Expected ')' after parameters.");

        let mut ret_type: Option<Box<TypeRef>> = None;
        if self.match_tokens(&[TokenType::Colon]) {
            ret_type = Some(Box::new(self.parse_type()));
        }

        self.consume(TokenType::LeftBrace, "Expected '{' before lambda body.");
        let mut body_stmts: Vec<Stmt> = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            let prev_pos = self.pos;
            if let Some(s) = self.statement() {
                body_stmts.push(s);
            }
            if self.pos == prev_pos {
                break;
            }
        }
        let end = self.consume(TokenType::RightBrace, "Expected '}' after lambda body.");
        let fn_body = FnBlock {
            body: body_stmts,
            span: self.make_span_from_token(self.previous(), &end),
        };

        Expr::Lambda(Lambda {
            params,
            return_type: ret_type,
            body: fn_body,
            span: self.make_span_from_token(&start, &end),
        })
    }

    // -- protocol / impl ----------------------------------------------------

    fn protocol_decl(&mut self) -> Stmt {
        let start = self.previous().clone();
        let name_tok = self.consume(TokenType::Identifier, "Expected protocol name.");
        self.consume(TokenType::LeftBrace, "Expected '{' after protocol name.");

        let mut methods: Vec<FunctionDecl> = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            let prev_pos = self.pos;
            self.consume(TokenType::Fn, "Expected 'fn' in protocol.");
            let method_name = self.consume(TokenType::Identifier, "Expected method name.");
            self.consume(TokenType::LeftParen, "Expected '(' after method name.");

            let mut params: Vec<AgentParam> = Vec::new();
            if !self.check(TokenType::RightParen) {
                params.push(self.agent_param());
                while self.match_tokens(&[TokenType::Comma]) {
                    if self.check(TokenType::RightParen) {
                        break;
                    }
                    params.push(self.agent_param());
                }
            }
            self.consume(TokenType::RightParen, "Expected ')' after parameters.");

            let mut ret_type: Option<Box<TypeRef>> = None;
            if self.match_tokens(&[TokenType::Colon]) {
                ret_type = Some(Box::new(self.parse_type()));
            }

            let empty_span = self.make_span_from_token(self.previous(), self.previous());
            let method = FunctionDecl {
                name: method_name.lexeme.clone(),
                params,
                return_type: ret_type,
                body: FnBlock {
                    body: Vec::new(),
                    span: empty_span,
                },
                span: self.make_span_from_token(&start, self.previous()),
            };
            methods.push(method);
            if self.pos == prev_pos {
                self.advance();
            }
        }

        let end = self.consume(TokenType::RightBrace, "Expected '}' after protocol body.");
        Stmt::ProtocolDecl(ProtocolDecl {
            name: name_tok.lexeme.clone(),
            methods,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn impl_decl(&mut self) -> Stmt {
        let start = self.previous().clone();
        let protocol_name = self.consume(TokenType::Identifier, "Expected protocol name.");
        self.consume(TokenType::For, "Expected 'for' after protocol name.");
        let struct_name = self.consume(TokenType::Identifier, "Expected struct name.");
        self.consume(TokenType::LeftBrace, "Expected '{' after struct name.");

        let mut methods: Vec<FunctionDecl> = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            if self.match_tokens(&[TokenType::Fn]) {
                methods.push(self.function_decl());
            } else {
                self.error("Expected 'fn' in impl block.");
                self.advance();
            }
        }

        let end = self.consume(TokenType::RightBrace, "Expected '}' after impl body.");
        Stmt::ImplDecl(ImplDecl {
            protocol_name: protocol_name.lexeme.clone(),
            struct_name: struct_name.lexeme.clone(),
            methods,
            span: self.make_span_from_token(&start, &end),
        })
    }

    // -- imports -------------------------------------------------------------

    fn import_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        if self.check(TokenType::Identifier) && self.current().lexeme == "std" {
            return self.import_stdlib_module(start);
        }
        let path_tok = self.consume(TokenType::String, "Expected string path after 'import'.");
        let mut alias: Option<String> = None;
        if self.match_tokens(&[TokenType::As]) {
            let alias_tok = self.consume(TokenType::Identifier, "Expected alias after 'as'.");
            alias = Some(alias_tok.lexeme.clone());
        }
        let end = self.previous().clone();
        let module_path = match &path_tok.literal {
            LiteralValue::Str(s) => s.clone(),
            _ => path_tok.lexeme.clone(),
        };
        Stmt::Import(ImportStmt {
            module_path,
            alias,
            span: self.make_span_from_token(&start, &end),
            is_stdlib_module: false,
            module_name: None,
            imported_names: None,
            namespace: None,
        })
    }

    fn import_stdlib_module(&mut self, start: Token) -> Stmt {
        self.advance(); // consume 'std'
        self.consume(TokenType::Dot, "Expected '.' after 'std'.");
        let is_keyword_module = matches!(
            self.current().kind,
            TokenType::Tools | TokenType::Transcript | TokenType::Llm
        );
        let module_tok = if self.check(TokenType::Identifier) || is_keyword_module {
            self.advance().clone()
        } else {
            self.consume(TokenType::Identifier, "Expected module name after 'std.'.")
                .clone()
        };
        let module_name = format!("std.{}", module_tok.lexeme);

        if self.match_tokens(&[TokenType::Dot]) {
            let mut imported_names: Vec<String> = Vec::new();
            if self.match_tokens(&[TokenType::Star]) {
                imported_names.push("*".to_string());
            } else if self.match_tokens(&[TokenType::LeftBrace]) {
                if !self.check(TokenType::RightBrace) {
                    let name_tok = self.consume(TokenType::Identifier, "Expected function name.");
                    imported_names.push(name_tok.lexeme.clone());
                    while self.match_tokens(&[TokenType::Comma]) {
                        let name_tok =
                            self.consume(TokenType::Identifier, "Expected function name.");
                        imported_names.push(name_tok.lexeme.clone());
                    }
                }
                self.consume(TokenType::RightBrace, "Expected '}' after import list.");
            } else {
                self.error("Expected '{' or '*' after 'std.module.'.");
                imported_names = Vec::new();
            }
            let end = self.previous().clone();
            Stmt::Import(ImportStmt {
                module_path: String::new(),
                alias: None,
                span: self.make_span_from_token(&start, &end),
                is_stdlib_module: true,
                module_name: Some(module_name),
                imported_names: Some(imported_names),
                namespace: None,
            })
        } else if self.match_tokens(&[TokenType::As]) {
            let ns_tok = self.consume(TokenType::Identifier, "Expected namespace after 'as'.");
            let end = self.previous().clone();
            Stmt::Import(ImportStmt {
                module_path: String::new(),
                alias: None,
                span: self.make_span_from_token(&start, &end),
                is_stdlib_module: true,
                module_name: Some(module_name),
                imported_names: Some(vec!["*".to_string()]),
                namespace: Some(ns_tok.lexeme.clone()),
            })
        } else {
            self.error("Expected '.', or 'as' after 'std.module'.");
            let end = self.previous().clone();
            Stmt::Import(ImportStmt {
                module_path: String::new(),
                alias: None,
                span: self.make_span_from_token(&start, &end),
                is_stdlib_module: true,
                module_name: Some(module_name),
                imported_names: Some(vec!["*".to_string()]),
                namespace: None,
            })
        }
    }

    fn alias_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        let canonical = self.consume(
            TokenType::Identifier,
            "Expected canonical name after 'alias'.",
        );
        self.consume(TokenType::As, "Expected 'as' after canonical name.");
        let alias_name = self.consume(TokenType::Identifier, "Expected alias name after 'as'.");
        let end = self.previous().clone();
        Stmt::Alias(AliasStmt {
            canonical: canonical.lexeme.clone(),
            alias_name: alias_name.lexeme.clone(),
            span: self.make_span_from_token(&start, &end),
        })
    }

    // -- types ----------------------------------------------------------------

    fn parse_type(&mut self) -> TypeRef {
        let start = self.current().clone();
        let first = self.simple_type();
        if self.match_tokens(&[TokenType::Pipe]) {
            let mut members: Vec<TypeRef> = vec![first];
            loop {
                let member = self.simple_type();
                members.push(member);
                if !self.match_tokens(&[TokenType::Pipe]) {
                    break;
                }
            }
            let end = self.previous().clone();
            return TypeRef {
                name: format!(
                    "union<{}>",
                    members
                        .iter()
                        .map(|m| m.name.clone())
                        .collect::<Vec<_>>()
                        .join("|")
                ),
                span: self.make_span_from_token(&start, &end),
                kind: TypeRefKind::Union(members),
            };
        }
        first
    }

    fn simple_type(&mut self) -> TypeRef {
        let name_tok = self.consume(TokenType::Identifier, "Expected type name.");
        let base = TypeRef {
            name: name_tok.lexeme.clone(),
            span: name_tok.span(),
            kind: TypeRefKind::Simple,
        };
        if self.check(TokenType::Question) {
            let q = self.advance().clone();
            return TypeRef {
                name: format!("optional<{}>", base.name),
                span: self.make_span_from_token(&name_tok, &q),
                kind: TypeRefKind::Optional(Box::new(base)),
            };
        }
        base
    }

    // -- llm if ----------------------------------------------------------------

    fn llm_if_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        self.consume(TokenType::If, "Expected 'if' after 'llm'.");
        let desc_expr = self.expression(precedence::NONE);
        self.consume(
            TokenType::LeftBrace,
            "Expected '{' after llm if description.",
        );
        let mut branches: Vec<LlmBranchNode> = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            if self.check(TokenType::Branch) {
                branches.push(self.llm_branch());
            } else if self.check(TokenType::Default) {
                self.advance();
                self.consume(TokenType::LeftBrace, "Expected '{' after default.");
                let body = self.block_body_list();
                self.consume(TokenType::RightBrace, "Expected '}' after default body.");
                branches.push(LlmBranchNode {
                    condition: None,
                    body,
                    span: self.make_span_from_token(self.previous(), self.previous()),
                });
            } else {
                self.error(&format!(
                    "Expected 'branch' or 'default', got {}",
                    self.current().kind.name()
                ));
                self.synchronize();
            }
        }
        self.consume(TokenType::RightBrace, "Expected '}' after llm if body.");
        let end = self.previous().clone();
        Stmt::LlmIf(LlmIfStmtNode {
            description: desc_expr,
            branches,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn llm_branch(&mut self) -> LlmBranchNode {
        let start = self.advance().clone(); // consume BRANCH
        let cond = self.expression(precedence::NONE);
        self.consume(TokenType::LeftBrace, "Expected '{' after branch condition.");
        let body = self.block_body_list();
        self.consume(TokenType::RightBrace, "Expected '}' after branch body.");
        LlmBranchNode {
            condition: Some(Box::new(cond)),
            body,
            span: self.make_span_from_token(&start, self.previous()),
        }
    }

    #[allow(clippy::if_same_then_else)]
    fn llm_act_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        self.consume(TokenType::Act, "Expected 'act' after 'llm'.");
        let act_token = self.previous().clone();

        let clause_keywords: &[&str] = &[
            "on_chunk",
            "on_complete",
            "on_tool_end",
            "on_media",
            "on_generate",
            "provider",
            "逐块处理",
            "完成",
            "工具结束",
            "处理媒体",
            "生成",
        ];

        if self.check(TokenType::Identifier) {
            let saved_pos = self.pos;
            let ident_tok = self.advance().clone();
            if (self.check(TokenType::LeftParen) || self.check(TokenType::String))
                && !matches!(ident_tok.lexeme.as_str(), "media" | "媒体")
            {
                self.error(&format!(
                    "'llm act {}(...)' is deprecated. Use 'call {}(...)' instead.",
                    ident_tok.lexeme, ident_tok.lexeme
                ));
            }
            self.pos = saved_pos;
        }

        let prompt_expr: Option<Box<Expr>> = if self.check_many_bare() {
            None
        } else if self.current().line > act_token.line {
            None
        } else if self.check(TokenType::Identifier)
            && clause_keywords.contains(&self.current().lexeme.as_str())
        {
            None
        } else if self.check(TokenType::Identifier)
            && matches!(self.current().lexeme.as_str(), "media" | "媒体")
            && self.peek_next().kind == TokenType::LeftParen
        {
            None
        } else {
            Some(Box::new(self.expression(precedence::NONE)))
        };

        let mut media: Vec<Expr> = Vec::new();
        let mut on_chunk: Option<Box<Expr>> = None;
        let mut on_complete: Option<Box<Expr>> = None;
        let mut on_tool_end: Option<Box<Expr>> = None;
        let mut on_media: Option<Box<Expr>> = None;
        let mut on_generate: Vec<Expr> = Vec::new();
        let mut provider: Option<Box<Expr>> = None;

        while !self.at_end() && !self.check_many_bare() {
            if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "media" | "媒体")
                && self.peek_next().kind == TokenType::LeftParen
            {
                media.push(self.expression(precedence::NONE));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_chunk" | "逐块处理")
            {
                self.advance();
                on_chunk = Some(Box::new(self.expression(precedence::NONE)));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_complete" | "完成")
            {
                self.advance();
                on_complete = Some(Box::new(self.expression(precedence::NONE)));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_tool_end" | "工具结束")
            {
                self.advance();
                on_tool_end = Some(Box::new(self.expression(precedence::NONE)));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_media" | "处理媒体")
            {
                self.advance();
                on_media = Some(Box::new(self.expression(precedence::NONE)));
            } else if self.check(TokenType::Identifier)
                && matches!(self.current().lexeme.as_str(), "on_generate" | "生成")
            {
                self.advance();
                on_generate.push(self.expression(precedence::NONE));
            } else if self.check(TokenType::Identifier) && self.current().lexeme == "provider" {
                self.advance();
                provider = Some(Box::new(self.expression(precedence::NONE)));
            } else {
                break;
            }
        }

        let span = self.make_span_from_token(&start, self.previous());
        Stmt::Expr(ExprStmt {
            expression: Expr::LlmAct(LlmAct {
                prompt: prompt_expr,
                media,
                on_chunk,
                on_complete,
                on_tool_end,
                on_media,
                on_generate,
                provider,
                span: span.clone(),
            }),
            span,
        })
    }

    // -- match -----------------------------------------------------------------

    fn match_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        let subject = self.expression(precedence::NONE);
        self.consume(TokenType::LeftBrace, "Expected '{' after match subject.");
        let mut cases: Vec<CaseNode> = Vec::new();
        let mut default: Vec<Stmt> = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            if self.check(TokenType::Case) {
                cases.push(self.case());
            } else if self.check(TokenType::Default) {
                self.advance();
                self.consume(TokenType::LeftBrace, "Expected '{' after default.");
                default = self.block_body_list();
                self.consume(TokenType::RightBrace, "Expected '}' after default body.");
            } else {
                self.error(&format!(
                    "Expected 'case' or 'default', got {}",
                    self.current().kind.name()
                ));
                self.synchronize();
            }
        }
        self.consume(TokenType::RightBrace, "Expected '}' after match body.");
        let end = self.previous().clone();
        Stmt::Match(MatchStmtNode {
            subject: Box::new(subject),
            cases,
            default,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn match_expr(&mut self) -> Expr {
        let start = self.previous().clone();
        let subject = self.expression(precedence::NONE);
        self.consume(TokenType::LeftBrace, "Expected '{' after match subject.");
        let mut cases: Vec<CaseNode> = Vec::new();
        let mut default_body: Option<Box<Expr>> = None;
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            if self.check(TokenType::Case) {
                let case_start = self.advance().clone();

                let pattern = if self.check(TokenType::Wildcard) {
                    let wildcard_tok = self.advance().clone();
                    Expr::WildcardPattern(WildcardPattern {
                        span: self.make_span_from_token(&wildcard_tok, &wildcard_tok),
                    })
                } else if self.check(TokenType::Is) {
                    let is_tok = self.advance().clone();
                    let type_tok =
                        self.consume(TokenType::Identifier, "Expected type name after 'is'.");
                    let mut binding_name: Option<String> = None;
                    if self.check(TokenType::Identifier) {
                        let binding_tok = self.advance().clone();
                        binding_name = Some(binding_tok.lexeme.clone());
                    }
                    Expr::TypePattern(TypePattern {
                        type_name: type_tok.lexeme.clone(),
                        span: self.make_span_from_token(&is_tok, self.previous()),
                        binding_name,
                    })
                } else {
                    let mut pattern = self.expression(precedence::NONE);
                    if let Expr::Variable(Variable { name, span }) = &pattern {
                        if self.check(TokenType::If) || self.check(TokenType::LeftBrace) {
                            pattern = Expr::VariablePattern(VariablePattern {
                                name: name.clone(),
                                span: span.clone(),
                            });
                        }
                    }
                    if self.match_tokens(&[TokenType::DotDot]) {
                        let pattern_start = span_of(&pattern);
                        let end_expr = self.expression(precedence::NONE);
                        let end_span = span_of(&end_expr);
                        pattern = Expr::RangePattern(RangePattern {
                            start: Box::new(pattern),
                            end: Box::new(end_expr),
                            span: SourceSpan::new(
                                pattern_start.file.clone(),
                                pattern_start.start_line,
                                pattern_start.start_col,
                                end_span.end_line,
                                end_span.end_col,
                            ),
                        });
                    }
                    pattern
                };

                let mut guard: Option<Box<Expr>> = None;
                if self.match_tokens(&[TokenType::If]) {
                    guard = Some(Box::new(self.expression(precedence::NONE)));
                }

                self.consume(TokenType::LeftBrace, "Expected '{' after case pattern.");
                let body_expr = self.expression(precedence::NONE);
                self.consume(TokenType::RightBrace, "Expected '}' after case expression.");
                let body_span = span_of(&body_expr);
                let body_stmt = Stmt::Expr(ExprStmt {
                    expression: body_expr,
                    span: body_span,
                });
                cases.push(CaseNode {
                    pattern,
                    body: vec![body_stmt],
                    guard,
                    span: self.make_span_from_token(&case_start, self.previous()),
                });
            } else if self.check(TokenType::Default) {
                self.advance();
                self.consume(TokenType::LeftBrace, "Expected '{' after default.");
                default_body = Some(Box::new(self.expression(precedence::NONE)));
                self.consume(
                    TokenType::RightBrace,
                    "Expected '}' after default expression.",
                );
            } else {
                self.error(&format!(
                    "Expected 'case' or 'default', got {}",
                    self.current().kind.name()
                ));
                self.synchronize();
            }
        }
        self.consume(TokenType::RightBrace, "Expected '}' after match body.");
        let end = self.previous().clone();
        Expr::MatchExpr(MatchExprNode {
            subject: Box::new(subject),
            cases,
            default_body,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn case(&mut self) -> CaseNode {
        let start = self.advance().clone(); // consume CASE

        let pattern = if self.check(TokenType::Wildcard) {
            let wildcard_tok = self.advance().clone();
            Expr::WildcardPattern(WildcardPattern {
                span: self.make_span_from_token(&wildcard_tok, &wildcard_tok),
            })
        } else if self.check(TokenType::Is) {
            let is_tok = self.advance().clone();
            let type_tok = self.consume(TokenType::Identifier, "Expected type name after 'is'.");
            let mut binding_name: Option<String> = None;
            if self.check(TokenType::Identifier) {
                let binding_tok = self.advance().clone();
                binding_name = Some(binding_tok.lexeme.clone());
            }
            Expr::TypePattern(TypePattern {
                type_name: type_tok.lexeme.clone(),
                span: self.make_span_from_token(&is_tok, self.previous()),
                binding_name,
            })
        } else {
            let mut pattern = self.expression(precedence::NONE);
            if let Expr::Variable(Variable { name, span }) = &pattern {
                if self.check(TokenType::If) || self.check(TokenType::LeftBrace) {
                    pattern = Expr::VariablePattern(VariablePattern {
                        name: name.clone(),
                        span: span.clone(),
                    });
                }
            }
            if self.match_tokens(&[TokenType::DotDot]) {
                let pattern_start = span_of(&pattern);
                let end_expr = self.expression(precedence::NONE);
                let end_span = span_of(&end_expr);
                pattern = Expr::RangePattern(RangePattern {
                    start: Box::new(pattern),
                    end: Box::new(end_expr),
                    span: SourceSpan::new(
                        pattern_start.file.clone(),
                        pattern_start.start_line,
                        pattern_start.start_col,
                        end_span.end_line,
                        end_span.end_col,
                    ),
                });
            }
            pattern
        };

        let mut guard: Option<Box<Expr>> = None;
        if self.match_tokens(&[TokenType::If]) {
            guard = Some(Box::new(self.expression(precedence::NONE)));
        }
        self.consume(TokenType::LeftBrace, "Expected '{' after case pattern.");
        let body = self.block_body_list();
        self.consume(TokenType::RightBrace, "Expected '}' after case body.");
        CaseNode {
            pattern,
            body,
            guard,
            span: self.make_span_from_token(&start, self.previous()),
        }
    }

    // -- try / throw / assert ----------------------------------------------------

    fn try_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        self.consume(TokenType::LeftBrace, "Expected '{' after 'try'.");
        let body = self.block_body_list();
        self.consume(TokenType::RightBrace, "Expected '}' after try body.");
        let mut catch_clauses: Vec<CatchClauseNode> = Vec::new();
        let mut catch_all: Option<CatchAllNode> = None;
        let mut finally_block: Option<FinallyBlockNode> = None;
        while self.check(TokenType::Catch) || self.check(TokenType::Finally) {
            if self.match_tokens(&[TokenType::Catch]) {
                if self.check(TokenType::Identifier) {
                    catch_clauses.push(self.catch_clause());
                } else {
                    catch_all = Some(self.catch_all());
                }
            } else if self.match_tokens(&[TokenType::Finally]) {
                finally_block = Some(self.finally_block());
            }
        }
        if catch_clauses.is_empty() && catch_all.is_none() && finally_block.is_none() {
            self.error("Expected at least one 'catch' or 'finally' after 'try'.");
        }
        let end = self.previous().clone();
        Stmt::Try(TryStmt {
            body,
            catch_clauses,
            catch_all,
            finally_block,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn throw_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        let exception_type = self.parse_type();
        let mut message: Option<Box<Expr>> = None;
        if self.match_tokens(&[TokenType::LeftParen]) {
            if !self.check(TokenType::RightParen) {
                message = Some(Box::new(self.expression(precedence::NONE)));
            }
            self.consume(
                TokenType::RightParen,
                "Expected ')' after exception message.",
            );
        }
        self.match_tokens(&[TokenType::Semicolon]);
        let end = self.previous().clone();
        Stmt::Throw(ThrowStmt {
            exception_type,
            message,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn assert_stmt(&mut self) -> Stmt {
        let start = self.previous().clone();
        let condition = self.expression(precedence::NONE);
        let mut message: Option<Box<Expr>> = None;
        if self.match_tokens(&[TokenType::Comma]) {
            message = Some(Box::new(self.expression(precedence::NONE)));
        }
        self.match_tokens(&[TokenType::Semicolon]);
        let end = self.previous().clone();
        Stmt::Assert(AssertStmt {
            condition: Box::new(condition),
            message,
            span: self.make_span_from_token(&start, &end),
        })
    }

    fn catch_clause(&mut self) -> CatchClauseNode {
        let start = self.previous().clone();
        let error_type = self.parse_type();
        let error_name = self.consume(TokenType::Identifier, "Expected error variable name.");
        self.consume(TokenType::LeftBrace, "Expected '{' after catch clause.");
        let body = self.block_body_list();
        self.consume(TokenType::RightBrace, "Expected '}' after catch body.");
        CatchClauseNode {
            error_type,
            error_name: error_name.lexeme.clone(),
            body,
            span: self.make_span_from_token(&start, self.previous()),
        }
    }

    fn catch_all(&mut self) -> CatchAllNode {
        let start = self.previous().clone();
        self.consume(TokenType::LeftBrace, "Expected '{' after catch.");
        let body = self.block_body_list();
        self.consume(TokenType::RightBrace, "Expected '}' after catch body.");
        CatchAllNode {
            body,
            span: self.make_span_from_token(&start, self.previous()),
        }
    }

    fn finally_block(&mut self) -> FinallyBlockNode {
        let start = self.previous().clone();
        self.consume(TokenType::LeftBrace, "Expected '{' after finally.");
        let body = self.block_body_list();
        self.consume(TokenType::RightBrace, "Expected '}' after finally body.");
        FinallyBlockNode {
            body,
            span: self.make_span_from_token(&start, self.previous()),
        }
    }

    // ------------------------------------------------------------------
    // Token helpers
    // ------------------------------------------------------------------

    fn consume(&mut self, tt: TokenType, message: &str) -> Token {
        if self.check(tt) {
            return self.advance().clone();
        }
        self.error(message);
        let peek = self.peek().clone();
        Token {
            kind: tt,
            lexeme: "<missing>".to_string(),
            literal: LiteralValue::Null,
            line: peek.line,
            col: peek.col,
            end_line: peek.line,
            end_col: peek.col,
            file: peek.file.clone(),
        }
    }

    fn check(&self, tt: TokenType) -> bool {
        if self.at_end() {
            return tt == TokenType::Eof;
        }
        self.current().kind == tt
    }

    fn check_many_bare(&self) -> bool {
        if self.at_end() {
            return true;
        }
        is_bare_form_token(self.current().kind)
    }

    fn is_context_keyword(&self, keywords: &[&str]) -> bool {
        if self.at_end() {
            return false;
        }
        let tok = self.current();
        if tok.kind != TokenType::Identifier {
            return false;
        }
        keywords.contains(&tok.lexeme.as_str())
    }

    fn peek(&self) -> &Token {
        self.current()
    }

    fn peek_next(&self) -> &Token {
        if self.pos + 1 < self.tokens.len() {
            &self.tokens[self.pos + 1]
        } else {
            self.tokens.last().unwrap()
        }
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.pos - 1]
    }

    fn advance(&mut self) -> &Token {
        if !self.at_end() {
            self.pos += 1;
        }
        self.previous()
    }

    fn match_tokens(&mut self, types: &[TokenType]) -> bool {
        for t in types {
            if self.check(*t) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn error(&mut self, message: &str) {
        let tok = self.peek().clone();
        let span = SourceSpan::new(
            tok.file.clone(),
            tok.line,
            tok.col,
            tok.end_line,
            tok.end_col,
        );
        self.errors.push(ParseError::new(
            ErrorCode::ParserError,
            message.to_string(),
            span,
        ));
    }

    fn synchronize(&mut self) {
        self.advance();
        while !self.at_end() {
            if self.previous().kind == TokenType::Semicolon {
                return;
            }
            if self.check(TokenType::Agent)
                || self.check(TokenType::Fn)
                || self.check(TokenType::Main)
                || self.check(TokenType::If)
                || self.check(TokenType::For)
                || self.check(TokenType::While)
                || self.check(TokenType::Return)
                || self.check(TokenType::Break)
                || self.check(TokenType::Continue)
                || self.check(TokenType::Let)
                || self.check(TokenType::Const)
                || self.check(TokenType::RightBrace)
            {
                return;
            }
            self.advance();
        }
    }

    fn at_end(&self) -> bool {
        self.current().kind == TokenType::Eof
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn make_span_from_token(&self, start: &Token, end: &Token) -> SourceSpan {
        SourceSpan::new(
            start.file.clone(),
            start.line,
            start.col,
            end.end_line,
            end.end_col,
        )
    }

    /// Python `_make_span(expr.span if hasattr(expr, 'span') else prev, end)`.
    fn make_span_from_expr(&self, expr: &Expr, end: &Token) -> SourceSpan {
        let s = span_of(expr);
        SourceSpan::new(
            s.file.clone(),
            s.start_line,
            s.start_col,
            end.end_line,
            end.end_col,
        )
    }
}

/// Helper: get the span of any expression.
fn span_of(e: &Expr) -> SourceSpan {
    match e {
        Expr::Literal(Lit { span, .. }) => span.clone(),
        Expr::Variable(Variable { span, .. }) => span.clone(),
        Expr::Binary(Binary { span, .. }) => span.clone(),
        Expr::Pipe(Pipe { span, .. }) => span.clone(),
        Expr::Unary(Unary { span, .. }) => span.clone(),
        Expr::Grouping(Grouping { span, .. }) => span.clone(),
        Expr::Call(Call { span, .. }) => span.clone(),
        Expr::Index(Index { span, .. }) => span.clone(),
        Expr::Access(Access { span, .. }) => span.clone(),
        Expr::List(ListLit { span, .. }) => span.clone(),
        Expr::Map(MapLit { span, .. }) => span.clone(),
        Expr::TemplateRef(TemplateRef { span, .. }) => span.clone(),
        Expr::Type(TypeRef { span, .. }) => span.clone(),
        Expr::OptionalType(OptionalType { span, .. }) => span.clone(),
        Expr::UnionType(UnionType { span, .. }) => span.clone(),
        Expr::LiteralType(LiteralType { span, .. }) => span.clone(),
        Expr::Lambda(Lambda { span, .. }) => span.clone(),
        Expr::Spawn(Spawn { span, .. }) => span.clone(),
        Expr::LlmAct(LlmAct { span, .. }) => span.clone(),
        Expr::MatchExpr(MatchExprNode { span, .. }) => span.clone(),
        Expr::RangePattern(RangePattern { span, .. }) => span.clone(),
        Expr::WildcardPattern(WildcardPattern { span }) => span.clone(),
        Expr::VariablePattern(VariablePattern { span, .. }) => span.clone(),
        Expr::TypePattern(TypePattern { span, .. }) => span.clone(),
    }
}

#[cfg(test)]
mod type_ref_kind_tests {
    use super::*;
    use helen_core::lexer::Scanner;

    fn parse_first_fn(src: &str) -> FunctionDecl {
        let mut scanner = Scanner::new(src, "<test>");
        let tokens = scanner.scan_all();
        let mut parser = Parser::new(tokens);
        let program = parser.parse();
        match &program.statements[0] {
            Stmt::FunctionDecl(f) => f.clone(),
            other => panic!("expected function decl, got {other:?}"),
        }
    }

    #[test]
    fn plain_annotation_is_simple() {
        let f = parse_first_fn("fn f(x: int) {}\n");
        let ann = f.params[0]
            .type_annotation
            .as_ref()
            .expect("param annotation");
        assert_eq!(ann.name, "int");
        assert_eq!(ann.kind, TypeRefKind::Simple);
    }

    #[test]
    fn optional_annotation_preserves_inner() {
        let f = parse_first_fn("fn f(x: int?) {}\n");
        let ann = f.params[0]
            .type_annotation
            .as_ref()
            .expect("param annotation");
        assert_eq!(ann.name, "optional<int>");
        match &ann.kind {
            TypeRefKind::Optional(inner) => {
                assert_eq!(inner.name, "int");
                assert_eq!(inner.kind, TypeRefKind::Simple);
            }
            other => panic!("expected Optional kind, got {other:?}"),
        }
    }

    #[test]
    fn union_annotation_preserves_members() {
        let f = parse_first_fn("fn f(x: str|int) {}\n");
        let ann = f.params[0]
            .type_annotation
            .as_ref()
            .expect("param annotation");
        assert_eq!(ann.name, "union<str|int>");
        match &ann.kind {
            TypeRefKind::Union(members) => {
                assert_eq!(members.len(), 2);
                assert_eq!(members[0].name, "str");
                assert_eq!(members[1].name, "int");
                assert!(members.iter().all(|m| m.kind == TypeRefKind::Simple));
            }
            other => panic!("expected Union kind, got {other:?}"),
        }
    }

    #[test]
    fn nested_optional_in_union_keeps_structure() {
        // `str|int?` → Union[Str, Optional[Int]] (matches Python's
        // UnionTypeNode(members=[TypeNode, OptionalTypeNode])).
        let f = parse_first_fn("fn f(x: str|int?) {}\n");
        let ann = f.params[0]
            .type_annotation
            .as_ref()
            .expect("param annotation");
        assert_eq!(ann.name, "union<str|optional<int>>");
        match &ann.kind {
            TypeRefKind::Union(members) => {
                assert_eq!(members.len(), 2);
                assert_eq!(members[0].kind, TypeRefKind::Simple);
                match &members[1].kind {
                    TypeRefKind::Optional(inner) => assert_eq!(inner.name, "int"),
                    other => panic!("expected Optional member, got {other:?}"),
                }
            }
            other => panic!("expected Union kind, got {other:?}"),
        }
    }
}
