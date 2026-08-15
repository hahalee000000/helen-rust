# Rust migration notes — known divergences

## llm act streaming (Task 3.6, session committed 3.7b)
Python's `_visit_llm_act_streaming` is **broken in the reference version**:
`LLMRuntime.act_stream()`'s base signature lacks `max_tokens`/callback kwargs,
so every streaming call raises `TypeError`; the outer `except Exception`
then references `HelenRuntimeError` — a local imported only inside the
error-event branch — raising `UnboundLocalError` instead of the intended
`Streaming LLM call failed: ...` runtime error.

Rust implements the **intended** HLD semantics instead:
- `act_stream` default (wraps `act()`) yields one `content` event.
- `on_chunk(text)` called per event; literal `false` return interrupts
  (Python checks `is False` identity, not truthiness).
- `on_complete()` called with **no args**, only when not interrupted.
- Returns the joined text (`""` when empty — not Null).
The corpus fixture `llm_act_streaming.helen` never spawns its agent, so the
run differential (29/29) is unaffected either way.

---

# M8 — context management lessons (session/transcript/history/compression/memory/observability)

## Reference quirks to match, not "fix"

1. **`_truncate_compress` break condition is unreachable.** Walking newest-first
   with `MIN_RECENT_MESSAGES=5`, the `break` never fires — the reference keeps
   ALL messages. Verified empirically: identical input → Python returns all 25,
   so does the port. Match observable behavior, not intent.
2. **`estimate_list_tokens` is `sum(chars) // 4`** (one division at the end),
   NOT per-item division — my first port dropped 4x more tokens than Python.
   Verified empirically before fixing.
3. **Reactive semantic recovery requires `llm_client is not None`.** Without a
   client only the structural path fires. The `is not None` gate is part of the
   observable contract; Rust must model it with an `Option<LlmClient>`.
4. **Two different context-window defaults live in different modules**:
   `token_utils.py` defaults to 131072, `history.py` to 128000. Port each
   module's own constant — do not "unify" them.
5. **Summary message content is a plain string**, not `{"text": ...}` — the
   latter was my wrapper assumption. Byte-verified against Python.
6. **`session_exists` requires `transcript.jsonl`**, not just the session dir —
   `create_session()` alone does not make a session "exist".
7. **Deprecated `Message.to_dict()` omits agent fields** (role/content/uuid/
   ts/mtype/priority only); the non-deprecated `message_to_dict` includes them.
8. **`role` defaults to `"user"`** when the field is missing on load.

## Constants / parity infrastructure

- `scripts/check_constants_parity.sh` greps Python `constants.py` for
  `NAME: Final[...] = value` and diffs against Rust `constants.rs`; exits
  non-zero on drift. Runs in CI-style gate, not just once.
- A constant that "looks right" can still be wrong: my earlier
  `assign_priority("user") = 100` diverged from Python's **90**. Re-verify
  every constant against the reference when porting the next dependent module.

## SQLite backend (byte-compat)

- Python-exact schema, WAL mode, `JSON_EXTRACT` query pushdown.
- Provenance: Rust reads a DB **written by the Python reference**
  (`crates/helen-runtime/tests/fixtures/python_session.db`, generated with a
  `python3 -c` script checked into `tests/fixtures/`). Round-trip test asserts
  meta, boundary markers, pinned flags, and message rows survive.
- rusqlite needs `--features bundled` (vendored libsqlite3, no system dep).

## Session lifecycle

- Session ID format `session_{ts}_{salt}_{short_uuid}` = **44 chars**.
- `get_session_id()` **lazily auto-creates** a session when none exists
  (Python v1.29.14 parity) — tests written against the old eager behavior
  must be updated to expect the extra lazy session.
- `create_session` is **NOT a stdlib export** (internal to SessionManager).
  Session functions live in **`std.transcript`** (`TRANSCRIPT_EXPORTS`),
  NOT `std.core` — `import std.core.*` does not bring them in.

## Misc Rust facts

- `PathBuf` needs `.display()` inside `format!` (no Display for PathBuf).
- `now_iso_utc` parity: Python `datetime.utcnow().isoformat() + "Z"`.
- Multimodal token count: `text_tokens + 85/media + 4` overhead.
- New deps: `indexmap` (serde feature) for insertion-ordered Value maps;
  `rusqlite` bundled.
- Task 8.6 modules (9): `observability`, `diagnostics` (error-diagnostics),
  `validator` (output-validator), `coverage`, `recording` (cassette),
  `transcript_replay`, `data_lineage` (SQLite-backed), `context_awareness`,
  `context_recovery`.

---

# Porting playbook — hard-won lessons (M1–M7)

## Differential testing methodology

- The Rust CLI exposes **`--lex`, `--semantic-only`, `--run`** only. There is
  NO `--conformance` flag — `scripts/diff.sh` is stale (M0-era). Run-mode
  differentials must use `--run`.
