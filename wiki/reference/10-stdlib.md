# Tutorial 10: Standard Library Reference

> 285 built-in functions covering all core needs for AI application development

## Overview

The Helen standard library provides 285 built-in functions organized into 17 categories:

| Category | Count | Description |
|----------|-------|-------------|
| **Core** | 11 | Type conversion, general operations |
| **String** | 39 | String processing, regex, text analysis, template interpolation |
| **Data** | 25 | JSON, HTML, CSV, Markdown, YAML, TOML, XML |
| **Collection** | 26 | List, dict, set operations (v1.33 added `remove_key`, `get`, `set_key`, `has_key`) |
| **Network** | 9 | HTTP requests, URL handling |
| **Time** | 13 | Date/time, formatting, arithmetic |
| **Math** | 21 | Math operations, statistical analysis, bitwise operations (v1.39.4) |
| **File** | 18 | File read/write, directory operations, temp files, file search |
| **System** | 18 | Environment variables, CLI args, process management, logging |
| **Crypto** | 11 | Hashing, random numbers |
| **IO** | 5 | Streaming output control |
| **Context** | 2 | Context management (added in v1.15) |
| **Transcript** | 11 | Session recording management (added in v1.16; v1.22 added `search_transcript` + invocation tree) |
| **Media** | 12 | Multimodal media processing (added in v1.17) |

## Multilingual stdlib (v1.10)

Helen's stdlib supports multilingual function names. Every stdlib function has an English canonical name and localized aliases, all loaded at startup and available on demand.

### Chinese stdlib Aliases

Helen includes 230+ built-in Chinese aliases covering all stdlib categories. For example:

| English | Chinese | Category |
|---------|---------|----------|
| `len` | `长度` | Core |
| `print` | `打印` | Core |
| `sort` | `排序` | Collection |
| `filter` | `过滤` | Collection |
| `json_parse` | `json解析` | Data |
| `http_get` | `http获取` | Network |
| `regex_match` | `正则匹配` | String |
| `sha256` | `sha256` | Crypto |
| `date_format` | `日期格式化` | Time |

Full list: see `helen/stdlib/locales/zh.py`.

### Usage Examples

```helen
// Directly use Chinese stdlib function names (no import or alias needed)
import std.core.*
函数 数据处理() {
    let 原始数据 = [3, 1, 4, 1, 5, 9, 2, 6]
    let 排序后 = 排序(原始数据)
    let 去重后 = 去重(排序后)
    return 长度(去重后)
}

// Mixing Chinese and English is perfectly valid
函数 混合使用() {
    let data = [1, 2, 3]
    let sorted = 排序(data)     // English variable + Chinese function
    return len(sorted)          // Switch back to English
}

// Handle network data
函数 获取数据() {
    let 响应 = http获取("https://api.example.com/data")
    let 解析后 = json解析(响应)
    return 解析后["name"]
}
```

### Custom Aliases

If you need to give your own functions or stdlib functions additional aliases, use the `alias` statement:

```helen
// Give stdlib functions custom aliases
alias len as 我的长度
alias print as 输出

// Give user functions aliases
函数 greet(name: str): str {
    返回 "Hello, " + name
}
alias greet as 打招呼
```

The Chinese keyword `别名` is equivalent:

```helen
别名 len as 长度
```

### Design Principles

- **One mechanism**: stdlib aliases and user `alias` use the same Environment binding, behavior is completely identical
- **Full loading**: All locale alias tables are registered at startup, not filtered by locale
- **Locale only affects display**: `locale: zh` in `~/.helen/config.yaml` only affects the language of docs/LSP/error messages, not the names available at runtime
- **Backward compatible**: English canonical names are always available

### Extending to New Languages

Adding stdlib aliases for a new language only requires creating a new file — no changes to syntax/parser/interpreter:

```python
# helen/stdlib/locales/ja.py
ALIASES = {
    "長さ": "len",
    "表示": "print",
    "ソート": "sort",
    # ...
}
```

All alias files matching `helen/stdlib/locales/*.py` are automatically loaded at startup.

## Modular Imports (v1.34)

Helen v1.34 introduces modular imports for stdlib functions, providing explicit control over which functions are imported and avoiding naming conflicts.

### Import Syntax

Helen supports three forms of stdlib module imports:

#### 1. Selective Import

Import specific functions from a module:

```helen
import std.str.{upper, lower}
import std.list.{sort, map}
import std.dict.{get, set_key}

main {
    print(upper("hello"))  // "HELLO"
    print(sort([3, 1, 2]))  // [1, 2, 3]
}
```

#### 2. Wildcard Import

Import all functions from a module:

```helen
import std.str.*
import std.list.*

main {
    // All string and list functions are now available
    print(upper("hello"))
    print(map([1, 2, 3], fn(x) { return x * 2 }))
}
```

#### 3. Namespace Import

Import a module under a namespace to avoid naming conflicts:

```helen
import std.dict as Dict
import std.list as List

main {
    let data = {"name": "Alice", "age": 30}
    
    // Use namespace prefix
    let keys = Dict.keys(data)
    let sorted = List.sort([3, 1, 2])
}
```

### Available Modules

| Module | Description | Key Functions |
|--------|-------------|---------------|
| `std.str` | String operations | `upper`, `lower`, `split`, `join`, `replace`, `find`, `contains`, `starts_with`, `ends_with`, `regex_match`, `regex_replace` |
| `std.list` | List operations | `map`, `filter`, `reduce`, `sort`, `unique`, `flatten`, `chunk`, `zip`, `find_if`, `every`, `some` |
| `std.dict` | Dictionary operations | `keys`, `values`, `entries`, `get`, `set_key`, `has_key`, `remove_key`, `merge`, `pick`, `omit` |
| `std.math` | Mathematical operations | `abs`, `round`, `floor`, `ceil`, `min`, `max`, `sum`, `pow`, `sqrt` |
| `std.time` | Time and date operations | `now`, `format_time`, `parse_time`, `sleep` |
| `std.file` | File operations | `read_file`, `write_file`, `file_exists`, `list_dir`, `mkdir` |
| `std.system` | System operations | `env_get`, `env_set`, `get_args`, `exit` |
| `std.io` | I/O operations | `print`, `input`, `read_line` |

### Benefits of Modular Imports

1. **Explicit Dependencies**: Clear which functions your code uses
2. **Avoid Naming Conflicts**: Use namespaces to prevent collisions
3. **Better IDE Support**: IDEs can provide better autocomplete and type checking
4. **Easier Refactoring**: Clear module boundaries make code easier to understand and modify

### Backward Compatibility

All stdlib functions are still available globally without explicit imports. Modular imports are optional and provide an alternative way to access stdlib functions.

```helen
// Both forms work identically
import std.str.{upper}
// or just use upper() directly (global stdlib)

main {
    print(upper("hello"))  // Works either way
}
```

## Core Functions (11)

### Type Conversion

```helen
import std.core.*
main {
    str(42)                       // "42"
    int("42")                     // 42
    float("3.14")                 // 3.14
}
```

### General Operations

```helen
import std.core.*
main {
    len("hello")                  // 5
    len([1, 2, 3])               // 3

    abs(-42)                      // 42
    min(3, 1, 4)                 // 1
    max(3, 1, 4)                 // 4

    range(5)                      // [0, 1, 2, 3, 4]
    range(1, 6)                   // [1, 2, 3, 4, 5]
}
```

### Type Checking

```helen
import std.core.*
main {
    type(42)                      // "int"
    isinstance(42, "int")         // true
}
```

## String Functions (39)

### Basic Operations (14)

