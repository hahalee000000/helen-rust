# Chapter 10: Testing & Debugging

## Why Test?

Even with the help of large language models, code can still go wrong. Testing helps you:
- Confirm that code behaves as expected
- Verify that changes haven't broken existing functionality
- Let others (and your future self) understand what the code is supposed to do

Helen ships with a complete testing framework built in — no extra tools needed.

## Your First Test

### Auto-Discovery Pattern (Simplest)

Any function whose name starts with `test_` is automatically a test function:

```helen
import std.test.*

fn test_addition() {
    assert_equal(2 + 3, 5)
}

fn test_string_concat() {
    assert_equal("hello" + " world", "hello world")
}

run_tests()  // runs every function whose name starts with test_
```

Run the tests:

```bash
helen test calculator_test.helen
```

### Callback Pattern (Great for Organizing Test Suites)

```helen
import std.test.*

test_suite("Calculator", fn() {
    test_case("addition", fn() {
        assert_equal(2 + 3, 5)
    })

    test_case("subtraction", fn() {
        assert_equal(10 - 4, 6)
    })

    test_case("multiplication", fn() {
        assert_equal(3 * 4, 12)
    })
})

run_tests()
```

## Assertion Functions

Assertions are the heart of testing. If an assertion fails, the test reports an error.

| Function | Description | Example |
|----------|-------------|---------|
| `assert_true(condition)` | Assert condition is truthy | `assert_true(5 > 3)` |
| `assert_equal(actual, expected)` | Assert equality | `assert_equal(2+3, 5)` |
| `assert_not_equal(a, b)` | Assert inequality | `assert_not_equal(1, 2)` |
| `assert_contains(container, item)` | Assert container holds item | `assert_contains("hello", "ell")` |
| `assert_throws(fn)` | Assert that calling fn raises | `assert_throws(fn() { throw ... })` |

### Many Uses of assert_contains

```helen
import std.test.*

fn test_contains() {
    // strings
    assert_contains("hello world", "world")

    // lists
    assert_contains([1, 2, 3], 2)

    // dict keys
    assert_contains({"name": "Helen", "version": "1.0"}, "name")
}
```

## Expect Chain API

If you prefer a more fluent style, use the `expect` chain API:

```helen
import std.test.*

fn test_expect() {
    expect(42).toBe(42)
    expect([1, 2, 3]).toContain(2)
    expect("hello").toStartWith("he")
    expect("hello").toEndWith("lo")
    expect({"a": 1}).toHaveProperty("a")
    expect([1, 2, 3]).toHaveLength(3)
}
```

## Testing Agents

### Testing a Simple Agent

```helen
import std.test.*

agent Adder(a: int, b: int) {    // 智能体 = agent
    main { return a + b }        // 主函 = main, 返回 = return
}

fn test_adder() {
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
    tools = ["read_file"]        // 工具 = tools
    main {
        let content = read_file(path)
        return len(content)      // 长度() = len()
    }
}

fn test_file_processor() {
    // prepare test file
    write_file("test_input.txt", "hello world")

    // run test
    let result = FileProcessor("test_input.txt")
    assert_equal(result, 11)

    // cleanup
    delete_file("test_input.txt")
}
```

### Testing Exception Handling

```helen
import std.test.*

agent MaybeFailingAgent(task: str) {
    main {                       // 主函 = main
        if task == "fail" {      // 如果 = if
            throw RuntimeError("intentional failure")  // 抛出 = throw
        }
        return "success: " + task  // 返回 = return
    }
}

fn test_normal_case() {
    let result = MaybeFailingAgent("normal")
    assert_equal(result, "success: normal")
}

fn test_error_case() {
    expect(fn() {
        MaybeFailingAgent("fail")
    }).toThrow()
}
```

## Setup and Teardown Around Tests

```helen
import std.core.*
import std.file.*
import std.io.*
import std.test.*

// runs before each test
before_each(fn() {
    write_file("test_data.txt", "initial content")
})

// runs after each test
after_each(fn() {
    delete_file("test_data.txt")
})

fn test_read_data() {
    let content = read_file("test_data.txt")
    assert_equal(content, "initial content")
}

fn test_modify_data() {
    write_file("test_data.txt", "modified")
    let content = read_file("test_data.txt")
    assert_equal(content, "modified")
}

run_tests()
```

## Running Tests

### Command Line

```bash
# run all tests
helen test filename_test.helen

# run a specific test
helen test filename_test.helen --filter "test_addition"

# skip certain tests
helen test filename_test.helen --skip "test_slow"

# output JSON format (great for CI)
helen test filename_test.helen --json

# show verbose output
helen test filename_test.helen -v
```

