---
name: differential-porting
description: "Differential conformance porting — reimplement a Python reference implementation in another language (e.g. Rust) byte-faithfully, using differential testing as the primary gate. Covers reference-first methodology, run-mode differential harnesses, Rust porting pitfalls (UFCS recursion, Arc<dyn Trait> &self, ureq into_reader, Send/Sync), parser/AST fidelity traps, behavioral parity gotchas, reference-quirk matching (unreachable break conditions, per-module constant drift, plain-string vs wrapper formats), MCP-client process-global state traps, pyo3/FFI traps (Bound API, PyTuple args, eval-subclass, class-vs-instance), maturin wheel packaging, CLI/LSP port gotchas, and the per-milestone gate checklist. Distilled from M1-M12 of the helen-rust project."
version: 1.2.0
author: Helen Team
license: MIT
tags: [porting, conformance, differential, python-reference, rust, byte-faithful, migration, reimplementation]
---

# Differential Conformance Porting

Port a Python reference implementation to another language with **byte-identical observable behavior**, verified by differential testing at every layer (lex → parse → semantic → run).

## When to Use

- Reimplementing a language/DSL runtime in another language (e.g. `~/helen/` Python → Rust)
- Any port where exact behavioral parity matters (error codes, exit codes, stdout bytes, JSON output)
- Adding a new milestone module and needing a repeatable verification gate

## Core Methodology (reference-first)

1. **Read the Python reference FIRST** — never port from memory or from your own expectations. When in doubt, run `python3 -c "..."` against the reference and record the actual output. Example: fuzzy_match strategy chain order — my hand-written expectations were wrong, the port was right.
2. **Port module-by-module, test-per-module** — write contract tests derived from `helen/tests/...` *before* the implementation (TDD), then run the differential after each module.
3. **Differential gates at each layer**:
   - Lex: compare token streams (key-sorted JSON — serde_json sorts keys alphabetically, Python dicts keep insertion order; content is identical).
   - Parse: `--parse` JSON AST mode vs `reference.py --parse`; AST printer must be byte-identical (replicate Python dataclass-repr quirks verbatim).
   - Semantic: exit-code parity (2) + E-code emission order.
   - Run: `{stdout, stderr, exit_code, error_classes}` JSON parity — exit 0/1/2/3 semantics, stderr span normalization, `RuntimeError: {e}` uncaught rendering.
4. **The AST printer can hide fidelity bugs.** If a printer omits a field (e.g. type annotations), the parse-diff passes while the AST is structurally wrong. Verify annotation/optional/union nodes explicitly, not just printed output.
5. **Broken corpus fixtures exist** — some fixtures error identically in both implementations and verify *error parity* only, not functionality. Port real behavior tests from the reference test suite by hand.

## Differential Harness Facts (helen-rust)

- Rust CLI exposes `--lex`, `--semantic-only`, `--run`. There is NO `--conformance` flag — `scripts/diff.sh` is stale (M0-era). Run differentials must use `--run`.
- `--mock-llm` is **reference-only**: `reference.py` accepts it, the Rust binary does not (passing it makes Rust treat `--mock-llm` as the file path, rc=2). Correct sweep: Rust `--run <file>` (mock built into test harness), reference `reference.py <file>` (default mode = run).
- Lex JSON key-order differs cosmetically — compare key-sorted or rely on the conformance pytest which normalizes.

## Rust Porting Pitfalls

