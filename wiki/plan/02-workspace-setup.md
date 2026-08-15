# M0 — Workspace Setup, CI, and Conformance Harness

> **Status: COMPLETE (2026-08-15)** — commit `742a480`. All DoD items green:
> workspace + `helen-core` primitives (7 tests), `reference.py` driver,
> `diff.sh`, golden capture, Tier-A extractor (18 programs from
> `tests/interpreter`), CI workflow. Candidate comparison activates once a
> Rust binary exists (M1+ can point `HELEN_CANDIDATE` at a partial build).

**Objective:** A compiling Cargo workspace at `~/helen-rust/` with CI, shared test infra, and a differential-testing harness ready before language work begins. The harness has two halves: (a) a **Python reference driver** (in-process, decision 1a) and (b) a **pytest adoption toolchain** (decision 2).

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
  "crates/helen-rust",   # installable crates.io package "helen-rust" (binary: helen)
  "crates/helen-lsp",
  # "crates/helen-ffi",             # PyO3, feature-gated; add in M10
  # "crates/helen-python-bridge",   # maturin cdylib; add in M11
]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
indexmap = "2"
num-bigint = "0.4"
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

`.github/workflows/ci.yml`: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` per member, and a **conformance job** running the differential harness (below) on the seeded corpus.

## Task 0.4: Conformance harness (used from M1 onward)

**Files:**
- Create: `tests/conformance/README.md` — how to run
- Create: `tests/conformance/reference.py` — **Python reference driver** (decision 1a)
- Create: `tests/conformance/diff.sh` — one-file differential runner
- Create: `tests/conformance/extract_corpus.py` — pytest source-string extractor (Tier A, decision 2)

### Reference driver (`reference.py`)

The Python side is **not** invoked through `python -m helen.cli` for LLM-dependent programs (there is no `HELEN_LLM_MOCK` env hook). Instead it constructs the interpreter in-process — the same pattern the pytest suite itself uses:

```python
# reference.py — runs one Helen source and returns (stdout, exit_code, error_classes)
from helen.core.lexer import Scanner
from helen.core.parser import Parser
from helen.core.errors import ErrorReporter
from helen.interpreter.interpreter import Interpreter
from helen.runtime.llm_runtime import MockLLMRuntime

def run(source: str, mock_llm: bool = False) -> dict:
    errors = ErrorReporter()
    tokens = Scanner(source=source, file="<test>").scan_all()
    program = Parser(tokens, errors).parse()
    interp = Interpreter(errors, llm_runtime=MockLLMRuntime() if mock_llm else None)
    # capture stdout via io.StringIO redirect, run program, map exceptions:
    #   semantic errors -> exit 2; runtime errors -> exit 3; success -> exit 0
    ...
```

- `--mode inprocess` (default): drives the corpus; LLM programs get `MockLLMRuntime` (deterministic canned strings — same as the Python suite's mock).
- `--mode cli`: `python -m helen.cli <file>` for CLI-level tests only, with `HELEN_API_KEY=test-dummy-key-for-ci` exported (mirrors `tests/conftest.py`, which sets a dummy key so the preflight config check passes). CLI preflight would otherwise exit 1 on fresh machines.
- Normalized error comparison: **exit code** (0/2/3 — verified mapping) + **error class names** (the 11 Helen-native names only) + **E-codes** from stderr, with span positions stripped.

### Candidate side

`~/helen-rust/target/release/helen <file>` — same three-tuple output, same normalization. Compare **stdout byte-equality**, exit code, and normalized error classes. Emits a JUnit/JSON report; fails CI on regression.

### Corpus seeding (decision 2: pytest adoption, not wiki fixtures)

`wiki/reference/tests` is **abandoned** as a corpus source. The corpus is built two ways:

1. **Authored smoke corpus** (M0): ~20 hand-written programs in `tests/programs/authored/` (hello, arithmetic, strings, lists, dicts, control flow, functions, exceptions, display parity) — gives `diff.sh` something real from day one.
2. **Extraction tooling** (`extract_corpus.py`, implemented in M0, refined in M13): parses pytest files with Python's `ast` module, finds inline Helen source strings passed to `run_helen(...)`/`run_helen_code(...)`/`write_text(...)`/`Interpreter(...)` helpers, **re-applies the stdlib import prefix the helper prepends** (`import std.core.* / std.str.* / std.list.* / std.dict.* / std.math.* / std.debug.*` — v1.39 removed global builtins), and emits `tests/programs/pytest/<suite>/<test_name>.helen` plus a provenance manifest. This is how the interpreter/stdlib/runtime suites become a differential corpus.

Golden capture: `tests/conformance/capture_golden.py` runs `reference.py` over the corpus and stores `golden/<suite>.jsonl` (stdout, exit, error classes). Goldens are **committed once** and refreshed deliberately when the Python version changes.

## Task 0.5: Benchmark harness (placeholder)

Port the shape of `tests/performance/test_benchmarks.py`: time `fib(25)`, string-join of 10k items, dict round-trips on both interpreters; store results in `tests/conformance/benchmarks/`.

## Task 0.6: Convenience scripts

`scripts/dev.sh` (fmt+clippy+test+conformance), `scripts/diff.sh <file.helen>` (one-file differential), `scripts/extract-corpus.sh` (rerun Tier-A extraction + golden capture).

## Definition of Done — M0

- [ ] `cargo build --workspace` clean; CI green on empty crates.
- [ ] `scripts/diff.sh` prints reference vs candidate output (stdout, exit, error classes) for a hello-world program.
- [ ] `reference.py` runs in-process with `MockLLMRuntime`; the mock LLM corpus path produces deterministic goldens.
- [ ] `extract_corpus.py` extracts ≥1 real suite (e.g. `tests/interpreter`) into `tests/programs/pytest/` with a provenance manifest.
- [ ] `rust-toolchain.toml`, `.gitignore`, workspace `Cargo.toml` committed.