- **`--mock-llm` is a reference-only flag.** `reference.py` accepts it; the Rust
  binary does not — passing it makes the Rust binary treat `--mock-llm` as the
  *file path* (rc=2). Correct sweep: Rust `--run <file>` (mock is built into
  the test harness), reference `reference.py <file>` (default mode = run).
- **Lex JSON key order differs cosmetically**: serde_json serializes Maps
  alphabetically (`col, end_col, ...`) while Python uses dict insertion order
  (`type, lexeme, ...`). Content is identical — compare key-sorted, or rely on
  the conformance pytest which already normalizes.
- **When in doubt about behavior, run the Python reference directly**
  (`python3 -c "..."`) instead of assuming. Example: fuzzy_match strategy chain
  order — my hand-written test expectations were wrong; the port was right.
  `line_trimmed` fires before `indentation_flexible`/`trimmed_boundary`.
- Some corpus fixtures are **broken placeholders** (e.g. `spawn_expr.helen`,
  `shared_store.helen` error identically in both Rust and Python). They verify
  parity of *errors*, not functionality — port the real behavior tests from
  `helen/tests/interpreter/test_spawn_*.py` by hand.

## Rust porting pitfalls

1. **UFCS recursion trap** — calling a trait method from a struct that has its
   own override dispatches to the *override* → infinite recursion
   (`MinimaxProtocol::parse_response`). Fix: extract a **free helper**
   (`default_parse_response()`) used by both the trait default and the override.
2. **`serde_json::json!` cannot be `const`.** Build schemas in plain
   `fn defs() -> Vec<Value>` (or `OnceLock`).
3. **ureq 2.x `Response`**: `resp.into_reader()` is *consuming* (drops the
   Response, returns an owned reader); `resp.body_mut().as_reader()` borrows.
   Use `into_reader()` when the Response must die before reading completes.
4. **`Arc<dyn LlmRuntime>` forces `&self` trait methods.** `&mut self` through
   a trait object requires nightly/unstable. Use interior mutability
   (`RefCell`/`Mutex`) in implementors; the Mock already uses `Rc<RefCell<...>>`.
5. **Send/Sync**: the interpreter is `Rc<RefCell<...>>`-heavy → not `Send`.
   For `spawn`, deep-own everything into a `SpawnPayload` + `unsafe impl Send`
   with documented single-owner discipline. Clippy's `arc_with_non_send_sync`
   needs an explicit `#[allow]` (it is intentional here).
6. **`clone_owned()` vs `clone_deep()`** — `Environment::snapshot()` must use
   `clone_owned()` (fresh `Rc<str>`, deep containers) so shared-let values stay
   shared; `clone_deep()` wrongly deep-copies everything.
7. **stdout must be `Arc<Mutex<String>>`** (was `RefCell`) for spawned threads
   to append output.

## Parser / AST quirks

- The `tools = [...]` list is serialized by the parser into
  **`\x1fname\x1ename\x1f`** (items joined with `\x1e`, wrapped in `\x1f`).
  Parse with `trim_matches('\u{1f}')` + `split('\u{1e}')`.
- Agent `functions {}` return type uses **colon** syntax:
  `fn add(a: int, b: int): int` — NOT `->`.
- `shared store` at **top level was a no-op** (stores only registered for
  imported `.helen` files). Factor a `register_shared_store()` helper used by
  both paths.
- Agent/LLM settings live in `AgentDecl.declarations`; read via
  `agent_setting(name)`.
- **New `Value` variants touch ~10 exhaustive-match sites in one pass**:
  `python_str`, `type_name`, `PartialEq`, `Hash`, `clone_owned`,
  `is_truthy`, `visit_access`, `visit_index`, `assign_access`, `visit_call`.

## Behavioral parity gotchas

- `sleep` lives in **`std.time`** (`TIME_EXPORTS`), NOT `std.core`.
- Channel **close pushes a sentinel** → a blocked receiver wakes and the next
  `receive()` returns `None`. Send-after-close is ignored.
- Spawn auto-injects the Channel endpoint: bind user positional args to
  **non-Channel params only** (declaration order) — binding by raw index
  consumed the endpoint and broke arg order.
- Agents get a **fresh env** — you must inject stdlib builtins + module consts
  (read-only) + shared lets (writable), otherwise `print` is undefined.
- `ReadOnlyView` has **no `__getattr__` delegation** in Python (only
  `__getitem__`/`__iter__`) — `.append` on a read-only param raises.
- `std.time` has `sleep`; polling loops in stdlib builtins use 10ms.

## Per-milestone gate checklist

```
cargo test --workspace        # all pass
cargo clippy --workspace      # 0 warnings (trait-signature mirrors need #[allow])
cargo fmt && cargo build --release
run-diff: Rust --run vs reference.py  -> 36/36 MATCH (authored+stdlib)
semantic: scripts/diff-semantic.sh     -> 54/54
conformance: python3 -m pytest tests/conformance/  -> 17/17
git commit + push origin/main
```
