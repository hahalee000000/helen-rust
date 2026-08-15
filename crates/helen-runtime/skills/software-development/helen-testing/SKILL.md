---
name: helen-testing
description: "Helen Testing Framework Guide — TDD Workflow, Assertion API, CLI Options, Agent Testing, v1.10 Exception Handling"
version: 1.1.0
author: Helen Team
license: MIT
tags: [helen, testing, tdd, assertions, cli, agent-testing, exception-handling, v1.10]
---
<!-- helen-rust edition: `helen test` (M12 CLI port) + cargo tests for the port itself. Assertion API parity via Tier A/B subprocess harness. -->


# Helen Testing Framework

## Overview

Helen includes a complete testing framework with TDD development workflow support. v1.10 enhanced exception handling and agent testing capabilities.

## Quick Start

### 1. Create a Test File

**Approach 1: Callback style (recommended)**

```helen
// calculator_test.helen

import std.test.*
test_suite("Calculator", fn() {
    test_case("adds numbers", fn() {
        assert_equal(2 + 3, 5)
    })
    test_case("subtracts numbers", fn() {
        assert_equal(10 - 4, 6)
    })
})

run_tests()
```

**Approach 2: Auto-discovery (simplest)**

```helen
// calculator_test.helen

import std.test.*
fn test_add() {
    assert_equal(2 + 3, 5)
}

fn test_subtract() {
    assert_equal(10 - 4, 6)
}

run_tests()
```

### 2. Run Tests

```bash
helen test calculator_test.helen
```

## Assertion API

### Basic Assertions

| Function | Description |
|------|------|
| `assert_true(condition)` | Asserts condition is true |
| `assert_equal(actual, expected)` | Asserts equality |
| `assert_not_equal(a, b)` | Asserts inequality |
| `assert_contains(haystack, needle)` | Asserts container contains element |
| `assert_throws(fn)` | Asserts an exception is thrown |

**assert_contains example:**

```helen
import std.test.*
fn test_contains() {
    // String
    assert_contains("hello world", "world")
    
    // Array
    assert_contains([1, 2, 3], 2)
    
    // Object
    assert_contains({"name": "Helen", "version": "1.0"}, "name")
}
```

### Expect Chain API

```helen
import std.test.*
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
    .toHaveProperty(key)      // Property exists
    .toThrow()                // Throws exception
```

**Example:**

```helen
import std.test.*
fn test_expect_api() {
    expect(42).toBe(42)
    expect([1, 2, 3]).toContain(2)
    expect("hello").toStartWith("he")
    expect({"a": 1}).toHaveProperty("a")
}
```

### Exception Testing (v1.10 enhanced)

```helen
import std.test.*
fn test_exceptions() {
    // Basic exception testing
    assert_throws(fn() {
        throw RuntimeError("error")
    })
    
    // Specific exception type (v1.10)
    expect(fn() {
        throw LLMError("API failed")
    }).toThrow()
    
    // Check exception message
    try {
        throw RuntimeError("specific error")
    } catch RuntimeError e {
        assert_contains(e.message, "specific")
    }
}
```

### v1.10 Exception Hierarchy

Helen v1.10 enhanced exception handling — all Python stdlib exceptions are wrapped as `RuntimeError`:

```
AnyError
├── LLMError
│   ├── TimeoutError
│   └── ModelError
├── ToolError
├── RuntimeError          // Wraps all stdlib Python exceptions
│   ├── ValueError
│   ├── TypeError
│   ├── KeyError
│   └── ...
├── AssertionError
└── AggregateError        // Aggregated concurrent task errors
```

**Exception testing examples:**

```helen
import std.core.*
import std.test.*
fn test_runtime_errors() {
    // stdlib exceptions are wrapped as RuntimeError
    expect(fn() {
        let x = int("not a number")  // Throws RuntimeError
    }).toThrow()
    
    // Catch and inspect
    try {
        let arr = [1, 2, 3]
        let x = arr[10]  // Index out of bounds
    } catch RuntimeError e {
        assert_contains(e.message, "index")
    }
}
```

