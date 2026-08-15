# Helen Rust Wiki Index

> **helen-rust** — A complete reimplementation of the Helen Agent Programming
> Language in Rust.
> Version: 1.45.0 | Status: feature-parity with the Python reference
> (differential-tested) | Install: `cargo install helen-rust` or
> `pip install helen-rust` | Tests: 685+ Rust tests + 3,860 pytest spec
> adopted across Tier A/B/C conformance

---

## 📖 Quick Navigation

### 1. Language Overview
- [[overview/design-philosophy|Design Philosophy]] — Why we need an Agent programming language (shared with the reference)
- [[overview/language-spec|Language Specification]] — 99 keywords (48 English + 51 Chinese), Tokens, AST nodes at a glance (shared with the reference)
- [[overview/architecture|Overall Architecture]] — helen-rust crate layout, value model, threading model

### 2. Frontend Compilation
- [[syntax/lexical|Lexical Analysis]] — 88 token types, maximal munch, CJK (rust: helen-core lexer, 36/36 diff)
- [[syntax/grammar|Grammar Specification]] — full EBNF, Pratt parsing, 10 precedence levels (rust: helen-parser, 47/47 diff)
- [[syntax/keywords|Keyword Reference]] — 99 keywords (48 English + 51 Chinese)
- [[compiler/ast|AST Node Definitions]] — Rust AST (`helen-core::ast`), Visitor, TypeRefKind fidelity
- [[compiler/semantic|Semantic Analysis]] — `helen-semantic` analyzer, symbol tables, scoping, type checking
- [[compiler/types|Type System]] — 14 types, gradual type checking (`helen-semantic::types`)

### 3. Interpretive Execution
- [[interpreter/execution|Execution Engine]] — `helen-interpreter`, Environment scope chain, closures
- [[interpreter/llm-integration|LLM Integration]] — `llm act/if`, conversation history, streaming
- [[interpreter/spawn|Concurrency and spawn]] — `spawn`, Channel message queue, mailbox_select, SharedStore

### 4. Runtime Systems
- [[../rust/architecture|Rust Architecture]] — crate layout, design decisions D1–D12
- [[../rust/migration-notes|Migration Notes]] — intentional deviations and reference quirks
- [[../MAINTENANCE|Maintenance Guide]] — operating manual, verification gates, runbook (read this first)

### 5. Toolchain
- [[reference/01-getting-started|Getting Started]] — installation, configuration, Hello World, REPL
- [[reference/12-testing|Testing Framework]] — TDD support, assertion API
- [[reference/16-quality-assessment|Quality Assessment]] — 7-dimension framework, security scoring
- [[reference/15-python-bridge|Python Bridge]] — Python → Helen (`helen_rust` wheel)

### 6. Beginner Guide (Agent-First)
- [[guide/README|Guide Overview]] — What is Helen, who is this for, skill-driven development
- [[guide/01-hello-agent|Chapter 1: Your First Agent]] — agents, prompts, LLM statements, tools, stdlib, testing

### 7. Language Reference (By Topic)
- [[reference/02-variables-and-types|Variables and Types]], [[reference/03-functions|Functions]], [[reference/04-control-flow|Control Flow]]
- [[reference/05-agents|Agent Programming]], [[reference/06-llm-statements|LLM Statements]], [[reference/07-spawn|Concurrent Programming]]
- [[reference/08-modules|Modules and Imports]], [[reference/09-python-ffi|Python FFI]], [[reference/10-stdlib|Standard Library]]
- [[reference/11-building-agents|Building Multi-Agent Systems]], [[reference/13-skills|Skills]], [[reference/14-observability|Observability]]
- [[reference/17-multimodal|Multimodal]], [[reference/18-helen-agent|Helen Agent]]

### 8. Appendix
- [[appendix/error-codes|Error Codes]] — E0300–E0357 (helen-rust `ErrorCode` enum)
- [[appendix/exceptions|Exception Hierarchy]] — helen-rust `ExceptionValue`
- [[appendix/changelog|Changelog]] — helen-rust M0–M14 version history
- [[appendix/hld-compliance|HLD Compliance]] — conformance with the Helen Language Design spec

### 9. Implementation Plan (M0–M14)
- [[plan/README|Master Implementation Plan]] — goals, source facts, non-goals, workspace layout, design decisions D1–D12
- [[plan/15-conformance-testing|M13 Conformance & Benchmarks]] — Tier A/B/C differential adoption, error parity, fuzz, coverage
- [[plan/16-release|M14 Packaging, Documentation, Acceptance]] — release artifacts, DoD checklist D1–D8

---

## Status

| Area | Status |
|------|--------|
| Language core (lex/parse/ast/semantic) | ✅ parity (Tier C 137 tests + fuzz) |
| Interpreter + stdlib | ✅ parity (Tier A/B/C; execution 48 + corpus 70) |
| Agent runtime, LLM runtime, tools, MCP | ✅ parity (Tier B agent 170/172) |
| CLI / REPL / check / test / doc / LSP | ✅ M12 complete |
| Python FFI (Helen → Python) | ✅ M10 (examples run unmodified) |
| Python Bridge (Python → Helen) | ✅ M11 (13 DoD tests; `helen_rust` wheel) |
| Display parity | ✅ 10/10 byte-identical corpus |
| Benchmarks | ✅ Rust 15–50× faster (no >2x regression) |
| Coverage | ⚠️ 68.82% overall (drivers committed; stdlib happy paths remaining) |
| Release | ✅ M14: `cargo install` + wheel + install.sh + CI |

## Install

```bash
# CLI binary (Rust)
cargo install helen-rust          # or: bash scripts/install.sh

# Python bridge (optional)
pip install helen-rust
python3 -c "import helen_rust; from translator import TranslatorAgent"
```

## Repository Layout

```
crates/           # 10-crate workspace (core → parser → semantic → interpreter → stdlib → runtime → CLI/LSP/FFI/bridge)
tests/            # conformance harness, corpora, goldens, error-diff, fixtures
scripts/          # diff harnesses, bench, installer, corpus sync, parity sweep
wiki/             # this wiki: language docs (shared) + implementation docs (rust)
```

## Document Sources

- **Language-level docs** (guide/, reference/, overview/design-philosophy, overview/language-spec)
  are shared with the reference implementation's wiki at `~/helen/wiki/` and describe the
  *same* language. The Rust port implements this language byte-faithfully (differential-verified).
- **Implementation docs** (overview/architecture, compiler/, interpreter/, appendix/) describe
  the helen-rust implementation specifically.
ort helen_rust; from translator import TranslatorAgent"
```

## Repository Layout

```
crates/           # 10-crate workspace (core → parser → semantic → interpreter → stdlib → runtime → CLI/LSP/FFI/bridge)
tests/            # conformance harness, corpora, goldens, error-diff, fixtures
scripts/          # diff harnesses, bench, installer, corpus sync, parity sweep
wiki/             # this wiki: language docs (shared) + implementation docs (rust)
```

## Document Sources

- **Language-level docs** (guide/, reference/, overview/design-philosophy, overview/language-spec)
  are shared with the reference implementation's wiki at `~/helen/wiki/` and describe the
  *same* language. The Rust port implements this language byte-faithfully (differential-verified).
- **Implementation docs** (overview/architecture, compiler/, interpreter/, appendix/) describe
  the helen-rust implementation specifically.
