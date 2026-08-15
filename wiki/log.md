# 项目日志 — helen-rust

> helen-rust 从 M0 到 M14 的演进日志。每次提交遵循 `Mn: <summary>` 格式。
> 38 次提交，从 2026 年初始计划文档到 M14 验收交付。

---

## 2026 — 初始规划（docs commits）

- `3e2186b` docs: add master implementation plan for Helen language Rust rewrite
- `39351ba` docs: drop code-point indexed strings (D4); name cargo+pip packages helen-rust
- `4c89c5a` docs: incorporate verified corrections (C1-C18) and resolved decisions into plan

## M0: 工作区与差分测试框架

- `742a480` M0: workspace, CI, and differential conformance harness

## M1: 词法 + 语法前端

- `364b0df` M1 (1.1-1.2): Python-faithful tokens + lexer with differential verification
- `9d394c1` M1 (1.3-1.5): AST, AstPrinter, Pratt parser — full frontend

## M2: 语义分析

- `4e6a6f6` M2: semantic analyzer port (types, symbols, analyzer, diagnostics)

## M3: 解释器核心

- `b4c36a8` M3 (3.1-3.3): value model, environment, exceptions — interpreter foundation
- `213d826` M3 (3.4-3.5): interpreter core — statements/expressions, closures, builtins, CLI --run
- `1dbae82` M3 (3.7): stdlib module imports — str/list/dict/math/debug registries, 3 import forms, 7 tests
- `40a9109` M3 (3.6): LLM runtime interface + MockLlmRuntime — llm act/if routing
- `debd2ff` M3 (3.7b): .helen file import resolver — path safety, circular detection, module objects
- `b1d8b74` M3 (3.6b): llm act streaming callbacks — on_chunk/on_complete dispatch

## M4: 标准库

- `50f2fc2` M4: stdlib registry — 22 modules, 378 builtins + aliases; tuple value; JSON/CSV python-format parity

## M5: LLM 运行时

- `9fd6b69` M5: LLM runtime crate — providers, HTTP+SSE client, config, prompt builder, model caps, token counting, tool dispatch

## M6: Agent 运行时

- `3a4ed08` M6: agent runtime — 11-tool registry (byte-identical schemas), fuzzy_match 9-strategy port, skills system, llm act tool loop + agent scope isolation

## M7: 并发

- `c776a7c` M7: concurrency — Channel, spawn OS-thread execution, SharedStore runtime, ReadOnlyView guard, mailbox_select

## M8: 上下文管理

- `cc15cbf` M8: context management — transcript/session/history/compression/memory/observability + SQLite backend

## M9: MCP

- `d3a3f27` M9: MCP client — JSON-RPC over stdio, server manager + registry, MCPError hierarchy, fixture mock server

## M10: Python FFI

- `3212bb0` M10: Python FFI — Value::Native, helen-ffi crate (pyo3), import-hook fallback, custom-provider loader

## M11: Python Bridge

- `aaccda1` M11: Python Bridge — maturin cdylib `helen_rust`, PyAgent/PyFunction wrappers, pure-Python shim, 13 pytest DoD tests

## M12: CLI / REPL / LSP

- `45af0f8` M12: CLI, REPL, Formatter, Docgen, LSP — full cli/* + lsp/server.py port

## M13: 一致性测试与基准

- `ac2f6cf` M13: Tier A differential harness green (42/42) — --run --mock-llm CLI parity
- `a6565e5` M13: Tier B subprocess adoption — language 100/100, agent 170/172, cli+ffi 128+1skip
- `0962a5c` M13: Tier C differential test generator — lexer 67/67, parser 49/49, semantic 21/21
- `bae71d2` M13: Execution Tier C (48 tests) + 3 interpreter parity fixes
- `4513501` M13: display corpus 10/10, env panic fix, bench.sh, gen-error-diff.py 70/70, proptest fuzz, coverage drivers

## M14: 打包、文档、验收

- `766e2ca` M14: packaging + docs + migration tooling — install.sh, check-parity.sh, sync-corpus.sh, release.yml, LICENSE, docs/ docgen parity, transcript JSONL interop tests
- `1d9a7cd` M14: handover — STATUS.md (DoD D1-D8 evidence, coverage 68.82%), coverage gate fix
- `029349c` docs(M14): maintenance guide (M0-M14 retrospective + runbook)

## 后续 (this wiki)

- 镜像 `~/helen/wiki` 结构：语言级文档 (guide/, reference/, overview/) + 实现级文档 (compiler/, interpreter/, appendix/)
- 内置 skills 镜像：`crates/helen-runtime/skills/`（20 个技能，含 Rust 适配说明）
