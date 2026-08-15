# Tutorial 02: Variables and Types

> let / const / type annotations / basic operations

## Variable Declarations

### `let` — Mutable Variables

```helen
import std.core.*
main {
    let x = 42
    x = 100       // ✅ Can be modified
    print(x)      // 100
}
```

### `const` — Immutable Constants

```helen
const PI = 3.14159

main {
    PI = 3        // ❌ E0346 CONST_ASSIGNMENT
}
```

## Data Types

### Primitive Types

```helen
main {
    let number = 42         // int
    let float_num = 3.14    // float
    let text = "hello"      // str
    let flag = true         // bool
    let nothing = null      // null
}
```

### Collection Types

```helen
main {
    let numbers = [1, 2, 3]                     // list<int>
    let mixed = [1, "two", true]                // list<any>
    let person = {"name": "Alice", "age": 30}   // map<str, any>
}
```

### Chinese Type Names (v1.10)

`list` and `map` support Chinese aliases: `列表` (list), `映射` (map).

```helen
函数 获取列表(): 列表 {
    返回 [1, 2, 3]
}

函数 获取映射(): 映射 {
    返回 {"a": 1}
}
```

## Type Annotations

```helen
main {
    let name: str = "Alice"
    let age: int = 30
    let score: float = 95.5
    let active: bool = true
}
```

### Optional Types

```helen
main {
    let email: str? = null      // Can be null
    email = "alice@example.com" // ✅
    email = null                // ✅

    let name: str = null        // ❌ str does not accept null
}
```

### Union Types

```helen
main {
    let id: int | str = 42      // Can be int or str
    id = "ABC-123"              // ✅
    id = true                   // ❌
}
```

## Operations

### Arithmetic Operations

```helen
main {
    let a = 10 + 3      // 13
    let b = 10 - 3      // 7
    let c = 10 * 3      // 30
    let d = 10 / 3      // 3.333...
    let e = 10 % 3      // 1
}
```

### Comparison Operations

```helen
main {
    let eq = 5 == 5     // true
    let ne = 5 != 3     // true
    let gt = 5 > 3      // true
    let ge = 5 >= 5     // true
    let lt = 3 < 5      // true
    let le = 5 <= 5     // true
}
```

### Logical Operations

Helen uses `&&` (and), `||` (or), `!` (not) as logical operators:

```helen
import std.core.*
import std.path.*
main {
    let and_result = true && false   // false
    let or_result = true || false    // true
    let not_result = !true           // false

    // Practical usage example
    let x = 5
    if x > 0 && x < 10 {
        print("x is between 0 and 10")
    }

    // File existence check example
    let file = "config.txt"
    if !path_exists(file) {
        print("File not found")
    }
}
```

**Note**: Helen does not use the `and`, `or`, `not` keywords; instead, it uses the symbols `&&`, `||`, `!`.

### String Concatenation

```helen
main {
    let greeting = "Hello, " + "World!"    // "Hello, World!"
    let message = "Score: " + 42           // "Score: 42"
}
```

## List Operations

```helen
import std.core.*
main {
    let nums = [1, 2, 3]
    let first = nums[0]        // 1
    let len = len(nums)        // 3
    let range_nums = range(5)  // [0, 1, 2, 3, 4]
}
```

### List Methods

Helen lists are based on Python lists and automatically support all common methods:

```helen
import std.list.*
import std.str.*
main {
    let items = [1, 2, 3]

    // Add elements
    items.append(4)           // [1, 2, 3, 4]
    items.insert(0, 0)        // [0, 1, 2, 3, 4]
    items.extend([5, 6])      // [0, 1, 2, 3, 4, 5, 6]

    // Remove elements
    items.pop()               // Remove and return the last element: 6
    items.remove(0)           // Remove the first element with value 0
    items.clear()             // Clear the list

    // Query
    let idx = items.index(2)  // Return the index of element 2
    let cnt = items.count(3)  // Return the count of element 3

    // Sort and reverse
    let unsorted = [3, 1, 4, 1, 5]
    unsorted.sort()           // [1, 1, 3, 4, 5]
    unsorted.reverse()        // [5, 4, 3, 1, 1]

    // Copy
    let copy = items.copy()   // Shallow copy
}
```