```helen
import std.str.*
main {
    // Case
    upper("hello")                // "HELLO"
    lower("HELLO")                // "hello"

    // Trimming
    strip("  hello  ")            // "hello"
    trim_prefix("hello", "he")    // "llo"
    trim_suffix("hello", "lo")    // "hel"

    // Splitting and joining
    split("a,b,c", ",")           // ["a", "b", "c"]
    join("-", ["a", "b", "c"])    // "a-b-c"

    // Checking
    startswith("hello", "hel")    // true
    endswith("hello", "lo")       // true

    // Finding and replacing
    find("hello", "ell")          // 1
    replace("hello", "l", "L")    // "heLLo"
    substring("hello", 1, 3)      // "el"   — end is exclusive INDEX, not length
    substring("id=repo1", 3)      // "repo1" — omit end to slice to string end
    substring("id=repo1", 3, 8)   // "repo1" — to extract N chars use (s, start, start+N)

    // Character ↔ code point (v1.31+)
    chr(65)                       // "A"
    chr(20013)                    // "中"
    ord("A")                      // 65
    ord("中")                     // 20013

    // Chinese aliases
    字符(65)                      // "A"
    码点("A")                     // 65

    // String interpolation (v1.8.1+)
    let template = "Hello, {{name}}! You are {{age}} years old."
    let vars = {"name": "Alice", "age": 30}
    interpolate(template, vars)
    // "Hello, Alice! You are 30 years old."

    // Supports nested attribute access
    let template2 = "User: {{user.name}}, Email: {{user.email}}"
    let vars2 = {"user": {"name": "Bob", "email": "bob@example.com"}}
    interpolate(template2, vars2)
    // "User: Bob, Email: bob@example.com"
}
```

### Regular Expressions (5)

```helen
import std.core.*
import std.str.*
main {
    // Match
    let m = regex_match(r"\d+", "123abc")
    print(m.match)                // "123"

    // Search
    let s = regex_search(r"\d+", "abc123def")
    print(s.match)                // "123"

    // Replace
    regex_replace(r"\d+", "abc123def", "NUM")
    // "abcNUMdef"

    // Split
    regex_split(r"\s+", "a  b  c")
    // ["a", "b", "c"]

    // Find all
    regex_findall(r"\d+", "a1b2c3")
    // ["1", "2", "3"]
}
```

### Text Analysis (8)

```helen
import std.str.*
main {
    // Tokenization
    tokenize("Hello, world!")     // ["Hello", "world"]

    // Word frequency count
    word_count("hello world hello")
    // {"hello": 2, "world": 1}

    // Edit distance
    levenshtein("hello", "hallo") // 1

    // Similarity
    similarity("hello", "hallo")  // 0.8

    // Remove punctuation
    remove_punctuation("Hello!")  // "Hello"

    // Normalize whitespace
    normalize_whitespace("a  b  c")  // "a b c"

    // Extract URLs
    extract_urls("Visit https://example.com")
    // ["https://example.com"]

    // Extract emails
    extract_emails("Contact user@example.com")
    // ["user@example.com"]
}
```

### Encoding Conversion (4)

```helen
import std.str.*
main {
    // Base64
    base64_encode("Hello")        // "SGVsbG8="
    base64_decode("SGVsbG8=")     // "Hello"

    // HTML escaping
    html_escape("<script>")       // "&lt;script&gt;"
    html_unescape("&lt;")         // "<"
}
```

### String Operations (7)

```helen
import std.str.*
main {
    repeat("ab", 3)               // "ababab"
    reverse("hello")              // "olleh"

    pad_left("42", 5, "0")        // "00042"
    pad_right("hi", 5)            // "hi   "
    center("hi", 6)               // "  hi  "

    count("hello", "l")           // 2
    index("hello", "ll")          // 2
}
```

## Data Functions (25)

### JSON (4)

```helen
import std.core.*
import std.data.*
main {
    // Parse
    let data = json_parse('{"name": "Alice", "age": 30}')
    print(data.name)              // "Alice"

    // Generate
    let json_str = json_stringify({"name": "Alice"})
    // '{"name": "Alice"}'

    // File operations
    json_save("data.json", data)
    let loaded = json_load("data.json")
}
```

### HTML (3)

```helen
import std.data.*
main {
    // Extract text
    html_text("<p>Hello <b>World</b></p>")
    // "Hello World"

    // Extract links
    html_links('<a href="http://example.com">Link</a>')
    // ["http://example.com"]

    // Parse
    let dom = html_parse("<div>content</div>")
}
```

### Markdown (2)

```helen
import std.data.*
main {
    // Convert to HTML
    markdown_to_html("# Title\n\nParagraph")
    // "<h1>Title</h1><p>Paragraph</p>"

    // Extract headings
    markdown_extract_headings("# H1\n## H2")
    // [{"level": 1, "text": "H1"}, {"level": 2, "text": "H2"}]
}
```

### CSV (4)

```helen
import std.data.*
main {
    // Parse
    let rows = csv_parse("name,age\nAlice,30")
    // [["name", "age"], ["Alice", "30"]]

    // Generate
    csv_stringify([["a", "b"], ["1", "2"]])
    // "a,b\n1,2\n"

    // File operations
    csv_save("data.csv", rows)
    let loaded = csv_load("data.csv")
}
```

### YAML (4)

```helen
import std.data.*
main {
    // Parse
    let data = yaml_parse("name: Alice\nage: 30")
    // {"name": "Alice", "age": 30}

    // Generate
    yaml_stringify({"name": "Alice"})
    // "name: Alice\n"

    // File operations
    yaml_save("config.yaml", data)
    let loaded = yaml_load("config.yaml")
}
```

### TOML (4)

```helen
import std.data.*
main {
    // Parse
    let data = toml_parse("name = \"Alice\"\nage = 30")
    // {"name": "Alice", "age": 30}

    // Generate
    toml_stringify({"name": "Alice"})
    // "name = \"Alice\"\n"

    // File operations
    toml_save("config.toml", data)
    let loaded = toml_load("config.toml")
}
```

## Collection Functions (26)

Collection functions handle list, dict, and set operations.

### Dict Operations (v1.33 Enhanced)

New dict manipulation functions for common operations:

```helen
import std.core.*
import std.dict.*
main {
    let user = {"name": "Alice", "age": 30, "city": "Beijing"}
    
    // Check if key exists
    if has_key(user, "email") {
        print("Has email: " + user["email"])
    }
    
    // Get value with default
    let email = get(user, "email", "N/A")
    print("Email: " + email)  // "N/A"
    
    // Add or update a key (immutable)
    let updated = set_key(user, "email", "alice@example.com")
    print(updated)  // {"name": "Alice", "age": 30, "city": "Beijing", "email": "alice@example.com"}
    
    // Remove a key (immutable)
    let removed = remove_key(updated, "city")
    print(removed)  // {"name": "Alice", "age": 30, "email": "alice@example.com"}
    
    // Chinese aliases also work
    let 检查 = 包含键(user, "name")  // true
    let 获取值 = 获取(user, "age")    // 30
    let 新增 = 设置键(user, "phone", "123")
    let 删除 = 删除键(新增, "phone")
}
```

**Dict Functions**:
- `keys(dict)` — Get all keys as list
- `values(dict)` — Get all values as list
- `entries(dict)` — Get all (key, value) pairs
- `merge(*dicts)` — Merge multiple dicts
- `pick(dict, keys)` — Keep only specified keys
- `omit(dict, keys)` — Remove specified keys (multiple)
- `remove_key(dict, key)` — Remove single key (**v1.33 new**)
- `get(dict, key, default?)` — Get value with optional default (**v1.33 new**)
- `set_key(dict, key, value)` — Set key-value pair, returns new dict (**v1.33 new**)
- `has_key(dict, key)` — Check if key exists (**v1.33 new**)