## Testing Agents

### Testing a Simple Agent

```helen
import std.test.*
agent Adder(a: int, b: int) {
    description "Add two numbers"
    
    main {
        return a + b
    }
}

fn test_adder_agent() {
    let result = Adder(2, 3)
    assert_equal(result, 5)
}
```

### Testing an Agent with Tools

```helen
import std.core.*
import std.file.*
import std.io.*
import std.test.*
agent FileProcessor(path: str) {
    description "Process a file"
    tools = ["read_file"]
    
    main {
        let content = read_file(path)
        return len(content)
    }
}

fn test_file_processor() {
    // Prepare test file
    write_file("test_input.txt", "hello world")
    
    // Test
    let result = FileProcessor("test_input.txt")
    assert_equal(result, 11)
    
    // Cleanup
    delete_file("test_input.txt")
}
```

### Testing Agent Scope Isolation (v1.10)

```helen
import std.test.*
shared let shared_counter = 0
const MAX_VALUE = 100

agent CounterAgent {
    description "Test scope isolation"
    
    main {
        // ✅ const is visible
        assert_true(MAX_VALUE > 0)
        
        // ✅ shared let is visible
        shared_counter = shared_counter + 1
        return shared_counter
    }
}

fn test_agent_scope_isolation() {
    shared_counter = 0  // Reset
    
    let r1 = CounterAgent()
    assert_equal(r1, 1)
    
    let r2 = CounterAgent()
    assert_equal(r2, 2)
    
    assert_equal(shared_counter, 2)
}
```

### Testing Concurrent Agents

```helen
import std.core.*
import std.test.*
import std.time.*
agent SlowWorker(id: str, delay: int) {
    description "Worker with delay"
    
    main {
        sleep(delay)
        return "done: " + id
    }
}

fn test_concurrent_agents() {
    let start = timestamp()
    
    // v1.18: spawn + Channel
    let m1 = spawn SlowWorker("A", 1)
    let m2 = spawn SlowWorker("B", 1)
    let m3 = spawn SlowWorker("C", 1)
    
    let r1 = m1.receive()
    let r2 = m2.receive()
    let r3 = m3.receive()
    let results = [r1, r2, r3]
    
    let elapsed = timestamp() - start
    
    // Should execute concurrently, total time ~1 second
    assert_true(elapsed < 2)
    assert_equal(len(results), 3)
}
```

### Testing Agent Error Handling

```helen
import std.test.*
agent FailingAgent(task: str) {
    description "Agent that may fail"
    
    main {
        if task == "fail" {
            throw RuntimeError("Intentional failure")
        }
        return "success: " + task
    }
}

fn test_agent_error_handling() {
    // Normal case
    let result = FailingAgent("ok")
    assert_equal(result, "success: ok")
    
    // Error case
    expect(fn() {
        FailingAgent("fail")
    }).toThrow()
}
```

## Test Suite Organization

### Using before_each / after_each

```helen
import std.core.*
import std.file.*
import std.io.*
import std.test.*
before_each(fn() {
    // Runs before each test
    write_file("test_data.txt", "initial")
})

after_each(fn() {
    // Runs after each test
    delete_file("test_data.txt")
})

fn test_read_data() {
    let content = read_file("test_data.txt")
    assert_equal(content, "initial")
}

fn test_modify_data() {
    write_file("test_data.txt", "modified")
    let content = read_file("test_data.txt")
    assert_equal(content, "modified")
}
```

### Nested Test Suites

```helen
import std.test.*
test_suite("Math", fn() {
    test_suite("Addition", fn() {
        test_case("positive numbers", fn() {
            assert_equal(2 + 3, 5)
        })
        test_case("negative numbers", fn() {
            assert_equal(-1 + -2, -3)
        })
    })
    
    test_suite("Multiplication", fn() {
        test_case("positive numbers", fn() {
            assert_equal(2 * 3, 6)
        })
        test_case("with zero", fn() {
            assert_equal(5 * 0, 0)
        })
    })
})

run_tests()
```

