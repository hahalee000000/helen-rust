# M14 — Packaging, Documentation, Acceptance

**Objective:** Distributable artifacts + docs + final acceptance checklist.

## Task 14.1: Packaging

- CLI binary: `cargo build --release --workspace` → `target/release/helen`; document `cargo install --path crates/helen-cli`.
- Bridge wheel: `cd crates/helen-python-bridge && maturin build --release` → `pip install` wheel (`helen-rust-bridge`).
- FFI feature: build instructions for `--features python-ffi` (requires Python 3.12 dev headers).
- Homebrew/standalone script `scripts/install.sh` (download release binary + optional bridge wheel).

## Task 14.2: Documentation

- `wiki/README.md` — port of the Python wiki index pointing at Rust artifacts (same doc tree layout so links survive).
- `wiki/rust/architecture.md` — crate layout, value model, threading model, design decisions D1–D10.
- `wiki/rust/migration-notes.md` — intentional deviations (strictness in spawn races, i64 vs arbitrary ints, custom-provider Python dependency).
- `docs/` user guides generated via `helen docgen` (parity-checked).
- License: MIT (match original).

## Task 14.3: Migration tooling

- `scripts/sync-corpus.sh` — pull latest `.helen` corpus from `~/helen/` (auto-add new test programs).
- `scripts/check-parity.sh` — full M13 sweep on a release build.
- CI release pipeline (crates.io for `helen-core` crates optional; binary + wheel on GitHub Releases).

## Task 14.4: Final acceptance checklist (Definition of Done from README §7)

- [ ] **D1** All corpus programs: byte-identical stdout + matching exit codes / error class names.
- [ ] **D2** `helen check`, `helen <file>`, `helen test`, REPL, LSP feature-complete.
- [ ] **D3** Python FFI: `examples/python_bridge` FFI examples run unmodified.
- [ ] **D4** Python Bridge: `from translator import TranslatorAgent` (sync/async/kwargs).
- [ ] **D5** Install paths: `cargo install` binary + `pip install` wheel.
- [ ] **D6** Benchmarks at parity or better.
- [ ] **D7** `tests/agent` + `tests/runtime` suites green with Mock LLM.
- [ ] **D8** Transcript/JSONL files interoperable between Python and Rust versions.

## Task 14.5: Handover

- Final report `wiki/plan/STATUS.md`: coverage, differential results, benchmarks, known gaps.
- Open-issues backlog (from M13 error-diff + deviations doc).

## Definition of Done — M14

- [ ] Release artifacts build from clean checkout.
- [ ] Docs published; migration notes complete.
- [ ] Acceptance checklist fully green.
- [ ] Backlog of known gaps is empty or explicitly waived.
