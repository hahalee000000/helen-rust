//! Token types, keyword mapping, and the Token struct.
//!
//! Mirrors `helen/core/tokens.py` exactly: 88 `TokenType` variants (order
//! preserved), a 99-entry bilingual keyword map (verified programmatically
//! against v1.44.0 — the Python comment "97" is stale), and the `Token`
//! struct with its derived `SourceSpan`.

use crate::source::SourceSpan;
use num_bigint::BigInt;

/// The literal value carried by a token (Python `LiteralValue`).
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    /// `null` / absent literal (`None`).
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// String literal value (after escape processing).
    Str(String),
    /// Integer literal value (arbitrary precision, like Python int).
    Int(BigInt),
    /// Float literal value.
    Float(f64),
}

/// All lexical token types in the Helen language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    // === Delimiters ===
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    DotDot, // ..
    Colon,
    Semicolon,
    Question,
    Pipe,
    PipeRight, // |>
    // === Operators ===
    Minus,
    Plus,
    Slash,
    Star,
    Percent,
    Arrow,      // ->
    Bang,       // !
    BangEqual,  // !=
    Assign,     // =
    EqualEqual, // ==
    Greater,    // >
    GreaterEqual,
    Less,
    LessEqual,
    And, // &&
    Or,  // ||
    At,  // @
    // === Literals ===
    Identifier,
    String,
    TripleQuoteString,
    Number,
    True,
    False,
    NullKw,
    // === Template ===
    TemplateOpen,  // {{
    TemplateClose, // }}
    // === Keywords ===
    Agent,
    Description,
    Model,
    Tools,
    Streaming,
    Temperature,
    MaxTurns,
    MaxTokens,
    Memory,
    Prompt,
    Llm,
    Import,
    Let,
    Const,
    If,
    Else,
    For,
    While,
    Break,
    Continue,
    Return,
    Spawn,
    Match,
    Case,
    Branch,
    Default,
    Act,
    Try,
    Catch,
    Finally,
    Throw,
    Assert,
    Fn,
    As,
    In,
    Functions,
    Main,
    Store,
    Protocol,
    Impl,
    Is,
    Wildcard,
    Shared,
    Alias,
    Transcript,
    ThinkingMode,
    ReasoningEffort,
    // === Special ===
    Eof,
}