## CLI Options

### Basic Usage

```bash
# Run all tests
helen test my_test.helen

# Run a specific test suite
helen test my_test.helen --suite "Math"

# Run a specific test case
helen test my_test.helen --only "adds numbers"

# JSON output
helen test my_test.helen --json

# Verbose output
helen test my_test.helen --verbose

# Watch mode (auto-rerun on file changes)
helen test my_test.helen --watch
```

### Filtering Tests

```bash
# Only run matching tests
helen test my_test.helen --only "test_add"

# Exclude matching tests
helen test my_test.helen --skip "slow_tests"

# Combined filtering
helen test my_test.helen --suite "Math" --only "addition"
```

## Testing Best Practices

### 1. Test Naming Conventions

```helen
// ✅ Clear test names
fn test_add_positive_numbers() { ... }
fn test_add_negative_numbers() { ... }
fn test_add_zero() { ... }

// ❌ Vague names
fn test_add() { ... }
fn test1() { ... }
```

### 2. Independent Tests

```helen
// ✅ Each test is independent
import std.test.*
fn test_feature_a() {
    let data = setup_data()
    assert_equal(process(data), expected_a)
}

fn test_feature_b() {
    let data = setup_data()  // Re-setup
    assert_equal(process(data), expected_b)
}

// ❌ Inter-test dependencies
fn test_feature_a() {
    global_data = setup_data()
    assert_equal(process(global_data), expected_a)
}

fn test_feature_b() {
    // Depends on test_feature_a's result
    assert_equal(process(global_data), expected_b)
}
```

### 3. Test Boundary Conditions

```helen
import std.test.*
fn test_edge_cases() {
    // Empty input
    assert_equal(process([]), [])
    
    // Single element
    assert_equal(process([1]), [1])
    
    // Maximum value
    assert_equal(process([MAX_INT]), [MAX_INT])
    
    // Boundary values
    assert_equal(process([0]), [0])
    assert_equal(process([-1]), [-1])
}
```

### 4. Using Mock Data

```helen
import std.test.*
fn test_with_mock_data() {
    // Prepare mock data
    let mock_user = {
        "id": 1,
        "name": "Test User",
        "email": "test@example.com"
    }
    
    // Test
    let result = format_user(mock_user)
    assert_equal(result, "Test User (test@example.com)")
}
```

## Continuous Integration

### GitHub Actions Example

```yaml
name: Helen Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install Helen
        run: pip install -e .
      - name: Run tests
        run: helen test tests/*.helen --json > results.json
      - name: Check results
        run: |
          if [ $(jq '.failed' results.json) -gt 0 ]; then
            exit 1
          fi
```

### Test Coverage

The test coverage dimension of `helen quality` (weight 15%) uses file-location heuristics for scoring:

| Strategy | Score | Condition |
|------|:----:|------|
| `// @test-location:` annotation | **8.0** | Annotation in source file points to existing test file |
| Sibling test file | **8.0** | `<name>_test.helen` or `test_<name>.helen` |
| Parent `tests/` match | **7.0** | `*.py` in parent `tests/` with matching filename |
| Sibling `tests/` directory | **6.0** | `tests/` directory next to source file contains any tests |
| No tests | **2.0** | No tests found |

**Tip for agent programs**: Integration test filenames often don't match the source file stem, so they tend to fall into the 6.0 tier. Use `// @test-location:` annotations to explicitly declare test locations and get 8.0:

```helen
// @test-location: tests/integration/test_my_agent.py

agent MyAgent {
    description "Example agent"
    main { llm act "Execute task" }
}
```

### Code Coverage Measurement

Helen provides built-in code coverage measurement to track which code is executed during tests.