1. **UFCS recursion trap** — a struct method calling the same-named trait method dispatches to the *override* → infinite recursion. Fix: extract a free helper (`default_parse_response()`) used by both the trait default and the override.
2. **`serde_json::json!` cannot be `const`.** Build schemas in plain `fn defs() -> Vec<Value>` or `OnceLock`.
3. **ureq 2.x `Response`**: `resp.into_reader()` is *consuming* (drops Response, returns owned reader); `resp.body_mut().as_reader()` borrows. Use `into_reader()` when Response must die before reading completes.
4. **`Arc<dyn LlmRuntime>` forces `&self` trait methods** — `&mut self` through a trait object needs nightly. Use interior mutability (`RefCell`/`Mutex`) in implementors.
5. **Send/Sync**: `Rc<RefCell<...>>`-heavy code is not `Send`. For `spawn`: deep-own into a `SpawnPayload` + `unsafe impl Send` with documented single-owner discipline. Clippy's `arc_with_non_send_sync` needs an explicit `#[allow]` (intentional).
6. **`clone_owned()` vs `clone_deep()`** — `Environment::snapshot()` must use `clone_owned()` (fresh `Rc<str>`, deep containers) so shared-let values stay shared; `clone_deep()` wrongly deep-copies everything.
7. **stdout must be `Arc<Mutex<String>>`** (not `RefCell`) for spawned threads to append output.
8. **New `Value` enum variants touch ~10 exhaustive-match sites** in one pass: `python_str`, `type_name`, `PartialEq`, `Hash`, `clone_owned`, `is_truthy`, `visit_access`, `visit_index`, `assign_access`, `visit_call`.

## Parser / AST Fidelity Traps

- **Type annotations**: Python keeps `OptionalTypeNode`/`UnionTypeNode`/`LiteralTypeNode` as expression nodes; a naive port collapses `int?`/`A|B` into synthetic `TypeRef` names (`optional<int>`). This breaks `type_from_typenode`. Match Python's class hierarchy: composite types are real nodes, not names.
- `tools = [...]` is serialized by the parser into `\x1fname\x1ename\x1f` (items joined `\x1e`, wrapped `\x1f`). Parse with `trim_matches('\u{1f}')` + `split('\u{1e}')`.
- Agent `functions {}` return type uses **colon** syntax: `fn add(a: int, b: int): int` — NOT `->`.
- Top-level `shared store` can silently no-op (stores only registered for imported `.helen` files). Factor a `register_shared_store()` helper used by both paths.
- Agent/LLM settings live in `AgentDecl.declarations`; read via an `agent_setting(name)` helper (Python `_get_agent_setting` parity).

## Behavioral Parity Gotchas

- Module placement: `sleep` lives in `std.time`, NOT `std.core`; polling loops in stdlib builtins use 10ms.
- Channel close pushes a **sentinel** → blocked receiver wakes, next `receive()` returns `None`. Send-after-close is ignored.
- Spawn auto-injects the Channel endpoint — bind user positional args to **non-Channel params only** (declaration order). Binding by raw index consumes the endpoint and breaks arg order.
- Agents get a **fresh env** — inject stdlib builtins + module consts (read-only) + shared lets (writable), otherwise `print` is undefined.
- `ReadOnlyView` has NO `__getattr__` delegation in Python (only `__getitem__`/`__iter__`) — `.append` on a read-only param raises. (`len()` DOES delegate via `__len__` — builtins must match the ReadOnly arm before concrete-type arms.)

## Reference Quirks: Match Observable Behavior, Not Intent (M8)

1. **Unreachable code paths in the reference are still the contract.** Python's
   `_truncate_compress` break condition never fires (`MIN_RECENT_MESSAGES=5`
   walks newest-first) → the reference keeps ALL messages. Verified: identical
   input → Python keeps all 25, the port must too. Port the observable
   behavior; document the dead code.
2. **Same concept, different defaults per module.** `token_utils.py` context
   window = 131072, `history.py` = 128000. Port each module's own constant —
   never "unify" them.
3. **Aggregation order matters in token estimates.** `estimate_list_tokens` is
   `sum(chars) // 4` (ONE division at the end). Per-item division drops 4x more
   than Python. Empirically verify before trusting your reading of the code.
4. **Optional-client gates are part of the observable contract.** Reactive
   semantic recovery only fires when `llm_client is not None`. Model as
   `Option<LlmClient>`, not a plain client.
5. **A "correct-looking" constant can still diverge.** My `assign_priority("user")
   = 100` was wrong — Python says 90. Re-verify every constant against the
   reference when you port the next dependent module.
