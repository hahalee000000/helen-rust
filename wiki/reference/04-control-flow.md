# Tutorial 04: Control Flow

> if / for / while / match / try-catch

## Conditional Branching

### if / else

```helen
import std.core.*
main {
    let score = 85

    if (score >= 90) {
        print("A")
    } else if (score >= 80) {
        print("B")
    } else if (score >= 70) {
        print("C")
    } else {
        print("F")
    }
}
```

**Note**: `if` conditions must be wrapped in parentheses: `if (cond) { ... }`.

### Truthy Rules

```helen
import std.core.*
main {
    if 0 { print("will not execute") }        // 0 → false
    if "" { print("will not execute") }       // empty string → false
    if [] { print("will not execute") }       // empty list → false
    if null { print("will not execute") }     // null → false
    if 1 { print("will execute") }          // nonzero → true
    if "hello" { print("will execute") }    // non-empty string → true
    if [1] { print("will execute") }        // non-empty list → true
}
```

### Short-circuit Evaluation (v1.10)

The `&&` and `||` operators support **short-circuit evaluation**, avoiding unnecessary computation:

First, let's define some example functions:

```helen
fn expensiveFunction(): str {
    // Simulate an expensive operation
    return "result"
}

fn getUser(): map? {
    return {"name": "Alice"}
}

fn isValid(): bool {
    return true
}

fn processData(): str {
    return "processed"
}

fn loadConfig(): map? {
    return null
}

fn defaultConfig(): map {
    return {"debug": false}
}

fn createDefaultUser(): map {
    return {"name": "Guest"}
}
```

#### && Short-circuit

```helen
main {
    // If the left side is false, the right side is not evaluated
    let result = false && expensiveFunction()  // expensiveFunction() is not called

    // Practical usage: safe access
    let user = getUser()
    let name = user != null && user.getName()  // If user is null, getName() is not called

    // Conditional execution
    let valid = isValid() && processData()  // Only process when valid
}
```

#### || Short-circuit

```helen
main {
    // If the left side is true, the right side is not evaluated
    let result = true || expensiveFunction()  // expensiveFunction() is not called

    // Practical usage: default values
    let config = loadConfig() || defaultConfig()  // Only use default when loading fails

    let user = getUser() || createDefaultUser()  // If fetching fails, create a default user
}
```

#### Precedence

```helen
main {
    // && has higher precedence than ||
    let result = a || b && c  // Equivalent to a || (b && c)

    // Use parentheses for clarity
    let result2 = (a || b) && c  // Explicit grouping
}
```

#### Practical Examples

```helen
import std.core.*
import std.dict.*
main {
    // Safe list access
    let items = [1, 2, 3]
    let first = len(items) > 0 && items[0]  // Avoid empty list errors

    // Cache check
    let cached = cache.get(key)
    let result = cached != null || computeExpensive()

    // Permission check
    let canAccess = isLoggedIn() && hasPermission("admin")
}
```

## Loops

### for ... in

```helen
import std.core.*
main {
    for item in ["apple", "banana", "cherry"] {
        print(item)
    }
    // apple
    // banana
    // cherry
}
```

### Iteration with Index

```helen
import std.core.*
main {
    let fruits = ["apple", "banana", "cherry"]
    for fruit in fruits {
        print(fruit)
    }
}
```

### range Iteration

```helen
import std.core.*
main {
    for i in range(5) {
        print(i)    // 0, 1, 2, 3, 4
    }

    for i in range(1, 6) {
        print(i)    // 1, 2, 3, 4, 5
    }

    for i in range(0, 10, 2) {
        print(i)    // 0, 2, 4, 6, 8
    }
}
```

### while

```helen
import std.core.*
main {
    let count = 0
    while (count < 5) {
        print(count)
        count = count + 1
    }
}
```

**Note**: `while` conditions must be wrapped in parentheses: `while (cond) { ... }`. Use `count = count + 1` (assignment) rather than `let count = count + 1` (new declaration), as the latter creates a local variable and causes an infinite loop.

### break / continue

```helen
import std.core.*
main {
    for i in range(10) {
        if i == 3 {
            continue    // Skip 3
        }
        if i == 7 {
            break       // Exit at 7
        }
        print(i)
    }
    // 0, 1, 2, 4, 5, 6
}
```

## Pattern Matching

```helen
import std.core.*
main {
    let status = "success"

    match status {
        case "success" { print("OK") }
        case "error" { print("Failed") }
        default { print("Unknown") }
    }
}
```

**Note**: `case` and `default` are followed by `{ }` blocks, not `:`.

### Number Matching

```helen
import std.core.*
main {
    let code = 404

    match code {
        case 200 { print("OK") }
        case 404 { print("Not Found") }
        case 500 { print("Server Error") }
        default { print("Other") }
    }
}
```

### Range Matching

Use the `..` operator to match numeric ranges (inclusive):

```helen
import std.core.*
main {
    let score = 85

    match score {
        case 90..100 { print("A") }
        case 80..89 { print("B") }
        case 70..79 { print("C") }
        case 60..69 { print("D") }
        default { print("F") }
    }
    // Output: B
}
```

