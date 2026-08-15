# Helen Tutorial Test System

## Overview

A complete test system that extracts code blocks from tutorial documentation and validates them. Achieves dual goals:
1. **Tests Helen language implementation** — validates syntax parsing and semantic analysis
2. **Validates tutorial correctness** — identifies errors and inconsistencies

## Quick Start

```bash
# Generate test files
python wiki/reference/generate_tests.py

# Run tests
bash wiki/reference/run_tests.sh
```

## Test Results

| Metric | Count | Percentage |
|--------|-------|------------|
| Passed | 98 | 41.7% |
| Should fail | 10 | 4.3% |
| Failed | 107 | 45.5% |
| Skipped | 19 | 8.1% |
| Unexpected OK | 1 | 0.4% |
| **Total** | **235** | **100%** |

## Files

- `wiki/reference/generate_tests.py` — Test generator (273 lines)
- `wiki/reference/run_tests.sh` — Test runner (80 lines)
- `wiki/reference/TUTORIAL_TEST_REPORT.md` — Detailed report
- `wiki/reference/tests/` — 235 generated test files in 17 subdirectories
- `wiki/reference/tests/README.md` — Test system documentation

## Tutorial Issues Found

### 1. Return Type Syntax (Critical)

Tutorial uses `->` but Helen uses `:`:

```helen
// Wrong:
fn greet(name: string) -> string { ... }

// Correct:
fn greet(name: string): string { ... }
```

**Impact:** ~15 test failures

### 2. Undefined References (High)

Fragments reference undefined functions like `expensiveFunction()`, `fix_code()`, `stream_print()`.

**Impact:** ~40 test failures

### 3. Duplicate Declarations

Context accumulation causes conflicts when blocks redefine variables.

**Impact:** ~10 test failures

### 4. Type Checking Gap

Tutorial claims type mismatches should fail, but Helen does not enforce strict checking.

**Impact:** 1 unexpected pass

## Recommendations

### Immediate

1. Fix `->` to `:` in tutorials — ~15 tests
2. Define placeholder functions — ~40 tests
3. Improve context handling — ~10 tests

### Medium-term

4. Create import stubs
5. Mark syntax templates

### Long-term

6. Execution testing with `helen`
7. Output validation

## How It Works

### Generator

1. **Extract** — Extract `helen` code blocks from markdown
2. **Clean** — Remove output annotations (`// -> 42`, `// Output: B`)
3. **Classify** — Determine type (complete/fragment/error/skip)
4. **Context** — Track declarations, prepend to fragments
5. **Generate** — Create `.helen` test files with metadata

### Runner

1. **Iterate** — Find all generated `.helen` files
2. **Check metadata** — Read `@should_fail` and `@skip` markers
3. **Execute** — Run `helen check <file>`
4. **Interpret** — Judge results based on metadata
5. **Report** — Output colored summary statistics

---

Created: 2026-07-09
Total test files: 235
Total tutorial files: 17
