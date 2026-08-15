<!-- helen-rust edition: `helen test` ported in M12; assertion API parity via Tier A/B harness. -->

# Testing Framework

Helen includes a complete testing framework with TDD (Test-Driven Development) workflow support.

## Overview

The Helen testing framework provides:

- **Simple API** — `test_suite` / `test_case` / `test_end_suite`
- **Rich assertions** — `assert_*` + `expect().toBe()` chainable
- **Flexible filtering** — `--only` / `--suite` / `--filter`
- **TDD support** — `--watch` mode
- **CI integration** — `--json` output
- **Skip tests** — `test_case_skip`
- **Hook functions** — `before_each` / `after_each`

## Quick Start

### Create a Test File

```helen
// calculator_test.helen

fn test_add() {
    assert_equal(2 + 3, 5)
}

fn test_subtract() {
    assert_equal(10 - 4, 6)
}

test_suite("Calculator")
test_case("adds numbers", test_add)
test_case("subtracts numbers", test_subtract)
test_end_suite()

run_tests()
```

### Run Tests

```bash
helen test calculator_test.helen
```

## Assertion Functions

| Function | Description |
|------|------|
| `assert_true(condition)` | Asserts condition is true |
| `assert_equal(actual, expected)` | Asserts equality |
| `assert_not_equal(a, b)` | Asserts inequality |
| `assert_throws(fn)` | Asserts an exception is thrown |

## Expect Chain API

```helen
expect(value)
    .toBe(expected)           // Strict equality
    .toEqual(expected)        // Deep equality
    .toContain(item)          // Contains
    .toBeGreaterThan(n)       // Greater than
    .toBeLessThan(n)          // Less than
    .toMatch(pattern)         // Regex match
    .toStartWith(prefix)      // Starts with
    .toEndWith(suffix)        // Ends with
    .toHaveLength(n)          // Length check
    .toBeEmpty()              // Is empty
    .toBeTruthy()             // Is truthy
    .toBeFalsy()              // Is falsy
    .toBeType("str")          // Type check
    .toThrow()                // Throws exception
    .not_.toBe(x)             // Negation
```

## CLI Options

### Filtering

```bash
helen test file.helen --only "test name"      # Single test
helen test file.helen --suite "Suite Name"    # Single suite
helen test file.helen --filter "pattern"      # Regex filter
```

### Output

```bash
helen test file.helen --json                  # JSON output
helen test file.helen --coverage              # Coverage hint
```

### Coverage Measurement

Use `helen coverage` command to measure test coverage:

```bash
# Basic usage
helen coverage test_file.helen

# Include source code coverage
helen coverage test_math.helen --source math_utils.helen

# Generate HTML report
helen coverage tests/ --html coverage_html/

# JSON output
helen coverage tests/ --format json
```

Coverage types:
- **Function coverage**: Which functions were called during tests
- **Line coverage**: Which code lines were executed
- **Branch coverage**: Which if/else branches were taken

Example output:

```
============================================================
HELEN COVERAGE REPORT
============================================================

  Lines:     22/46  (47.8%)
  Functions: 7/7  (100.0%)
  Branches:  6/6  (100.0%)

Files:
  File                                          Lines      Funcs
  ---------------------------------------- ---------- ----------
  calculator.helen                            15/20      3/4    
  calculator_test.helen                       7/7        4/4    

============================================================
```

### Watch Mode

```bash
helen test file.helen --watch                 # Auto-rerun on file changes
helen test file.helen --watch --filter "add"  # Watch + filter
```

## TDD Workflow

### 1. RED — Write a failing test

```helen
fn test_new_feature() {
    assert_equal(feature.do_something(), expected)
}

test_suite("Feature")
test_case("works", test_new_feature)
test_end_suite()

run_tests()
```

### 2. GREEN — Implement the feature

```bash
helen test test.helen --watch
```

Edit code, tests auto-rerun on save.

### 3. REFACTOR — Improve the code

Improve the code while keeping tests passing.

## Hook Functions

```helen
fn setup() {
    // Runs before each test
}

fn teardown() {
    // Runs after each test
}

test_suite("With hooks")
before_each(setup)
after_each(teardown)
test_case("test1", test_something)
test_end_suite()
```

## Skipping Tests

```helen
test_case_skip("not ready", test_wip)
```

## Output Example

```
============================================================
  HELEN TEST RESULTS
============================================================

  Calculator
    ✓ adds numbers (0.1ms)
    ✓ subtracts numbers (0.0ms)

------------------------------------------------------------
  2 passed, 0 failed, 0 skipped (2 total)
  Duration: 0.5ms
============================================================
  ✓ ALL TESTS PASSED
============================================================
```

## Related Documentation

- [Tutorial](../tutorial/01-getting-started.md)
- [CLI Tools](../toolchain/cli.md)
- [Standard Library](../toolchain/stdlib.md)