#### CLI Usage

```bash
# Run tests with coverage measurement
helen coverage test_file.helen

# Measure coverage for a directory
helen coverage tests/

# Include source code coverage
helen coverage test_math.helen --source math_utils.helen

# Generate HTML report
helen coverage tests/ --html coverage_html/

# JSON output
helen coverage tests/ --format json > coverage.json
```

#### Coverage Types

| Type | Description |
|------|-------------|
| **Function Coverage** | Which functions were called during tests |
| **Line Coverage** | Which code lines were executed |
| **Branch Coverage** | Which if/else branches were taken |

#### Programmatic API

```helen
import std.core.*
import std.debug.*
main {
    // Enable coverage tracking
    coverage_on()
    
    // Run code to measure
    let result = tested_function()
    
    // Get coverage summary (one-line)
    let summary = coverage_summary()
    // Output: "Coverage: Lines 85.0% (17/20) | Functions 100.0% (4/4) | ..."
    
    // Get detailed report
    let report = coverage_report("text")  // or "json" or "html"
    print(report)
    
    // Disable coverage
    coverage_off()
}
```

#### Example Output

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

#### CI Integration

```yaml
# GitHub Actions example
- name: Run tests with coverage
  run: |
    helen coverage tests/ --format json > coverage.json
    COVERED=$(jq '.summary.functions.covered' coverage.json)
    TOTAL=$(jq '.summary.functions.total' coverage.json)
    PERCENT=$((COVERED * 100 / TOTAL))
    if [ $PERCENT -lt 80 ]; then
      echo "Coverage $PERCENT% is below 80% threshold"
      exit 1
    fi
```

#### Design Features

- **Zero overhead by default**: Coverage tracking is disabled unless explicitly enabled
- **Minimal logging**: Only records file/line/function names, never parameter values
- **Resource-bounded**: 1M counter limit prevents memory exhaustion
- **Thread-safe**: Uses locks to protect counter updates

## Debugging Tests

> **Core mental model**: `pytest` tells you "whether something is broken", Helen's built-in tools (`debug`/`trace_on`/`:last_error`/`:llm_log`) tell you "where it's broken and why". Use both together when developing Helen applications.

### When to Use Which Tool

| Scenario | What to Use | Why |
|------|--------|--------|
| Verify no regressions after code changes | `pytest` | Automated regression testing |
| Verify stdlib function behavior | `pytest` (Python unit tests) | Can assert directly at Python layer |
| Verify new agent behavior | `helen <agent.helen>` + `:llm_log` | Needs real LLM call chain |
| Program throws an error | REPL + `:last_error` | Structured error snapshot (call_stack/scope) |
| Trace interpreter execution flow | `trace_on()` + `get_trace()` | Reveals what Python unit tests can't see |
| Unexpected variable values | `debug()` at key checkpoints | Structured variable state output |
| LLM behaving oddly | `:llm_log -v` | See actual prompt/response |
| Performance issues | `context_stats()` / `stopwatch_*()` | Context usage + timing |

### Using debug() Function

Place checkpoints at key positions to output structured debug info to stderr:

```helen
import std.core.*
import std.debug.*
import std.test.*
fn test_complex_logic() {
    let input = [1, 2, 3, 4, 5]
    
    // Entry checkpoint: log input state
    debug("test_complex_logic input", {"input": input, "len": len(input)})
    
    let result = process(input)
    
    // Exit checkpoint: log output state
    debug("test_complex_logic output", {"result": result})
    
    assert_equal(result, [2, 4, 6, 8, 10])
}
```

**Best practices for placing debug in agents**:

```helen
import std.core.*
import std.debug.*
agent MyAgent(task: str) {
    main {
        // 1. Entry checkpoint: log parameters
        debug("MyAgent started", {"task": task})
        
        // 2. Pre-condition assertion
        assert len(task) > 0, "task must not be empty"
        
        // 3. LLM call
        let result = llm act task
        debug("LLM returned", {"len": len(result)})
        
        // 4. Result validation
        assert len(result) > 0, "LLM returned empty"
        
        // 5. Exit checkpoint
        debug("MyAgent completed", {})
        return result
    }
}
```

