# Helen Language → Rust Rewrite: Master Implementation Plan

**Goal:** Reimplement the complete Helen language (currently Python v1.44.0 at `~/helen/`) in Rust at `~/helen-rust/`, achieving **feature parity** — including the Python FFI (Helen → Python) and the Python Bridge (Python → Helen).

**Architecture:** A Cargo workspace with layered crates mirroring the Python package layout. The language core (lexer → parser → AST → semantic → interpreter → stdlib) is pure Rust with zero heavy dependencies; Python interop lives in two PyO3-based crates (`helen-ffi`, `helen-python-bridge`) that depend on the core. A differential-testing harness (Python interpreter vs. Rust interpreter) is the primary conformance mechanism; the **3,860 existing pytest tests are the specification** and are adopted/reimplemented across three tiers (see §8 and `15-conformance-testing.md`).

**Tech Stack:** Rust 1.97+ (workspace), `serde`/`serde_json`, `indexmap` (dict order), `num-bigint` (arbitrary-precision ints, D3), `rusqlite` (transcript SQLite backend), `ureq` or `reqwest::blocking` (sync HTTP for LLM), `pyo3` 0.23+ (FFI + bridge), `maturin` (bridge packaging), `unicode-normalization` (string case ops). Tokio only inside the bridge's Python-side `async_call` support.

---

## 1. Verified Source Facts (from `~/helen/`, v1.44.0)

| Metric | Value |
|---|---|
| Python code | ~35k lines in `helen/` (excl. webui frontend) |
| Tests | 203 files, 3,860 tests (pytest) — **no extractable `.helen` fixtures** (only 2 `.helen` files under `tests/`; suites build ASTs programmatically or inline `run_helen(src)` helpers) |
| Bilingual keywords | **99** (48 English + 51 Chinese) |
| Token types | **88** (`TokenType` variants) |
| AST node classes | **61** (`*Node` classes) |
| Precedence levels | **10** (`Precedence` 0–9, CALL=11) |
| Type system | **14 types** (Any, Bool, Int, Float, String, Null, Optional, List, Map, Union, Literal, Agent + Number, Type) |
| Stdlib builtins | **378** `BuiltinFunction` registrations across **22 user-facing namespaces** (`std.core, std.str, std.list, std.dict, std.math, std.time, std.file, std.system, std.io, std.data, std.network, std.path, std.tools, std.debug, std.context, std.transcript, std.media, std.test, std.quality, std.llm, std.crypto, std.concurrency`) |
| Built-in tools | **11** (web_search, web_fetch, read_file, write_file, shell_exec, calculate, patch_file, find_files, search_files, load_skill, list_skill_references) |
| FFI | `helen/ffi/` — 5 modules (contracts, python_module, python_object, python_runtime, type_converter) |
| Bridge | `helen/python_bridge/` — 6 modules (agent_wrapper, decorators, function_wrapper, import_hook, type_converter) |
| LLM runtime | `LLMRuntime` ABC (`route`/`act`/`act_stream`) + `MockLLMRuntime`; `PlatformProtocol` for 6+ OpenAI-compatible providers; custom providers from `~/.helen/providers/*.py` |
| Transcript | JSONL/SQLite backends, LRU cache, BoundaryMarker compression |
| Interpreter model | **Synchronous tree-walk**; `spawn` uses daemon threads (not asyncio) |
| CLI | `helen <file> | check | test | repl | docgen | ...` |
| LSP | `helen/lsp/server.py` (1,375 lines) |
| Exit codes | **0** success · **2** semantic/compile error · **3** runtime error |

