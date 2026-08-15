# Chapter 5: Variables and Data Types

## Variables

### Declaring Variables

Helen uses `let` to declare mutable variables. In Chinese mode, the keyword is `设` or `定义`:

```helen
main {                     // 主函 — Chinese form of main
    let name = "Alice"     // 设 / 定义 — Chinese forms of let
    let age = 18           // same keyword
    let height = 175       // English keywords work too
}
```

### Declaring Constants

Use `const` (Chinese: `常量`) to declare immutable constants:

```helen
const PI = 3.14159         // 常量 PI = ... — Chinese form
const MAX_USERS = 1000
```

Once defined, a constant cannot be modified — ideal for configuration values, fixed parameters, and other things that should never change.

> **Important**: Constants are **automatically visible** inside agents, while ordinary variables (`let`) are **not visible** inside agents. This is part of Helen's scope isolation design — see Chapter 11 for details.

### Variables vs. Constants

| Feature | `let` (variable) | `const` (constant) |
|---------|-----------------|-------------------|
| Modifiable | ✅ | ❌ |
| Visible in agents | ❌ | ✅ auto-visible |
| Typical use | Temporary data during program execution | Configuration, fixed values |

## Data Types

### Primitive Types

```helen
main {
    // Integer
    let age: int = 18
    let negative: int = -7

    // Float
    let height: float = 1.75
    let temperature: float = -0.5

    // String
    let name: str = "Alice"
    let greeting: str = 'hello'  // single quotes work too

    // Boolean
    let isAdult: bool = true
    let isStudent: bool = false

    // Null
    let note: str? = null  // str? means "string or null"
}
```

| Type | English | Chinese | Examples |
|------|---------|---------|----------|
| Integer | `int` | `整数` | `42`, `-7` |
| Float | `float` | `浮点` | `3.14`, `-0.5` |
| String | `str` | `字符串` | `"hello"`, `'世界'` |
| Boolean | `bool` | `布尔` | `true`, `false` |
| Null | `null` | `空` | `null` |

### Type Annotations

Type annotations are optional. Helen infers types automatically:

```helen
main {
    let name = "Alice"        // inferred as str
    let age = 18              // inferred as int
    let score = 95.5          // inferred as float
}
```

### Optional Types

`?` marks a value that can be either a certain type or `null`:

```helen
main {
    let note: str? = null               // may be a string, may be null
    let phone: str? = "13800138000"     // has a value now

    if note != null {                   // 如果 — Chinese form of if
        print(note)                     // 打印 — Chinese form of print
    }
}
```

### Union Types

`|` means a value can be one of several types:

```helen
main {
    let id: int | str = 42      // can be an integer or a string
    let id2: int | str = "abc"  // also valid
}
```

## Lists

A List is an ordered sequence of values:

```helen
main {
    // Creating lists
    let numbers = [1, 2, 3, 4, 5]
    let fruits = ["apple", "banana", "orange"]
    let mixed = [1, "hello", true, null]  // types can be mixed

    // Accessing elements (0-indexed)
    print(numbers[0])     // 1
    print(fruits[1])      // banana

    // Modifying elements
    numbers[0] = 10
    print(numbers[0])     // 10

    // Appending elements
    numbers.append(6)     // [10, 2, 3, 4, 5, 6]

    // List length
    print(len(numbers))   // 6
}
```

### Common List Operations

```helen
main {
    let numbers = [3, 1, 4, 1, 5, 9, 2, 6]

    // Sort
    let sorted = sort(numbers)           // [1, 1, 2, 3, 4, 5, 6, 9]

    // Reverse
    let reversed = reverse(numbers)      // [6, 2, 9, 5, 1, 4, 1, 3]

    // Deduplicate
    let unique = unique(numbers)         // [3, 1, 4, 5, 9, 2, 6]

    // Filter (过滤)
    let evens = filter(numbers, fn(x) { return x % 2 == 0 })  // [4, 2, 6]  // fn/return = 函/返回 in Chinese

    // Map (映射)
    let doubled = map(numbers, fn(x) { return x * 2 })        // [6, 2, 8, 2, 10, 18, 4, 12]
}
```

## Maps

A Map is a collection of key-value pairs, similar to dictionaries in other languages:

