# helen-rust

A complete reimplementation of the [Helen programming language](https://github.com/hahalee000000/helen) in Rust.

**Goal:** Feature-parity with Helen v1.44.0 — including the Python FFI (Helen → Python) and the Python Bridge (Python → Helen) — verified by differential conformance testing against the reference implementation.

## Project Conventions

- **All code and documentation in this repository are written in English.**
- The only non-English text allowed is Helen source-level identifiers that are part of the language itself (e.g. Chinese keyword aliases like `智能体`/`agent`, `分生`/`spawn`), and stdlib locale data (`stdlib/locales/zh.py` equivalents).
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
  helen-cli/        # helen binary, REPL, formatter, docgen
  helen-ffi/        # Python FFI (Helen → Python), PyO3, feature-gated
  helen-python-bridge/  # Python Bridge (Python → Helen), PyO3 cdylib
helen-test/         # conformance harness + golden tests
```

## Status

- [x] Master implementation plan (M0–M14) — `wiki/plan/`
- [ ] M0 Workspace setup & conformance harness
- [ ] M1–M14 Implementation phases

See `wiki/plan/README.md` for the roadmap and phase dependencies.
