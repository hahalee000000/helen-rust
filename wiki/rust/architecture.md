# helen-rust — Architecture

This document describes the Rust reimplementation of the Helen programming
language (reference: Python `helen` v1.44/1.45 at `~/helen/`). It covers the
crate layout, the value model, the threading model, and the design decisions
D1–D12 that drive observable behavior.

## 1. Crate layout

The workspace mirrors the Python package layout, with the language core
(lexer → parser → AST → semantic → interpreter → stdlib) pure Rust and Python
interop isolated in two PyO3 crates.

```
crates/
  helen-core/         # source spans, error codes (E-codes), tokens, lexer, AST, ast_printer
  helen-parser/       # Pratt parser (10 precedence levels, bilingual keywords)
  helen-semantic/     # type system (14 types), symbol tables, analyzer (E03xx)
  helen-interpreter/  # Value model, Environment, tree-walk execution, exceptions
  helen-stdlib/       # 378 builtin functions across 22 namespaces (std.*)
  helen-runtime/      # LLM runtime, agents, tools, skills, MCP, context, transcript
  helen-rust/         # crates.io package `helen-rust` — binary `helen` (CLI, REPL, docgen, LSP)
  helen-lsp/          # Language server (JSON-RPC 2.0)
  helen-ffi/          # Helen → Python FFI (PyO3, feature-gated `python-ffi`)
  helen-python-bridge/# Python → Helen bridge (maturin cdylib; PyPI dist `helen-rust`)
```

Dependency direction is strictly downward: `core ← parser ← semantic ←
interpreter ← stdlib`; `runtime` depends on interpreter+stdlib; `helen-rust`
is the only binary crate.

## 2. Value model

`Value` is the runtime representation of Helen values (design decision D2):

```
Value::Null | Bool | Int(num_bigint::BigInt) | Float(f64) | Str(Rc<str>)
     | List(Rc<RefCell<Vec<Value>>>) | Map(Rc<RefCell<IndexMap<Value, Value>>>)
     | Function(...) | NativeFn(...) | Agent(...) | Channel(...) | ...
```

Key properties:

- **Arbitrary-precision integers from M3** (D3) — `Int` wraps
  `num_bigint::BigInt`; there is no overflow path. The corpus already
  contained >i64 literals and `10^26 * 2` prints correctly.
- **Mutable collections via `Rc<RefCell<…>>`** — Python object
  identity/sharing semantics. `snapshot()` is a shallow scope-copy that
  clones the `Rc` (not deep), matching Python's alias behavior.
- **Maps are `indexmap::IndexMap`** (D5) — insertion-ordered, arbitrary
  hashable keys (`Value` implements `Hash`/`Eq` structurally), so
  `{"a": 1, 2: "two"}` is legal and `m[2]` works.
- **Strings are native UTF-8 byte-based** (D4) — `len()` is the byte length;
  index/slice use byte offsets with UTF-8 boundary validation. String
  iteration is intentionally unsupported (Helen raises at runtime; only
  lists iterate). ASCII behavior is identical to Python; non-ASCII
  divergences are tracked in `wiki/rust/migration-notes.md`.
- **Exceptions are a value-carrying struct** (D6) — `class_name`, `message`,
  fields. `catch` matches exactly the 11 Helen-native predefined names
  (`AnyError, LLMError, TimeoutError, ModelError, PromptTooLongError,
  AgentError, LLMOutputContractError, ToolError, RuntimeError,
  AssertionError, AggregateError`); no hierarchy map; `catch X err` requires
  a bound variable (E0301).

## 3. Threading model

Synchronous tree-walk interpreter (D1), mirroring Python exactly:

- **`spawn` → `std::thread`** (Python uses daemon threads, not asyncio).
  The spawned thread runs an independent `Interpreter` sharing the parent
  environment snapshot; a `Channel` endpoint is injected as the **last**
  parameter (user args bind positionally to non-Channel params).
- **Channel close** pushes a sentinel; `receive` after close returns `None`.
- **LLM HTTP is blocking** (`ureq`), matching Python's sync `httpx`; there is
  no async poison throughout the core.
- **Spawn thread safety** requires `unsafe impl Send` on the shared
  `Arc<dyn LlmRuntime>` wrapper plus `#[allow(arc_with_non_send_sync)]` —
  see `crates/helen-interpreter/src/` spawn code.
- The **Python bridge** runs Helen agents inside the GIL on the calling
  thread; `async_call` offloads to an executor thread.

## 4. Design decisions (D1–D12)

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | Sync tree-walk interpreter; `spawn` → `std::thread`; blocking LLM HTTP | Mirrors Python exactly (sync interpreter + daemon threads + sync `httpx`) |
| D2 | `Value` enum with `Rc<RefCell<…>>` mutable collections | Python identity/sharing semantics; shallow `snapshot()` |
| D3 | `Int(BigInt)` — arbitrary precision from M3 | Corpus has >i64 literals; no overflow path |
| D4 | Native UTF-8 byte-based strings; byte-offset index/slice; no string iteration | Deliberate non-ASCII deviation; ASCII identical; tracked in migration-notes |
| D5 | `Map` = `indexmap::IndexMap` with arbitrary hashable `Value` keys | Python-dict semantics (insertion order, any hashable key) |
| D6 | Exceptions = value-carrying struct; exactly 11 predefined names | Matches `interpreter/exceptions.py`; no hierarchy map |
| D7 | `match` over enum AST instead of Python Visitor OOP | Idiomatic Rust; visitor only where dispatch-by-type is needed |
| D8 | Custom LLM providers (`~/.helen/providers/*.py`) only via PyO3 FFI | Python scripts can't run in pure Rust; documented limitation |
| D9 | `helen-ffi` embeds CPython (`auto-initialize`); bridge is a maturin cdylib | Both interop directions preserved |
| D10 | Conformance = differential + pytest adoption (Tier A/B/C) | 3,860 pytest tests are the spec; measurable parity |
| D11 | Print/str/repr display parity is a first-class task; dedicated display corpus | Byte-identical stdout requires exact top-level-str vs repr-in-container rules |
| D12 | LLM tests run the Python reference in-process with `MockLLMRuntime` | No env-var mock hook; `~/helen/` must not be modified |

## 5. Module system & stdlib

- 22 user-facing namespaces (`std.core, std.str, std.list, std.dict,
  std.math, std.time, std.file, std.system, std.io, std.data, std.network,
  std.path, std.tools, std.debug, std.context, std.transcript, std.media,
  std.test, std.quality, std.llm, std.crypto, std.concurrency`), 378
  builtins with Chinese aliases.
- Builtins are **not globals** — `print` requires `import std.core.*`
  (v1.39+ semantics); bare `print` → E0332 `undeclared variable 'print'`.
- `type()` returns strings (`"int","str","list","dict","NoneType","bool"`);
  `isinstance(x, "int")` takes a string; `range(n)` returns a list; only
  lists iterate in `for`.

## 6. Conformance infrastructure

- `tests/conformance/` — golden capture, reference driver
  (`reference.py`, D12), error-diff sweep, expected divergences doc.
- `tests/programs/` — authored + interpreter + display corpora with goldens.
- `scripts/` — diff harnesses (Tier A/B), benchmark, error-diff generator,
  corpus sync, parity sweep, installer.
- CI (`.github/workflows/ci.yml`) — Rust build/lint/test, bridge wheel +
  DoD suite, conformance vs the Python reference.
