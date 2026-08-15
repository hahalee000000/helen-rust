# M13 — Conformance Testing & Benchmarks

**Objective:** Prove feature parity with measurable coverage against the real specification — the **3,860 pytest tests** (203 files) of `~/helen/` — via the three-tier adoption strategy (D10), plus benchmarks. Exit criterion: all three tiers green + benchmark report committed.

## Task 13.1: Tier A — extracted differential corpus

`tests/conformance/extract_corpus.py` (skeleton in M0) parses each pytest file with Python's `ast`, finds inline Helen source strings passed to `run_helen`/`run_helen_code`/`run_helen_with_session`/`Interpreter(...)` helpers, **prepends the stdlib import prefix the helper uses** (v1.39 removed global builtins), and emits `tests/programs/pytest/<suite>/<test_name>.helen` + `manifest.json` (provenance: file, test name, helper kind).

Reference goldens: `capture_golden.py` → `tests/conformance/golden/<suite>.jsonl` (stdout bytes, exit code, error class names, E-codes), captured with `reference.py` (in-process, `MockLLMRuntime` for LLM-dependent cases — D12). Goldens are committed and reviewed; refreshed deliberately.

| Suite | Tests | Assert |
|---|---|---|
| interpreter | 355 | stdout byte-identical + exit code + error class |
| stdlib | 942 | stdout + error class (per-module suites) |
| runtime | 928 | with Mock LLM; JSONL byte-compat |
| language (in-process part) | ~50 | stdout + exit code |
| agent (in-process part) | ~80 | stdout + session artifacts |
| cli (in-process part) | ~20 | CLI golden |

Run: `scripts/diff.sh --tier A --suite <name>` — candidate `helen` vs golden (or live reference via `--live`).

## Task 13.2: Tier B — subprocess adoption (drop-in binary swap)

Tests that run `helen <file>` via `subprocess` are executed **as-is** against the Rust binary:

- CI job prepends `~/helen-rust/target/release/` to `PATH` so `helen` resolves to the Rust binary; exports `HELEN_API_KEY=test-dummy-key-for-ci` (the suite's `conftest.py` already sets this for the Python side — same pattern).
- Suites: `tests/language` (module-import via subprocess), `tests/agent` (cross-platform helper tests), `tests/ffi` (subprocess parts), `tests/cli` (golden output).
- A test failure = parity bug; the Python side is the oracle. Any test that imports `helen.*` internals for its *assertions* (not just to launch) is moved to Tier C.

## Task 13.3: Tier C — reimplemented Rust tests

Suites that construct ASTs programmatically cannot be extracted. Port each test case to Rust, mirroring the Python test logic against the Rust AST builder + interpreter:

| Suite | Tests | Rust home |
|---|---|---|
| lexer | 181 | `helen-core/tests/lexer_tests.rs` (token streams) |
| parser | 114 | `helen-parser/tests/parser_tests.rs` (AST-printer snapshots) |
| core (AST/spans) | 121 | `helen-core/tests/ast_tests.rs` |
| semantic | 207 | `helen-semantic/tests/semantic_tests.rs` (E-code lists) |
| execution | 360 | `helen-interpreter/tests/execution_tests.rs` (construct Rust AST → run → assert) |

## Task 13.4: Authored edge-case corpus + display parity

- `tests/programs/authored/` — edge cases absent from pytest suites: empty inputs, unicode, negative indices, NaN/Inf, big ints (>i64), deep nesting.
- `tests/programs/display/` — **print/str/repr display parity (D11)**: nested containers with bools/nulls/strings, float repr thresholds (`1e-20`, `1.5e-05`, `1e+16`, `1e+20`), dict ordering, error-message embedding. Assert byte-identical stdout.
- `tests/conformance/expected-diffs.md` — documented divergences (non-ASCII string byte-vs-code-point semantics, spawn race strictness) that are *accepted*; everything else must match byte-for-byte.

## Task 13.5: Error-parity sweep

- Normalized diff of error messages across all suites: **class name (11 Helen-native) + E-code + message minus spans**. Produce `tests/conformance/error-diff.csv`; any class/name mismatch fails; cosmetic message differences require documented exceptions.
- **Exit-code parity**: 0 success / 2 semantic / 3 runtime — asserted on every failing case.

## Task 13.6: Benchmarks

Port `tests/performance/test_benchmarks.py` scenarios: fib recursion, string ops, list/dict churn, agent spawn (Mock), transcript append. CI job publishes a table: `python helen` vs `rust helen` (median of 5). Target: parity or better; regressions > 2x fail CI. Also track `num-bigint` cost in hot loops (D3).

## Task 13.7: Fuzz/property corpus

Add a small fuzz target (`cargo fuzz` or `proptest`) for the lexer/parser: random token streams must never panic; errors must carry codes. Property: `parse(valid program) → parse(AstPrinter output)` round-trip.

## Task 13.8: Coverage gate

- `cargo llvm-cov` on Rust crates with the corpus as integration tests.
- Compare against Python's `.coverage` report (existing `reports/`).
- Target: ≥ 85% line coverage on core+parser+interpreter crates; ≥ 70% overall.

## Definition of Done — M13

- [ ] Tier A: 100% differential pass on interpreter/stdlib/runtime suites (byte-identical stdout, exit, error classes).
- [ ] Tier B: subprocess suites green against the Rust binary.
- [ ] Tier C: ported lexer/parser/core/semantic/execution Rust tests green.
- [ ] Display-parity corpus byte-identical; expected-diffs.md reviewed and frozen.
- [ ] Error-diff.csv: zero unmatched class/E-code; exit-code mapping asserted.
- [ ] Benchmarks: no > 2x regression vs Python.
- [ ] Coverage targets met; report committed.