### Using Trace

Wrap suspicious code blocks with `trace_on()` / `trace_off()` to trace execution:

```helen
import std.core.*
import std.debug.*
import std.test.*
fn test_with_trace() {
    trace_on()
    
    let result = complex_function()
    
    let trace_log = get_trace(50)
    print("Execution trace: " + str(trace_log))
    
    trace_off()
    
    assert_true(result > 0)
}
```

### Using :last_error Structured Errors

When a program errors, enter REPL and use `:last_error` to see the structured error snapshot:

```bash
$ helen myagent.helen
Error: ...

$ helen repl
> :last_error
{
  "error": {"type": "RuntimeError", "message": "...", "location": "..."},
  "call_stack": [{"function": "main", "args": {...}}],
  "scope": {"task": "...", "result": "..."},
  "trace": [...]
}
```

Analyze `call_stack` to locate which function failed, analyze `scope` to check if variable values match expectations.

### Using :llm_log to Inspect LLM Calls

When an agent behaves oddly (wrong answers, abnormal tool calls):

```bash
$ helen repl
> :llm_log -v
```

See the actual prompt the LLM received, the response, token usage, and call duration. Common diagnoses:

- **Wrong prompt** → Check variable substitution in the prompt template
- **Truncated response** → Check `max_tokens` / `timeout` configuration
- **Abnormal tool_calls** → Check `tools` registration and schema
- **Call failed** → Check `error` field and `duration_ms`

### Common Debugging Scenarios Quick Reference

| Symptom | First Step |
|------|--------|
| Agent gives wrong answer | `:llm_log -v` to see actual LLM calls |
| Tool call infinite loop | `debug()` before and after each tool call |
| Context unexpectedly compressed | `context_stats()` to check usage |
| Spawned child agent misbehaves | Add `debug("spawned", {...})` at child agent entry |
| Closure captures wrong value | `debug("captured", {"x": x})` inside closure body |
| Multi-agent data corruption | Add `debug()` on both send and receive ends to compare |
| LLM streaming interrupted | `on_chunk fn(c) { debug("chunk", c) }` |
| Slow performance | `stopwatch_start()` + `debug("elapsed", {...})` |

> **Full cookbook in the `debugging` skill §5**: includes a decision tree + detailed code examples for 10 scenarios.

## Related Skills

- **test-driven-development** — TDD methodology in depth
- **helen-agent-patterns** — Agent design patterns
- **debugging** — Debugging methodology



## Testing Pitfalls and Notes

### `is` Type Check Cannot Be Used Inside Function Arguments

```helen
// ❌ Wrong: `is` operator cannot be used inside function call arguments
import std.test.*
fn test_type_check() {
    assert_true(x is list)      // Parse error!
    assert_true(x is str)       // Parse error!
}

// ✅ Correct: use isinstance() or type() function
fn test_type_check() {
    assert_true(isinstance(x, "list"))   // isinstance checks type
    
    // Or use the type() function
    let t = type(x)
    assert_equal(t, "list")
}
```

### Agent Tests Require LLM Calls

When testing agents that contain `llm act`, the tests will make actual LLM API calls. Recommendations:
- Separate pure logic functions (no LLM dependency) from agent tests
- Pure logic tests can run quickly; mark agent tests as integration tests
- Use `--filter` or `--skip` to run selectively

```helen
// Pure logic function — fast unit test
import std.core.*
import std.dict.*
import std.test.*
fn test_get_config() {
    let config = get_default_config()
    assert_equal(len(config), 4)
}

// Agent test — requires LLM, slower
fn test_agent_returns_valid_structure() {
    let result = MyAgent("test input")
    assert_true(has_key(result, "output"))
}
```