## Debugging Techniques

### Output Debug Info with debug()

```helen
import std.core.*
import std.debug.*

fn complex_process(input: list) {
    debug("input", {"input": input, "length": len(input)})

    let result = process(input)

    debug("output", {"result": result})
    return result
}
```

`debug()` writes to stderr, so it won't interfere with normal output.

### Trace Execution with trace

```helen
import std.core.*
import std.debug.*

main {
    trace_on()

    // run the code you want to trace
    let result = complex_function()

    let trace_log = get_trace(50)
    print("execution trace: " + str(trace_log))  // 字符串化() = str(), 打印 = print

    trace_off()
}
```

### REPL Debug Commands

Inside the Helen REPL, use these special commands to debug:

| Command | Description |
|---------|-------------|
| `:last_error` | View the last error's details (call stack, scope) |
| `:llm_log -v` | View detailed LLM call logs (prompts, responses, token usage) |
| `:stats` | View context statistics |
| `:transcript` | View message history |

### Common Debugging Scenarios

| Symptom | First Step |
|---------|------------|
| Agent gives wrong answer | Use `:llm_log -v` to see the prompts the LLM actually received |
| Tool-call infinite loop | Add `debug()` before and after tool calls |
| Variable has wrong value | Add `debug("check", {"var": var})` at key points |
| Agent behaves strangely | Use `trace_on()` to trace execution flow |
| Performance is slow | Time sections with `stopwatch_start()` |

## Testing Tips

### Don't Use `is` Inside Assertions

```helen
// ❌ Wrong: `is` cannot be used inside function arguments
fn test_type() {
    assert_true(x is list)  // Syntax error!
}

// ✅ Right: use isinstance() or type()
fn test_type() {
    assert_true(isinstance(x, "list"))
    // or
    assert_equal(type(x), "list")
}
```

### Separate Pure Logic Tests from Agent Tests

```helen
// ✅ Pure logic test: fast, no LLM dependency
fn test_calculation() {
    assert_equal(calculate_shipping(10, 5), 50)
}

// ⚠️ Agent test: needs LLM API, slower
fn test_agent_answer() {
    let result = my_agent("question")
    assert_true(len(result) > 0)
}
```

It's a good idea to keep LLM-free logic tests and LLM-dependent agent tests in separate files.

## TDD: Test-Driven Development

The TDD (Test-Driven Development) cycle is **Red – Green – Refactor**:

1. **Red**: Write a test first, run it, and confirm it fails (because the feature isn't implemented yet)
2. **Green**: Write the simplest code that makes the test pass
3. **Refactor**: Clean up the code while keeping the tests green

```helen
// Step 1: Red — write the test first
import std.test.*

fn test_is_palindrome() {
    assert_true(is_palindrome("racecar"))
    assert_true(!is_palindrome("hello"))
}

run_tests()  // run → fails! because is_palindrome isn't defined yet

// Step 2: Green — write the minimal implementation
fn is_palindrome(s: str): bool {
    return s == reverse(s)     // 返回 = return, 反转() = reverse()
}

run_tests()  // run → passes!

// Step 3: Refactor — optimize (if needed)
fn is_palindrome(s: str): bool {
    let length_val = len(s)    // 长度() = len()
    for i in range(length_val / 2) {   // 对于 = for, 属于 = in, 范围() = range()
        if substring(s, i, i+1) != substring(s, length_val-1-i, length_val-i) {  // 如果 = if, 子串() = substring()
            return false       // 返回 = return
        }
    }
    return true
}

run_tests()  // still passes!
```

## Chapter Summary

- Functions starting with `test_` become tests automatically; run them with `run_tests()`
- Common assertions: `assert_equal`, `assert_true`, `assert_contains`, `assert_throws`
- The `expect` chain API offers a more fluent writing style
- `before_each` / `after_each` handle setup and teardown around each test
- Use `debug()` to emit debug info, `trace_on()` to trace execution
- In the REPL, use `:last_error` and `:llm_log` to inspect errors and LLM calls
- TDD cycle: Red (write test) → Green (implement) → Refactor (optimize)

## Further Reading

- [[reference/12-testing|Testing Framework and TDD]] - Complete testing API: `test()`, `assert_equal()`, `assert_true()`, `assert_throws()`, expect chains, suites, filtering, JSON output, `--watch` mode
- [[reference/14-observability|AI-Native Observability]] - `assert`, `debug()`, `trace_on()`, LLM audit trail

## Next Chapter

[Chapter 11: Advanced Topics](11-advanced.md) →
