//! Abstract Syntax Tree node definitions for the Helen language.
//!
//! Byte-faithful port of `helen/core/ast.py` (v1.44.0): every node type,
//! field, and ordering mirrors the Python dataclasses so that the
//! `AstPrinter` (and the Python-style dataclass repr used by map printing)
//! produce byte-identical output.

use crate::source::SourceSpan;
use crate::tokens::{LiteralValue, Token};

/// Root program node: `(program stmt*)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
    pub span: SourceSpan,
}

/// Expression node base — all expressions are variants of [`Expr`].
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Lit),
    Variable(Variable),
    Binary(Binary),
    Pipe(Pipe),
    Unary(Unary),
    Grouping(Grouping),
    Call(Call),
    Index(Index),
    Access(Access),
    List(ListLit),
    Map(MapLit),
    TemplateRef(TemplateRef),
    Type(TypeRef),
    OptionalType(OptionalType),
    UnionType(UnionType),
    LiteralType(LiteralType),
    Lambda(Lambda),
    Spawn(Spawn),
    LlmAct(LlmAct),
    MatchExpr(MatchExprNode),
    RangePattern(RangePattern),
    WildcardPattern(WildcardPattern),
    VariablePattern(VariablePattern),
    TypePattern(TypePattern),
}

/// Statement node base — all statements are variants of [`Stmt`].
/// Large variants (declarations with embedded blocks) are intentional,
/// mirroring the Python node classes; boxing them per-variant would not
/// reduce the enum's size below the largest variant anyway.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    VarDecl(VarDecl),
    SharedStoreDecl(SharedStoreDecl),
    If(IfStmt),
    For(ForStmt),
    While(WhileStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Return(ReturnStmt),
    Expr(ExprStmt),
    PromptDef(PromptDef),
    Declaration(Declaration),
    AgentParam(AgentParam),
    ContextConfig(ContextConfig),
    AgentDecl(AgentDecl),
    MainBlock(MainBlock),
    FunctionDecl(FunctionDecl),
    FnBlock(FnBlock),
    ProtocolDecl(ProtocolDecl),
    ImplDecl(ImplDecl),
    Import(ImportStmt),
    Alias(AliasStmt),
    Case(CaseNode),
    CatchClause(CatchClauseNode),
    CatchAll(CatchAllNode),
    FinallyBlock(FinallyBlockNode),
    Try(TryStmt),
    Throw(ThrowStmt),
    Assert(AssertStmt),
    LlmBranch(LlmBranchNode),
    LlmIf(LlmIfStmtNode),
    Match(MatchStmtNode),
}

// ---------------------------------------------------------------------------
// Expression nodes
// ---------------------------------------------------------------------------

/// Literal value: `42`, `3.14`, `"hello"`, `true`, `false`, `null`.
#[derive(Debug, Clone, PartialEq)]
pub struct Lit {
    pub value: LiteralValue,
    pub span: SourceSpan,
}

/// Identifier: `x`, `my_var`.
#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: String,
    pub span: SourceSpan,
}

/// Binary operation: `a + b`, `x == y`.
#[derive(Debug, Clone, PartialEq)]
pub struct Binary {
    pub left: Box<Expr>,
    pub operator: Token,
    pub right: Box<Expr>,
    pub span: SourceSpan,
}

/// Pipe expression: `value |> fn` (desugars to `fn(value)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Pipe {
    pub value: Box<Expr>,
    pub function: Box<Expr>,
    pub span: SourceSpan,
}

/// Unary operation: `!x`, `-n`.
#[derive(Debug, Clone, PartialEq)]
pub struct Unary {
    pub operator: Token,
    pub operand: Box<Expr>,
    pub span: SourceSpan,
}

/// Grouped expression: `(a + b)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Grouping {
    pub expression: Box<Expr>,
    pub span: SourceSpan,
}

/// Call argument: `name = value`.
#[derive(Debug, Clone, PartialEq)]
pub struct CallArg {
    pub name: Option<String>,
    pub value: Expr,
}

/// Function call: `print(x)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub callee: Box<Expr>,
    pub arguments: Vec<CallArg>,
    pub span: SourceSpan,
}

/// Index access: `arr[0]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Index {
    pub target: Box<Expr>,
    pub index: Box<Expr>,
    pub span: SourceSpan,
}

/// Member access: `obj.field`.
#[derive(Debug, Clone, PartialEq)]
pub struct Access {
    pub target: Box<Expr>,
    pub property: String,
    pub span: SourceSpan,
}

/// List literal: `[1, 2, 3]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ListLit {
    pub elements: Vec<Expr>,
    pub span: SourceSpan,
}

/// Map entry: `key: value`.
#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    pub key: Expr,
    pub value: Expr,
    pub span: SourceSpan,
}