**Note**: The range operator `..` does not conflict with floating-point numbers. `1..10` is parsed as a range; `1.5` is parsed as a float.

### Guard Conditions

Use `if` to add additional conditional checks:

```helen
import std.core.*
main {
    let x = 25

    match x {
        case 21..30 if x == 25 { print("exactly 25") }
        case 21..30 { print("other in range") }
        default { print("out of range") }
    }
    // Output: exactly 25
}
```

Guard conditions are evaluated after the range match; both must be satisfied for the corresponding case block to execute.

## Exception Handling

### throw — Raising Exceptions

Use the `throw` statement to actively raise exceptions of predefined types:

```helen
import std.core.*
main {
    // With a message — caught by try-catch
    try {
        throw RuntimeError("something went wrong")
    } catch RuntimeError err {
        print("Caught: " + err.message)
    }

    // Without a message (uses default message)
    try {
        throw LLMError
    } catch LLMError err {
        print("Caught LLM error")
    }
}
```

Using throw in functions for parameter validation:

```helen
import std.core.*
fn validate_age(age: int) {
    if (age < 0) {
        throw RuntimeError("age cannot be negative")
    }
    if (age > 150) {
        throw RuntimeError("age seems unrealistic")
    }
    return age
}

main {
    try {
        let result = validate_age(-5)
    } catch RuntimeError err {
        print("Validation failed: " + err.message)
    }
}
```

**Predefined exception types**:

| Type | Description |
|------|-------------|
| `RuntimeError` | Runtime error |
| `LLMError` | LLM-related error (base class) |
| `TimeoutError` | LLM call timeout (inherits LLMError) |
| `ModelError` | Model unavailable or quota exceeded (inherits LLMError) |
| `ToolError` | Tool call failure |
| `AggregateError` | Multiple concurrent agent tasks failed (spawn scenarios) |

**Exception inheritance**: `catch LLMError` also catches `TimeoutError` and `ModelError`.

### try / catch

```helen
import std.core.*
main {
    try {
        let result = validate_age(-5)
        print(result)
    } catch RuntimeError err {
        print("Runtime error: " + err.message)
    } catch TimeoutError err {
        print("Timeout: " + err.message)
    }
}
```

**Syntax**: `catch Type varname { ... }` — the variable name directly follows the type name; no `as` keyword needed.

### catch-all

```helen
import std.core.*
main {
    try {
        risky_operation()
    } catch {
        // Catches any unmatched error
        print("Something went wrong")
    }
}
```

### finally

```helen
import std.core.*
main {
    try {
        open_file()
        process_data()
    } catch RuntimeError err {
        print("Error: " + err.message)
    } finally {
        close_file()    // Always executes
    }
}
```

### catch Ordering

```helen
main {
    // ✅ Specific types first, catch-all last
    try {
        ...
    } catch TimeoutError err {
        ...
    } catch LLMError err {
        ...
    } catch RuntimeError err {
        ...
    } catch {
        ...
    }

    // ❌ catch-all must be last
    try {
        ...
    } catch {
        ...
    } catch TimeoutError err {    // E0343
        ...
    }
}
```

### Complete Example: Custom Validation

```helen
import std.core.*
fn divide(a: int, b: int): int {
    if (b == 0) {
        throw RuntimeError("division by zero")
    }
    return a / b
}

main {
    try {
        let result = divide(10, 0)
        print("Result: " + str(result))
    } catch RuntimeError err {
        print("Cannot divide: " + err.message)
    }
    // Output: Cannot divide: division by zero
}
```

### Catching Standard Library Exceptions (v1.9+)

Python exceptions thrown by stdlib functions (`TypeError`, `ValueError`, `FileNotFoundError`, etc.) are automatically wrapped as `RuntimeError` and can be caught with try-catch:

```helen
import std.core.*
import std.str.*
main {
    try {
        let x = len(42)        // Python TypeError
    } catch RuntimeError err {
        print(err.message)     // "Python TypeError: object of type 'int' has no len()"
    }

    try {
        let data = read_file("/nonexistent/path")  // Python FileNotFoundError
    } catch RuntimeError err {
        // Distinguish types via the err.message prefix
        if (startswith(err.message, "Python FileNotFoundError")) {
            print("File not found")
        }
    }
}
```

## Comprehensive Example: FizzBuzz

```helen
import std.core.*
main {
    for i in range(1, 101) {
        if (i % 15 == 0) {
            print("FizzBuzz")
        } else if (i % 3 == 0) {
            print("Fizz")
        } else if (i % 5 == 0) {
            print("Buzz")
        } else {
            print(i)
        }
    }
}
```

**Note**: Executable statements such as `for`/`if` must be wrapped inside a `main { }` block. Only declarations (`fn`, `agent`, `const`, `import`, `alias`, `shared`, `protocol`, `impl`) are allowed at the module top level.

## Exercises

1. Use a for loop to calculate the sum of 1 to 100
2. Use a while loop to implement binary search
3. Write a function that uses match to determine the day of the week (1-7)
4. Write try-catch to handle a division-by-zero error

---
