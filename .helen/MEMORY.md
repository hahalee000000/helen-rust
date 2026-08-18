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