/// Map literal: `{"key": value}`.
#[derive(Debug, Clone, PartialEq)]
pub struct MapLit {
    pub entries: Vec<MapEntry>,
    pub span: SourceSpan,
}

/// Template variable reference: `{{expr}}`.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRef {
    pub expression: Box<Expr>,
    pub span: SourceSpan,
}

// ---------------------------------------------------------------------------
// Type nodes
// ---------------------------------------------------------------------------

/// Type reference: `int`, `str`, `MyType`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeRef {
    pub name: String,
    pub span: SourceSpan,
}

/// Optional type: `T?`.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionalType {
    pub inner: Box<TypeRef>,
    pub span: SourceSpan,
}

/// Union type: `A|B|C`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionType {
    pub members: Vec<TypeRef>,
    pub span: SourceSpan,
}

/// Literal type: `Literal["hello", 42]`.
#[derive(Debug, Clone, PartialEq)]
pub struct LiteralType {
    pub values: Vec<Expr>,
    pub span: SourceSpan,
}

// ---------------------------------------------------------------------------
// Statement nodes
// ---------------------------------------------------------------------------

/// Variable declaration: `let x = 42`, `const MAX = 100`, `shared let buf = ""`.
#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub name: String,
    pub type_annotation: Option<Box<TypeRef>>,
    pub initializer: Option<Box<Expr>>,
    pub mutable: bool,
    pub span: SourceSpan,
    pub shared: bool,
}

/// Shared store declaration: `shared store Name { fields, methods }` (v1.12).
#[derive(Debug, Clone, PartialEq)]
pub struct SharedStoreDecl {
    pub name: String,
    pub fields: Vec<VarDecl>,
    pub methods: Vec<FunctionDecl>,
    pub span: SourceSpan,
}

/// Conditional: `if cond { ... } else { ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub condition: Box<Expr>,
    pub then_branch: Box<Stmt>,
    pub else_branch: Option<Box<Stmt>>,
    pub span: SourceSpan,
}

/// For loop: `for x in items { ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub iterator: Option<Variable>,
    pub iterable: Box<Expr>,
    pub body: Box<Stmt>,
    pub span: SourceSpan,
}

/// While loop: `while cond { ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub condition: Box<Expr>,
    pub body: Box<Stmt>,
    pub span: SourceSpan,
}

/// Break statement.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakStmt {
    pub span: SourceSpan,
}

/// Continue statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinueStmt {
    pub span: SourceSpan,
}

/// Return statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Box<Expr>>,
    pub span: SourceSpan,
}

/// Expression as a statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub expression: Expr,
    pub span: SourceSpan,
}

/// Prompt definition.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptDef {
    pub content: String,
    pub span: SourceSpan,
}

/// Agent declaration config block.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub description: Option<Expr>,
    pub model: Option<Expr>,
    pub tools: Option<Expr>,
    pub memory: Option<Expr>,
    pub temperature: Option<Expr>,
    pub max_turns: Option<Expr>,
    pub max_tokens: Option<Expr>,
    pub span: SourceSpan,
    pub streaming: bool,
    pub thinking_mode: Option<Expr>,
    pub reasoning_effort: Option<Expr>,
    pub provider: Option<Expr>,
}

/// Agent parameter declaration: `name: Type? = default?`.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentParam {
    pub name: String,
    pub type_annotation: Option<Box<TypeRef>>,
    pub default_value: Option<Box<Expr>>,
    pub span: SourceSpan,
}

/// Agent context configuration: `context { ... }` (Phase 7).
#[derive(Debug, Clone, PartialEq)]
pub struct ContextConfig {
    pub compression: String,
    pub cache_aware: bool,
    pub working_memory: bool,
    pub working_memory_tokens: i64,
    pub span: Option<SourceSpan>,
}

/// Agent declaration: `agent Name(params?) { declarations, prompt, logic }`.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDecl {
    pub name: String,
    pub params: Vec<AgentParam>,
    pub declarations: Vec<Declaration>,
    pub prompt: Option<PromptDef>,
    pub logic: Option<Box<Stmt>>,
    pub span: SourceSpan,
    pub functions: Vec<FunctionDecl>,
    pub function_vars: Vec<VarDecl>,
    pub has_streaming: bool,
    pub isolation_level: String,
    pub context_config: Option<ContextConfig>,
    pub transcript: String,
    pub output_contract: Option<String>,
}

/// Main block: `main { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct MainBlock {
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
}

/// Function declaration: `fn name(params) -> type { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<AgentParam>,
    pub return_type: Option<Box<TypeRef>>,
    pub body: FnBlock,
    pub span: SourceSpan,
}

/// Function body block: `{ stmt* }`.
#[derive(Debug, Clone, PartialEq)]
pub struct FnBlock {
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
}

