# Chapter 7: Functions and Closures

## Defining Functions

Use `fn` (`函数`) to define a function:

```helen
import std.core.*

// Basic function
fn add(a: int, b: int): int {
    return a + b
}

// English keyword form
fn greet(name: str): str {
    return "Hello, " + name + "!"
}

main {
    print(add(3, 4))      // 7
    print(greet("Alice")) // Hello, Alice!
}
```

> **Bilingual note**: `fn` and `函数` are interchangeable keywords. Throughout this chapter we use the English form; the Chinese equivalent is shown in comments where new constructs appear.

### Anatomy of a Function

```
fn function_name(param1: Type1, param2: Type2): ReturnType {
    // function body
    return value
}
```

| Part | English keyword | Chinese keyword | Notes |
|------|----------------|----------------|-------|
| Function declaration | `fn` | `函数` | Introduces a function definition |
| Parameter | `name: Type` | `名字：类型` | Type annotation required |
| Return type | `: Type` after `)` | `：类型` | Placed between `)` and `{` |
| Return statement | `return` | `返回` | Helen does not support implicit return — `return` is required |

### Functions Without a Return Value

Omit the return type and `return`:

```helen
fn print_separator() {
    print("========================")
}

main {
    print_separator()
    print("content")
    print_separator()
}
```

## Where Can Functions Be Defined?

### Module-Level Functions

Defined at the top level of a file; accessible throughout the file:

```helen
import std.core.*

fn double(x: int): int {
    return x * 2
}

agent MyAgent {
    functions {
        fn helper(): int {
            return double(5)  // can call module-level functions
        }
    }
    main {
        return llm act "Do a task"
    }
}

main {
    print(double(10))  // 20
}
```

### Functions Inside an Agent

Defined inside the agent's `functions` (`函数区`) block. These functions serve two purposes:

1. Called directly inside `main`
2. Automatically become tools the LLM can invoke

```helen
agent CalcHelper {
    prompt "You are a calculation helper."
    tools = ["square", "cube"]

    functions {
        fn square(n: int): int {
            return n * n
        }
        fn cube(n: int): int {
            return n * n * n
        }
    }

    main {
        // Direct call
        let result = square(5)  // 25
        return llm act "Calculate the square and cube of 3"
    }
}
```

## Anonymous Functions

Functions without a name — handy for one-off use:

```helen
import std.core.*
import std.list.*

main {
    // Anonymous function
    let add = fn(x, y) { return x + y }
    print(add(1, 2))  // 3

    // Passed as an argument
    let numbers = [1, 2, 3, 4, 5]
    let doubled = map(numbers, fn(x) { return x * 2 })
    print(doubled)  // [2, 4, 6, 8, 10]

    let evens = filter(numbers, fn(x) { return x % 2 == 0 })
    print(evens)  // [2, 4]
}
```

## Closures

A closure is a function that *remembers* variables from its enclosing scope:

```helen
import std.core.*

fn create_counter() {
    let count = 0
    return fn() {
        count = count + 1
        return count
    }
}

main {
    let counter = create_counter()
    print(counter())  // 1
    print(counter())  // 2
    print(counter())  // 3
}
```

Properties of closures:

- Capture the values of outer variables (deep-copy snapshot)
- Each creation produces an independent instance

### Closures in Agents

```helen
import std.core.*

agent StreamAssistant {
    prompt "You are an assistant."

    main {
        let word_count = 0

        // The closure captures the word_count variable
        llm act "Write a paragraph" on_chunk fn(chunk) {
            word_count = word_count + len(chunk)
            print(chunk)
        }

        print("Total words: " + str(word_count))
    }
}
```

## The Pipe Operator

`|>` passes the result on the left as the first argument to the function on the right, making code read like an assembly line:

```helen
import std.core.*
import std.str.*

fn double(x: int): int { return x * 2 }
fn add_one(x: int): int { return x + 1 }

main {
    // Traditional (nested) form
    let result1 = add_one(double(add_one(5)))  // ((5+1)*2)+1 = 13

    // Pipeline form: left to right, more readable
    let result2 = 5 |> add_one |> double |> add_one  // 13

    print(result1)  // 13
    print(result2)  // 13
}
```

The pipe operator shines when chaining multiple transformations:

```helen
// Traditional form: nested calls, read inside-out
let result = sort(dedup(filter(raw_data, fn(x) { return x > 0 })))

// Pipeline form: read left to right, like an assembly line
let result = raw_data
    |> fn(data) { return filter(data, fn(x) { return x > 0 }) }
    |> dedup
    |> sort
```

## Functions as Arguments

Functions in Helen are *first-class citizens* — they can be passed as arguments and returned as values:

```helen
import std.core.*
import std.list.*

fn apply_fn(data: list, handler): list {
    return map(data, handler)
}

main {
    let numbers = [1, 2, 3]

    // Pass an anonymous function
    let squares = apply_fn(numbers, fn(x) { return x * x })
    print(squares)  // [1, 4, 9]
}
```

## Importing Other Modules

Helen supports importing other files as modules:

```helen
// utils.helen
import std.core.*

fn util_function(x: str): str {
    return upper(x)
}
```

```helen
// main.helen
import std.core.*
import "utils.helen"  // Import a Helen module

main {
    print(util_function("hello"))  // HELLO
}
```

### Importing Different Formats

```helen
import "config.json" as config   // Import a JSON file
import "data.yaml" as data       // Import a YAML file
import "utils.helen"             // Import a Helen module
import std.core.*                // Import a standard library module
import std.str.{upper, lower}    // Import specific functions
import std.list as list_utils    // Namespace import
```

## Aliases

Give a function or variable an alternative name:

```helen
alias print as output     // 别名 print as 输出
alias len as length_fn    // alias len as 长度函数
```

## Chapter Summary

- `fn` (`函数`) defines a function; `return` (`返回`) returns a value
- Functions can be defined at module top level or inside an agent's `functions` (`函数区`) block
- Functions inside `functions` automatically become tools the LLM can call
- Anonymous functions `fn(x) { ... }` are ideal for one-off use
- Closures *remember* outer variables; each creation is independent
- The pipe operator `|>` makes chained calls read left-to-right
- Functions are first-class citizens — passable as arguments and returnable as values
- `import` brings in other modules and the standard library

## Further Reading

- [[reference/03-functions|Functions]] - Complete function reference: parameters, return type syntax (`fn foo(): int`), default values, `alias`, closures as first-class callables (v1.32)
- [[reference/08-modules|Modules and Imports]] - `import` system, cross-file reuse, path safety, explicit stdlib import (`import std.xxx.*`)

## Next Chapter

→ [Chapter 8: Agent Collaboration](08-collaboration.md)