6. **Serialized formats have subtle shapes.** Summary `content` is a plain
   string, not `{"text": ...}`. Deprecated `Message.to_dict()` omits agent
   fields; the non-deprecated free function includes them. `role` defaults to
   `"user"` on load.
7. **`session_exists` = dir + `transcript.jsonl`**, not just the dir.
   `create_session()` alone doesn't make a session exist.

## Data-Backend Parity (SQLite/JSONL)

- Keep a **Python-written fixture** checked into the repo
  (`tests/fixtures/python_session.db`) and assert the Rust port round-trips it:
  schema, meta rows, boundary markers, pinned flags, message rows.
- Match the reference schema exactly (WAL mode, `JSON_EXTRACT` pushdown for
  query) — don't "improve" the schema; parity tests read the raw DB.
- `rusqlite` needs `--features bundled` (vendored libsqlite3).

## Constants Drift Detection

- Add a **drift-failing script** (e.g. `scripts/check_constants_parity.sh`)
  that greps the Python source for `NAME: Final[...] = value` and diffs against
  the Rust `constants.rs`. Exit non-zero on any mismatch. Run it as part of the
  gate, not just once.

## Process-Global State & Parallel Tests (M9)

- **Python module-global state becomes `static OnceLock<Mutex<...>>` in Rust.**
  Tests touching it MUST serialize (shared `with_mcp_clean`-style helper) or
  parallel tests observe leaked state (`Unknown tool` → `Unknown MCP tool`).
- **`pathlib` normalizes `./subdir`** in cwd resolution; Rust `Path::join` does
  not — add a `normalize_path` (drop `CurDir`, pop on `ParentDir`).
- **Best-effort shutdown is a contract**: set `is_running=false` FIRST, then
  the shutdown RPC fails the check and is swallowed; the server dies via
  process termination only. Mirror the quirk exactly.
- **Reader-thread design for child processes**: move `ChildStdout` into a
  BufReader thread; dispatch responses to per-request `mpsc::Sender`s keyed by
  id in `Arc<Mutex<HashMap>>`; `recv_timeout` implements the Python queue
  timeout. `Drop` must kill+wait the child and join the thread.
- **Shared temp fixtures race under parallel tests** — `std::fs::write` is
  non-atomic; a concurrent reader can hit a truncated file → spurious "Failed
  to parse". Use a per-call unique filename (atomic counter).

## Python FFI (pyo3) Traps (M10)

1. **pyo3 0.23 Bound API**: `call_method(name, (arg,), kwargs)` with a Rust
   1-tuple of `Py<PyAny>` silently drops the arg. Build args with
   `PyTuple::new(py, [x.bind(py)])` + `obj.call_method(name, tuple, Some(&kw))`.
   `call_method1(name, (a,))` is fine (the tuple IS the expected Target).
2. **eval-based subclass detection is a trap**: `eval("issubclass(dict['X'], B)")`
   — `dict` resolves to the builtin, `dict['X']` is PEP-585 generic subscription
   → `GenericAlias` → `issubclass` silently False. Use Rust-side
   `PyType::is_subclass(&Bound<PyAny>)` (pyo3 0.23 — NOT `is_subclass_of`).
3. **Registries store classes; adapters hold instances**: `_PROTOCOL_NAME_MAP`
   stores the class; detection returns `protocol_class()`. The Rust adapter must
   `val.call0()` to instantiate before delegating, else `TypeError: X() takes no
   arguments` or `'X' object is not callable`.
