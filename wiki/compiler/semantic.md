# Semantic Analysis — helen-rust

> Rust port of `helen/semantic/` (analyzer.py, symbols.py, types.py,
> type_utils.py, diagnostics). Source: `crates/helen-semantic/src/`.
> Differential gate: exit-code parity (2) + E-code emission order.

---

## Modules

| Rust file | Python source | Contents |
|---|---|---|
| `types.rs` | `types.py` | Type enum (Int, Float, Str, Bool, None, List, Map, Tuple, Agent, Channel, Unknown, Any, TypeVar, Void), any/assignable/equality/type-of, BigInt-aware numeric promotions |
| `symbols.rs` | `symbols.py` | `SymbolTable` — scopes, define/undefine/resolve, nested agent scopes |
| `type_utils.rs` | `type_utils.py` | `type_from_typenode` — `TypeRef`/`TypeRefKind` → `Type` (handles Optional/Union/Literal recursively) |
| `diagnostics.rs` | `errors.py#ErrorReporter` | E-code collection in emission order |
| `analyzer.rs` | `analyzer.py` (1976 LOC) | The analyzer: ~60 visitor methods over `Stmt`/`Expr` |
| `stdlib.rs` | stdlib registry snapshot | module/function/alias tables for import analysis |

## Error Codes (semantic range E0330–E0357)

The `ErrorCode` enum in `helen-core::errors` mirrors the reference:
E0330 SemanticError, E0331 SemanticTypeError, E0332 UndeclaredVariable,
E0333 DuplicateSymbol, ... E0355 TopLevelStatement, E0356
UndeclaredAgentFunction, E0357 AgentFunctionArgMismatch. Full table in
`appendix/error-codes.md`.

### Catch whitelist (plan C1 correction)

The plan claimed 11 native exceptions in the catch whitelist; the current
reference `analyzer.py` uses a **15-entry frozenset** — it accepts
`ValueError`/`TypeError`/`KeyError`/`IndexError`/`FileNotFoundError`/
`PermissionError` and *rejects* `PromptTooLongError`/`LLMOutputContractError`.
The port mirrors actual reference behavior, not the plan note.

## Grammar quirk (C2)

`catch X err` binds the exception value to a variable — validated in the
analyzer (`InvalidCatchType` E0342, `CatchAllNotLast` E0343).

## Scoping

- Module scope → function scopes → agent scopes.
- `shared let` must be module-level (E0351 in reference; verified).
- Agent main cannot see module-level `let` (E0350 ScopeViolation).
- Constants are assignment-protected (E0346 ConstAssignment).