```helen
main {
    // Creating a map
    let student = {
        "name": "Alice",
        "age": 18,
        "score": 95.5
    }

    // Accessing values
    print(student["name"])     // Alice
    print(student.age)         // 18 (dot notation also works)

    // Modifying values
    student["age"] = 19
    student.score = 98.0

    // Adding new keys
    student["class"] = "Grade 12, Class 1"

    // Checking if a key exists (有键 — Chinese form of has_key)
    if has_key(student, "name") {
        print("name field exists")
    }
}
```

## Strings

### String Operations

```helen
main {
    let text = "Hello, World!"

    // Length
    print(len(text))           // 13

    // Case conversion
    print(to_upper(text))      // HELLO, WORLD!
    print(to_lower(text))      // hello, world!

    // Split
    let parts = split(text, ", ")  // ["Hello", "World!"]

    // Join
    let joined = join(["a", "b", "c"], "-")  // "a-b-c"

    // Find
    let position = find(text, "World")  // 7 (returns index, -1 if not found)

    // Replace
    let replaced = replace(text, "World", "Helen")  // "Hello, Helen!"

    // Substring
    let sub = substring(text, 0, 5)  // "Hello"
}
```

### String Interpolation

Use `{{}}` to embed variables inside strings:

```helen
main {
    let name = "Alice"
    let age = 18
    let intro = "My name is {{name}}, I am {{age}} years old."
    print(intro)  // My name is Alice, I am 18 years old.
}
```

### String ↔ Number Conversion

```helen
main {
    // Number to string
    let text = str(42)       // "42"
    let float_text = str(3.14)  // "3.14"

    // String to number
    let number = int("42")   // 42
    let pi = float("3.14")   // 3.14
}
```

## Operators

### Arithmetic Operators

```helen
main {
    let a = 10
    let b = 3

    print(a + b)    // 13    addition
    print(a - b)    // 7     subtraction
    print(a * b)    // 30    multiplication
    print(a / b)    // 3.333...  division
    print(a % b)    // 1     modulo
    print(a ** b)   // 1000  exponentiation
}
```

### Comparison Operators

```helen
main {
    print(1 == 1)   // true   equal to
    print(1 != 2)   // true   not equal to
    print(1 < 2)    // true   less than
    print(1 <= 1)   // true   less than or equal to
    print(2 > 1)    // true   greater than
    print(2 >= 2)   // true   greater than or equal to
}
```

### Logical Operators

```helen
main {
    let adult = true
    let employed = false

    print(adult && employed)   // false  AND (true only if both are true)
    print(adult || employed)   // true   OR  (true if at least one is true)
    print(!adult)              // false  NOT (inverts the value)
}
```

> **Note**: Helen uses `&&`, `||`, and `!` for logical operations. The keywords `and` / `or` / `not` are **not** supported as operators.

### Fullwidth Punctuation

Helen supports Chinese fullwidth punctuation so you never need to switch input methods:

```helen
main {
    // The following two lines are completely equivalent
    let a = (1 + 2) * 3
    let b = （1 ＋ 2）＊ 3          // fullwidth parens, plus, multiply

    // These two lines are also completely equivalent
    print("hello")                   // ASCII parens
    打印（"hello"）                   // 打印 = print, fullwidth parens
}
```

| ASCII | Fullwidth | | ASCII | Fullwidth |
|-------|-----------|-|-------|-----------|
| `()` | `（）` | | `+` | `＋` |
| `{}` | `｛｝` | | `-` | `－` |
| `[]` | `［］` | | `*` | `＊` |
| `,` | `，` | | `/` | `／` |
| `:` | `：` | | `=` | `＝` |

## Chapter Summary

- `let` declares variables, `const` declares constants
- Primitive types: `int`, `float`, `str`, `bool`, `null`
- Optional type `?` and union type `|` handle "maybe null" or "one of several types"
- Lists use `[]`, maps use `{}`
- String interpolation uses `{{variable_name}}`
- Logical operators are `&&`, `||`, `!`
- Fullwidth punctuation is supported — no need to switch input methods

## Further Reading

- [[reference/02-variables-and-types|Variables and Types]] - Complete type system reference: all 14 types, `Optional` (`str?`), `Union` (`int | str`), `Literal`, type annotations, gradual typing

## Next Chapter

[Chapter 6: Control Flow](06-control-flow.md) ->