**Verified semantics (used as spec throughout — see `05-interpreter-core.md` §3.1):**
- Operator set is `+ - * / % == != < <= > >= && || ! |> .. -> =`. **No `//`, no `**`, no bitwise `| & ^ ~ << >>`** (all parse errors). `/` **always returns float** (`7/2` = 3.5). `%` follows Python's sign-of-divisor semantics (`-7 % 3` = 2, `7 % -3` = -2). `int()` truncates toward zero.
- Integer literals are **arbitrary-precision** already today (corpus contains >i64 literals; `99999999999999999999999999 * 2` prints correctly) → `num-bigint` from M3.
- Map keys are **arbitrary hashable values** (`{"a": 1, 2: "two"}` is legal; `m[2]` works). Python-dict semantics, insertion-ordered.
- `type()` returns **strings** (`"int","str","list","dict","NoneType","bool"`); `isinstance(x, "int")` takes a string.
- `range(n)` returns a **list**. Only **lists** are iterable in `for`: iterating a string or dict is a runtime error.
- Builtins (`print`, `len`, `str`, …) are **not globals** — they require `import std.core.*` (v1.39+). Bare `print` → E0332 `undeclared variable 'print'`.
- Predefined exceptions are **exactly 11 Helen-native names**: `AnyError, LLMError, TimeoutError, ModelError, PromptTooLongError, AgentError, LLMOutputContractError, ToolError, RuntimeError, AssertionError, AggregateError`. **No Python exception names** (`TypeError`, `ValueError`, … are invalid in `throw`/`catch` → error). No exception-hierarchy map. `catch X err` **requires a bound variable** (`catch X` alone → E0301 `Expected error variable name`).
- `try/catch` calls in stdlib wrap Python-side errors as `RuntimeError` (e.g. `int("abc")` → `RuntimeError: Python ValueError: invalid literal ...`).
- **No `async`/`await`/`for await` in the language.** No AWAIT token, no ForAwait AST node. LLM streaming is via **callbacks**: `llm act "..." { on_chunk(chunk) {} on_complete(result) {} }`. Task-level async exists only on the bridge's Python side (`async_call`).
- No `HELEN_LLM_MOCK` env hook exists. Tests inject `MockLLMRuntime` **in-process** (`Interpreter(llm_runtime=MockLLMRuntime())`). CLI preflight requires an API key unless `HELEN_API_KEY` is set (the test suite's `conftest.py` sets a dummy value).

## 2. Non-Goals (explicitly out of scope for v1)

- `helen/agent/webui` (FastAPI + React frontend) — deployable later against the Rust runtime.
- `helenagent` (the long-lived actor orchestrator program built in Helen) — a Helen program; can run on the Rust interpreter once complete.
- Rust-bytecode JIT/compiler — the Rust port is a tree-walk interpreter like the original.

## 3. Cargo Workspace Layout

```
helen-rust/
├── Cargo.toml                     # [workspace] members = crates/*
├── crates/
│   ├── helen-core/                # source, tokens, lexer, errors, ast, ast_printer
│   ├── helen-parser/              # Pratt parser (dep: core)
│   ├── helen-semantic/            # types, symbols, analyzer (dep: core)
│   ├── helen-interpreter/         # value, env, exceptions, interpreter, agent, import,
│   │                              #   closure, pattern, readonly_view, shared_store (dep: core, parser, semantic)
│   ├── helen-stdlib/              # 378 builtins + zh aliases (dep: interpreter)
│   ├── helen-runtime/             # llm, provider, tools, skills, transcript, history,
│   │                              #   compression, memory, mcp, config, observability (dep: interpreter)
│   ├── helen-ffi/                 # PyO3: Helen→Python (feature-gated, dep: interpreter, pyo3)
│   ├── helen-python-bridge/       # PyO3 cdylib: Python→Helen (maturin; PyPI dist `helen-rust`, module `helen_rust`)
│   ├── helen-rust/                # crates.io package `helen-rust`, binary `helen` (dep: all core crates + runtime)
│   └── helen-lsp/                 # LSP server (dep: core, parser, semantic)
├── tests/
│   ├── conformance/               # differential harness (Rust candidate + Python reference driver)
│   │   ├── reference.py           # in-process Python driver w/ MockLLMRuntime (D12)
│   │   ├── extract_corpus.py      # pytest source-string extraction (Tier A)
│   │   └── golden/                # captured reference outputs (stdout/exit/error classes)
│   ├── programs/                  # differential corpus (authored + extracted)
│   └── rust/                      # Tier-C reimplemented pytest suites as Rust tests
└── wiki/
    └── plan/                      # ← this plan
```

Dependency direction is strictly **upward**: `helen-core` has no internal deps; nothing except `helen-ffi`/`helen-python-bridge`/`helen-rust` touches PyO3.

## 4. Key Design Decisions

| # | Decision | Rationale |
|---|---|---|
| D1 | **Sync tree-walk interpreter** in Rust; `spawn` → `std::thread`; LLM HTTP via blocking client | Mirrors Python exactly (sync interpreter + daemon threads + sync `httpx`); avoids async poison throughout |
| D2 | **`Value` enum** with `Rc<RefCell<…>>` for mutable collections | Python object identity/sharing semantics; `snapshot()` = shallow scope-copy + `Rc` clone |
| D3 | `Int(num_bigint::BigInt)` — **arbitrary precision from M3** | Verified: corpus already contains >i64 literals (`10^26 * 2` runs today). No overflow path, full parity |
| D4 | Strings are **native UTF-8 byte-based** `String`/`Rc<str>`: `len()` = byte length; index/slice by byte offsets with UTF-8 boundary validation; **no code-point-indexed wrapper**. **String iteration intentionally unsupported** (`for c in "abc"` is a runtime error in Helen — only lists iterate) | Deliberate deviation from Python for non-ASCII (CJK); ASCII behavior identical. Simplifies value model + stdlib. Divergences tracked in `wiki/rust/migration-notes.md` (M14) |
| D5 | `Map` backed by `indexmap::IndexMap` with **arbitrary hashable keys** (`Value` wrapper impls `Hash`/`Eq` structurally) | Python-dict semantics verified: `{"a": 1, 2: "two"}` legal, insertion-order iteration, any-hashable keys |
| D6 | Exceptions as a **value-carrying struct** (`class_name`, `message`, fields). `catch` matches against **exactly the 11 Helen-native predefined names**; no hierarchy map; `catch X err` requires a bound variable (E0301) | Verified against `interpreter/exceptions.py` — Python exception names are *not* valid catch/throw types |
| D7 | Interpreter and semantic analyzer use **`match` over enum AST** instead of Python's Visitor OOP | Idiomatic Rust; visitor-trait only in `ast_printer`/analyzer where dispatch-by-type is needed |
| D8 | Custom LLM providers (`~/.helen/providers/*.py`) loaded **only via PyO3** (FFI on); built-in providers native Rust | Python provider scripts cannot run in pure Rust; document limitation |
| D9 | `helen-ffi` embeds CPython via `pyo3` (`auto-initialize`); `helen-python-bridge` is a `cdylib` built by maturin | Both directions of Python interop preserved |
| D10 | Conformance = **differential testing + pytest adoption/reimplementation**. The 3,860 pytest tests are the spec: Tier A extracts inline source strings → differential corpus; Tier B runs subprocess-driven tests directly against the Rust binary; Tier C reimplements AST-constructed suites as Rust tests | Pytest tests cannot run as-is against Rust (they import `helen.*` Python internals); this three-tier mapping gives measurable parity on the real spec, not an invented corpus |
| D11 | **print/str/repr display parity is a first-class M3 task** | Verified asymmetry: top-level `print(true)`→`true`, `print(null)`→`None`, but nested `[1,'a',true,null]`→`[1, 'a', True, None]` (Python repr of elements); floats use Python repr (`1e+20`, `1.5e-05`). Byte-identical stdout requires exact `str()`-at-top-level vs repr-in-container rules |
| D12 | LLM-dependent differential tests run the **Python reference via an in-process driver** (`tests/conformance/reference.py`) that constructs `Interpreter(llm_runtime=MockLLMRuntime())` | No env-var mock hook exists and `~/helen/` must not be modified; CLI subprocess runs are only used for non-LLM programs with `HELEN_API_KEY` set |

## 5. Phases & Milestones (dependency order)

| Phase | Doc | Scope | Est. effort |
|---|---|---|---|
| M0 | `02-workspace-setup.md` | Workspace, CI, diff harness + reference driver, corpus extraction tooling | 1 wk |
| M1 | `03-core-frontend.md` | source, tokens (99 bilingual keywords), lexer, AST (61 nodes), Pratt parser | 2–3 wk |
| M2 | `04-semantic-analyzer.md` | 14 types, symbols, analyzer, 100+ semantic error codes | 2 wk |
| M3 | `05-interpreter-core.md` | Value model (BigInt), env, exceptions (11 native), control flow, fn/closures, pattern match, pipe, display parity, imports | 3–4 wk |
| M4 | `06-stdlib.md` | 378 builtins across 22 namespaces + Chinese aliases | 4–6 wk |
| M5 | `07-llm-runtime.md` | LLMRuntime, provider protocols, HTTP streaming, prompt builder, model caps | 2–3 wk |
| M6 | `08-agent-runtime.md` | agent exec, `llm act/if`, tools registry (11 tools), skills system, tool-calling loop, on_chunk streaming | 2–3 wk |
| M7 | `09-concurrency.md` | `spawn` + Channel, `shared let`/`shared store`, `mailbox_select`, ReadOnlyView | 2 wk |
| M8 | `10-context-management.md` | TranscriptStore, history, 5-layer compression, working memory, observability, session mgmt | 3 wk |
| M9 | `11-mcp.md` | MCP client, registry, server manager | 1–2 wk |
| M10 | `12-python-ffi.md` | Helen → Python (PyO3 embedded CPython) | 2 wk |
| M11 | `13-python-bridge.md` | Python → Helen (PyO3 cdylib + import hook) | 2 wk |
| M12 | `14-cli-lsp.md` | `helen` CLI, REPL, check/test, docgen, formatter, LSP | 2–3 wk |
| M13 | `15-conformance-testing.md` | Three-tier pytest adoption, full differential run, benchmarks, coverage report | 3 wk |
| M14 | `16-release.md` | Packaging, docs, acceptance checklist | 1 wk |

**Total estimate:** ~30–38 person-weeks (single experienced Rust dev; parallelizable in parts).

## 6. Critical Path

```
M0 setup ──► M1 frontend ──► M3 interpreter ──► M6 agent runtime
                              │                    │
                              ├─► M4 stdlib ────────┤
                              ├─► M5 llm runtime ───┤
                              └─► M7 concurrency ───┘
                                       │
                    M10 FFI ──► M11 bridge  (can start after M3, in parallel)
                    M2 semantic (parallel with M3)
                    M8 context (after M3+M5)
                    M9 MCP (after M5+M6)
                    M12 CLI/LSP (incremental from M1)
```

## 7. Definition of Done (acceptance)

1. All 203 Python test modules / 3,860 tests mapped and green across the three tiers: **Tier A** extracted differential corpus byte-identical (stdout, exit code, error classes); **Tier B** subprocess-driven suites pass against the Rust `helen` binary; **Tier C** reimplemented lexer/parser/semantic/execution Rust tests pass.
2. `helen check`, `helen <file>`, `helen test`, REPL, and LSP feature-complete (feature matrix in `14-cli-lsp.md`).
3. Python FFI: `import "numpy" as np` and `requests` examples from `examples/python_bridge` run unmodified.
4. Python Bridge: `from translator import TranslatorAgent` works via import hook; sync + async + keyword-arg calls pass.
5. Installable packages are named **`helen-rust`** in both ecosystems — crates.io `helen-rust` (CLI binary `helen`) and PyPI wheel `helen-rust` (module `helen_rust`). `cargo install helen-rust` and `pip install helen-rust` both work.
6. Benchmark suite (ported from `tests/performance/test_benchmarks.py`) shows parity or better vs. Python.

## 8. Conformance Strategy (three-tier pytest adoption)

| Tier | Mechanism | Suites (test counts) |
|---|---|---|
| **A — extracted differential corpus** | `extract_corpus.py` parses pytest files (Python `ast`), pulls inline Helen source strings from `run_helen()`-style helpers, applies the stdlib import prefix (`import std.core.* …`, v1.39 no-globals rule), emits `.helen` files; `reference.py` captures goldens (stdout/exit/error-class) | interpreter (355), stdlib (942), runtime (928, Mock LLM), language in-process (part of 100), agent in-process (part of 139), cli in-process (2) |
| **B — subprocess adoption** | Tests that invoke `helen <file>` via `subprocess` run as-is in CI with the Rust binary shadowing `helen` on `PATH` + `HELEN_API_KEY` set (the suite's own `conftest.py` pattern) | language module-import, agent cross-platform, ffi subprocess, cli golden |
| **C — reimplementation** | Suites that construct ASTs programmatically (`LiteralNode`, `ProgramNode`, …) cannot be extracted; port each test case to Rust (mirror the Python test logic against the Rust AST + interpreter) | execution (360), parser (114), core (121), semantic (207), lexer (181) |

Details, tooling contracts, and per-suite DoD: `15-conformance-testing.md`.

## 9. Risks

| Risk | Mitigation |
|---|---|
| String divergence for non-ASCII (byte- vs code-point semantics) | D4: native UTF-8 strings; ASCII-only differential for string ops + expected-diff list for non-ASCII; `%` formatting ported byte-safely; string iteration intentionally unsupported (matches Helen) |
| Display parity (`print`/`str`/repr of nested containers, floats) | D11: dedicated M3 task + authored `tests/programs/display/` corpus asserting byte-identical stdout |
| `num-bigint` perf in hot loops | Benchmarks in M13; `num-bigint` only, no hybrid — parity first |
| Custom Python LLM providers in pure Rust | D8; FFI fallback + docs |
| 378 stdlib functions — subtle behavioral differences | Differential corpus per stdlib namespace; port Python unit-test inputs as data tables |
| `spawn` thread semantics & channel fairness | Port `tests/interpreter/test_spawn*`; stress tests |
| PyO3 + Python 3.12 ABI | Pin pyo3 0.23+, test in CI with python3.12 headers |
| Scope-isolation rules (v1.10/1.12) interplay with closures | Port `tests/semantic` + `tests/interpreter` isolation cases first |
| pytest extraction fidelity (Tier A misses assertions) | Extraction emits provenance (test file + test name); goldens are committed and code-reviewed; every extracted case is listed in `tests/conformance/manifest.json` |

---

**Navigation:** continue to `01-feature-inventory.md` (the source-of-truth feature checklist), then phase docs `02`…`16`.