### List Concatenation

Use the `+` operator to concatenate two lists, returning a new list (original lists unchanged):

```helen
main {
    let a = [1, 2]
    let b = [3, 4]
    let c = a + b             // [1, 2, 3, 4]

    // Commonly used for incremental building
    let items = []
    items = items + ["a"]     // ["a"]
    items = items + ["b", "c"]  // ["a", "b", "c"]
}
```

> Note: `+` returns a new list and does not modify the original. For in-place modification, use `append()` or `extend()`.

**Available methods**:
| Method | Description |
|--------|-------------|
| `append(x)` | Add an element at the end |
| `extend(iterable)` | Extend the list |
| `insert(i, x)` | Insert an element at position i |
| `remove(x)` | Remove the first element with value x |
| `pop([i])` | Remove and return the element at position i (default: last) |
| `clear()` | Clear the list |
| `index(x)` | Return the index of the first element with value x |
| `count(x)` | Return the number of occurrences of x |
| `sort()` | Sort in place |
| `reverse()` | Reverse in place |
| `copy()` | Shallow copy |

## Map Operations

```helen
main {
    let person = {"name": "Alice", "age": 30}
    let name = person["name"]  // "Alice"
}
```

### Subscript/Field Assignment (v1.10)

v1.10 added **subscript assignment** and **field assignment** support, allowing direct modification of array elements and object fields:

#### Array Index Assignment

```helen
main {
    let arr = [1, 2, 3]
    arr[0] = 10  // ✅ arr becomes [10, 2, 3]
    arr[1] = 20  // ✅ arr becomes [10, 20, 3]

    // Dynamic index
    let i = 2
    arr[i] = 30  // ✅ arr becomes [10, 20, 30]
}
```

#### Object Field Assignment

```helen
main {
    let person = {"name": "Alice", "age": 30}
    person["age"] = 31  // ✅ person becomes {"name": "Alice", "age": 31}
    person.name = "Bob"  // ✅ person becomes {"name": "Bob", "age": 31}
}
```

#### Nested Access

```helen
main {
    let matrix = [[1, 2], [3, 4]]
    matrix[0][1] = 99  // ✅ matrix becomes [[1, 99], [3, 4]]

    let data = {"users": [{"name": "Alice"}, {"name": "Bob"}]}
    data["users"][0]["name"] = "Charlie"  // ✅ Nested modification
}
```

#### Error Examples

```helen
const arr = [1, 2, 3]
const obj = {"name": "Alice"}

main {
    arr[0] = 10  // ❌ E0346 CONST_ASSIGNMENT: const cannot be modified
    obj.name = "Bob"  // ❌ E0346 CONST_ASSIGNMENT: const cannot be modified
}
```

#### Practical Example

```helen
import std.core.*
main {
    // Update a record in an array
    let users = [
      {"name": "Alice", "score": 85},
      {"name": "Bob", "score": 90}
    ]

    // Update the first user's score
    users[0]["score"] = 95

    // Add a new field
    users[1]["grade"] = "A"

    print(users)
    // [
    //   {"name": "Alice", "score": 95},
    //   {"name": "Bob", "score": 90, "grade": "A"}
    // ]
}
```

## Type Checking

```helen
import std.core.*
main {
    let x = 42
    let t = type(x)            // "int"
    let is_int = isinstance(x, "int")    // true
    let is_str = isinstance(x, "str")    // false
}
```

## Exercises

1. Declare a `const` constant holding your birth year
2. Create a map containing your info (name, age, city)
3. Calculate the area of a circle (PI * r * r), r = 5
4. Use a type annotation to declare an optional string variable

---
