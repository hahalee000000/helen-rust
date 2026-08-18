---
file: ./.helen/skills/software-development/differential-porting/SKILL.md
---
name: differential-porting
description: "Differential conformance porting — reimplement a Python reference implementation in another language (e.g. Rust) byte-faithfully, using differential testing as the primary gate. Covers reference-first methodology, run-mode differential harnesses, Rust porting pitfalls (UFCS recursion, Arc<dyn Trait> &self, ureq into_reader, Send/Sync), parser/AST fidelity traps, behavioral parity gotchas, reference-quirk matching (unreachable break conditions, per-module constant drift, plain-string vs wrapper formats), MCP-client process-global state traps, pyo3/FFI traps (Bound API, PyTuple args, eval-subclass, class-vs-instance), maturin wheel packaging, CLI/LSP port gotchas, and the per-milestone gate checklist. Distilled from M1-M12 of the helen-rust project."
version: 1.2.0
author: Helen Team
license: MIT

