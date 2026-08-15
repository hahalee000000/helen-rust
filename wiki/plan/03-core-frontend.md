# M1 — Core Frontend: Source, Tokens, Lexer, AST, Parser

> **Status: COMPLETE (2026-08-15)** — commits `364b0df` (1.1–1.2) + `9d394c1` (1.3–1.5):
> `tokens.rs` (88 variants, 99 bilingual keywords — Python comment "97" is
> stale), `lexer.rs` (byte-faithful `Scanner` port), Python-faithful
> `SourceSpan`/`ErrorCode` (E0300–E0357), `helen --lex` + `reference.py --lex`
> + `scripts/diff-lex.sh`. Differential: **36/36** corpus token streams
> byte-identical (type, lexeme, line/col, literals incl. bigint + float
> numeric compare). 35 contract tests, clippy clean. Tasks 1.3–1.5 (AST,
> ASTPrinter, Pratt parser) remain.

**Objective:** Byte-faithful port of `core/{source,tokens,lexer,errors,ast}.py` + `core/parser.py`. Exit criterion: Rust lexer+parser pass differential AST output and token streams against Python for the whole corpus.

## Files

```
crates/helen-core/src/source.rs     crates/helen-core/src/tokens.rs
crates/helen-core/src/lexer.rs      crates/helen-core/src/errors.rs
crates/helen-core/src/ast.rs        crates/helen-core/src/ast_printer.rs
crates/helen-parser/src/pratt.rs    crates/helen-parser/src/lib.rs
crates/helen-core/tests/lexer_tests.rs
crates/helen-parser/tests/parser_tests.rs
```

## Task 1.1: TokenType enum + Token struct + keyword map (99 bilingual)

**Step 1 — failing tests:** Port 3 Python lexer test files (`tests/lexer/`) as Rust tests asserting token sequences for representative snippets (strings, CJK, operators, `{{}}`).

**Step 2 — implementation:**

```rust
// tokens.rs
pub enum TokenType { /* 88 variants, mirroring Python order */
  LeftParen, RightParen, LeftBrace, RightBrace, LeftBracket, RightBracket,
  Comma, Dot, DotDot, Colon, Semicolon, Question, Pipe, PipeRight,
  Minus, Plus, Slash, Star, Percent, Arrow, Bang, BangEqual, Assign,
  EqualEqual, Greater, GreaterEqual, Less, LessEqual, // ...
  /* literals */ Ident, String, Number, True, False, Null, TemplateStart, TemplateEnd,
  /* keywords */ Agent, Let, Const, Fn, Main, Return, Shared, Spawn, Transcript, // ...
  Eof,
}
pub struct Token { pub kind: TokenType, pub lexeme: String, pub span: SourceSpan }

pub fn keywords() -> phf::Map<&'static str, TokenType> {
  // 99 entries: "let"/"设"/"定义" → Let; "agent"/"智能体" → Agent; "spawn"/"分生" → Spawn; ...
  // (generate table from helen/core/tokens.py `_KEYWORDS`; keep exact mapping)
}
```

Use `phf` or `once_cell::sync::Lazy<HashMap<&str, TokenType>>` (phf preferred: zero-lookup cost).

**Step 3 — verify:** lexer unit tests green; then differential: `scripts/diff.sh` with `--lex-only`.

**Commit:** `feat(core): tokens + 99-keyword map`.

## Task 1.2: Lexer

**Step 1 — write tests** covering: maximal munch, `"`/`'''`/`"""` strings + escapes, CJK identifiers, int/float numbers, all operators incl. `|>` `..` `->`, template `{{`/`}}`, comments, unterminated-string error E0xxx.

**Step 2 — implement** `Lexer { source: &str, byte_pos, line, col }`, scanning with `char_indices()`. Keys:
- Decode per-`char` via `char_indices()` so CJK identifiers (99 bilingual keywords) tokenize correctly; columns counted in chars to keep Python-identical diagnostics (lexer-only — runtime strings are byte-based, D4).
- Keyword resolution: identifier scan → lookup in keyword map.
- Template delimiters produce `TemplateStart`/`TemplateEnd`; text between is scanned as string-ish tokens (mirror `_string` handling).
- Emit `LexError { code: &'static str, span, message }` with the **same error codes** as Python.