impl TokenType {
    /// The Python `TokenType` enum member name (e.g. `"LEFT_PAREN"`).
    pub fn name(&self) -> &'static str {
        match self {
            TokenType::LeftParen => "LEFT_PAREN",
            TokenType::RightParen => "RIGHT_PAREN",
            TokenType::LeftBrace => "LEFT_BRACE",
            TokenType::RightBrace => "RIGHT_BRACE",
            TokenType::LeftBracket => "LEFT_BRACKET",
            TokenType::RightBracket => "RIGHT_BRACKET",
            TokenType::Comma => "COMMA",
            TokenType::Dot => "DOT",
            TokenType::DotDot => "DOTDOT",
            TokenType::Colon => "COLON",
            TokenType::Semicolon => "SEMICOLON",
            TokenType::Question => "QUESTION",
            TokenType::Pipe => "PIPE",
            TokenType::PipeRight => "PIPE_RIGHT",
            TokenType::Minus => "MINUS",
            TokenType::Plus => "PLUS",
            TokenType::Slash => "SLASH",
            TokenType::Star => "STAR",
            TokenType::Percent => "PERCENT",
            TokenType::Arrow => "ARROW",
            TokenType::Bang => "BANG",
            TokenType::BangEqual => "BANG_EQUAL",
            TokenType::Assign => "ASSIGN",
            TokenType::EqualEqual => "EQUAL_EQUAL",
            TokenType::Greater => "GREATER",
            TokenType::GreaterEqual => "GREATER_EQUAL",
            TokenType::Less => "LESS",
            TokenType::LessEqual => "LESS_EQUAL",
            TokenType::And => "AND",
            TokenType::Or => "OR",
            TokenType::At => "AT",
            TokenType::Identifier => "IDENTIFIER",
            TokenType::String => "STRING",
            TokenType::TripleQuoteString => "TRIPLE_QUOTE_STRING",
            TokenType::Number => "NUMBER",
            TokenType::True => "TRUE",
            TokenType::False => "FALSE",
            TokenType::NullKw => "NULL_KW",
            TokenType::TemplateOpen => "TEMPLATE_OPEN",
            TokenType::TemplateClose => "TEMPLATE_CLOSE",
            TokenType::Agent => "AGENT",
            TokenType::Description => "DESCRIPTION",
            TokenType::Model => "MODEL",
            TokenType::Tools => "TOOLS",
            TokenType::Streaming => "STREAMING",
            TokenType::Temperature => "TEMPERATURE",
            TokenType::MaxTurns => "MAX_TURNS",
            TokenType::MaxTokens => "MAX_TOKENS",
            TokenType::Memory => "MEMORY",
            TokenType::Prompt => "PROMPT",
            TokenType::Llm => "LLM",
            TokenType::Import => "IMPORT",
            TokenType::Let => "LET",
            TokenType::Const => "CONST",
            TokenType::If => "IF",
            TokenType::Else => "ELSE",
            TokenType::For => "FOR",
            TokenType::While => "WHILE",
            TokenType::Break => "BREAK",
            TokenType::Continue => "CONTINUE",
            TokenType::Return => "RETURN",
            TokenType::Spawn => "SPAWN",
            TokenType::Match => "MATCH",
            TokenType::Case => "CASE",
            TokenType::Branch => "BRANCH",
            TokenType::Default => "DEFAULT",
            TokenType::Act => "ACT",
            TokenType::Try => "TRY",
            TokenType::Catch => "CATCH",
            TokenType::Finally => "FINALLY",
            TokenType::Throw => "THROW",
            TokenType::Assert => "ASSERT",
            TokenType::Fn => "FN",
            TokenType::As => "AS",
            TokenType::In => "IN",
            TokenType::Functions => "FUNCTIONS",
            TokenType::Main => "MAIN",
            TokenType::Store => "STORE",
            TokenType::Protocol => "PROTOCOL",
            TokenType::Impl => "IMPL",
            TokenType::Is => "IS",
            TokenType::Wildcard => "WILDCARD",
            TokenType::Shared => "SHARED",
            TokenType::Alias => "ALIAS",
            TokenType::Transcript => "TRANSCRIPT",
            TokenType::ThinkingMode => "THINKING_MODE",
            TokenType::ReasoningEffort => "REASONING_EFFORT",
            TokenType::Eof => "EOF",
        }
    }
}

/// A single lexical token (mirrors `tokens.Token`).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenType,
    pub lexeme: String,
    pub literal: LiteralValue,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub file: String,
}

impl Token {
    /// The `SourceSpan` covering this token (Python `Token.span`).
    pub fn span(&self) -> SourceSpan {
        SourceSpan::new(
            self.file.clone(),
            self.line,
            self.col,
            self.end_line,
            self.end_col,
        )
    }
}

