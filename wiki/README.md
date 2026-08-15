# Helen Rust Wiki Index

> **helen-rust** — A complete reimplementation of the Helen Agent Programming
> Language in Rust.
> Version: 1.45.0 | Status: feature-parity with the Python reference
> (differential-tested) | Install: `cargo install helen-rust` or
> `pip install helen-rust` | Tests: 685+ Rust tests + 3,860 pytest spec
> adopted across Tier A/B/C conformance

---

## 📖 Quick Navigation

### 1. Implementation Plan (M0–M14)
- [[plan/README|Master Implementation Plan]] — goals, source facts, non-goals, workspace layout, design decisions D1–D12
- [[plan/15-conformance-testing|M13 Conformance & Benchmarks]] — Tier A/B/C differential adoption, error parity, fuzz, coverage
- [[plan/16-release|M14 Packaging, Documentation, Acceptance]] — release artifacts, DoD checklist D1–D8

### 2. Rust Architecture
- [[rust/architecture|Architecture]] — crate layout, value model, threading model, design decisions D1–D12
- [[rust/migration-notes|Migration Notes]] — intentional deviations and reference quirks (byte-based strings D4, string iteration unsupported, spawn race strictness, custom-provider Python dependency, context/compression quirks)

### 3. Language Reference (from the Python reference implementation)
> The Rust port targets the same language; the authoritative language docs
> live in the reference wiki at `~/helen/wiki/`:
- [[syntax/lexical|Lexical Analysis]] — 88 token types, maximal munch, triple-quoted strings, CJK
- [[syntax/grammar|Grammar Specification]] — full EBNF, Pratt parsing, 10 precedence levels
- [[syntax/keywords|Keyword Reference]] — 99 keywords (48 English + 51 Chinese)
- [[compiler/semantic|Semantic Analysis]] — symbol tables, scoping, 14-type system
- [[interpreter/execution|Execution Engine]] — AST traversal, environment scope chain
- [[interpreter/llm-integration|LLM Integration]] — `llm act/if`, streaming callbacks
- [[interpreter/spawn|Concurrency and spawn]] — `spawn`, Channel, mailbox_select
- [[runtime/transcript-store|TranscriptStore SSOT]] — SQLite/JSONL backends, LRU cache, UUID addressing
- [[runtime/context-management|Context Management]] — four-layer lifecycle, compression
- [[toolchain/cli|Command-Line Tools]] — `helen <file>/check/test/repl/doc/provider/lsp`
- [[toolchain/stdlib|Standard Library]] — 378 builtins across 22 modules

### 4. Guides
- [[guide/README|Beginner Guide]] — agent-first, 11 chapters (reference wiki)
- [[guide/01-hello-agent|Chapter 1: Your First Agent]] — agents, prompts, LLM statements, tools, stdlib, testing

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
wiki/             # this wiki: plan/, rust/
```
