# Helen-Rust Maintenance Guide

> How to develop, verify, and maintain the Rust reimplementation of the Helen
> Agent Programming Language.
>
> **Status:** M0–M14 complete · Rust 1.97+ · reference Python `helen` v1.44/1.45
> at `~/helen/` · **687 Rust tests + 3,860 pytest spec** adopted via Tier A/B/C
> conformance.
>
> This is the *operating manual*. Read `wiki/plan/STATUS.md` for the handover
> report, `wiki/rust/architecture.md` for the design, and
> `wiki/rust/migration-notes.md` for the detailed gotchas this guide condenses.

---

## 1. What this project is

`~/helen-rust/` is a from-scratch **Rust reimplementation** of the Helen
language, built for **feature parity** with the Python reference
(`~/helen/`, the source of truth). Parity means: byte-identical stdout,
identical exit codes / error classes, identical CLI/REPL/LSP behavior, and
interoperable on-disk formats (SQLite transcripts, JSONL transcripts, goldens).

Two interop directions:
- **Helen → Python (FFI):** `--features python-ffi` → `helen-ffi` crate
  (PyO3), lets Helen `import "math"` and load custom LLM providers from
  `~/.helen/providers/*.py`.
- **Python → Helen (Bridge):** `crates/helen-python-bridge` (maturin cdylib,
  PyPI dist `helen-rust`), lets Python `from translator import TranslatorAgent`.

**Ground rules (never violate):**
1. `~/helen/` is the **reference and the oracle** — never modify it. When
   behavior is ambiguous, probe the reference (`python3 -c "..."` or
   `python3 tests/conformance/reference.py <file>`) before writing code.
2. Match **observable behavior, not intent** — Python bugs/quirks that are
   reachable are part of the contract (e.g. unreachable compression `break`,
   `estimate_list_tokens = sum//4`).
3. When Rust *must* diverge (e.g. byte-based strings D4, spawn race
   strictness), **document it in `wiki/rust/migration-notes.md`** and freeze
   it in `tests/conformance/expected-diffs.md`.
4. Every new `Value` variant / stdlib builtin / error code must be verified
   **differentially**, not just unit-tested.

---

## 2. Milestone map (M0–M14) — what each delivered

| MS | Plan | Delivered | Key gate passed |
|----|------|-----------|-----------------|
| M0 | 02-workspace | Workspace, CI, differential harness (lex JSON diff, `diff.sh`, reference driver) | `--lex` diff |
| M1 | 03-core-frontend | Tokens (88 types) + lexer, AST, AstPrinter, Pratt parser (10 precedence levels) | lex/parse key-sorted diff |
| M2 | 04-semantic | Type system (14 types), symbols, analyzer (E03xx) | `diff-semantic.sh` 54/54 |
| M3 | 05-interpreter | Value model, Environment, exceptions, tree-walk interpreter, builtins, `.helen` imports, `llm act/if` + MockLLMRuntime | run-diff 36/36 (authored+stdlib) |
| M4 | 06-stdlib | 22 modules / 378 builtins + zh aliases, tuple value, JSON/CSV python-format parity | 7/7 stdlib run-diff |
| M5 | 07-llm-runtime | Providers (OpenAI/Anthropic/etc.), HTTP+SSE, config, prompt builder, tool dispatch | llm act passthrough |
| M6 | 08-agent-runtime | 11-tool registry (byte-identical schemas), `fuzzy_match` 9-strategy, skills, llm act tool loop, agent scope isolation | agent run-diff |
| M7 | 09-concurrency | Channel (bidirectional/close), `spawn` → OS thread + deep-owned snapshot, SharedStore, ReadOnlyView, mailbox_select | interpreter run-diff |
| M8 | 10-context | transcript/session/history/compression/memory/observability + SQLite backend (byte-compat) | SQLite round-trip vs Python-written DB |
| M9 | 11-mcp | JSON-RPC stdio client, server manager/registry, `cwd/.mcp.json` auto-discovery, MCP tool schema merge | 14 integration tests |
| M10 | 12-python-ffi | `Value::Native`, `helen-ffi` (pyo3 0.23), import-hook fallback, custom-provider loader, CLI `install()` | 23 FFI tests; `import math sqrt(16)→4.0` |
| M11 | 13-python-bridge | maturin cdylib `helen_rust`, PyAgent/PyFunction wrappers, pure-Python shim (import hook), 13 DoD pytest | wheel in clean venv; `TranslatorAgent` |
| M12 | 14-cli-lsp | CLI (`<file>/check/test/repl/doc/provider`), REPL, Formatter, Docgen, LSP server | cli 128+1skip; LSP E2E |
| M13 | 15-conformance | Tier A/B/C differential adoption, display corpus, error-diff 70/70, bench (15–50×), fuzz, coverage drivers | **10/10 parity gates** |
| M14 | 16-release | install.sh, check-parity.sh, sync-corpus.sh, release.yml, LICENSE, wiki/docs, D1–D8 acceptance | parity sweep on release build |