/// The bilingual keyword map, in Python `_KEYWORD_MAP` order (99 entries).
/// `MEMORY` and `WILDCARD` are context keywords and are intentionally absent.
pub const KEYWORDS: &[(&str, TokenType)] = &[
    ("agent", TokenType::Agent),
    ("description", TokenType::Description),
    ("model", TokenType::Model),
    ("tools", TokenType::Tools),
    ("streaming", TokenType::Streaming),
    ("temperature", TokenType::Temperature),
    ("max-turns", TokenType::MaxTurns),
    ("max-tokens", TokenType::MaxTokens),
    ("prompt", TokenType::Prompt),
    ("llm", TokenType::Llm),
    ("import", TokenType::Import),
    ("let", TokenType::Let),
    ("const", TokenType::Const),
    ("if", TokenType::If),
    ("else", TokenType::Else),
    ("for", TokenType::For),
    ("while", TokenType::While),
    ("break", TokenType::Break),
    ("continue", TokenType::Continue),
    ("return", TokenType::Return),
    ("spawn", TokenType::Spawn),
    ("match", TokenType::Match),
    ("case", TokenType::Case),
    ("branch", TokenType::Branch),
    ("default", TokenType::Default),
    ("act", TokenType::Act),
    ("try", TokenType::Try),
    ("catch", TokenType::Catch),
    ("finally", TokenType::Finally),
    ("throw", TokenType::Throw),
    ("assert", TokenType::Assert),
    ("fn", TokenType::Fn),
    ("as", TokenType::As),
    ("in", TokenType::In),
    ("functions", TokenType::Functions),
    ("main", TokenType::Main),
    ("store", TokenType::Store),
    ("protocol", TokenType::Protocol),
    ("impl", TokenType::Impl),
    ("is", TokenType::Is),
    ("shared", TokenType::Shared),
    ("alias", TokenType::Alias),
    ("transcript", TokenType::Transcript),
    ("thinking-mode", TokenType::ThinkingMode),
    ("reasoning-effort", TokenType::ReasoningEffort),
    ("仓库", TokenType::Store),
    ("true", TokenType::True),
    ("false", TokenType::False),
    ("null", TokenType::NullKw),
    ("设", TokenType::Let),
    ("定义", TokenType::Let),
    ("常量", TokenType::Const),
    ("函数", TokenType::Fn),
    ("返回", TokenType::Return),
    ("如果", TokenType::If),
    ("否则", TokenType::Else),
    ("对于", TokenType::For),
    ("属于", TokenType::In),
    ("当", TokenType::While),
    ("中断", TokenType::Break),
    ("继续", TokenType::Continue),
    ("匹配", TokenType::Match),
    ("情况", TokenType::Case),
    ("默认", TokenType::Default),
    ("尝试", TokenType::Try),
    ("捕获", TokenType::Catch),
    ("最终", TokenType::Finally),
    ("抛出", TokenType::Throw),
    ("断言", TokenType::Assert),
    ("且", TokenType::And),
    ("或", TokenType::Or),
    ("真", TokenType::True),
    ("假", TokenType::False),
    ("空", TokenType::NullKw),
    ("是", TokenType::Is),
    ("智能体", TokenType::Agent),
    ("大模型", TokenType::Llm),
    ("执行", TokenType::Act),
    ("分生", TokenType::Spawn),
    ("提示词", TokenType::Prompt),
    ("描述", TokenType::Description),
    ("模型", TokenType::Model),
    ("工具", TokenType::Tools),
    ("流式输出", TokenType::Streaming),
    ("温度", TokenType::Temperature),
    ("最大轮次", TokenType::MaxTurns),
    ("最大tokens", TokenType::MaxTokens),
    ("函数区", TokenType::Functions),
    ("主函", TokenType::Main),
    ("导入", TokenType::Import),
    ("作为", TokenType::As),
    ("协议", TokenType::Protocol),
    ("实现", TokenType::Impl),
    ("分支", TokenType::Branch),
    ("共享", TokenType::Shared),
    ("别名", TokenType::Alias),
    ("记录", TokenType::Transcript),
    ("思考模式", TokenType::ThinkingMode),
    ("推理强度", TokenType::ReasoningEffort),
];

/// Number of entries in the keyword map (99, verified against v1.44.0).
pub fn keyword_type_count() -> usize {
    KEYWORDS.len()
}

/// Look up a keyword lexeme, returning its `TokenType` or `None`.
pub fn keyword_type(lexeme: &str) -> Option<TokenType> {
    KEYWORDS.iter().find(|(k, _)| *k == lexeme).map(|(_, t)| *t)
}
