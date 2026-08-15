# Helen Rust Wiki Index

> **helen-rust** — A complete reimplementation of the Helen Agent Programming
> Language in Rust.
> Version: 1.45.0 | Status: feature-parity with the Python reference
> (differential-tested) | Install: `cargo install helen-rust` or
> `pip install helen-rust` | Tests: 685+ Rust tests + 3,860 pytest spec
> adopted across Tier A/B/C conformance

This wiki is the **independent documentation set for helen-rust**. It mirrors
the reference wiki structure (`~/helen/wiki/`): language-level docs
(guide/, reference/, overview/) describe the shared Helen language; the
implementation docs (compiler/, interpreter/, appendix/) describe the Rust
implementation specifically.

---

## 📖 Quick Navigation

### 1. Language Overview
- [[index|Wiki Index]] — full table of contents, status, install, repo layout
- [[overview/design-philosophy|Design Philosophy]] — why an Agent programming language
- [[overview/language-spec|Language Specification]] — 99 keywords, tokens, AST
- [[overview/architecture|Overall Architecture]] — helen-rust crate layout, value model, threading

### 2. Frontend Compilation
- [[compiler/ast|AST Node Definitions]] — Rust AST, Visitor, TypeRefKind fidelity
- [[compiler/semantic|Semantic Analysis]] — symbol tables, scoping, type checking
- [[compiler/types|Type System]] — 14 types, gradual typing

### 3. Interpretive Execution
- [[interpreter/execution|Execution Engine]] — value model, environment, closures, builtins
- [[interpreter/llm-integration|LLM Integration]] — `llm act/if`, streaming
- [[interpreter/spawn|Concurrency and spawn]] — `spawn`, Channel, mailbox_select, SharedStore

### 4. Implementation Plan & Porting
- [[plan/README|Master Implementation Plan]] — goals, non-goals, workspace layout, design decisions D1–D12
- [[rust/architecture|Rust Architecture]] — crate layout, value model, threading model, D1–D12
- [[rust/migration-notes|Migration Notes]] — intentional deviations and reference quirks
- [[MAINTENANCE|Maintenance Guide]] — operating manual, verification gates, runbook (read this first)
- [[plan/15-conformance-testing|M13 Conformance & Benchmarks]] — Tier A/B/C differential adoption
- [[plan/16-release|M14 Packaging, Documentation, Acceptance]] — release artifacts, DoD D1–D8
- [[plan/STATUS|Project Status & Handover]] — coverage, differential results, open issues

### 5. Beginner Guide (Agent-First)
- [[guide/README|Guide Overview]] — what is Helen, who is this for
- [[guide/01-hello-agent|Chapter 1: Your First Agent]] — agents, prompts, LLM statements
- [[guide/02-prompt|Chapter 2: Prompts]] — template variables, Ground Truth Injection
- [[guide/03-llm-statements|Chapter 3: Talking to LLMs]] — `llm act`, `llm if`
- [[guide/04-tools|Chapter 4: Tools]], [[guide/05-basics|Chapter 5: Basics]], [[guide/06-control-flow|Chapter 6: Control Flow]]
- [[guide/07-functions|Chapter 7: Functions]], [[guide/08-collaboration|Chapter 8: Collaboration]], [[guide/09-stdlib|Chapter 9: Stdlib]]
- [[guide/10-testing|Chapter 10: Testing]], [[guide/11-advanced|Chapter 11: Advanced]], [[guide/appendix|Appendix]]

### 6. Language Reference (By Topic)
- [[reference/01-getting-started|Getting Started]], [[reference/02-variables-and-types|Variables and Types]], [[reference/03-functions|Functions]]
- [[reference/04-control-flow|Control Flow]], [[reference/05-agents|Agent Programming]], [[reference/06-llm-statements|LLM Statements]]
- [[reference/07-spawn|Concurrent Programming]], [[reference/08-modules|Modules and Imports]], [[reference/09-python-ffi|Python FFI]]
- [[reference/10-stdlib|Standard Library]], [[reference/11-building-agents|Multi-Agent Systems]], [[reference/12-testing|Testing]]
- [[reference/13-skills|Skills]], [[reference/14-observability|Observability]], [[reference/15-python-bridge|Python Bridge]]
- [[reference/16-quality-assessment|Quality Assessment]], [[reference/17-multimodal|Multimodal]], [[reference/18-helen-agent|Helen Agent]]

### 7. Appendix
- [[appendix/error-codes|Error Codes]] — E0300–E0357
- [[appendix/exceptions|Exception Hierarchy]] — ExceptionValue
- [[appendix/changelog|Changelog]] — M0–M14 version history
- [[appendix/hld-compliance|HLD Compliance]] — conformance gates
- [[log|Project Log]] — commit history M0–M14

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
crates/helen-runtime/skills/  # bundled built-in skills (17 skills, mirror of reference + differential-porting)
tests/            # conformance harness, corpora, goldens, error-diff, fixtures
scripts/          # diff harnesses, bench, installer, corpus sync, parity sweep
wiki/             # this wiki: language docs + implementation docs
```
