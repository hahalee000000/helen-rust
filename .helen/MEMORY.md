# Memory — Helen Rust Port

## Project Overview
- **Goal**: Complete Rust reimplementation of [Helen language](https://github.com/hahalee000000/helen) (v1.44→v1.45.2)
- **Scope**: Feature-parity incl. Python FFI (Helen→Python) + Python Bridge (Python→Helen)
- **Verification**: Differential conformance vs Python reference (3,860 pytest tests)
- **Python reference**: `~/helen/`
- **Language policy**: All code, comments, commits, docs in **English**

## Workspace (10 crates, ~70K lines Rust)
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
```

## Milestones (M0–M15 complete)
M0–M8: core→runtime. M9: quality (unwrap 325→140, stdlib split). M10–M11: FFI+Bridge. M12–M14: CLI/REPL/LSP, conformance, release. M15: Chinese aliases, input(), clippy.

## Test & Quality
| Metric | Value |
|---|---|
| Workspace tests (excl. bridge) | **1628 passed, 0 failed** |
| Tier A differential | 52/52 byte-identical |
| Tier B pytest | lang 100/100, cli 64/64, ffi 64+1skip, agent 170/172 |
| Error parity | 70/70 E-code + exit-code match |
| Benchmarks | Rust 15–50× faster |
| Coverage | 68.82% (target 70%) |
| Quality | B+ (6.93/10) |
| Clippy | 0 warnings |

## Key Scripts
- `scripts/check-parity.sh` — full sweep (--fast/--quick/full)
- `scripts/sync-corpus.sh` — pull corpus from HELEN_SRC
- `scripts/diff-semantic.sh` / `diff-tier-a.sh` / `diff-tier-b.sh`
- `scripts/bench.sh` / `scripts/install.sh`

## Key Dirs
- `wiki/plan/` — 17-doc implementation plan (M0–M14)
- `wiki/rust/` — architecture, migration notes
- `reports/` — quality assessments, stdlib parity
- `tests/conformance/` — golden files, fixtures
- `tests/programs/` — authored(29), display(10), stdlib(7), pytest/
- `docs/` — generated stdlib.md, language.md (byte-identical to ref)

## Porting Gotchas (details: `wiki/rust/migration-notes.md`)
- Diff harness: `--run` + `--mock-llm`; lex JSON key-order cosmetic
- Rust: UFCS self-recursion→free fn; `json!` not const; ureq `into_reader()` consumes; `Arc<dyn LlmRuntime>`→`&self`; spawn needs `unsafe impl Send`; `snapshot()`=clone_owned not clone_deep
- Parser: tools=`\x1f`-sep string; agent fn return `:` not `->`; new Value variant→~10 match sites
- Behavior: `sleep` in std.time; channel close→sentinel→None; spawn injects channel LAST; ReadOnlyView NO `__getattr__`
- Strings: UTF-8 byte-based (len=bytes); iteration unsupported
- Integers: num-bigint (Python parity)

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

## Tool Usage
- Use codebase-memory-mcp-helen for code search/graph queries
- `save_code_file`/`patch_code_file` for .helen (auto helen check)
- Load skills before coding: `helen-syntax`, `helen-stdlib`, `helen-testing`
- Load `debugging` skill before investigating bugs
en-testing`
- Load `debugging` skill before investigating bugs
esting`
- Load `debugging` skill before investigating bugs