### List Operations

```helen
import std.list.*
main {
    let numbers = [3, 1, 4, 1, 5, 9, 2, 6]
    
    // Map, filter, reduce
    let doubled = map(numbers, fn(x) { return x * 2 })
    let evens = filter(numbers, fn(x) { return x % 2 == 0 })
    let sum = reduce(numbers, fn(acc, x) { return acc + x }, 0)
    
    // Sort and unique
    let sorted = sort(numbers)
    let unique_vals = unique(numbers)
    
    // Chunk and zip
    let chunks = chunk(numbers, 3)  // [[3,1,4], [1,5,9], [2,6]]
    let zipped = zip([1, 2, 3], ["a", "b", "c"])  // [[1,"a"], [2,"b"], [3,"c"]]
}
```

### Set Operations

```helen
import std.core.*
main {
    let s1 = make_set([1, 2, 3, 4])
    let s2 = make_set([3, 4, 5, 6])
    
    let union = set_union(s1, s2)        // {1, 2, 3, 4, 5, 6}
    let intersection = set_intersection(s1, s2)  // {3, 4}
    let difference = set_difference(s1, s2)      // {1, 2}
    
    if set_has(s1, 3) {
        print("s1 contains 3")
    }
}
```

## Math Functions (21)

Math functions cover basic arithmetic, statistics, trigonometry, and bitwise operations (v1.39.4).

### Basic Math Operations

```helen
import std.core.*
import std.math.*
main {
    // Rounding
    let rounded = round(3.14159, 2)     // 3.14
    let ceiling = ceil(3.2)             // 4
    let flooring = floor(3.8)           // 3
    
    // Powers and roots
    let power = pow(2, 10)              // 1024
    let root = sqrt(16)                 // 4.0
    
    // Logarithms
    let natural = log(2.718)            // Natural log (ln)
    let base2 = log2(8)                 // 3 (2^3 = 8)
    let base10 = log10(100)             // 2 (10^2 = 100)
    let exponential = exp(1)            // 2.718... (e^1)
}
```

### Trigonometric Functions

```helen
import std.math.*
main {
    // Basic trig (radians)
    let cosine = cos(0)                 // 1
    let sine = sin(3.14159 / 2)         // 1
    let tangent = tan(0)                // 0
    
    // Inverse trig (returns radians)
    let angle = acos(0.5)               // 1.047... (60°)
    let angle2 = asin(0.5)              // 0.523... (30°)
    let angle3 = atan(1)                // 0.785... (45°)
    let angle4 = atan2(1, 1)            // 0.785... (45°, y/x)
}
```

### Statistical Functions

```helen
import std.core.*
import std.math.*
main {
    let numbers = [1, 2, 3, 4, 5]
    
    // Central tendency
    let avg = mean(numbers)             // 3.0
    let mid = median(numbers)           // 3
    let mode_val = mode([1, 2, 2, 3])   // [2]
    
    // Dispersion
    let v = variance(numbers)           // 2.0
    let std = stddev(numbers)           // 1.414...
    
    // Aggregation
    let total = sum(numbers)            // 15
    let prod = product([1, 2, 3, 4])    // 24
    let min_val = stats_min(numbers)    // 1
    let max_val = stats_max(numbers)    // 5
    
    // Correlation and percentiles
    let corr = correlation([1, 2, 3], [2, 4, 6])  // 1.0 (perfect correlation)
    let p90 = percentile(numbers, 90)   // 4.6
}
```

### Bitwise Operations (v1.39.4)

Bitwise operations manipulate individual bits of integer values. These are useful for low-level data processing, encoding algorithms, and performance-critical calculations.

```helen
import std.core.*
import std.math.*
main {
    // Basic bitwise operations
    let and_result = bit_and(5, 3)          // 1 (101 & 011 = 001)
    let or_result = bit_or(5, 3)            // 7 (101 | 011 = 111)
    let xor_result = bit_xor(5, 3)          // 6 (101 ^ 011 = 110)
    let not_result = bit_not(5)             // -6 (~5 = -6)
    
    // Bit shifting
    let left_shift = bit_shift_left(5, 2)   // 20 (5 << 2 = 20)
    let right_shift = bit_shift_right(20, 2) // 5 (20 >> 2 = 5)
    
    // Practical example 1: Check if a number is even
    let n = 42
    let is_even = bit_and(n, 1) == 0        // true
    // Explanation: bit_and(n, 1) extracts the least significant bit
    // If it's 0, the number is even; if it's 1, the number is odd
    
    // Practical example 2: Fast multiplication/division by powers of 2
    let x = 7
    let times_8 = bit_shift_left(x, 3)      // 56 (7 * 8 = 7 * 2^3)
    let div_4 = bit_shift_right(20, 2)      // 5 (20 / 4 = 20 / 2^2)
    // Shifting left by n is equivalent to multiplying by 2^n
    // Shifting right by n is equivalent to dividing by 2^n (integer division)
    
    // Practical example 3: Extract specific bits (masking)
    let value = 0b11010110                  // 214 in binary
    let mask = 0b00001111                   // 15 (extract lower 4 bits)
    let lower_4_bits = bit_and(value, mask) // 6 (0b0110)
    
    // Practical example 4: Set specific bits
    let base = 0b10100000                   // 160
    let flags = 0b00001111                  // 15 (set lower 4 bits)
    let result = bit_or(base, flags)        // 175 (0b10101111)
    
    // Practical example 5: Toggle bits with XOR
    let original = 0b1010                   // 10
    let toggle_mask = 0b1100                // 12
    let toggled = bit_xor(original, toggle_mask) // 6 (0b0110)
    // XOR with 1 flips the bit, XOR with 0 keeps it unchanged
}
```

**Bitwise function reference**:
- `bit_and(a, b)`: Bitwise AND — returns 1 where both bits are 1
- `bit_or(a, b)`: Bitwise OR — returns 1 where at least one bit is 1
- `bit_xor(a, b)`: Bitwise XOR — returns 1 where bits differ
- `bit_not(a)`: Bitwise NOT — flips all bits (returns ~a)
- `bit_shift_left(a, n)`: Left shift — shifts bits left by n positions (equivalent to a * 2^n)
- `bit_shift_right(a, n)`: Right shift — shifts bits right by n positions (equivalent to a / 2^n)

**Common use cases**:
- **Performance optimization**: Bit shifts are faster than multiplication/division by powers of 2
- **Flag manipulation**: Store multiple boolean flags in a single integer
- **Masking**: Extract or modify specific bits in a value
- **Encoding algorithms**: Many compression and encryption algorithms use bitwise operations
- **Hardware interfacing**: Working with registers and low-level protocols

## CLI Arguments (System Module)

Helen programs can directly access command-line arguments. All arguments after the filename are passed to the program:

```bash
$ helen my_tool.helen --verbose --output=json --port=8080 input.txt
```

### argv Predefined Constant

`argv` is a predefined `const list<str>` containing all command-line arguments:

```helen
import std.core.*
import std.str.*
main {
    // Direct access
    print(argv)  // ["--verbose", "--output=json", "--port=8080", "input.txt"]
    print(len(argv))  // 4

    // Check for specific arguments
    if contains(argv, "--verbose") {
        print("Verbose mode enabled")
    }

    // Iterate over all arguments
    for arg in argv {
        print("Argument: " + arg)
    }
}
```

`argv` is `const` and cannot be reassigned. It is automatically visible in agent scope (through the const read-only sharing mechanism).

### get_cli_args() Function

Standard library function form equivalent to `argv`:

```helen
import std.system.*
main {
    let args = get_cli_args()  // Same as argv
}
```

### parse_cli_args() Structured Parsing

