# Memory — Helen Rust Port

## Project Overview
- **Goal**: Complete Rust reimplementation of [Helen language](https://github.com/hahalee000000/helen) (v1.44→v1.45.2)
- **Scope**: Feature-parity incl. Python FFI (Helen→Python) + Python Bridge (Python→Helen)
- **Verification**: Differential conformance vs Python reference (3,860 pytest tests)
- **Python reference**: `~/helen/`
- **Language policy**: All code, comments, commits, docs in **English**

## Workspace (11 crates, ~85K lines Rust)
```
crates/
  helen-core/        # tokens, lexer, AST, errors
  helen-parser/      # Pratt parser
  helen-semantic/    # types, symbols, analyzer (2322-line analyzer.rs)
  helen-interpreter/ # values, env, tree-walk exec, builtins (1962-line)
  helen-stdlib/      # 378+ builtins (374 Chinese aliases, complete parity)
  helen-runtime/     # LLM runtime, agents, tools, skills, MCP, channels
  helen-rust/        # CLI binary `helen` (REPL, docgen, check, test)
  helen-lsp/         # Language Server Protocol
  helen-ffi/         # Python FFI via PyO3 (feature-gated)
  helen-python-bridge/ # Python→Helen via maturin cdylib
  helen-agent/       # WebUI server (Axum + React SPA, rust-embed) — M17+
```

## Milestones
M0–M24 complete: core→runtime, FFI+Bridge, CLI/REPL/LSP, conformance, Chinese aliases, helen-agent (WebUI), quality improvements. Details: `wiki/rust/`

## Test & Quality
- **~1824 tests passed**, 0 failed (workspace excl. bridge)
- Tier A: 52/52 byte-identical | Tier B: 100% pass rate
- Coverage: 68.82% | Quality: B+ (6.93/10) | Clippy: 0 warnings

## Key Scripts & Dirs
- Scripts: `scripts/check-parity.sh`, `diff-tier-b.sh`, `bench.sh`
- Docs: `wiki/rust/` (architecture), `docs/` (stdlib.md, language.md)
- Tests: `tests/conformance/`, `tests/programs/`

## Porting Gotchas
Details: `wiki/rust/migration-notes.md`. Key points:
- Rust: UFCS recursion→free fn, `Arc<dyn LlmRuntime>`→`&self`, spawn needs `unsafe impl Send`
- Parser: tools=`\x1f`-sep, agent fn return `:` not `->`
- Strings: UTF-8 byte-based (len=bytes), iteration unsupported

## Gate Checklist
`cargo test` → `cargo build --release` → `cargo clippy` (0 warn) → `diff-semantic.sh` → run-diff → parse/lex diff → conformance pytest → commit+push

## Conventions
- Commits: `Mn: <summary> — ...`, push origin/main
- `.helen/` partially gitignored; durable docs in `wiki/`, reports in `reports/`
- Style: `rustfmt` + `cargo clippy -- -D warnings`

## Known Gaps
1. ~~Coverage 68.82% vs 70%~~ — ✅ COMPLETE: +77 tests (1598→1675)
2. ~~`export_transcript` stub needs session wiring~~ — FIXED
3. ~~helen-ffi tests need libpython3.12.so~~ — FIXED: `.cargo/config.toml` rpath + test fix
4. `cargo publish` / PyPI not executed (needs credentials)
5. 2 Python-internal Tier B tests excluded (reference-side bugs)

## Mandatory Rules
1. **TDD workflow required** — All development and maintenance MUST follow TDD (test-first). New test files must be included in regression testing. After any code change, run full regression tests (including page/frontend code) to verify correctness before considering the task done.
2. **NEVER auto-exit or kill helen agent** — Do NOT automatically exit the helen agent session, and NEVER kill the helen agent process. If exit/kill is needed, notify the user to perform it manually.

## TDD Development Workflow

### Rust Code (Backend)
1. **RED** — Write failing test in `crates/<crate>/tests/` or inline `#[cfg(test)]`
2. **GREEN** — Implement minimal code to pass test
3. **REFACTOR** — Clean up while keeping tests green
4. **VERIFY** — Run `cargo test` (workspace-wide) + `cargo clippy -- -D warnings`

### Frontend Code (React/TypeScript)
1. **RED** — Write failing test in `crates/helen-agent/frontend/src/**/*.test.{ts,tsx}`
2. **GREEN** — Implement fix/feature
3. **REFACTOR** — Clean up while keeping tests green
4. **VERIFY** — Run `cd crates/helen-agent/frontend && npm test -- --run`

### Bug Fix Protocol
- **ALWAYS write regression test FIRST** that reproduces the bug
- Test should fail before fix (RED), pass after fix (GREEN)
- Include test in regression suite permanently
- Examples: `chatStore.test.ts` (M25.20), `transcript_tests.rs` (M25.20)

### Test Commands
```bash
# Rust workspace tests
cargo test

# Frontend tests
cd crates/helen-agent/frontend && npm test -- --run

# Full regression (after significant changes)
cargo test && cd crates/helen-agent/frontend && npm test -- --run
```

### Test File Naming
- Rust: `<module>_tests.rs` or inline `#[cfg(test)] mod tests`
- Frontend: `<Component>.test.tsx` or `<module>.test.ts`

## Tool Usage
- Use codebase-memory-mcp-helen for code search/graph queries
- `save_code_file`/`patch_code_file` for .helen (auto helen check)
- Load skills before coding: `helen-syntax`, `helen-stdlib`, `helen-testing`
- Load `debugging` skill before investigating bugs
