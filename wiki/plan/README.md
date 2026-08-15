# Helen Language → Rust Rewrite: Master Implementation Plan

**Goal:** Reimplement the complete Helen language (currently Python v1.44.0 at `~/helen/`) in Rust at `~/helen-rust/`, achieving **feature parity** — including the Python FFI (Helen → Python) and the Python Bridge (Python → Helen).

**Architecture:** A Cargo workspace with layered crates mirroring the Python package layout. The language core (lexer → parser → AST → semantic → interpreter → stdlib) is pure Rust with zero heavy dependencies; Python interop lives in two PyO3-based crates (`helen-ffi`, `helen-python-bridge`) that depend on the core. A differential-testing harness (Python interpreter vs. Rust interpreter on a shared `.helen` corpus) is the primary conformance mechanism.

**Tech Stack:** Rust 1.97+ (workspace), `serde`/`serde_json`, `indexmap` (dict order), `rusqlite` (transcript SQLite backend), `ureq` or `reqwest::blocking` (sync HTTP for LLM), `pyo3` 0.23+ (FFI + bridge), `maturin` (bridge packaging), tokio only inside the bridge's `async_call` support.

---

## 1. Verified Source Facts (from `~/helen/`, v1.44.0)

| Metric | Value |
|---|---|
| Python code | ~35k lines in `helen/` (excl. webui frontend) |
| Tests | 203 files, ~55k lines, 3,694+ tests |
| Bilingual keywords | **99** (48 English + 51 Chinese) |
| Token types | **88** (`TokenType` variants) |
| AST node classes | **61** (`*Node` classes) |
| Precedence levels | **10** (`Precedence` 0–9, CALL=11) |
| Type system | **14 types** (Any, Bool, Int, Float, String, Null, Optional, List, Map, Union, Literal, Agent + Number, Type) |
| Stdlib builtins | **378** `BuiltinFunction` registrations (~25 modules) |
| Built-in tools | **11** (web_search, web_fetch, read_file, write_file, shell_exec, calculate, patch_file, find_files, search_files, load_skill, list_skill_references) |
| FFI | `helen/ffi/` — 5 modules (contracts, python_module, python_object, python_runtime, type_converter) |
| Bridge | `helen/python_bridge/` — 6 modules (agent_wrapper, decorators, function_wrapper, import_hook, type_converter) |
| LLM runtime | `LLMRuntime` ABC (`route`/`act`/`act_stream`) + `MockLLMRuntime`; `PlatformProtocol` for 6+ OpenAI-compatible providers; custom providers from `~/.helen/providers/*.py` |
| Transcript | JSONL/SQLite backends, LRU cache, BoundaryMarker compression |
| Interpreter model | **Synchronous tree-walk**; `spawn` uses daemon threads (not asyncio) |
| CLI | `helen <file> | check | test | repl | docgen | ...` |
| LSP | `helen/lsp/server.py` (1,375 lines) |

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
│   ├── helen-python-bridge/       # PyO3 cdylib: Python→Helen (maturin, dep: interpreter, pyo3)
│   ├── helen-cli/                 # binary `helen` (dep: all core crates + runtime)
│   └── helen-lsp/                 # LSP server (dep: core, parser, semantic)
├── tests/
│   ├── conformance/               # differential harness (Rust + Python reference)
│   └── programs/                  # .helen corpus used by conformance + smoke tests
└── wiki/
    └── plan/                      # ← this plan
