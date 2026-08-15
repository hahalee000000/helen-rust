# Helen Rust Overall Architecture

> **helen-rust** — the Rust reimplementation of the Helen Agent Programming
> Language. This document describes the implementation-specific architecture.
> The *language* architecture (3-layer: Core / Runtime / Toolchain) is shared
> with the Python reference — see `design-philosophy.md` and `language-spec.md`.

---

## Crate Layout (10-crate workspace)

```
crates/
├── helen-core/        # tokens, lexer, AST, AST printer, source spans, errors (ErrorCode)
├── helen-parser/      # Pratt parser (expressions, statements, agents, protocols, shared stores)
├── helen-semantic/    # type system, symbol tables, analyzer, diagnostics, stdlib registry
├── helen-interpreter/ # value model, environment, exceptions, closures, builtins, statement/expression eval
├── helen-stdlib/      # standard library registry (22 modules, 378 builtins + aliases)
├── helen-runtime/     # LLM runtime, agent runtime, tools, skills, MCP, transcript/context, shared store
├── helen-lsp/         # language server (JSON-RPC 2.0)
├── helen-ffi/         # Python FFI (Helen → Python) — pyo3, `python-ffi` feature
├── helen-python-bridge/ # Python Bridge (Python → Helen) — maturin cdylib `helen_rust`
└── helen-rust/        # CLI binary (`helen`)
```

Dependency direction is strictly forward: `core ← parser ← semantic ←
interpreter ← stdlib ← runtime ← {lsp, ffi, bridge, cli}`.

## Value Model

- `helen_interpreter::value::Value` — the single runtime value type, with
  variants mirroring the Python reference: Int (BigInt arbitrary precision),
  Float, Str (byte-based strings, D4), Bool, None, List, Map, Tuple,
  Function/Closure, Native, Exception, Channel (M7).
- Python-parity `==` (int/float coercion, NaN handling), truthiness rules,
  and `python_str`/`to_display` rendering (ryu-based `py_str_float`).
- **Each new `Value` variant touches ~10 exhaustive-match sites** (see
  `migration-notes.md`).

## Threading Model

- Single-threaded interpreter by default; `spawn` (M7) uses OS threads with a
  **deep-owned `SpawnPayload` snapshot** (fresh `Rc`s via `clone_owned`), plus
  `unsafe impl Send` under a documented single-owner discipline.
- stdout is `Arc<Mutex<String>>` so spawned threads append output safely.
- `Arc<dyn LlmRuntime>` trait objects force `&self` methods; implementors use
  interior mutability (`RefCell`/`Mutex`).

## Design Decisions D1–D12

See `wiki/rust/architecture.md` for the full design-decision record
(byte-based strings D4, string iteration unsupported, spawn race strictness,
custom-provider Python dependency, context/compression quirks, and more).

## Frontend Pipeline

```
source ──Scanner(lexer)──▶ Vec<Token>
       ──Parser(pratt)───▶ Vec<Stmt>          (helen-parser)
       ──Analyzer───────▶ diagnostics / types  (helen-semantic)
       ──Interpreter────▶ stdout / exit code   (helen-interpreter)
```

Differential verification at every layer: lex JSON, `--parse` AST JSON,
`--semantic-only` (exit 2 + E-code order), `--run` (`{stdout, stderr,
exit_code, error_classes}`).
