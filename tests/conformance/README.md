# Conformance Harness (M0)

The differential-testing harness compares the **Python reference interpreter**
(`~/helen/`, v1.44.0) against the **Rust candidate** (`helen-rust`) on the same
programs. It exists to make feature parity measurable from M1 onward.

## Layout

```
tests/conformance/
  README.md            # this file — the M0 contract
  reference.py         # Python reference driver (decision 1a)
  diff.sh              # one-file differential runner
  capture_golden.py    # snapshot reference results as committed goldens
  extract_corpus.py    # pytest source-string extractor (decision 2, Tier A)
  golden/              # committed goldens: <suite>.jsonl
  fixtures/            # synthetic inputs for extract_corpus tests
  test_reference.py    # contract tests for reference.py
  test_diff_sh.py      # contract tests for diff.sh
  test_extract_corpus.py
tests/programs/
  authored/            # hand-written smoke corpus (M0)
  pytest/              # extracted corpus (Tier A; provenance manifest.json)
```

## Contract: `reference.py`

```
reference.py <file.helen> [--mock-llm] [--mode inprocess|cli]
reference.py - [--mock-llm] [--mode inprocess]      # read source from stdin
```

Emits **one JSON object on stdout** (never writes result text to stdout):

```json
{"stdout": "", "stderr": "", "exit_code": 0, "error_classes": []}
```

`exit_code` follows the Python CLI mapping (verified against v1.44.0):

| code | meaning |
|------|---------|
| 0    | success |
| 1    | lex / parse / IO error |
| 2    | semantic error |
| 3    | runtime error (uncaught exception) |

`error_classes` lists the **Helen-native exception class names** of uncaught
runtime errors (the 11 predefined names: `AnyError, LLMError, TimeoutError,
ModelError, PromptTooLongError, AgentError, LLMOutputContractError, ToolError,
RuntimeError, AssertionError, AggregateError`).

Modes:
- `--mode inprocess` (default): constructs `Scanner → Parser →
  SemanticAnalyzer → Interpreter` in-process with `io.StringIO` stdout
  capture — the same pattern the pytest suite's `run_helen` helpers use.
  `--mock-llm` injects `MockLLMRuntime(act_return="MOCK_REPLY", ...)` for
  deterministic LLM programs.
- `--mode cli`: `python -m helen.cli <file>` with `HELEN_API_KEY` exported
  (mirrors `tests/conftest.py`); used only for CLI-level parity checks.

Environment:
- `HELEN_SRC` — path to the Python interpreter source (default `~/helen`).
  The driver adds it to `sys.path` and runs the CLI from it.

Normalization: `stderr` has ` at <path>:<line>:<col>-<col>` location suffixes
stripped so goldens are machine-portable.

## Contract: `diff.sh`

```
diff.sh <file.helen>
```

Runs `reference.py` (inprocess) and, when a candidate binary exists, the
candidate with the same three-tuple contract. Prints both outputs and a
VERDICT line; exits 0 on match, 1 on mismatch, 2 if the candidate is missing
(reference output still shown). Candidate binary: `$HELEN_CANDIDATE` env var,
default `target/release/helen`.

## Contract: `capture_golden.py`

```
capture_golden.py <corpus_dir> --out tests/conformance/golden --suite <name>
```

Runs `reference.py` over every `.helen` file in `<corpus_dir>`, emitting
`golden/<name>.jsonl` (one JSON object per program). Goldens are committed and
only refreshed deliberately.

## Contract: `extract_corpus.py` (Tier A)

```
extract_corpus.py <pytest_suite_dir> --out tests/programs/pytest --suite <name>
```

Parses `test_*.py` with `ast`, finds Helen source strings passed to the
suite's `run_helen`/`run_helen_code`/`write_text` helpers, re-applies the
stdlib import block the helpers prepend (v1.39 removed global builtins), and
writes `tests/programs/pytest/<suite>/<test_name>.helen` plus a provenance
`manifest.json` (source file, test function, line, extracted-source hash).
Only **literal triple-quoted** sources are extracted (f-string / variable
sources are skipped and counted in the manifest).

## M0 Definition of Done

- [x] `cargo build --workspace` clean.
- [x] `scripts/diff.sh` prints reference vs candidate output for a hello-world
      program (candidate comparison verified via stub; real binary lands M12).
- [x] `reference.py` runs in-process with `MockLLMRuntime`; mock-LLM programs
      produce deterministic output.
- [x] `extract_corpus.py` extracts ≥1 real suite (`tests/interpreter`, 18
      sources) into `tests/programs/pytest/` with a provenance manifest.
      Known Tier-A limitation: sources that depend on test-file module-level
      scaffolding (session plumbing, helper functions) are recorded in the
      manifest and surface as `exit_code: 2` in `golden/interpreter.jsonl`
      (4 of 18) until scaffolding is captured in a later milestone.
- [x] `rust-toolchain.toml`, `.gitignore`, workspace `Cargo.toml` committed.

## M13 Tier B (subprocess drop-in swap)

`scripts/diff-tier-b.sh <suite...>` runs the Python pytest suites that invoke
`helen` via subprocess against the Rust binary (PATH prepend, D10).

| Suite | Result | Notes |
|---|---|---|
| language | 100/100 | module import, closures, patterns, pipe, struct methods |
| agent | 170/172 | 2 pre-existing Python-side failures (`TestStdlibWrappers::test_debug*` — `helen.stdlib._debug` returns '' in this env; fails identically against the Python `helen`, not a parity bug) |
| cli | 64/64 | golden output parity |
| ffi | 64/64 + 1 skip | subprocess parts |

Any suite test that imports `helen.*` internals for its *assertions* is Tier C.
