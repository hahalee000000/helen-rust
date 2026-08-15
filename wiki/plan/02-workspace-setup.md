# M0 — Workspace Setup, CI, and Conformance Harness

**Objective:** A compiling Cargo workspace at `~/helen-rust/` with CI, shared test infra, and a differential-testing harness ready before language work begins.

## Task 0.1: Initialize workspace

**Files:**
- Create: `~/helen-rust/Cargo.toml`
- Create: `~/helen-rust/.gitignore` (target/, .venv/, *.pyc, dist/)
- Create: `~/helen-rust/rust-toolchain.toml` (pin `channel = "stable"`)

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = [
  "crates/helen-core",
  "crates/helen-parser",
  "crates/helen-semantic",
  "crates/helen-interpreter",
  "crates/helen-stdlib",
  "crates/helen-runtime",
  "crates/helen-cli",
  "crates/helen-lsp",
  # "crates/helen-ffi",             # PyO3, feature-gated; add in M10
  # "crates/helen-python-bridge",   # maturin cdylib; add in M11
]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
indexmap = "2"
thiserror = "2"
anyhow = "1"
```

**Verify:** `cargo metadata` succeeds; `cargo new` each member crate, all `cargo build` green (empty crates).

## Task 0.2: Bootstrap a skeleton `helen-core` with the error/span primitives

Create the two types every other crate needs:

```rust
// crates/helen-core/src/source.rs
pub struct SourceSpan { pub start: usize, pub end: usize, pub line: u32, pub col: u32 }
// crates/helen-core/src/errors.rs
#[derive(Debug, thiserror::Error)]
pub enum HelenCompileError { #[error("...")] Lex(LexError), Parse(ParseError), Semantic(SemanticError) }
```

## Task 0.3: Install CI (GitHub Actions)

`.github/workflows/ci.yml`: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` per member, and a **conformance job** running the differential harness (below) on a corpus subset.

## Task 0.4: Conformance harness (used from M1 onward)

**Files:**
- Create: `tests/conformance/README.md` — how to run
- Create: `tests/conformance/harness.py` — Python-side driver

**Approach:** A Python driver that, for each `.helen` file in `tests/programs/`:
1. Runs reference: `cd ~/helen && python -m helen.cli <file>` (capture stdout, exit code, stderr).
2. Runs candidate: `~/helen-rust/target/release/helen <file>`.
3. Compares **stdout byte-equality**, exit code, and normalized error class names (`RuntimeError`, `TypeError`, …) from stderr.
4. Emits a JUnit/JSON report; fails CI on regression.

Add a `--suite stdlib` mode that injects a deterministic `MockLLMRuntime` for LLM-dependent programs (env var `HELEN_LLM_MOCK=1` honored by both interpreters).

**Corpus seeding:** copy `tests/execution`, `tests/language`, `tests/parser` fixture `.helen` programs into `tests/programs/` as-is (they are the spec).

## Task 0.5: Benchmark harness (placeholder)

Port the shape of `tests/performance/test_benchmarks.py`: time `fib(25)`, string-join of 10k items, dict round-trips on both interpreters; store results in `tests/conformance/benchmarks/`.

## Task 0.6: Convenience scripts

`scripts/dev.sh` (fmt+clippy+test+conformance), `scripts/diff.sh <file.helen>` (one-file differential), `scripts/new-corpus.sh` (add a program to both suites).

## Definition of Done — M0

- [ ] `cargo build --workspace` clean; CI green on empty crates.
- [ ] `scripts/diff.sh` prints reference vs candidate output for a hello-world program.
- [ ] Harness produces a report with pass/fail counts and error-class diffing.
- [ ] `rust-toolchain.toml`, `.gitignore`, workspace `Cargo.toml` committed.
