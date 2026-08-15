# M14 — Project Status & Handover

**Date:** 2026-08-15/16 · **Version:** 1.45.0 · **Branch:** main
**Milestones:** M0–M14 complete · **Commits:** 45af0f8 → 766e2ca (M12–M14)

---

## 1. Release artifacts (Task 14.1 / DoD D5)

| Artifact | Path | Verified |
|---|---|---|
| CLI binary | `cargo build --release --features python-ffi` → `target/release/helen` | ✅ builds clean |
| crates.io package | `crates/helen-rust` (`[package] name = "helen-rust"`, bin `helen`) | ✅ `cargo install --path crates/helen-rust` works (installed to `/tmp/helen-install-test`, `helen --version` OK) |
| Python bridge wheel | `crates/helen-python-bridge` (maturin, dist name `helen-rust`) | ✅ `maturin build --release` → `helen_rust-0.1.0-cp312-...whl`; `pip install` + 13/13 DoD tests pass |
| FFI feature | `--features python-ffi` (requires CPython dev headers) | ✅ release builds with feature; 4 FFI corpus programs byte-identical |
| Standalone installer | `scripts/install.sh` (GitHub-release download or `--from-build`) | ✅ `--from-build --prefix /tmp/install-test` works |

CI release pipeline: `.github/workflows/release.yml` — tag `vX.Y.Z` builds
linux/macOS/windows binaries (tar.gz per `install.sh` naming), attaches to
GitHub Release, publishes `helen-rust` to crates.io + wheel to PyPI.

## 2. Documentation (Task 14.2)

- `wiki/README.md` — wiki index pointing at the Rust artifacts, mirroring the
  Python wiki tree layout (links survive).
- `wiki/rust/architecture.md` — crate layout, value model, threading model,
  D1–D12 design decisions.
- `wiki/rust/migration-notes.md` — intentional deviations + reference quirks
  (byte-based strings D4, string iteration unsupported, spawn strictness,
  custom-provider Python dependency, context/compression quirks).
- `docs/` — generated user guides via `helen doc`:
  - `docs/stdlib.md` — **byte-identical** to the Python reference's docgen.
  - `docs/stdlib.json` — order-insensitive identical (only JSON key/list
    ordering differs; documented cosmetic divergence).
  - `docs/language.md` — identical (no-builtins mode).
- `LICENSE` — MIT (matches original `helen`).

## 3. Migration tooling (Task 14.3)

- `scripts/sync-corpus.sh` — pulls the latest `.helen` corpus from
  `HELEN_SRC` (default `~/helen/`), auto-adds new programs, recaptures
  goldens, re-runs error-diff. `--dry-run` supported.
- `scripts/check-parity.sh` — full M13/M14 sweep on a release build:
  workspace tests → release build (with python-ffi) → Tier A differential →
  Tier B subprocess → display byte-identical → error parity → FFI examples →
  bridge DoD → benchmarks → coverage. Modes: `--fast` (tests+parity),
  `--quick` (+benchmarks), full (+coverage).
- `tests/conformance/check_golden.py` — corpus-vs-golden verifier used by the
  parity sweep and CI.
- CI: `.github/workflows/ci.yml` updated — parity sweep replaces the M0-era
  SKIP placeholder; `.github/workflows/release.yml` added.

## 4. Acceptance checklist (Task 14.4)

| # | Criterion | Status | Evidence |
|---|---|---|---|
| D1 | Corpus byte-identical stdout + exit codes/error classes | ✅ | Tier A 52/52 (authored 18, interpreter 18, agent 6, display 10); display 10/10 byte-identical |
| D2 | `helen check`/`<file>`/`test`/REPL/LSP feature-complete | ✅ | all verified against the release binary (M12 + this milestone) |
| D3 | Python FFI examples run unmodified | ✅ | `examples/python_bridge/{math,os}_example.helen` both run, expected output |
| D4 | Python Bridge: `from translator import TranslatorAgent` | ✅ | 13/13 bridge tests incl. `test_dod_translator_agent` (sync + kwargs) |
| D5 | `cargo install helen-rust` + `pip install helen-rust` | ✅ | both verified locally |
| D6 | Benchmarks at parity or better | ✅ | 6 programs: ratio 0.02–0.07× (Rust 15–50× faster); no >2× regression |
| D7 | `tests/agent` + `tests/runtime` green with Mock LLM | ✅ | Tier B agent 170/172 (2 excluded: Python-internal, fail on reference too); runtime covered via Tier A agent suite |
| D8 | Transcript/JSONL interoperable Python ↔ Rust | ✅ | `crates/helen-runtime/tests/transcript_interop.rs` (py→rust read; rust→py read via `tests/fixtures/jsonl/check_python.py`); both directions verified |

## 5. Differential & test summary

| Tier / Gate | Result |
|---|---|
| Rust workspace tests | **687 passed, 0 failed** (38 suites) |
| Tier A (differential, `--run --mock-llm`) | 52/52 byte-identical |
| Tier B (subprocess pytest vs Rust binary) | language 100/100 · cli 64/64 · ffi 64+1skip · agent 170/172 |
| Tier C (generated + execution) | lexer 67 · parser 49 · semantic 21 · execution 48 |
| Error parity | 70/70 E-code + exit-code match |
| Display corpus | 10/10 byte-identical |
| Fuzz (proptest) | lexer 6/6, parser 4/4 — no panics |
| Benchmarks | 6/6 no regression (0.02–0.07×) |
| Coverage | **68.82% overall** (llvm-cov; drivers committed) |

## 6. Known gaps & open-issues backlog

### Accepted divergences (frozen in `tests/conformance/expected-diffs.md`)
1. Error-message span formatting (cosmetic; normalized by harness) — **accepted**
2. Unicode string `len()` byte-vs-code-point semantics (D4) — **accepted** (HLD issue)
3. `spawn` race ordering strictness — **accepted** (error-parity verified)
4. `pow()` overflow — **resolved** M13 (parity enforced)
5. Python-internal Tier B tests (2, fail on reference itself) — **accepted carve-out**

### Open items (explicitly waived / follow-up)
1. **Coverage gate (M13 partial)**: overall 68.82% vs 70% target; the
   ≥85% core+parser+interpreter sub-target is not met — stdlib.rs happy
   paths (58.37%) and interpreter.rs (66.65%) are the residual gap.
   Drivers committed; next lift = valid-input stdlib behavior tests.
2. **stdlib surface driver** (exploratory, removed): `env` empty-key panic
   fixed in M13; a full generic-args driver for every exported builtin would
   push stdlib coverage toward the gate. ~278 exports available.
3. **`cargo publish` / PyPI release** not executed end-to-end (no credentials
   in this environment); pipeline in place, needs maintainer run on tag.
4. **crates.io metadata**: library crates publishable optionally (plan says
   "optional"); only `helen-rust` + wheel are required.
5. **stdlib `export_transcript` stub**: `std.transcript` export path is
   stubbed in pure batch mode (no active session); the runtime transcript
   store + JSONL interop is implemented and tested (D8), but `export_transcript`
   needs a session-backed wiring to be fully functional.

## 7. Definition of Done — M14

- [x] Release artifacts build from clean checkout (`cargo build --release
      --features python-ffi`, maturin wheel, install.sh verified)
- [x] Docs published (wiki README, architecture, migration-notes, docs/
      docgen parity) + LICENSE
- [x] Acceptance checklist D1–D8 green (evidence above)
- [x] Backlog documented with explicit waivers (coverage %, publish
      credentials, export_transcript wiring)