Each milestone ends with the **gate checklist** (§4). Milestone *planning*
docs live in `wiki/plan/01…16-*.md` and are the per-phase task breakdown.

---

## 3. Repository anatomy — where everything lives

```
crates/                      # 10-crate workspace (strict downward deps)
  helen-core/                # spans, E-codes, tokens, lexer, AST, ast_printer   (~3.0k LOC)
  helen-parser/              # Pratt parser                                        (~2.6k)
  helen-semantic/            # types, symbols, analyzer                            (~3.4k)
  helen-interpreter/         # Value, Environment, exceptions, execution, agents,
                             #   closures, shared_store, stdlib builtins (std.core etc.) (~15.5k)
  helen-stdlib/              # stdlib registry glue
  helen-runtime/             # llm, providers, tools, skills, mcp, transcript,
                             #   context, history, compression, memory, observability  (~16k)
  helen-rust/                # crates.io package `helen-rust`, binary `helen` (CLI/REPL/docgen)
  helen-lsp/                 # LSP server
  helen-ffi/                 # Helen→Python (feature `python-ffi`, default off)
  helen-python-bridge/       # Python→Helen maturin wheel

tests/
  conformance/               # reference.py, capture/check_golden.py, error-diff.csv,
                             #   expected-diffs.md, golden/*.jsonl, extract_corpus.py
  programs/                  # corpora: authored(29) interpreter(stdlib/pytest) display(10)
  fixtures/                  # jsonl/ (transcript interop), python_session.db, etc.
scripts/                     # all harnesses — see §4 table
benchmarks/programs/         # 6 bench programs (fib, string, churn, arith, bigint, fns)
examples/python_bridge/      # FFI examples (math, os) — D3 gate
docs/                        # generated stdlib.md / stdlib.json / language.md (docgen)
wiki/                        # plan/ (master + per-M), rust/ (architecture, migration-notes),
                             #   README.md (index), STATUS.md (handover), THIS FILE
```

**Key constants/anchors:** E-code table in `crates/helen-core/src/errors.rs`;
TokenType enum (88 variants) in `crates/helen-core/src/tokens.rs`; the 11
predefined catch exception names in `interpreter/exceptions`; stdlib export
lists (`STD_*_EXPORTS`) in `crates/helen-interpreter/src/stdlib.rs` (or the
per-module files it registers).

---

## 4. Development workflow & verification gates

### 4.1 Day-to-day loop

```bash
bash scripts/dev.sh          # fmt + clippy(-D warnings) + test + conformance + authored diff
```
For a targeted iteration: `cargo test -p <crate>`, then the relevant
differential gate (§4.2). **Never ship without the parity gates.**

### 4.2 The gate checklist (per milestone / per change)

| # | Gate | Command | What it proves |
|---|------|---------|----------------|
| 1 | Workspace tests | `cargo test --workspace` | 687+ Rust unit/integration/Tier-C tests |
| 2 | Release build | `cargo build --release --features python-ffi` | D1/D5 artifact; FFI corpus needs the feature |
| 3 | Clippy 0 warnings | `cargo clippy --workspace -- -D warnings` | lint-clean (trait mirrors need `#[allow]`) |
| 4 | Tier A differential | `bash scripts/diff-tier-a.sh --all` | 52/52 byte-identical stdout+exit+errors vs goldens |
| 5 | Tier B subprocess | `bash scripts/diff-tier-b.sh language agent cli ffi` | pytest suites run against the Rust binary |
| 6 | Display corpus | `python3 tests/conformance/check_golden.py tests/programs/display --suite display` | 10/10 byte-identical |
| 7 | Error parity | `python3 scripts/gen-error-diff.py --all` | 70/70 E-code + exit-code match |
| 8 | FFI examples | run `examples/python_bridge/*.helen` | D3 |
| 9 | Bridge DoD | `cd crates/helen-python-bridge && pytest tests/test_bridge_python.py` | D4 |
| 10 | Benchmarks | `bash scripts/bench.sh --runs 3` | no >2× regression |
| 11 | Coverage | `cargo llvm-cov --no-clean --workspace` | ≥70% overall (target; 68.82% as of M14) |

**One command for all of it:** `bash scripts/check-parity.sh` (modes:
`--fast` = tests only, `--quick` = +benchmarks, full = +coverage). CI runs
the same sweep. Exit 0 = all green; per-gate logs in `/tmp/parity-*.log`.

### 4.3 When you change the reference version

`~/helen/` may advance (currently v1.44/1.45). To absorb new behavior:
1. `bash scripts/sync-corpus.sh` — re-extract pytest inline programs + copy
   new authored `.helen` files, recapture goldens.
