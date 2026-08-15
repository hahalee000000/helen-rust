# AST Node Definitions — helen-rust

> Rust port of `helen/core/ast.py`. Source: `crates/helen-core/src/ast.rs`.
> The *language* AST shape is identical to the reference; this page documents
> the Rust representation and the fidelity traps that matter when extending it.

---

## Rust Representation

- `Expr` — expression enum (~60 variants), matching the Python expression node
  classes 1:1. Fields carry `span: SourceSpan`.
- `Stmt` — statement enum (var/const declarations, fn decl, agent decl,
  protocol decl, shared store, try/match/llm/spawn/pipe/import, throw/catch,
  control flow).
- `TypeRef` — type reference with `TypeRefKind`:
  - `Simple` — plain name (`int`, `str`, `MyType`)
  - `Optional` — `T?` (Python `OptionalTypeNode`)
  - `Union` — `A|B` (Python `UnionTypeNode`)

### ⚠️ Type-annotation fidelity trap

The Python reference keeps composite type annotations as **real expression
nodes** (`OptionalTypeNode` / `UnionTypeNode`). A naive port collapses
`int?` / `str|int` into synthetic `TypeRef` names (`optional<int>`,
`union<str|int>`) — this breaks `type_from_typenode` in the semantic layer.

helen-rust fixes this via `TypeRefKind` (M2 Phase B): composite types are
structurally represented, not string names. When the M1 AST printer omits
annotations, a parse-diff can pass while the AST is structurally wrong —
**always verify annotation/optional/union nodes explicitly, not just printed
output** (see `differential-porting` skill).

## Visitor Pattern

The Python `Visitor` base class (47 methods) is not ported as a trait; the
interpreter and printer use exhaustive `match` over `Expr`/`Stmt` instead.
When adding a variant, expect to touch every exhaustive-match site (interpreter
eval, printer, analyzer, semantic type-from-typenode).

## AST Printer

`crates/helen-core/src/ast_printer.rs` — S-expression output that must be
**byte-identical** to the Python `AstPrinter`, including the dataclass-repr
quirk for map entries and `Token.__repr__` formatting (`py_repr_value`).
