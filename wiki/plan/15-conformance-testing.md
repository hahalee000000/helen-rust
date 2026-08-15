# M13 — Conformance Testing & Benchmarks

**Objective:** Prove feature parity with measurable coverage. Exit criterion: full differential suite green + benchmark report committed.

## Task 13.1: Differential suite expansion

Run the harness (M0.4) across every corpus program with per-suite reports:

| Suite | Source corpus | Assert |
|---|---|---|
| lex | `tests/lexer` fixtures | token stream + error codes |
| parse | `tests/parser` + `tests/core` | AstPrinter S-expressions |
| semantic | `tests/semantic` | E-code lists |
| execution | `tests/execution` (24 files) | stdout + exit code |
| language | `tests/language` (11) | stdout + exit code |
| interpreter | `tests/interpreter` (22) | stdout + exit code + error class |
| stdlib | `tests/stdlib` (34) | stdout + error class |
| runtime | `tests/runtime` (47) | with Mock LLM; JSONL byte-compat |
| agent | `tests/agent` (10) | stdout + session artifacts |
| cli | `tests/cli` (7) | CLI golden |
| lsp | `tests/lsp` (1) | protocol messages |
| ffi | `tests/ffi` (4) | Python objects round-trip |
| multimodal | `tests/multimodal` (3) | media pipeline outputs |

> **String-op differential (D4):** programs exercising string functions assert **ASCII parity**; non-ASCII (CJK) cases are checked against `tests/conformance/expected-diffs.md` (deliberate byte- vs code-point divergence).

## Task 13.2: Ported Rust unit tests

Beyond differential runs, port high-value Python unit tests as Rust tests (they give precise blame when differential fails): lexer edge cases, precedence, sentinel propagation, scope isolation, exception hierarchy, spawn determinism, TranscriptStore formats, tool schemas.

## Task 13.3: Fuzz/property corpus

Add a small fuzz target (`cargo fuzz` or `proptest`) for the lexer/parser: random token streams must never panic; errors must carry codes. Property: `parse(valid program) → parse(AstPrinter output)` round-trip.

## Task 13.4: Benchmarks

Port `tests/performance/test_benchmarks.py` scenarios: fib recursion, string ops, list/dict churn, agent spawn (Mock), transcript append. CI job publishes a table: `python helen` vs `rust helen` (median of 5). Target: parity or better; regressions > 2x fail CI.

## Task 13.5: Coverage gate

- `cargo llvm-cov` on Rust crates with the corpus as integration tests.
- Compare against Python's `.coverage` report (existing `reports/`).
- Target: ≥ 85% line coverage on core+parser+interpreter crates; ≥ 70% overall.

## Task 13.6: Error-message parity sweep

Normalized diff of error messages (class + code + message minus spans) across all error fixtures. Produce `tests/conformance/error-diff.csv`; fix any mismatch class/name-level before cosmetic message differences are accepted (documented exceptions).

## Definition of Done — M13

- [ ] 100% differential pass on execution/language/interpreter/stdlib suites.
- [ ] CLI/LSP/FFI/Bridge suites green.
- [ ] Benchmarks: no > 2x regression vs Python.
- [ ] Coverage targets met; report committed.