2. `python3 scripts/gen-error-diff.py --all` — refresh the 70-row error sweep.
3. `python3 scripts/gen-tier-c.py` — regenerate Tier C lexer/parser/semantic
   tests from any changed Python test files.
4. `bash scripts/check_constants_parity.sh` — diff `constants.py` vs
   `constants.rs` (drift detection).
5. Full `check-parity.sh`, then update `STATUS.md` + `expected-diffs.md` with
   any new accepted divergences.

### 4.4 Adding a corpus program (standard procedure)

1. Write `.helen` in the right corpus dir (`tests/programs/{authored,display,…}`).
2. Capture goldens: `python3 tests/conformance/capture_golden.py tests/programs/<dir> --out tests/conformance/golden --suite <name>`.
3. Verify: `check_golden.py … --suite <name>` must pass.
4. Run Tier A (`diff-tier-a.sh <name>`) and error parity.

---

## 5. Hard-won gotchas (condensed; full detail in migration-notes.md)

**Differential methodology**
- `--mock-llm` is **reference-only**; Rust binary treats it as a file path.
  Rust side: `--run <file>`; mocks live inside the harness.
- Lex JSON key order differs cosmetically (serde_json alphabetical vs Python
  insertion order) — compare key-sorted or via conformance pytest.
- Broken corpus fixtures (`spawn_expr.helen`, `shared_store.helen`) error
  identically on both sides — they prove *error* parity only.

**Rust pitfalls**
- UFCS self-recursion: calling a trait method that has an override dispatches
  to the override → infinite recursion. Extract a free helper.
- `json!` can't be `const`; ureq 2.x `into_reader()` consumes the Response;
  `Arc<dyn LlmRuntime>` forces `&self` methods (interior mutability).
- `spawn` payload needs `unsafe impl Send` + `#[allow(arc_with_non_send_sync)]`.
- `Environment::snapshot()` uses `clone_owned()` (shared Rc), **not**
  `clone_deep()`.
- stdout must be `Arc<Mutex<String>>` (spawned threads append).
- New `Value` variant → ~10 exhaustive-match sites (`python_str`, `type_name`,
  `PartialEq`, `Hash`, `clone_owned`, `is_truthy`, `visit_access`,
  `visit_index`, `assign_access`, `visit_call`) + ffi/bridge sites.

**Parser/AST quirks**
- `tools = [...]` serializes to `\x1f`-wrapped, `\x1e`-joined string.
- Agent fn return type is `fn f(...): int` (colon), not `->`.
- Top-level `shared store` needs a `register_shared_store()` helper (was a no-op).
- `;` after `let`/`const` is a parse error in **both** implementations.

**Behavioral parity**
- `sleep` is in `std.time`, not `std.core`; `print` needs `import std.core.*`.
- Channel close pushes a sentinel → receiver wakes, next receive → `None`.
- Spawn injects the Channel endpoint as the **last** param; user args bind
  positionally to non-Channel params.
- Agents get a fresh env — inject stdlib builtins + module consts + shared lets.
- `ReadOnlyView` has no `__getattr__` delegation (only `__getitem__`/`__iter__`).
- `main { 42 }` → 42, `main { let x = 42 }` → 42 (last expr / initializer value
  threads out); `null` collapses to None at the interpret boundary.