**Auto mode** (no arguments) — automatically recognizes various argument types:

```helen
import std.system.*
main {
    let parsed = parse_cli_args()
    // Input: --verbose --output=json --port 8080 input.txt
    // Result: {
    //   verbose: true,
    //   output: "json",
    //   port: "8080",
    //   _positional: ["input.txt"]
    // }
}
```

Supported argument formats:
- `--flag` → boolean `true`
- `--key=value` → string value (split at the first `=`)
- `--key value` → string value (space-separated)
- `-v` → short flag, boolean `true`
- Anything else → positional argument (collected into `_positional`)

**Spec mode** (pass a spec map) — with type conversion and defaults:

```helen
import std.core.*
import std.system.*
main {
    let spec = {
        "verbose": {"type": "flag", "default": false},
        "output": {"type": "string", "default": "text"},
        "port": {"type": "int", "default": 3000}
    }
    let config = parse_cli_args(spec)
    // port is automatically converted to int type
    print(type(config["port"]))  // "int"
}
```

Supported spec types: `flag`, `string`, `int`, `float`.

> **Note**: In nested map literals, `}}` is recognized by the lexer as a template reference closing tag. You need to add a space between the two `}`: `} }`.

## Context Functions (2) (v1.15)

Context management functions for controlling the lifecycle of LLM conversation context.

### Clearing Context

```helen
import std.context.*
import std.core.*
main {
    // Clear the current conversation history
    let result = clear_context()
    print("Cleared " + str(result["cleared_messages"]) + " messages")
    print("Freed approximately " + str(result["cleared_tokens"]) + " tokens")
    // Returns: {"status": "ok", "cleared_messages": 5, "cleared_tokens": 1200, "warning": "..."}
}
```

**Use cases**:
- User requests to "start over" the conversation
- Reset context during error recovery
- Periodic cleanup in long-conversation agents

### Compressing Context

```helen
import std.context.*
import std.core.*
main {
    // Auto compression (based on token thresholds)
    let result = compress_context("auto")
    print("Compressed from " + str(result["original_tokens"]) + " to " + str(result["compressed_tokens"]))
    // Returns: {"status": "ok", "original_messages": 10, "compressed_messages": 5, ...}

    // Force LLM summary compression
    compress_context("summarize")

    // Truncate to keep the most recent 10 messages
    compress_context("truncate")
}
```

**Compression strategies**:
- `"auto"`: automatic selection (default, only compresses when tokens exceed threshold)
- `"summarize"`: uses LLM to generate a summary (slow but preserves context)
- `"truncate"`: truncates old messages (fast but loses context)
- `"none"`: no compression (no-op)

**Long-conversation agent example**:

```helen
import std.context.*
import std.core.*
agent ChatBot {
    main {
        let message_count = 0
        while true {
            let input = prompt("you> ")
            let response = llm act { ... }
            
            message_count += 1
            
            // Auto-compress every 10 conversation turns
            if message_count % 10 == 0 {
                let result = compress_context("auto")
                if result["status"] == "ok" {
                    print("Compressed context, saved " + 
                          str(result["original_tokens"] - result["compressed_tokens"]) + 
                          " tokens")
                }
            }
            
            // User command: /clear to clear context
            if input == "/clear" {
                clear_context()
                print("Context cleared")
            }
        }
    }
}
```

### Restoring Context (v1.21)

Resumes an old transcript session into the current active context — the LLM can see all restored messages at the next `llm act` call.

```helen
import std.context.*
import std.core.*
import std.transcript.*
main {
    // 1. List all old sessions
    let sessions = list_sessions()
    for s in sessions {
        print("{s.session_id}: {s.message_count} messages, scope={s.scope}")
    }

    // 2. Restore a specific session to the current active context
    let r = restore_context("session_1783492628_d9d9c0aa")
    if r["status"] == "ok" {
        print("Restored " + str(r["restored_messages"]) + " messages")
        print("Skipped " + str(r["boundary_markers"]) + " compression boundaries")
    } else {
        print("Restore failed: " + r["error"])
    }
}

// Chinese alias
main {
    恢复上下文("session_1783492628_d9d9c0aa")
}
```

**Difference from `resume_session`**:

- `restore_context(session_id)`: restores **active context**, supports precise filtering by agent/invocation, suitable for resuming specific parts of an old session
- `resume_session(session_id)`: restores **active context**, imports all messages from the old session, suitable for recovering an entire old session (after v1.23 fix, LLM can see restored messages)

