# Tutorial 09: Python FFI

> Import Python libraries / Call Python functions / Automatic type conversion

---

## Overview

Helen supports directly importing and using Python libraries through the Python FFI (Foreign Function Interface). This gives Helen access to Python's entire ecosystem (400K+ packages), including numerical computation, network requests, data processing, and more.

**Core features:**
- ✅ Use `import` syntax to import Python modules
- ✅ Automatic type conversion (Helen ↔ Python)
- ✅ Call Python functions, access attributes and constants
- ✅ Support for nested modules (e.g. `os.path`)
- ✅ Complex objects are automatically wrapped

---

## Basic Usage

### Importing Python Modules

```helen
import "math" as math
import "json" as json
import "os.path" as path
```

**Syntax rules:**
- No file extension → Python module
- `.py` extension → Python module
- `.helen` → Helen file
- `.json`/`.md`/`.yaml` → data file

### Calling Python Functions

```helen
import std.core.*
import std.math.*
import "math" as math

main {
    let sqrt_result = math.sqrt(16)
    print(sqrt_result)    // 4.0
    
    let power = math.pow(2, 10)
    print(power)          // 1024.0
}
```

### Accessing Python Constants

```helen
import std.core.*
import "math" as math

main {
    let pi = math.pi
    print(pi)             // 3.141592653589793
    
    let e = math.e
    print(e)              // 2.718281828459045
}
```

---

## Type Conversion

### Helen → Python (automatic)

| Helen Type | Python Type |
|------------|-------------|
| `int` | `int` |
| `float` | `float` |
| `str` | `str` |
| `bool` | `bool` |
| `null` | `None` |
| `list` | `list` (recursive conversion) |
| `map` | `dict` (recursive conversion) |

### Python → Helen (automatic)

| Python Type | Helen Type |
|-------------|------------|
| `int` | `int` |
| `float` | `float` |
| `str` | `str` |
| `bool` | `bool` |
| `None` | `null` |
| `list` | `list` (recursive conversion) |
| `dict` | `map` (recursive conversion) |
| `tuple` | `list` |
| Complex object | Wrapped as `PythonObject` |

### Example: JSON Processing

```helen
import std.core.*
import "json" as json

main {
    // Helen map → Python dict → JSON string
    let data = {"name": "Alice", "age": 30, "active": true}
    let json_str = json.dumps(data)
    print(json_str)
    // {"name": "Alice", "age": 30, "active": true}
    
    // JSON string → Python dict → Helen map
    let parsed = json.loads(json_str)
    print(parsed["name"])    // Alice
}
```

---

## Nested Modules

Supports importing nested modules (e.g. `os.path`):

```helen
import std.core.*
import std.str.*
import "os.path" as path

main {
    let joined = path.join("home", "user", "docs")
    print(joined)    // home/user/docs
    
    let ext = path.splitext("file.txt")
    print(ext)       // ["file", ".txt"]
}
```

---

## Python Classes

### Instantiating Python Classes

Create instances by accessing the class name in a module and calling `()`:

```helen
import std.core.*
import "json" as json

main {
    // JSONEncoder is a Python class, () creates an instance
    let encoder = json.JSONEncoder()
    print(encoder)    // <json.encoder.JSONEncoder object at ...>
}
```

### Calling Instance Methods

Two approaches, same effect:

```helen
import std.core.*
import "json" as json

main {
    let encoder = json.JSONEncoder()
    
    // Approach 1: Natural method call (recommended)
    let result1 = encoder.encode({"name": "Alice"})
    
    // Approach 2: .call() by method name
    let result2 = encoder.call("encode", {"name": "Alice"})
    
    print(result1)    // {"name": "Alice"}
    print(result2)    // {"name": "Alice"}
}
```

**Selection guide:**
- Method name is known (vast majority of cases) → use `obj.method()`, more concise and intuitive
- Method name determined dynamically (only known at runtime) → use `obj.call("method_name")`

### Example: Using a Custom Python Class

Suppose there is a Python module `mylib/greeter.py`:

```python
class Greeter:
    def __init__(self, name: str):
        self.name = name
    
    def greet(self, prefix: str) -> str:
        return f"{prefix}, {self.name}!"
    
    def set_name(self, name: str):
        self.name = name
```

Using it in Helen:

```helen
import std.core.*
import "mylib.greeter" as PyGreeter

main {
    // 1. Instantiate: ClassName()
    let greeter = PyGreeter.Greeter("Alice")
    
    // 2. Call methods (natural syntax)
    let msg = greeter.greet("Hello")
    print(msg)    // Hello, Alice!
    
    // 3. Modify attributes
    greeter.set_name("Bob")
    print(greeter.greet("Hi"))    // Hi, Bob!
    
    // 4. Dynamic method name
    let method = "greet"
    let msg2 = greeter.call(method, "Hey")
    print(msg2)    // Hey, Bob!
}
```

### Using Python Classes in Modules

Python classes can be instantiated in imported `.helen` modules, working correctly across modules:

```helen
// bridge.helen
import "mylib.greeter" as PyGreeter

shared let _greeter = null

fn init_greeter(name: str) {
    _greeter = PyGreeter.Greeter(name)
}

fn greet(prefix: str): str {
    return _greeter.greet(prefix)
}
```

