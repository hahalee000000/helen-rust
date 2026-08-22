# helen-rust

A complete reimplementation of the [Helen programming language](https://github.com/hahalee000000/helen) in Rust.

**Goal:** Feature-parity with Helen v1.44.0 — including the Python FFI (Helen → Python) and the Python Bridge (Python → Helen) — verified by differential conformance testing against the reference implementation and its 3,860 pytest tests.

## Project Conventions

- **All code and documentation in this repository are written in English.**
- The only non-English text allowed is Helen source-level identifiers that are part of the language itself (e.g. Chinese keyword aliases like `智能体`/`agent`, `分生`/`spawn`), and stdlib locale data (`stdlib/locales/zh.py` equivalents).
- String semantics: **native UTF-8 byte-based strings** (`len()` = bytes; byte-offset index/slice). String **iteration is intentionally unsupported** (Helen raises at runtime; only lists are iterable). Deliberate, documented deviation from Python's code-point `str` for non-ASCII — see design decisions D4/D11 in `wiki/plan/README.md`.
- Integers are arbitrary-precision (`num-bigint`, D3) — matching Python's big-int behavior from day one.
- Code style follows `rustfmt` + `cargo clippy -- -D warnings`.

## Repository Layout

```
wiki/plan/          # Master implementation plan (17 docs, phases M0–M14)
```

Planned layout once implementation starts (per `wiki/plan/02-workspace-setup.md`):

```
crates/
  helen-core/       # tokens, lexer, AST, parser
  helen-semantic/   # type system, symbol tables, analyzer
  helen-interpreter/# value model, environment, tree-walk execution
  helen-stdlib/     # 378 builtin functions
  helen-runtime/    # LLM runtime, agents, tools, skills, MCP, context
  helen-rust/       # crates.io package `helen-rust`, binary `helen` (CLI, REPL, docgen)
  helen-ffi/        # Python FFI (Helen → Python), PyO3, feature-gated
  helen-python-bridge/  # Python Bridge (Python → Helen), PyO3 cdylib; PyPI dist `helen-rust`
helen-test/         # conformance harness + golden tests
```

## Status

- [x] Master implementation plan (M0–M14) — `wiki/plan/`
- [x] Plan review: C1–C18 source-verified corrections + resolved decisions (pytest adoption, in-process Mock LLM driver, num-bigint from M3, string-iteration unsupported, display parity) — 2026-08-15
- [x] M0–M14 Implementation phases — complete
- [x] Published to **crates.io** (9 crates, v0.1.0) — `cargo install helen-rust`
- [x] Published to **PyPI** (`helen-rust` v0.1.0) — `pip install helen-rust`

## Installation

```bash
# CLI binary (Rust)
cargo install helen-rust

# Python bridge (Python → Helen)
pip install helen-rust
```
See `wiki/plan/README.md` for the roadmap and phase dependencies.