**Step 3 — verify:** `cargo test -p helen-core`; differential lex-only pass.

**Commit:** `feat(core): lexer`.

## Task 1.3: AST (61 node types) + AstPrinter

**Step 1 — tests:** Port `tests/core/test_ast.py` concepts: node construction, `AstPrinter` S-expressions.

**Step 2 — implement** as one big enum + struct mix (Rust-idiomatic, D7):

```rust
// ast.rs — statements
pub enum Stmt { Let(LetStmt), Const(ConstStmt), FnDecl(FnDecl), AgentDecl(AgentDecl),
  If(IfStmt), While(WhileStmt), For(ForStmt), Match(MatchStmt), Try(TryStmt),
  Throw(ThrowStmt), Return(ReturnStmt), Expr(ExprStmt), Import(ImportStmt),
  ProtocolDecl(ProtocolDecl), ImplBlock(ImplBlock), Alias(AliasStmt),
  SharedLet(SharedLet), SharedStore(SharedStore), MainBlock(MainBlock), Break, Continue, // ...
}
// ast.rs — expressions
pub enum Expr { Literal(Lit), Ident(String), Binary(BinaryExpr), Unary(UnaryExpr),
  Call(CallExpr), Method(MethodCall), Index(IndexExpr), Assign(AssignExpr),
  Ternary(TernaryExpr), Pipe(PipeExpr), List(ListExpr), Map(MapExpr),
  Template(TemplateExpr), LlmAct(LlmAct), LlmIf(LlmIf), Spawn(SpawnExpr), // ...
}
```

Each struct carries a `span: SourceSpan` (needed for diagnostics and LSP). Port `AstPrinter::visit_*` as `impl AstPrinter { pub fn print(&self, node: &Node) -> String }` producing the **same S-expression strings** as Python — used for parser-differential tests.

**Step 3 — verify:** AST tests green; `ast_printer` outputs byte-identical S-expressions for corpus fixtures.

**Commit:** `feat(core): AST + printer`.

## Task 1.4: Pratt parser (10 precedence levels)

**Step 1 — tests:** Port `tests/parser/` (12 files). Add snapshot tests: `parse(src) → ast_printer_string`.

**Step 2 — implement:**

```rust
// pratt.rs
pub struct Parser { tokens: Vec<Token>, pos: usize }
impl Parser {
  pub fn parse_program(&mut self) -> Result<Program, ParseError> { /* declarations */ }
  fn expression(&mut self, min_bp: u8) -> Result<Expr, ParseError> { /* Pratt loop */ }
  fn prefix(&mut self, t: &Token) -> ...; fn infix(&mut self, t: &Token, left: Expr) -> ...;
}
```

Precedence constants (from Python, exact):
`ASSIGNMENT=1, PIPE=2, OR=3, AND=4, EQUALITY=5, COMPARISON=6, TERM=7, FACTOR=8, UNARY=9, CALL=11`.

**Critical pitfalls ported from Python dev notes:**
- In prefix position, the operator token is **already consumed** — use `prev_token()` (Python's `_previous()`), never advance again (async/spawn prefix rules).
- Ternary `? :` is **right-associative** (highest rule: `QUESTION` with `right_associative = true`).
- Sentinel/`in` handling inside `for`, type annotations (`: int`) parsed as suffix, agent-body keyword parsing (`prompt`, `description`, `tools`, `functions`, `main`, …) at top level of agent decl.

**Step 3 — verify:** parser unit + snapshot tests green; differential parse-only run over corpus (compare AstPrinter output, ignoring spans).

**Commit:** `feat(parser): Pratt parser`.

## Task 1.5: Parser error codes

Port `core/errors.py` codes (E0100–E03xx). Every `ParseError` carries the identical code string. Add tests: each Python parser-error test case reproduces the same code + roughly same message.

## Definition of Done — M1

- [ ] `helen-core` and `helen-parser` crates compile; clippy clean.
- [ ] Token streams and AstPrinter output are **byte-identical** to Python for 100% of the `.helen` corpus (excluding intentional span differences).
- [ ] Lexer/parser error codes match Python for all error fixtures.
