# Execution Engine — helen-rust

> Rust port of `helen/interpreter/`. Source: `crates/helen-interpreter/src/`.

---

## Components

| Rust file | Python source | Contents |
|---|---|---|
| `value.rs` | `value.py` | `Value` enum (BigInt ints, byte strings, structural maps, exceptions, callables) |
| `environment.rs` | `environment.py` | lexical scope chain, const protection, shared-store hooks |
| `exceptions.rs` | `exceptions.py` | `ExceptionValue`, Python-identical `__str__`, `Flow` sentinels (break/continue/return), `error_matches` (hierarchy-based) |
| `closure.rs` | closure | free-variable analysis, `Closure` capture |
| `interpreter.rs` | `interpreter/` | statement/expression dispatch, binary ops (py-mod sign semantics), try/catch/finally, throw, match/patterns, call_function/call_closure with runtime type checks, closures, index/access (map methods get/keys/values/items), core builtins |

## Key parity semantics

- **Integer arithmetic** — arbitrary precision; division/modulo use Python
  floor-division sign semantics.
- **Control flow** — break/continue/return are `Flow` sentinels propagated
  through the interpreter, not errors.
- **try/catch/finally** — catch-all rethrows; finally overrides the pending
  exception/return (Python semantics).
- **`match`** — pattern matching with default branch (E0345/E0349).
- **`null` vs `None`** — converted at the interpret boundary (M13 fix:
  `null → None` at interpret boundary, matching reference).
- **Uncaught exception rendering** — `RuntimeError: {e}` with the exception's
  `__str__` (`RuntimeError:{loc} msg` for generic runtime errors).

## Stdlib binding

Core builtins (`print`/`len`/`str`/`int`/`float`/`bool`/`type`/`isinstance`/
`range`/`abs`/`min`/`max`/`list`/`dict`) are pre-bound into the global
environment at `Interpreter::new`, mirroring what `import std.core.*` binds
in the reference. Full stdlib modules live in `helen-stdlib` (M4).