/// Lambda expression: `fn(params) { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Lambda {
    pub params: Vec<AgentParam>,
    pub return_type: Option<Box<TypeRef>>,
    pub body: FnBlock,
    pub span: SourceSpan,
}

/// Protocol declaration: `protocol Name { fn signatures }` (v1.7).
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDecl {
    pub name: String,
    pub methods: Vec<FunctionDecl>,
    pub span: SourceSpan,
}

/// Protocol implementation: `impl Protocol for Struct { fn implementations }` (v1.7).
#[derive(Debug, Clone, PartialEq)]
pub struct ImplDecl {
    pub protocol_name: String,
    pub struct_name: String,
    pub methods: Vec<FunctionDecl>,
    pub span: SourceSpan,
}

/// Import statement: `import "path" as alias | import std.mod.{func1, func2}`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportStmt {
    pub module_path: String,
    pub alias: Option<String>,
    pub span: SourceSpan,
    pub is_stdlib_module: bool,
    pub module_name: Option<String>,
    pub imported_names: Option<Vec<String>>,
    pub namespace: Option<String>,
}

/// Alias statement: `alias <canonical> as <alias_name>`.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasStmt {
    pub canonical: String,
    pub alias_name: String,
    pub span: SourceSpan,
}

/// Spawn agent expression: `spawn AgentName(...)` (v1.18).
#[derive(Debug, Clone, PartialEq)]
pub struct Spawn {
    pub call: Box<Call>,
    pub span: SourceSpan,
    pub resume_session: Option<Box<Expr>>,
}

/// Match case: `case pattern { ... }` or `case pattern if guard { ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseNode {
    pub pattern: Expr,
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
    pub guard: Option<Box<Expr>>,
}

/// Range pattern for match: `start..end` (inclusive).
#[derive(Debug, Clone, PartialEq)]
pub struct RangePattern {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    pub span: SourceSpan,
}

/// Wildcard pattern for match: `_` (matches anything).
#[derive(Debug, Clone, PartialEq)]
pub struct WildcardPattern {
    pub span: SourceSpan,
}

/// Variable binding pattern for match: `case x { ... }` binds value to `x`.
#[derive(Debug, Clone, PartialEq)]
pub struct VariablePattern {
    pub name: String,
    pub span: SourceSpan,
}

/// Type pattern for match: `case is Type` or `case is Type name`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypePattern {
    pub type_name: String,
    pub span: SourceSpan,
    pub binding_name: Option<String>,
}

/// Typed catch clause.
#[derive(Debug, Clone, PartialEq)]
pub struct CatchClauseNode {
    pub error_type: TypeRef,
    pub error_name: String,
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
}

/// Catch-all clause.
#[derive(Debug, Clone, PartialEq)]
pub struct CatchAllNode {
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
}

/// Finally block.
#[derive(Debug, Clone, PartialEq)]
pub struct FinallyBlockNode {
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
}

/// Try-catch-finally statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TryStmt {
    pub body: Vec<Stmt>,
    pub catch_clauses: Vec<CatchClauseNode>,
    pub catch_all: Option<CatchAllNode>,
    pub finally_block: Option<FinallyBlockNode>,
    pub span: SourceSpan,
}

/// Throw statement: `throw ExceptionType` or `throw ExceptionType(message)`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrowStmt {
    pub exception_type: TypeRef,
    pub message: Option<Box<Expr>>,
    pub span: SourceSpan,
}

/// Assert statement: `assert condition` or `assert condition, message`.
#[derive(Debug, Clone, PartialEq)]
pub struct AssertStmt {
    pub condition: Box<Expr>,
    pub message: Option<Box<Expr>>,
    pub span: SourceSpan,
}

/// LLM if branch.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmBranchNode {
    pub condition: Option<Box<Expr>>,
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
}

/// LLM if statement.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmIfStmtNode {
    pub description: Expr,
    pub branches: Vec<LlmBranchNode>,
    pub span: SourceSpan,
}

/// LLM act as an expression.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmAct {
    pub prompt: Option<Box<Expr>>,
    pub media: Vec<Expr>,
    pub on_chunk: Option<Box<Expr>>,
    pub on_complete: Option<Box<Expr>>,
    pub on_tool_end: Option<Box<Expr>>,
    pub on_media: Option<Box<Expr>>,
    pub on_generate: Vec<Expr>,
    pub provider: Option<Box<Expr>>,
    pub span: SourceSpan,
}

/// Match statement.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchStmtNode {
    pub subject: Box<Expr>,
    pub cases: Vec<CaseNode>,
    pub default: Vec<Stmt>,
    pub span: SourceSpan,
}

/// Match expression — returns the value of the matched branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExprNode {
    pub subject: Box<Expr>,
    pub cases: Vec<CaseNode>,
    pub default_body: Option<Box<Expr>>,
    pub span: SourceSpan,
}
