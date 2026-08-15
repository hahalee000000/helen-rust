# Chapter 6: Control Flow

## Conditional Branching

### if / else

```helen
import std.core.*

main {                       // 主函
    let score = 85

    if score >= 90 {         // 如果
        print("Excellent")   // 打印
    } else if score >= 80 {  // 否则 如果
        print("Good")
    } else if score >= 60 {  // 否则 如果
        print("Pass")
    } else {                 // 否则
        print("Fail")
    }
}
```

> **Keywords**: `if` (如果), `else` (否则). Both forms work identically — pick whichever reads better for you.

### Simple condition

```helen
main {
    let age = 20
    if age >= 18 {
        print("Adult")
    }
}
```

> **Note**: Helen's `if` does **not** require a `then` keyword — just go straight to `{`.

## Loops

### for loop

Iterating over a list:

```helen
import std.core.*

main {
    let fruits = ["apple", "banana", "orange"]

    for fruit in fruits {         // 对于 水果 属于 fruits
        print(fruit)              // 打印
    }
}
```

Creating a loop with a range:

```helen
main {
    // From 0 to 4 (5 excluded)
    for i in range(5) {           // 对于 i 属于 范围(5)
        print(i)                  // 0, 1, 2, 3, 4
    }

    // From 1 to 10 (11 excluded)
    for i in range(1, 11) {       // 对于 i 属于 范围(1, 11)
        print(i)                  // 1, 2, 3, ..., 10
    }
}
```

### while loop

```helen
import std.core.*

main {
    let count = 0
    while count < 5 {              // 当 计数 < 5
        print("Iteration " + str(count))   // 打印, 字符串化
        count = count + 1
    }
}
```

### break and continue

```helen
main {
    // break: exit the loop early
    for i in range(10) {           // 对于 i 属于 范围(10)
        if i == 5 {                // 如果
            break                  // 中断 — exit the loop
        }
        print(i)                   // prints 0, 1, 2, 3, 4
    }

    // continue: skip this iteration, move to the next
    for i in range(5) {            // 对于 i 属于 范围(5)
        if i == 2 {                // 如果
            continue               // 继续 — skip this iteration
        }
        print(i)                   // prints 0, 1, 3, 4
    }
}
```

## Pattern Matching

Pattern matching (`match`) is a more powerful alternative to `if-else` branching:

### Basic match

```helen
import std.core.*

main {
    let status_code = 200

    match status_code {                   // 匹配 状态码
        case 200 { print("Success") }     // 情况 200
        case 404 { print("Not Found") }   // 情况 404
        case 500 { print("Server Error") } // 情况 500
        default { print("Unknown") }      // 默认
    }
}
```

### Range matching

```helen
main {
    let score = 85

    match score {                         // 匹配 分数
        case 90..100 { print("A") }       // 情况 90..100
        case 80..89  { print("B") }
        case 70..79  { print("C") }
        case 60..69  { print("D") }
        default      { print("F") }       // 默认
    }
}
```

> `..` denotes an inclusive range. `90..100` includes both 90 and 100.

### Match with guards

```helen
main {
    let number = 42

    match number {                              // 匹配 数字
        case n if n > 0 { print("Positive: {{n}}") }     // 情况 n 如果 n > 0
        case 0          { print("Zero") }
        case n if n < 0 { print("Negative: {{n}}") }
    }
}
```

### Wildcard

```helen
main {
    let value = "hello"

    match value {                                    // 匹配 值
        case 1 { print("The number 1") }             // 情况 1
        case s { print("Other value: " + s) }        // s binds to the matched value
    }
}
```

### match vs if-else

| Feature              | `if-else`              | `match`                  |
|----------------------|------------------------|--------------------------|
| Test condition       | Any boolean expression | Value matching           |
| Range matching       | Not supported          | `90..100`                |
| Binding variables    | Not supported          | `case n if n > 0`        |
| Best suited for      | Simple conditions      | Multi-way branching      |

## Exception Handling

### try / catch / finally

```helen
import std.core.*

main {
    try {                                          // 尝试
        let content = read_file("missing.txt")     // 读文件
        print(content)
    } catch RuntimeError err {                     // 捕获 RuntimeError 错误
        print("Error: " + err.message)
    } finally {                                    // 最终
        print("This always runs")
    }
}
```

### Catching different exception types

```helen
import std.core.*

main {
    try {
        // An operation that may fail
        let result = risky_operation()
    } catch TimeoutError err {                     // 捕获 TimeoutError 错误
        print("Timed out: " + err.message)
    } catch RuntimeError err {                     // 捕获 RuntimeError 错误
        print("Runtime error: " + err.message)
    } finally {
        cleanup_resources()                        // 清理资源
    }
}
```

### Throwing exceptions

```helen
main {
    let age = -5

    if age < 0 {
        throw RuntimeError("Age cannot be negative")   // 抛出 RuntimeError
    }
}
```

### Assertions

Assertions check conditions during development — failure raises an error immediately:

```helen
import std.core.*

main {
    let quantity = 10
    assert quantity > 0                        // 断言 — raises on failure
    assert quantity > 0, "Quantity must be positive"   // with custom message
}
```

### Helen's exception hierarchy

```
AnyError
├── LLMError           // LLM-related errors
│   ├── TimeoutError    // Timeout
│   └── ModelError      // Model error
├── ToolError           // Tool invocation error
├── RuntimeError        // Runtime error (wraps Python exceptions)
├── AssertionError      // Assertion failure
└── AggregateError      // Aggregate error from concurrent tasks
```

> **Tip**: Helen automatically wraps Python exceptions as `RuntimeError`. You can identify the original exception type via the prefix in `err.message` (e.g., `"Python TypeError: ..."`).

## Short-Circuit Evaluation

Helen's `&&` and `||` (equivalently `且` and `或`) short-circuit:

```helen
main {
    // && — if the left side is false, the right side is not evaluated
    let safe = user != null && user.get_name()

    // || — if the left side is true, the right side is not evaluated
    let config = load_config() || default_config()   // 加载配置 || 默认配置
}
```

## Chapter Summary

- `if` / `else if` / `else` (如果 / 否则) for conditional branching
- `for` / `in` (对于 / 属于) iterate over lists and ranges; `while` (当) for conditional loops
- `break` (中断) exits a loop; `continue` (继续) skips the current iteration
- `match` / `case` / `default` (匹配 / 情况 / 默认) for pattern matching — supports ranges, guards, and variable binding
- `try` / `catch` / `finally` (尝试 / 捕获 / 最终) handle exceptions; `throw` (抛出) raises them
- `assert` (断言) for runtime checks
- `&&` and `||` short-circuit

## Further Reading

- [[reference/04-control-flow|Control Flow]] - Full reference for `if`/`for`/`while`/`match`/`try-catch`: pattern matching (range, wildcard, type patterns), exception hierarchy (`AnyError` -> `LLMError` -> ...), short-circuit semantics

## Next Chapter

-> [Chapter 7: Functions and Closures](07-functions.md)