```helen
// main.helen
import std.core.*
import "bridge.helen"

main {
    init_greeter("Alice")
    print(greet("Hello"))    // Hello, Alice!
}
```

---

## Practical Examples

### Example 1: Math Computation

```helen
import std.core.*
import std.math.*
import "math" as math

main {
    // Trigonometric functions
    let angle = math.pi / 4
    let sin_val = math.sin(angle)
    let cos_val = math.cos(angle)
    print("sin(π/4) = " + str(sin_val))
    print("cos(π/4) = " + str(cos_val))
    
    // Logarithms
    let log_val = math.log(100, 10)
    print("log₁₀(100) = " + str(log_val))
    
    // Rounding
    print(math.floor(3.7))    // 3
    print(math.ceil(3.2))     // 4
}
```

### Example 2: File Path Operations

```helen
import std.core.*
import "os.path" as path

main {
    let filepath = "/home/user/documents/report.txt"
    
    // Extract filename
    let basename = path.basename(filepath)
    print(basename)    // report.txt
    
    // Extract directory
    let dirname = path.dirname(filepath)
    print(dirname)     // /home/user/documents
    
    // Split extension
    let parts = path.splitext(filepath)
    print(parts[0])    // /home/user/documents/report
    print(parts[1])    // .txt
}
```

### Example 3: Data Processing

```helen
import std.core.*
import "json" as json

main {
    // Create data
    let users = [
        {"name": "Alice", "age": 30},
        {"name": "Bob", "age": 25},
        {"name": "Charlie", "age": 35}
    ]
    
    // Serialize to JSON
    let json_data = json.dumps(users)
    print(json_data)
    
    // Parse JSON
    let parsed = json.loads(json_data)
    for user in parsed {
        print(user["name"] + " is " + str(user["age"]) + " years old")
    }
}
```

### Example 4: Using Python Libraries in Agents

```helen
import std.core.*
import std.math.*
import "math" as math

agent DataAnalyzer(data: list) {
    description "Analyze numerical data"
    prompt """
    Analyze the following data: {{data}}
    """
    
    functions {
        fn calculate_stats(): map {
            let n = len(data)
            let sum = 0
            for value in data {
                sum = sum + value
            }
            let mean = sum / n
            
            // Use Python's math.sqrt
            let variance = 0
            for value in data {
                let diff = value - mean
                variance = variance + diff * diff
            }
            variance = variance / n
            let std_dev = math.sqrt(variance)
            
            return {
                "mean": mean,
                "std_dev": std_dev,
                "min": min(data),
                "max": max(data)
            }
        }
    }
    
    main {
        let stats = calculate_stats()
        return "Mean: " + str(stats["mean"]) + 
               ", Std Dev: " + str(stats["std_dev"])
    }
}

main {
    let data = [10, 20, 30, 40, 50]
    let analyzer = DataAnalyzer(data)
    let result = analyzer()
    print(result)
}
```

---

## Error Handling

### Importing a Nonexistent Module

```helen
import "nonexistent_module" as bad

main {
    // Runtime error: Cannot import Python module 'nonexistent_module'
}
```

### Accessing a Nonexistent Attribute

```helen
import "math" as math

main {
    let value = math.nonexistent_function()
    // Runtime error: 'math' has no property 'nonexistent_function'
}
```

### Handling with try-catch

```helen
import std.core.*
import std.math.*
import "math" as math

main {
    try {
        let result = math.sqrt(-1)
        print(result)
    } catch RuntimeError err {
        print("Error: " + err.message)
    }
}
```

---

## Performance Considerations

- **Type conversion**: Simple type (int/float/str) conversion has minimal overhead
- **Complex objects**: Large list/dict conversion has some overhead; batch processing is recommended
- **Function calls**: Each call incurs cross-language overhead; avoid frequent calls in tight loops

---

## Comparison with Helen Native Features

| Feature | Helen Native | Python FFI |
|---------|-------------|------------|
| String processing | ✅ 36 string functions | ✅ Can use Python re, etc. |
| Math computation | ✅ 15 math functions | ✅ Can use numpy/scipy |
| File operations | ✅ 16 file functions | ✅ Can use os/pathlib |
| Network requests | ✅ 9 network functions | ✅ Can use requests (advanced scenarios) |
| Data processing | ✅ 25 data functions (JSON/CSV/HTML/XML) | ✅ Can use pandas (large datasets) |
| Machine learning | ❌ None | ✅ Can use torch/tensorflow |

**Recommendation**: Prefer Helen native features (285 built-in functions cover common needs); use Python FFI when you need advanced capabilities (e.g. big data processing, machine learning).

---

## Exercises

1. Import the `math` module and calculate the area of a circle (radius = 5)
2. Import the `json` module, convert a map to a JSON string and parse it back
3. Import the `os.path` module, extract the directory and filename from a file path
4. Create an Agent that uses Python's `math` module for complex calculations

---

> **Next**: Learn [[reference/15-python-bridge|Python Bridge]] — let Python directly use Helen Agents