```

Dependency direction is strictly **upward**: `helen-core` has no internal deps; nothing except `helen-ffi`/`helen-python-bridge`/`helen-cli` touches PyO3.

## 4. Key Design Decisions

| # | Decision | Rationale |
|---|---|---|
| D1 | **Sync tree-walk interpreter** in Rust; `spawn` → `std::thread`; LLM HTTP via blocking client | Mirrors Python exactly (sync interpreter + daemon threads + sync `httpx`); avoids async poison throughout |
| D2 | **`Value` enum** with `Rc<RefCell<…>>` for mutable collections | Python object identity/sharing semantics; `snapshot()` = shallow scope-copy + `Rc` clone |
| D3 | `Int(i64)` with checked arithmetic, overflow → `OverflowError` | Python ints are arbitrary-precision; i64 first, upgrade to `num-bigint` only if conformance corpus overflows |
| D4 | Strings as **code-point indexed** (operate on `char_indices()`), `len()` = code-point count | Must match Python `str` semantics — **top compatibility risk** (see `04-…` risks) |
| D5 | `Map` backed by `indexmap::IndexMap` | Python dict insertion-order iteration |
| D6 | Exceptions as a **value-carrying struct** (`class_name`, `message`, fields) matched by name in `catch` | Mirrors Python's class-based catch (`catch TypeError`), incl. predefined-exception whitelist |
| D7 | Interpreter and semantic analyzer use **`match` over enum AST** instead of Python's Visitor OOP | Idiomatic Rust; visitor-trait only in `ast_printer`/analyzer where dispatch-by-type is needed |
| D8 | Custom LLM providers (`~/.helen/providers/*.py`) loaded **only via PyO3** (FFI on); built-in providers native Rust | Python provider scripts cannot run in pure Rust; document limitation |
| D9 | `helen-ffi` embeds CPython via `pyo3` (`auto-initialize`); `helen-python-bridge` is a `cdylib` built by maturin | Both directions of Python interop preserved |
| D10 | Conformance = **differential testing** against the Python interpreter on a `.helen` corpus | 55k lines of pytest cannot be mechanically ported; differential testing plus targeted ported tests gives parity with measurable coverage |

## 5. Phases & Milestones (dependency order)

| Phase | Doc | Scope | Est. effort |
|---|---|---|---|
| M0 | `02-workspace-setup.md` | Workspace, CI, diff harness, feature inventory tooling | 1 wk |
| M1 | `03-core-frontend.md` | source, tokens (99 bilingual keywords), lexer, AST (61 nodes), Pratt parser | 2–3 wk |
| M2 | `04-semantic-analyzer.md` | 14 types, symbols, analyzer, 100+ semantic error codes | 2 wk |
| M3 | `05-interpreter-core.md` | Value model, env, exceptions, control flow, fn/closures, pattern match, pipe, imports | 3–4 wk |
| M4 | `06-stdlib.md` | 378 builtins across ~25 modules + Chinese aliases | 4–6 wk |
| M5 | `07-llm-runtime.md` | LLMRuntime, provider protocols, HTTP streaming, prompt builder, model caps | 2–3 wk |
| M6 | `08-agent-runtime.md` | agent exec, `llm act/if`, tools registry (11 tools), skills system, tool-calling loop | 2–3 wk |
| M7 | `09-concurrency.md` | `spawn` + Channel, `shared let`/`shared store`, `mailbox_select`, ReadOnlyView | 2 wk |
| M8 | `10-context-management.md` | TranscriptStore, history, 5-layer compression, working memory, observability, session mgmt | 3 wk |
| M9 | `11-mcp.md` | MCP client, registry, server manager | 1–2 wk |
| M10 | `12-python-ffi.md` | Helen → Python (PyO3 embedded CPython) | 2 wk |
| M11 | `13-python-bridge.md` | Python → Helen (PyO3 cdylib + import hook) | 2 wk |
| M12 | `14-cli-lsp.md` | `helen` CLI, REPL, check/test, docgen, formatter, LSP | 2–3 wk |
| M13 | `15-conformance-testing.md` | Full differential run, benchmarks, coverage report | 3 wk |
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

1. All 203 Python test modules mapped to Rust unit tests **or** differential coverage: every `.helen` program in the corpus produces **byte-identical stdout** and matching exit codes / error class names on both interpreters.
2. `helen check`, `helen <file>`, `helen test`, REPL, and LSP feature-complete (feature matrix in `14-cli-lsp.md`).
3. Python FFI: `import "numpy" as np` and `requests` examples from `examples/python_bridge` run unmodified.
4. Python Bridge: `from translator import TranslatorAgent` works via import hook; sync + async + keyword-arg calls pass.
5. `helen` on PyPI-equivalent install (`cargo install --path crates/helen-cli`) and `pip install helen-rust-bridge` both work.
6. Benchmark suite (ported from `tests/performance/test_benchmarks.py`) shows parity or better vs. Python.

## 8. Risks & Open Questions

| Risk | Mitigation |
|---|---|
| Python `str` semantics (code points, slicing, `%` formatting) | D4 + dedicated conformance corpus; early spike in M3 |
| Arbitrary-precision ints | D3; corpus check in M13 |
| Custom Python LLM providers in pure Rust | D8; FFI fallback + docs |
| 378 stdlib functions — subtle behavioral differences | Differential corpus per stdlib module; port Python unit-test inputs as data tables |
| `spawn` thread semantics & channel fairness | Port `tests/interpreter/test_spawn*`; stress tests |
| PyO3 + Python 3.12 ABI | Pin pyo3 0.23+, test in CI with python3.12 headers |
| Scope-isolation rules (v1.10/1.12) interplay with closures | Port `tests/semantic` + `tests/interpreter` isolation cases first |

**Open questions to resolve before M3:**
1. Does Helen expose floor division `//` and modulo semantics on negatives? (Check `tests/execution`; port behavior.)
2. Does `/` return float or int-when-divisible? (Python behavior likely; confirm in corpus.)
3. Is `int` overflow possible in the corpus, or is i64 safe? (Decide D3 final.)

---

**Navigation:** continue to `01-feature-inventory.md` (the source-of-truth feature checklist), then phase docs `02`…`16`.