4. **`Value::Native(NativeHandle)`** touches 9 exhaustive match sites (truthy,
   type_name, python_str, python_repr, PartialEq identity, Hash identity, clone,
   call, access/index/assign). `as_any()` must be a required method of the
   `NativeObject` trait (default impl can't coerce `&dyn Trait` → `&dyn Any`).
5. **Class `__dict__` is a mappingproxy**: `get_item` raises KeyError (returns
   `PyResult<Bound>`), unlike `PyDictMethods::get_item` (`PyResult<Option<Bound>>`)
   — don't `.flatten()` the former.
6. **Feature-gated crate pattern**: workspace crate with a default-off feature
   (`python-ffi`) keeps the default build/test/clippy unaffected; pyo3 fetched
   only when enabled. The CLI binary gates its FFI install the same way.

## Bridge / Wheel Packaging (M11)

- **`extension-module` feature is mandatory for wheel compliance**: a pyo3
  cdylib linking libpython fails `maturin build` manylinux checks. Optional
  feature `extension-module = ["pyo3/extension-module"]`, enabled only by
  `[tool.maturin] features = [...]` in pyproject.toml. `cargo test` never
  enables it — both builds coexist.
- **Python wrapper classes use plain instance attributes, not `@property`** —
  getter-only properties break `__init__` (`AttributeError: property 'x' ... has
  no setter`). Mirror the reference: no properties.
- **`Rc`-based interpreter is not `Send`** → cannot live in a pyclass. Hold the
  parsed `Arc<Program>` and build a fresh interpreter per call (keeps wrappers
  Send-safe for `async_call` via `run_in_executor`).
- **Default-param evaluation needs the agent's isolated env** — compute
  defaults first, then define into the borrowed env (defaults may evaluate
  arbitrary expressions).
- **Inline `llm act "literal"` does NOT substitute `{{param}}`** — only the
  agent's `prompt "..."` template renders; an expression-level literal stays
  literal. DoD asserts the call flows, not the prompt content.
- **Wheel layout**: `[lib] name` + `crate-type = ["cdylib", "rlib"]`;
  `[tool.maturin] python-source = "python"` (pure-Python shim with import hook)
  + `module-name = "pkg._core"`. Editable `maturin develop` links the shim from
  source → .py fixes need no rebuild.

## CLI / LSP Port Gotchas (M12)

- **CLI parity**: `helen <file>` exit codes 0/1/2/3; `Error: [E0332] ...` + HLD
  3.11.2 caret format; `helen check` → `✓ file: OK`; `helen test` report
  byte-identical to Python `_format_report`; `--json` matches semantically (key
  order cosmetic).
- **Formatter caret width** = `end_col-1 - (start_col-1) + 1`; pad with spaces
  for multi-line messages; blank source lines get `|` gutter with spaces.
- **Python `list.append(obj)` holds a REFERENCE** to the object — Rust must use
  indices (`Vec<(indent, usize)>`) into the symbols vec, not clones, or child
  mutations don't propagate (document-symbol nesting bug).
- **`re.finditer` gives char offsets; Rust `regex` gives byte offsets** —
  ASCII-safe, but CJK needs explicit `[\w\u4e00-\u9fff]` classes (`\b` in Rust
  regex is ASCII-only; Python's is Unicode-aware).
- **`_uri_to_path` percent-decodes `file://` URIs** (import-resolution fix) —
  pass the real fs path to the Scanner and base_dir to the analyzer.
- **Clippy `int_plus_one`**: `idx <= col-1` → `idx < col` is right, but
  `col-1 <= x` is NOT `col <= x` (off-by-one!) — only apply the lint to the
  `y <= x-1` form.
- **JSON-RPC framing**: `Content-Length: N\r\n\r\n` + body; publish
  `textDocument/publishDiagnostics` notifications after didOpen/didChange.

## Per-Milestone Gate Checklist

```
cargo test --workspace        # all pass
cargo clippy --workspace      # 0 warnings (trait-signature mirrors need #[allow])
cargo fmt && cargo build --release
run-diff: Rust --run vs reference.py  -> N/N MATCH
semantic: scripts/diff-semantic.sh     -> N/N
conformance: python3 -m pytest tests/conformance/  -> N/N
git commit + push origin/main
```

## Project-Specific Detail

Full project-specific record (exact fixtures, file names, divergence history): `wiki/rust/migration-notes.md` in the helen-rust repo. Keep that as the authoritative detailed log; this skill holds the transferable methodology.
