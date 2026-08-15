# Type System — helen-rust

> Rust port of `helen/semantic/types.py`. Source: `crates/helen-semantic/src/types.rs`.

---

## The 14 Types

| Type | Notes |
|---|---|
| Int | arbitrary-precision (BigInt-backed) |
| Float | f64 |
| Str | byte-based string (D4) |
| Bool | bool |
| None | `null` |
| List | homogeneous elements |
| Map | heterogeneous key/value |
| Tuple | heterogeneous (M4) |
| Agent | agent type |
| Channel | M7 concurrency |
| Unknown | unannotated / unbound |
| Any | `any` |
| TypeVar | generic param placeholder |
| Void | function return `void` |

## Type Relations

- `is_assignable` — gradual typing: `Unknown`/`Any` assignable to anything;
  numeric promotion `Int → Float`; exact match otherwise.
- `type_of` — value → type (BigInt-aware: ints stay Int regardless of size).
- `type_from_typenode` — converts a `TypeRef` (with `TypeRefKind::Optional`/
  `Union`) into a `Type`. Composite annotations require the structural
  `TypeRefKind` representation — see `compiler/ast.md` fidelity trap.

## Where the type system is used

- `analyzer.rs` — declaration checking, call argument type checks
  (`AgentFunctionArgMismatch` E0357, `AgentParamMismatch` E0347), const
  assignment protection, catch-type validation.
- Runtime type checks in `helen-interpreter` (`call_function`/
  `call_closure`) mirror the analyzer for direct calls.