- Out-of-bounds list/dict access **raises** RuntimeError (pytest helpers catch
  it → None; don't assert the caught value).
- `throw ValueError` falls back to **RuntimeError** (only 11 native classes
  match); catch syntax is `catch Type name { }`.
- Undefined var → E0332, CLI exit **2** (Python CLI parity confirmed).
- `math_pow` overflow raises `RuntimeError: Python OverflowError: math range error`.
- `env_set("", …)` / `env_delete("")` must NOT panic — match Python's
  OSError / `Variable not found` behavior (std::env panics on invalid keys).
- MCP registry is process-global — tests must serialize or leak state.
- `estimate_list_tokens = sum(chars)//4` (one division at the end).
- Two different context-window defaults (131072 vs 128000) — keep per-module.

**Bridge/FFI**
- pyo3 0.23: `call_method` with `(arg,)` silently drops the arg → use
  `PyTuple::new`; `PyType::is_subclass` (not `eval issubclass`).
- Registry stores classes; adapters instantiate before delegating.
- Extension-module feature is mandatory for manylinux wheel compliance.
- Rc-based Interpreter is not Send → bridge builds a fresh interpreter per call.

---

## 6. Maintenance operations (runbook)

### 6.1 Publish a release (tag `vX.Y.Z`)
1. Update version in `crates/helen-rust/Cargo.toml` + bridge `Cargo.toml`/
   `pyproject.toml`; `cargo build --release --features python-ffi`.
2. `bash scripts/check-parity.sh` full sweep green.
3. Push tag → `.github/workflows/release.yml` builds linux/macOS/windows
   binaries (tar.gz per `install.sh` naming), attaches to GitHub Release,
   publishes `helen-rust` to crates.io + wheel to PyPI.
4. **Credentials are not in this environment** — a maintainer must run the
   publish step on a tagged push.

### 6.2 Recapture goldens after an intentional change
```bash
python3 tests/conformance/capture_golden.py tests/programs/<dir> \
  --out tests/conformance/golden --suite <name>
```
Always review the diff with `git diff tests/conformance/golden/` before
committing — goldens are the parity oracle.

### 6.3 Coverage (M13 Task 13.8, partial)
- Install once: `rustup component add llvm-tools-preview` +
  `cargo install cargo-llvm-cov`.
- Run: `cargo llvm-cov --no-clean --workspace --exclude helen-lsp
  --exclude helen-ffi --exclude helen-python-bridge`.
- As of M14: **68.82% overall** (target ≥70%); core+parser+interpreter ≥85%
  **not met** — gap is stdlib.rs happy paths (58.37%) and interpreter.rs
  (66.65%). Highest-ROI next work: valid-input behavior tests exercising
  stdlib bodies (drivers exist: `corpus_tests.rs` 70 programs, core surface
  sweep, fuzz targets).

### 6.4 Docgen parity check
```bash
./target/release/helen doc -o docs/stdlib.md --with-builtins   # vs reference's helen doc
diff <(cd ~/helen && helenenv/bin/helen doc -o /tmp/py-stdlib.md --with-builtins && cat /tmp/py-stdlib.md) docs/stdlib.md
```
MD must be byte-identical; JSON is order-insensitive identical.

### 6.5 Backup/restore of session data
- Session store lives under `.helen/` (gitignored). Transcript SQLite/JSONL
  are the SSOT; `crates/helen-runtime/tests/transcript_interop.rs` verifies
  Python↔Rust cross-reads. Don't commit `.helen/`.

---

## 7. Known gaps & open issues (backlog)

| # | Item | Status |
|---|------|--------|
| 1 | Coverage gate ≥85% core+parser+interpreter / ≥70% overall | 68.82% overall; stdlib happy-path tests needed |
| 2 | Full generic-args stdlib surface driver (~278 exports) | exploratory test removed (env panic fixed); would push coverage |
| 3 | ~~`cargo publish` / PyPI end-to-end~~ | ✅ **DONE** (2026-08-22): 9 crates on crates.io, `helen-rust` on PyPI |
| 4 | Library crates publishable? | ✅ All 9 crates published to crates.io (v0.1.0) |
| 5 | `export_transcript` stub in pure batch mode | runtime store + JSONL interop done (D8); session-backed wiring pending |
| 6 | Python-internal Tier B tests (2) | fail on reference too — accepted carve-out |
| 7 | Fuzz corpus growth | proptest targets exist; extend strategies as grammar evolves |

Accepted divergences are frozen in `tests/conformance/expected-diffs.md`
(span cosmetics, byte-vs-codepoint `len`, spawn race strictness, pow
overflow, Python-internal tests). **New divergences must be added there, not
silently introduced.**

---

## 8. How to extend the language (checklist)

**New stdlib builtin:** add export to the module's `*_EXPORTS` list + a
Rust `fn` in the module block → verify with a display-corpus or execution
Tier C test → run error parity + Tier A. Cross-check argument validation
against Python's `_validate_args` behavior (TypeError message text).

**New error code:** add to `errors.rs` (`ErrorCode` enum + message) — verify
the E-code number matches Python's, then cover with a Tier C semantic test
and an error-diff row.

**New `Value` variant:** touch all ~10 match sites (§5) + ffi/bridge
mapping + `python_str`/`type_name`; add a display-corpus case; run the full
sweep.

**New keyword/syntax:** lexer + parser + semantic + (formatter/REPL/LSP if
affected); add Tier C parser tests; verify with a diff-semantic run.

**New tool (agent registry):** schema must be byte-identical to Python's
tool JSON — verify with the tool-schema test; dispatch routing in
`tools.rs`.

---

## 9. Reference docs

- `wiki/plan/STATUS.md` — M14 handover: artifacts, acceptance D1–D8 evidence,
  differential/test summary, waivers.
- `wiki/rust/architecture.md` — crates, value model, threading, D1–D12.
- `wiki/rust/migration-notes.md` — the full gotcha corpus (M1–M12).
- `wiki/plan/01…16-*.md` — per-milestone task breakdown (the plan we executed).
- `tests/conformance/expected-diffs.md` — frozen accepted divergences.
- `tests/conformance/README.md` — conformance harness usage.
- Python reference source: `~/helen/` (the oracle; do not modify).