**Note**: Before v1.23, `resume_session` only replaced the transcript store reference (LLM couldn't see it). Now it imports messages into the current store (LLLM can see them).

**Important notes**:

- `restore_context` only restores messages (full fields: `role`, `content`, `tool_calls`, `tool_call_id`, `uuid`, `compressed`, `pinned`)
- Does **not** restore working_memory and context config (transcript doesn't persist these)
- To restore working_memory, use `working_memory_set()` manually

**Saving/restoring full context across sessions (including working_memory)**:

```helen
import std.context.*
import std.core.*
import std.io.*
main {
    // Before session ends: export full snapshot to file
    let snapshot = export_context()
    write_file("context_snapshot.json", to_json(snapshot.context))

    // When new session starts: read and import
    let saved = parse_json(read_file("context_snapshot.json"))
    import_context(saved)
}
```

## Transcript Functions (11) (v1.16)

Session recording management functions for accessing and manipulating Helen's conversation history (v1.16+). Provides persistence, session recovery, and compression auditing.

### Getting Session ID

```helen
import std.core.*
import std.transcript.*
main {
    // Get current session ID
    let session_id = get_session_id()
    print("Current session: " + session_id)
    // Returns: "session_1783492628_d9d9c0aa"
}
```

**Use cases**:
- Log session identifiers for debugging
- Tag sessions in logs
- Verify ID during session recovery

### Listing All Sessions

```helen
import std.core.*
import std.transcript.*
main {
    // List all transcript sessions
    let sessions = list_sessions()
    for session in sessions {
        print(session["session_id"] + ": " + str(session["message_count"]) + " messages")
    }
    // Returns: [{"session_id": "...", "message_count": 50, "size_bytes": 2500, ...}]
}
```

**Return fields**:
- `session_id`: Session ID
- `message_count`: Number of messages
- `size_bytes`: File size (bytes)
- `created_at`: Creation time
- `modified_at`: Last modification time

### Replaying Sessions

```helen
import std.core.*
import std.transcript.*
main {
    // Replay current session (valid view only)
    let messages = replay_transcript()
    for msg in messages {
        print(msg["role"] + ": " + msg["content"])
    }

    // Replay a specified session, including compressed messages
    let full = replay_transcript("session_1783492628_d9d9c0aa", true)
}
```

**Parameters**:
- `session_id` (optional): Session ID to replay, defaults to current session
- `include_compressed` (optional): Whether to include compressed messages, defaults to false

**Returns**: Message list, each message contains `role`, `content`, `uuid`, `timestamp`

### Replaying Full Session (v1.23.7+)

Use `replay_full_session()` to view the complete execution flow of the main session and all its spawned child sessions:

```helen
import std.core.*
import std.transcript.*
main {
    // Aggregate view of main session + all spawns
    let messages = replay_full_session()
    for msg in messages {
        // Each message includes a session_id field identifying the source
        print("[" + msg["session_id"] + "] " + msg["role"] + ": " + msg["content"][:50])
    }

    // Specify root session
    let messages2 = replay_full_session("session_abc123")
}
```

**Parameters**:
- `session_id` (optional): Root session ID, defaults to current session

**Returns**: Message list from all related sessions, sorted by timestamp, each message includes a `session_id` field

**Use cases**:
- View the complete execution flow (including all spawned subtasks)
- Debug multi-agent collaboration
- Analyze message distribution across the spawn tree

### Exporting Sessions

```helen
import std.transcript.*
main {
    // Export as JSON
    export_transcript("my_chat.json", "json")

    // Export as Markdown
    export_transcript("my_chat.md", "markdown")

    // Export as plain text
    export_transcript("my_chat.txt", "text")

    // Export a specific session
    export_transcript("old_chat.json", "json", "session_1783492600_abc12345")

    // v1.23.7+: Export full spawn tree
    export_transcript("full_chat.json", "json", include_spawned=true)
}
```

**Parameters**:
- `output_path`: Output file path
- `format`: Export format ("json", "markdown", "text")
- `session_id` (optional): Session ID to export
- `include_spawned` (optional, v1.23.7+): Whether to export all spawned child sessions, defaults to false

**Returns**: Output file path (success) or empty string (failure)

### Searching Sessions (v1.22)

Search persisted transcripts by **content**. In general situations you can't remember session IDs, but you remember what was discussed — use `search_transcript` to find relevant sessions.

```helen
import std.core.*
import std.transcript.*
main {
    // Search within the current session
    let matches = search_transcript("authentication bug")
    for m in matches {
        print("Match: {m.snippet}")
        print("Position: {m.match_position}")
    }

    // Search across all sessions (cross-session discovery)
    let matches_all = search_transcript("database schema", scope="all", limit=10)

    // Regex matching
    let matches_re = search_transcript("fix.*bug", regex=true)

    // Search only user messages
    let matches_user = search_transcript("TODO", role="user")

    // v1.23.7+: Cross-spawn search (search current session and all its spawned child sessions)
    let matches_sp = search_transcript("error", include_spawned=true)
    for m in matches_sp {
        print("[" + m["session_id"] + "] " + m["snippet"])
    }

    // Chinese alias
    let matches_zh = 搜索会话("authentication bug", scope="all")
}
```

**Parameters**:
- `query` (required): Search content (substring by default, regex when `regex=true`)
- `scope`: `"current"` (default) / `"all"` (across all sessions) / `"global"` / `"project"`
- `session_id`: Specify session (ignored when `scope="all"`)
- `role`: Filter by role (`"user"` / `"assistant"` / `"tool"` / `""` for all)
- `regex`: Whether to match by regex
- `limit`: Maximum number of results (default 50)
- `include_spawned` (optional, v1.23.7+): Whether to search all spawned child sessions, defaults to false

**Typical usage**: Search then restore full context

```helen
import std.context.*
import std.core.*
import std.transcript.*
main {
    // Find relevant historical discussions
    let matches = search_transcript("authentication bug", scope="all", limit=5)
    if len(matches) > 0 {
        // Find the session of the most recent match
        let target_session = matches[0]["session_id"]
        // Restore the full context of that session
        restore_context(target_session)
        print("Restored to session: " + target_session)
    }

    // v1.23.7+: Search within current session and its spawns
    let errors = search_transcript("error", include_spawned=true)
    if len(errors) > 0 {
        print("Found " + str(len(errors)) + " errors (including spawned subtasks)")
    }
}
```

### Invocation Tree Query (v1.22)

Each agent `main {}` execution is an **invocation** with a unique `invocation_id`. The transcript records all invocations completely, and you can use query functions to trace the call structure.

```helen
import std.core.*
import std.transcript.*
main {
    // List all invocations
    let invs = list_invocations()
    for inv in invs {
        print("{inv.agent_name}: {inv.message_count} messages")
    }

    // Filter by agent
    let a_runs = list_invocations(agent="Researcher")

    // Query a single invocation
    let info = get_invocation("inv_1784272795_a61bcdaf")
    print("Agent: " + str(info["agent_name"]))
    print("Message count: " + str(info["message_count"]))

    // Get the full call tree
    let tree = get_invocation_tree()
    // tree.children is a nested list of invocations

    // Invocation path string
    print(invocation_path("inv_3"))
    // "top -> A -> C"

    // Chinese aliases
    列出调用()
    获取调用树()
    调用路径("inv_3")
}
```

**Combined with replay_transcript filtering**:

```helen
import std.transcript.*
main {
    // See only agent A's messages
    let a_msgs = replay_transcript(agent="A")

    // See only A's last run
    let last = replay_transcript(agent="A", last_only=true)

    // See a specific invocation and its sub-calls
    let subtree = replay_transcript(invocation_id="inv_1", include_subtree=true)
}
```

**Combined with restore_context for precise recovery**:

```helen
import std.context.*
main {
    // Restore only agent A's most recent run
    restore_context("session_xxx", agent="A", last_only=true)

    // Restore a specific invocation and its subtree
    restore_context("session_xxx", invocation_id="inv_1", include_subtree=true)
}
```

### Getting Compression Audit

```helen
import std.core.*
import std.transcript.*
main {
    // Get audit trail of all compression events
    let audit = get_compression_audit()
    for event in audit {
        print("Layer: " + event["layer"])
        print("Before compression: " + str(event["original_token_count"]) + " tokens")
        print("After compression: " + str(event["compressed_token_count"]) + " tokens")
        print("Summary: " + event["summary"])
    }
}
```

**Return fields**:
- `layer`: Compression strategy ("graduated", "traditional", etc.)
- `summary`: Compression summary
- `original_token_count`: Token count before compression
- `compressed_token_count`: Token count after compression
- `timestamp`: Compression timestamp

**Use cases**:
- Analyze compression efficiency
- Debug compression issues
- Audit conversation history

### Spawn Relationship Tracking (v1.23.7+)

v1.23.7 introduces spawn relationship tracking, allowing you to query and manage spawned child sessions:

```helen
import std.core.*
import std.transcript.*
main {
    // Get direct child sessions of the current session
    let children = get_spawned_sessions()
    for child in children {
        print("Spawned: " + child["session_id"])
        print("  Agent: " + child["agent_name"])
        print("  Time: " + str(child["timestamp"]))
    }

    // Get the full spawn tree (including nested spawns)
    let tree = get_spawn_tree()
    print("Root: " + tree["session_id"])
    for child in tree["children"] {
        print("  Child: " + child["session_id"])
        // Recursively access child["children"]
    }

    // Specify root session
    let tree2 = get_spawn_tree("session_abc123")
}
```

**Functions**:
- `get_spawned_sessions(session_id?)`: Get direct child session list
- `get_spawn_tree(session_id?)`: Get full spawn tree (recursive)

**Returns**:
- `get_spawned_sessions`: List of session metadata containing `session_id`, `agent_name`, `timestamp`
- `get_spawn_tree`: Tree structure containing `session_id` and `children` array

**Use cases**:
- Analyze spawn structure in multi-agent collaboration
- Debug spawn relationships
- Visualize execution flow

### Cascading Deletion (v1.23.7+)

When deleting a session, all spawned child sessions are cascade-deleted by default to avoid orphan transcripts:

```helen
import std.transcript.*
main {
    // Delete session and all its spawns (default)
    delete_session("session_abc123")

    // Delete only the specified session, keep spawns
    delete_session("session_abc123", cascade=false)

    // Delete current session and its spawns
    delete_current_session(confirm=true)  // cascade=true is the default
    delete_current_session(confirm=true, cascade=false)  // Keep spawns

    // Clean up old sessions (cascade-deletes spawns)
    cleanup_sessions(keep_count=10)  // Keep the most recent 10, cascade-delete spawns
    cleanup_sessions(older_than_days=30, cascade=true)  // Delete those older than 30 days
    cleanup_sessions(keep_count=5, cascade=false)  // No cascade, keep spawns
}
```

**Parameters**:
- `delete_session(session_id, cascade?)`: `cascade` defaults to true
- `delete_current_session(confirm?, cascade?)`: `cascade` defaults to true
- `cleanup_sessions(keep_count?, older_than_days?, cascade?)`: `cascade` defaults to true

**Return value**: Contains a `deleted_sessions` list showing all actually deleted session IDs

**Design rationale**:
- Spawns are child tasks; their lifecycle should be bound to the main session
- Avoids orphan transcripts (spawns lose context after the main session is deleted)
- Simplifies cleanup — no need to manually find and delete all spawns

### Session Recovery

```helen
import std.core.*
import std.transcript.*
main {
    // Resume a specific session
    let success = resume_session("session_1783492628_d9d9c0aa")
    if success {
        print("Session resumed")
        // v1.23: Restored messages are now visible to the LLM
        let messages = replay_transcript()
        print("Loaded " + str(len(messages)) + " messages")
    } else {
        print("Resume failed, session may not exist")
    }
}
```

**Parameters**:
- `session_id`: Session ID to resume

**Returns**: true (success) or false (failure)

**Use cases**:
- Resume a previous conversation (after v1.23, LLM can see historical messages)
- Continue previous work in the REPL
- Load historical sessions for analysis

**v1.23 changes**:
- Before: Replaced transcript store reference; LLM couldn't see restored messages
- Now: Imports messages into the current store; LLM can see restored messages (marked with current invocation_id)
- For precise restoration by agent/invocation, use `restore_context`

### Session Recovery at Startup (v1.24+)

v1.24 adds CLI arguments to specify a historical session to recover at startup, rather than creating a new session each time:

```bash
# 1. Start with a specific session_id
helen --session=session_xxx file.helen
helen repl --session=session_xxx

# 2. Automatically recover the most recent session
helen --resume-latest file.helen
helen repl --resume-latest
helen repl -r  # Shorthand
```

**Python API**:

```python
from helen.interpreter import Interpreter

# Recover a specific session
interp = Interpreter(session_id="session_xxx")
interp.execute_file("file.helen")

# Recover the most recent session
from helen.runtime.session_manager import SessionManager
manager = SessionManager()
sessions = manager.list_sessions()
if sessions:
    latest_sid = sessions[0]["session_id"]
    interp = Interpreter(session_id=latest_sid)
```

**Difference from `resume_session()`**:

| Feature | `--session` (at startup) | `resume_session()` (at runtime) |
|---------|--------------------------|----------------------------------|
| Timing | Before interpreter starts | During program execution |
| Behavior | Directly reuses the specified session | Imports historical messages into the current new session |
| Transcript | One file | Two files |
| Use case | REPL continuation, debugging | Context switching in code |

**Design rationale**:
- Continuity: Continue work where you left off
- Debug-friendly: Repeatedly debug the same session
- Resource efficiency: Avoid creating many short-lived transcript files
- Explicit over implicit: Must explicitly specify which session to recover

### Chinese Aliases

Transcript functions support Chinese aliases — you can directly use Chinese function names:

| English | Chinese | Description |
|---------|---------|-------------|
| `get_session_id` | `获取会话id` | Get current session ID |
| `list_sessions` | `列出会话` | List all sessions |
| `replay_transcript` | `回放会话` | Replay session messages |
| `replay_full_session` | `回放完整会话` | Replay session and its spawns (v1.23.7) |
| `export_transcript` | `导出会话` | Export session to file |
| `get_compression_audit` | `压缩审计` | Get compression history |
| `resume_session` | `恢复会话` | Resume a specific session |
| `get_spawned_sessions` | `获取子会话` | Get spawned child sessions (v1.23.7) |
| `get_spawn_tree` | `获取会话树` | Get full spawn tree (v1.23.7) |

**Usage examples**:

```helen
import std.core.*
main {
    // Use Chinese function names
    let 会话id = 获取会话id()
    print("当前会话: " + 会话id)

    // List all sessions
    let 会话列表 = 列出会话()
    for 会话 in 会话列表 {
        print(会话["session_id"] + ": " + str(会话["message_count"]) + " 条消息")
    }

    // Replay current session
    let 消息列表 = 回放会话()
    for 消息 in 消息列表 {
        print(消息["role"] + ": " + 消息["content"])
    }

    // Export session
    导出会话("我的对话.json", "json")

    // Resume session
    let 成功 = 恢复会话("session_1783492628_d9d9c0aa")
}
```

> **Tip**: Chinese aliases and English function names can be mixed freely; Helen loads all aliases at startup. Full alias list: see `helen/stdlib/locales/zh.py`.

### REPL Commands

In addition to stdlib functions, you can use these commands in the REPL:

```
:transcript           # Show current transcript view
:transcript --full    # Show full transcript (including compressed messages)
:transcript --audit   # Show compression audit trail
:sessions             # List all sessions
:session_id           # Show current session ID
:resume <session_id>  # Resume a specific session
```

### Configuration

Configure transcript in `~/.helen/config.yaml`:

```yaml
transcript:
  enabled: true              # Enable session recording (default true)
  backend: "sqlite"          # Backend type: "jsonl" or "sqlite"
  session_dir: "~/.helen/sessions"
```

**Backend selection**:
- `jsonl`: Simple, human-readable, crash-safe (default)
- `sqlite`: High-performance, index-optimized (suitable for large sessions)

**Detailed docs**: see [[runtime/transcript-store]]

## Debug Functions (v1.40+)

Debug functions provide AI-native debugging capabilities, including structured error diagnostics, output validation, transcript querying, LLM recording/replay, and data lineage tracking.

### Structured Error Diagnostics (4)

#### last_error_detail — Get detailed error information

```helen
import std.debug.*
main {
    // After an error occurs, get detailed diagnostics
    let err = last_error_detail()
    if err != null {
        debug("Error category", error_category(err))
        debug("Fix suggestion", error_suggestion(err))
        debug("Data flow", error_data_flow(err))
    }
}
```

**Returns**: `dict | null`
- `error_type`: str - The type of error (e.g., "RuntimeError", "LLMError")
- `message`: str - The error message
- `location`: str - Source location (file:line:col)
- `call_stack`: list - Call stack at error time
- `scope`: dict - Local variables in scope
- `trace`: list - Recent execution trace
- `timestamp`: float - When the error occurred
- `diagnostic_category`: str - Semantic classification (e.g., "LLMTimeout")
- `suggestion`: str - Actionable suggestion for fixing the error
- `data_flow`: list - Data flow tracing information

Returns `null` if no error has occurred yet.

#### error_category — Extract diagnostic category

```helen
import std.debug.*
main {
    let err = last_error_detail()
    if err != null {
        let category = error_category(err)
        debug("Error category", category)
        // category: "LLMTimeout", "AgentCallFailed", "RuntimeGenericError", etc.
    }
}
```

**Parameters**:
- `error`: `dict` - Error dictionary from `last_error_detail()`

**Returns**: `str` - The diagnostic category, or "Unknown" if not available.

#### error_suggestion — Extract fix suggestion

```helen
import std.debug.*
main {
    let err = last_error_detail()
    if err != null {
        let suggestion = error_suggestion(err)
        debug("Fix suggestion", suggestion)
        // suggestion: "LLM 调用超时。考虑：(1) 增加 timeout 配置..."
    }
}
```

**Parameters**:
- `error`: `dict` - Error dictionary from `last_error_detail()`

**Returns**: `str` - The suggestion string, or empty string if not available.

#### error_data_flow — Extract data flow information

```helen
import std.debug.*
main {
    let err = last_error_detail()
    if err != null {
        let flows = error_data_flow(err)
        for flow in flows {
            debug("Data flow", flow)
            // flow: {"variable": "x", "source": "msg-123", "via": "Coder"}
        }
    }
}
```

**Parameters**:
- `error`: `dict` - Error dictionary from `last_error_detail()`

**Returns**: `list` - List of data flow entries (each a dict with variable/source/via fields).

### Output Validation (1)

#### validate_output — Validate output against contract

```helen
import std.debug.*
main {
    // Validate JSON
    let result = validate_output('{"name": "Alice"}', "json")
    if result.valid {
        debug("Valid JSON", result.parsed)
    }
    
    // Validate schema
    let schema = {type: "object", required: ["name"]}
    let result = validate_output('{"name": "Alice"}', schema)
    if !result.valid {
        debug("Validation failed", result.violation)
    }
}
```

**Parameters**:
- `output`: `str` - The output string to validate
- `contract`: `str | dict | null` - Contract specification ("json", "text", or schema dict)

**Returns**: `dict`
- `valid`: `bool` - Whether output matches contract
- `violation`: `str` - Description of violation (empty if valid)
- `parsed`: `Any` - Parsed output if applicable (e.g., parsed JSON)

**Supported contract types**:
- `"json"`: Validate that output is valid JSON
- `"text"`: Always passes (for explicit marking)
- `null`: Always passes
- Schema dict: Validate against JSON Schema-like specification
  - `type`: "string", "number", "integer", "boolean", "array", "object"
  - `required`: List of required field names
  - `properties`: Dict of property schemas
  - `enum`: List of allowed values
  - `min`, `max`: Number constraints
  - `minLength`, `maxLength`: String length constraints

### Transcript Query (1)

#### query_transcript — Query transcript with filtering and pagination

```helen
import std.debug.*
main {
    // Query all assistant messages
    let msgs = query_transcript(role="assistant")
    
    // Query messages from specific agent
    let coder_msgs = query_transcript(agent="Coder")
    
    // Paginated query
    let page1 = query_transcript(limit=100, offset=0)
    let page2 = query_transcript(limit=100, offset=100)
    
    // Regex search
    let errors = query_transcript(content_regex="Error:")
    
    // Time range query
    let recent = query_transcript(since=now() - 3600)  // Last hour
    
    // Combined query
    let filtered = query_transcript(
        role="assistant",
        agent="Reviewer",
        content_regex="verdict",
        limit=50
    )
}
```

**Parameters**:
- `session_id`: `str` - Session ID (empty = current session)
- `role`: `str` - Role filter ("user", "assistant", "tool")
- `agent`: `str` - Agent name filter
- `invocation_id`: `str` - Invocation ID filter
- `since`: `float` - Timestamp lower bound
- `until`: `float` - Timestamp upper bound
- `content_regex`: `str` - Content regex match
- `message_type`: `str` - Message type filter
- `limit`: `int` - Maximum results to return (default: 1000)
- `offset`: `int` - Results to skip for pagination (default: 0)

**Returns**: `list[dict]` - List of message dictionaries matching the filters.

**Backend optimization**:
- JSONL backend: Streaming filter + 100k item limit (prevents OOM)
- SQLite backend: SQL WHERE pushdown (O(log n) query)

### Recording/Replay (3)

#### record_session — Start recording LLM interactions

```helen
import std.debug.*
main {
    let result = record_session("debug/session.jsonl")
    // result: {"status": "recording", "cassette_path": "debug/session.jsonl"}
    
    // Run agent (all LLM calls are recorded)
    agent Reviewer {
        main {
            llm act "Review this code..."
        }
    }
    
    stop_recording()
}
```

**Parameters**:
- `cassette_path`: `str` - Path to save the cassette file (JSONL format)

**Returns**: `dict`
- `status`: `"recording"` or `"error"`
- `cassette_path`: Path to the cassette file
- `message`: Status message

#### stop_recording — Stop recording LLM interactions

```helen
import std.debug.*
main {
    record_session("debug/session.jsonl")
    // ... run agent ...
    let result = stop_recording()
    // result: {"status": "stopped", "message": "Recording stopped"}
}
```

**Returns**: `dict`
- `status`: `"stopped"` or `"error"`
- `message`: Status message

#### replay_session — Replay LLM interactions from cassette

```helen
import std.debug.*
main {
    let result = replay_session("debug/session.jsonl")
    // result: {"status": "replaying", "entry_count": 5}
    
    // Now all LLM calls use recorded responses
    agent Reviewer {
        main {
            llm act "Review this code..."  // Returns recorded response
        }
    }
}
```

**Parameters**:
- `cassette_path`: `str` - Path to the cassette file to replay from

**Returns**: `dict`
- `status`: `"replaying"` or `"error"`
- `cassette_path`: Path to the cassette file
- `entry_count`: Number of recorded interactions
- `message`: Status message

**Cassette file format** (JSONL):
```json
{
  "type": "llm_call",
  "seq": 0,
  "timestamp": 1234567890.123,
  "agent_name": "Reviewer",
  "model": "qwen3.7-plus",
  "request": {"messages": [...], "tools": [...]},
  "response": {"content": "...", "tool_calls": [...]},
  "usage": {"prompt_tokens": 100, "completion_tokens": 50},
  "duration_ms": 1234.5
}
```

### Data Lineage Tracking (4)

#### trace_value_origin — Trace data origin

```helen
import std.debug.*
main {
    let origins = trace_value_origin("msg_xyz")
    // origins: [
    //   {"producer_uuid": "msg_abc", "flow_type": "agent_call", ...}
    // ]
}
```

**Parameters**:
- `message_uuid`: `str` - UUID of the message to trace

**Returns**: `list[dict]` - List of data flow entries showing where the data came from.

#### trace_value_consumers — Trace data consumers

```helen
import std.debug.*
main {
    let consumers = trace_value_consumers("msg_abc")
    // consumers: [
    //   {"consumer_uuid": "msg_xyz", "flow_type": "agent_call", ...}
    // ]
}
```

**Parameters**:
- `message_uuid`: `str` - UUID of the message to trace

**Returns**: `list[dict]` - List of data flow entries showing which messages consumed the data.

#### get_data_lineage — Get complete data lineage graph

```helen
import std.debug.*
main {
    let lineage = get_data_lineage()
    // lineage: {
    //   "nodes": ["msg_abc", "msg_xyz", ...],
    //   "edges": [
    //     {"source": "msg_abc", "target": "msg_xyz", "flow_type": "agent_call", ...}
    //   ]
    // }
}
```

**Returns**: `dict`
- `nodes`: List of message UUIDs
- `edges`: List of data flow edges

#### record_data_flow — Manually record a data flow event

```helen
import std.debug.*
main {
    let result = record_data_flow(
        "msg_abc",           // Producer UUID
        "msg_xyz",           // Consumer UUID
        "agent_call",        // Flow type
        {"arg": "input"}     // Metadata
    )
    // result: {"status": "recorded", "message": "Recorded agent_call flow..."}
}
```

**Parameters**:
- `producer_uuid`: `str` - UUID of the message that produced the data
- `consumer_uuid`: `str` - UUID of the message that consumed the data
- `flow_type`: `str` - Type of flow ("channel", "agent_call", "prompt", or custom)
- `metadata`: `dict | null` - Optional metadata dict

**Returns**: `dict`
- `status`: `"recorded"` or `"error"`
- `message`: Status message

---

## File Functions (18)

File operation functions are organized into three groups: basic I/O, directory operations, and file search.

### File Search (2) (added in v1.15)

#### glob_files — Recursively find files

```helen
import std.file.*
main {
    // Find all Python files (recursive)
    let py_files = glob_files("src", "*.py")
    // Returns: ["main.py", "utils/helper.py", "tests/test_main.py"]

    // Find files matching a specific pattern
    let test_files = glob_files(".", "*test*.py")
    // Returns: ["test_main.py", "tests/test_utils.py"]

    // Use ** for explicit recursion
    let md_files = glob_files("docs", "**/*.md")
    // Returns: ["readme.md", "guide/intro.md", "api/reference.md"]

    // Complex patterns
    let config_files = glob_files("config", "**/*.{json,yaml,yml}")
    // Returns list of config files
}
```

**Parameters**:
- `path` (str): Search root directory
- `pattern` (str, optional): Glob pattern, defaults to `"**/*"` (all files)

**Returns**: `list<str>` — list of relative paths of matching files

**Example**: Count lines of all Python files in a project

```helen
import std.core.*
import std.file.*
import std.str.*
fn 统计代码行数(目录: str) {
    let files = glob_files(目录, "*.py")
    let total_lines = 0
    for file in files {
        let content = read_file(file)
        total_lines += len(split(content, "\n"))
    }
    return {"files": len(files), "lines": total_lines}
}

main {
    let stats = 统计代码行数("src")
    print("Found " + str(stats["files"]) + " files, " + str(stats["lines"]) + " lines total")
}
```

#### grep_files — Search file contents

```helen
import std.file.*
main {
    // Literal search
    let matches = grep_files("src/", "TODO")
    // Returns: [{"file": "main.py", "line": 42, "text": "    # TODO: fix this"}]

    // Regex search
    let functions = grep_files("src/", "def \\w+\\(", regex=true)
    // Returns all function definitions

    // Case-insensitive search
    let errors = grep_files("logs/", "error", case_sensitive=false)
    // Returns all lines containing "error" (case-insensitive)

    // Limit number of results
    let first_10 = grep_files(".", "pattern", max_results=10)
}
```

**Parameters**:
- `path` (str): File path or directory
- `pattern` (str): Search pattern (literal or regex)
- `regex` (bool, optional): Whether to use regex, defaults to `false`
- `case_sensitive` (bool, optional): Case-sensitive search, defaults to `true`
- `max_results` (int, optional): Maximum results to return, defaults to `100`

**Returns**: `list<map>` — list of match results, each match contains `file`, `line`, `text` fields

**Example**: Find all unhandled exceptions

```helen
import std.core.*
import std.file.*
agent 异常检查助手 {
    description "Check for unhandled exceptions in code"
    main {
        let todos = grep_files("src/", "TODO.*exception", regex=true)
        if len(todos) > 0 {
            print("Found " + str(len(todos)) + " pending exception(s):")
            for match in todos {
                print("  " + match["file"] + ":" + str(match["line"]))
                print("    " + match["text"])
            }
        }
    }
}
```

### Basic File I/O (2)

```helen
import std.core.*
import std.io.*
main {
    // Read file
    let content = read_file("config.json")

    // Write file (auto-creates parent directories)
    write_file("output/result.txt", "Hello World")
}
```

### File Information (2)

```helen
import std.core.*
import std.file.*
main {
    // File size (bytes)
    let size = file_size("document.pdf")
    print("File size: " + str(size) + " bytes")

    // Modification time (ISO 8601 format)
    let mtime = file_modified("data.csv")
    print("Last modified: " + mtime)
}
```

### Directory Operations (6)

```helen
import std.core.*
import std.file.*
import std.io.*
main {
    // List directory contents
    let files = list_dir("src")
    // Returns: ["main.py", "utils.py", "tests/"]

    // With pattern filter
    let py_files = list_dir("src", "*.py")
    // Returns: ["main.py", "utils.py"]

    // Recursively walk directory tree
    let tree = walk_dir("project")
    // Returns: [(dirpath, dirnames, filenames), ...]
    for entry in tree {
        let dir = entry[0]
        let subdirs = entry[1]
        let files = entry[2]
        print(dir + ": " + str(len(files)) + " files")
    }

    // Create directories
    mkdir("new_dir")
    mkdir_p("deep/nested/dir")  // Recursive creation

    // Delete
    delete_file("temp.txt")
    delete_dir("old_dir", recursive=true)
}
```

### File Operations (2)

```helen
import std.file.*
main {
    // Copy file
    copy_file("source.txt", "backup.txt")

    // Move/rename file
    move_file("old_name.txt", "new_name.txt")
}
```

### Temporary Files (2)

```helen
import std.file.*
import std.io.*
main {
    // Create temporary file
    let tmp = temp_file(suffix=".txt", prefix="data_")
    write_file(tmp, "temporary data")
    // Must manually delete after use
    delete_file(tmp)

    // Create temporary directory
    let tmp_dir = temp_dir(prefix="build_")
    // Must manually delete after use
    delete_dir(tmp_dir, recursive=true)
}
```

### Path Operations (6)

```helen
import std.path.*
main {
    // Path joining
    let full_path = path_join("src", "utils", "helper.py")
    // Returns: "src/utils/helper.py"

    // Extract path components
    let base = path_basename("/path/to/file.txt")  // "file.txt"
    let dir = path_dirname("/path/to/file.txt")    // "/path/to"

    // Path checks
    let exists = path_exists("config.json")
    let is_dir = path_is_dir("src")
    let is_file = path_is_file("main.py")
}
```

## Exception Handling (v1.9+)

Python exceptions thrown during standard library function calls are automatically wrapped as `RuntimeError` and can be caught with try-catch:

```helen
import std.core.*
main {
    try {
        let x = len(42)           // Python TypeError
    } catch RuntimeError err {
        print(err.message)        // "Python TypeError: object of type 'int' has no len()"
    }

    try {
        let data = read_file("/nonexistent")
    } catch RuntimeError err {
        print(err.message)        // "Python FileNotFoundError: [Errno 2] ..."
    }
}
```

Exception messages follow the format `"Python <type name>: <original message>"`, and you can distinguish specific Python exception types by the message prefix in catch blocks.

## MCP Tools Integration (v1.33+)

Helen v1.33 introduces MCP (Model Context Protocol) client support, allowing agents to dynamically discover and call tools from external MCP servers. MCP tools integrate seamlessly with built-in tools.

### Configuration

Create `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "codebase-memory": {
      "command": "npx",
      "args": ["-y", "@anthropic-ai/codebase-memory-mcp"],
      "tool_timeout_sec": 60
    }
  }
}
```

### Using MCP Tools

MCP tools are automatically discovered and can be used in agent `tools` declarations:

```helen
import std.core.*
agent CodeAnalyzer {
    description = "Analyze code using codebase-memory"
    
    // MCP tools are referenced by name, just like built-in tools
    tools = ["search_code", "get_code_snippet", "read_file"]
    
    main {
        // LLM can now call search_code and get_code_snippet from MCP
        let result = llm act "Search for authentication-related functions"
        print(result)
    }
}
```

### How It Works

1. **Lazy initialization**: MCP is initialized on first tool access
2. **Tool discovery**: Helen queries each MCP server for available tools
3. **Schema conversion**: MCP tool schemas are converted to OpenAI format
4. **Unified dispatch**: MCP tools are called via `dispatch_tool()` like built-in tools

### Priority Order

When LLM calls a tool, Helen checks in this order:
1. Built-in tools (highest priority)
2. Agent functions (`functions {}` block)
3. MCP tools (lowest priority)

### Error Handling

MCP errors don't crash your program:

```helen
main {
    // If MCP tool fails, LLM receives error JSON
    // and can handle it appropriately
    let result = llm act "Try calling an MCP tool"
    
    // MCP tools return JSON results
    // {"output": "..."} or {"error": "..."}
}
```

### Complete Guide

See [MCP Integration Guide](../runtime/mcp-integration.md) for:
- Architecture details
- Multiple MCP servers
- Error handling strategies
- Performance considerations
- Troubleshooting

