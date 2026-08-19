# Memory — Helen Rust port (M1–M8)

## Porting playbook (source of truth)
Full M1–M7 lessons: `wiki/rust/migration-notes.md`

Top recurring gotchas (details in wiki):
- Diff harness: `--run` + `--mock-llm` (reference-only); lex JSON key-order is cosmetic; broken corpus fixtures (spawn_expr.helen, shared_store.helen) verify error parity only.
- Rust pitfalls: UFCS self-recursion (use free helper fn); `json!` can't be const; ureq 2.x `into_reader()` consumes; `Arc<dyn LlmRuntime>` → trait methods must be `&self`; spawn thread needs `unsafe impl Send` + `#[allow(arc_with_non_send_sync)]`; `snapshot()` uses `clone_owned` (fresh Rc), not `clone_deep`.
- Parser/AST quirks: tools list = `\x1f`-separated string (`\x1e` items); agent fn return type uses `:` not `->`; top-level `shared store` was a no-op (fixed via `register_shared_store`); each new `Value` variant touches ~10 exhaustive match sites.
- Behavior parity: `sleep` is in `std.time`; channel close pushes sentinel then receive→None; spawn injects channel endpoint as LAST param (bind user args positionally to non-Channel params); ReadOnlyView has NO `__getattr__` delegation.

## Per-milestone gate checklist
1. `cargo test --workspace` → 2. `cargo build --release` → 3. `cargo clippy --workspace` (0 warnings) → 4. `bash scripts/diff-semantic.sh` → 5. run-diff vs Python ref (`--mock-llm`) → 6. parse/lex diff (key-sorted) → 7. conformance pytest → 8. commit+push.

## Conventions
- Commit style: `Mn: <summary> — ...`, push to origin/main.
- `.helen/` is gitignored (runtime session store) → shared durable docs go in `wiki/`.
- Reports and analysis docs go in `reports/` directory.

## Codebase analysis (2026-08-19)
Full report: `reports/CODEBASE_ANALYSIS_REPORT.md`

### Cleanup completed
- Removed 6 dead `impl_*` session functions from stdlib.rs (~107 lines) — orphaned duplicates of transcript.rs implementations
- Fixed 4 deprecated `base64::encode/decode` calls in media.rs → use `base64::engine::general_purpose::STANDARD`
- Added `has_llm_client: bool` field to `ReactiveCompactor` — semantic compression now configurable (was hardcoded `false`)
- Removed duplicate `set_session_dir` entry from TRANSCRIPT_EXPORTS
- Removed dead `make_str_map` helper (only used by removed function)

### Session functions architecture
- Public API: `std.transcript.*` functions (7 total)
- Implementation: all in `crates/helen-interpreter/src/transcript.rs`
- Registration: via `TRANSCRIPT_EXPORTS` in stdlib.rs
- The removed `impl_*` functions were never registered — they were migration leftovers

### Remaining issues
1. **LLM recording trait defaults** (llm.rs:79-84): `enable_recording/disable_recording` return errors by default; `HttpLLMRuntime` should override
2. **Quality dimension scoring** (cli_commands.rs:220): `helen quality --dimension <name>` falls back to aggregate score
3. **Compiler warnings** (6): output filename collision, unused doc comments (2), unused variable, unnecessary mut

### Key insight
stdlib.rs `impl_*` functions are NOT automatically public API — only functions registered in `*_EXPORTS` tables are exposed to Helen users. Dead `impl_*` functions can be safely removed if not in any EXPORTS table.
