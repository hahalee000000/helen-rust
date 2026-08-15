# Chapter 9: Standard Library at a Glance

## Overview

The Helen standard library provides **364 built-in functions** across 22 modules, covering all the core needs of AI application development. You don't need to memorize all of them — this chapter gives you a map of what's available so you can look things up when you need them.

## Import Forms

```helen
// Import all core functions
import std.core.*

// Import specific functions from a module
import std.str.{upper, lower, split}
import std.list.{sort, filter, map}

// Import under a namespace (avoids name collisions)
import std.dict as Dict
import std.list as List

// Use directly (stdlib functions are globally available; imports make dependencies explicit)
```

## Module Catalog

### Core Functions (`std.core`)

The most commonly used primitives:

```helen
import std.core.*

main {
    print("hello")           // print  (打印)
    let n = len("hello")     // len    (长度)
    let s = str(42)          // str    (字符串化 — "to string")
    let i = int("42")        // int    (整数化 — "to integer")
    let f = float("3.14")    // float  (浮点化 — "to float")
    let b = bool(1)          // bool   (布尔化 — "to boolean")
    let t = type(42)         // type   (类型)
    let m = max(1, 2, 3)     // max    (最大值)
    let m2 = min(1, 2, 3)    // min    (最小值)
    let r = range(5)         // range  (范围)
    let a = abs(-5)          // abs    (绝对值)
}
```

| Function | Chinese Alias | Description |
|----------|---------------|-------------|
| `print` | `打印` | Print to console |
| `len` | `长度` | Get length |
| `str` | `字符串化` | Convert to string |
| `int` | `整数化` | Convert to integer |
| `float` | `浮点化` | Convert to float |
| `bool` | `布尔化` | Convert to boolean |
| `type` | `类型` | Get type name |
| `isinstance` | `是实例` | Check type |
| `range` | `范围` | Generate integer range |
| `abs` | `绝对值` | Absolute value |
| `min` / `max` | `最小值` / `最大值` | Minimum / maximum |
| `exit` | `退出` | Exit the program |

### String Functions (`std.str`)

```helen
import std.str.*

main {
    // Case
    upper("hello")          // "HELLO"
    lower("WORLD")          // "world"

    // Whitespace
    strip("  hello  ")      // "hello"

    // Split and join
    split("a,b,c", ",")     // ["a", "b", "c"]
    join(["a", "b", "c"], "-")  // "a-b-c"

    // Find and replace
    find("hello world", "world")  // 6
    replace("hello", "l", "L")    // "heLLo"

    // Regex
    regex_match("\\d+", "abc123")  // "123"
    regex_replace("\\d+", "x", "abc123")  // "abcx"
    regex_split("\\s+", "a b  c")  // ["a", "b", "c"]

    // Misc
    reverse("hello")        // "olleh"
    repeat("ab", 3)         // "ababab"
    contains("hello", "ell")  // true
    substring("hello", 1, 3)  // "el"
}
```

### Collection Functions (`std.list` / `std.dict`)

```helen
import std.list.*
import std.dict.*

main {
    let nums = [3, 1, 4, 1, 5, 9, 2, 6]

    // List operations
    sort(nums)                    // [1, 1, 2, 3, 4, 5, 6, 9]
    reverse(nums)                 // [6, 2, 9, 5, 1, 4, 1, 3]
    unique(nums)                  // [3, 1, 4, 5, 9, 2, 6]
    flatten([[1, 2], [3, 4]])    // [1, 2, 3, 4]
    chunk([1,2,3,4,5], 2)        // [[1,2], [3,4], [5]]
    zip([1,2,3], ["a","b","c"])  // [[1,"a"], [2,"b"], [3,"c"]]

    // Higher-order functions
    map([1,2,3], fn(x) { return x * 2 })          // [2, 4, 6]
    filter([1,2,3,4], fn(x) { return x % 2 == 0 }) // [2, 4]
    reduce([1,2,3,4], fn(a,b) { return a + b }, 0) // 10

    // Dict operations
    let d = {"name": "Alice", "age": 30}
    keys(d)           // ["name", "age"]
    values(d)         // ["Alice", 30]
    get(d, "name")    // "Alice"
    has_key(d, "name")  // true
}
```

### Data Processing (`std.data`)

```helen
import std.data.*

main {
    // JSON
    let obj = json_parse('{"name": "Alice", "age": 30}')
    let text = json_stringify({"name": "Bob", "age": 25})

    // Lenient parsing (tolerant of malformed JSON)
    let loose = json_parse_lenient('{name: "Alice"}')

    // YAML / TOML / CSV
    let yaml_data = yaml_parse("name: Alice")
    let toml_data = toml_parse('title = "Config"')
    let csv_data = csv_parse("name,age\nAlice,30")

    // XML / HTML
    let xml_data = xml_parse("<root><item>hello</item></root>")
}
```

### File Operations (`std.file`)

```helen
import std.file.*
import std.path.*

main {
    // Read and write files
    write_file("test.txt", "hello world")
    let content = read_file("test.txt")
    append_file("test.txt", "\nmore content")

    // Directory operations
    mkdir_p("output/sub/dir")
    let files = list_dir(".")

    // File info
    let size = file_size("test.txt")
    let exists = path_exists("test.txt")

    // Path operations
    let base = path_basename("/home/user/file.txt")  // "file.txt"
    let dir = path_dirname("/home/user/file.txt")    // "/home/user"
    let joined = path_join("/home", "user", "file.txt")  // "/home/user/file.txt"

    // Search
    let matches = glob_files("*.py")
    let hits = grep_files("import", ["main.py"])
}
```

### Network Functions (`std.network`)

```helen
import std.network.*

main {
    // HTTP requests
    let resp = http_get("https://api.example.com/data")
    let result = http_post("https://api.example.com/create", '{"name":"test"}')
    http_put("https://api.example.com/update/1", '{"name":"new"}')
    http_delete("https://api.example.com/delete/1")

    // Download a file
    http_download("https://example.com/file.pdf", "local_file.pdf")

    // URL operations
    let parsed = url_parse("https://example.com/path?q=1")
    let encoded = url_encode("hello world")
    let decoded = url_decode("hello%20world")
}
```

### Time Functions (`std.time`)

```helen
import std.time.*

main {
    let ts = now()                         // Current timestamp
    let d = date()                         // Current date
    let fmt = date_format(now(), "%Y-%m-%d %H:%M:%S")
    let parsed = date_parse("2024-01-15", "%Y-%m-%d")
    let shifted = date_add(now(), 1, "day")     // Add one day
    let delta = date_diff(date1, date2, "day")  // Difference in days
    sleep(2)                                    // Pause for 2 seconds
}
```

### Math Functions (`std.math`)

```helen
import std.math.*

main {
    round(3.14159, 2)   // 3.14  Round to N decimal places
    floor(3.7)          // 3     Round down
    ceil(3.2)           // 4     Round up
    sqrt(16)            // 4.0   Square root
    pow(2, 10)          // 1024  Exponentiation
    sum([1, 2, 3, 4])   // 10    Sum of list
    mean([1, 2, 3, 4])  // 2.5   Arithmetic mean
    median([1, 3, 2])   // 2     Median
}
```

### System Functions (`std.system`)

```helen
import std.system.*

main {
    // Environment variables
    let home = env_get("HOME")
    env_set("MY_VAR", "value")

    // Run shell commands
    let result = shell_exec("ls -la")
    let plat = platform()  // "linux" / "darwin" / "win32"

    // CLI arguments
    let args = get_cli_args()
}
```

### Crypto Functions (`std.crypto`)

```helen
import std.crypto.*

main {
    md5("hello")          // Hash
    sha256("hello")       // Stronger hash
    random()              // Random float in 0..1
    randint(1, 100)       // Random integer in 1..100
    choice(["a", "b", "c"])  // Pick one at random
    shuffle([1, 2, 3, 4])    // Shuffle in place
    uuid_generate()       // Generate a UUID
}
```

### Multimedia Functions (`std.media`)

```helen
import std.media.*

main {
    // Load image / video / audio
    let img = media("photo.jpg")
    let kind = media_type(img)  // "image"

    // Type checks
    is_image(img)                    // true
    is_video(media("video.mp4"))     // true

    // Save media
    save_media(img, "output.png")
}
```

### Observability Functions (`std.debug`)

```helen
import std.debug.*

main {
    // Debug printing
    debug("checkpoint", {"variable": value})

    // Execution tracing
    trace_on()
    // ... code under trace ...
    let trace_log = get_trace(50)
    trace_off()

    // Code coverage
    coverage_on()
    // ... code under coverage ...
    let report = coverage_report("text")
    coverage_off()
}
```

## Chinese Aliases

Most standard library functions have Chinese aliases that can be used directly without any extra import:

```helen
// Chinese and English forms are equivalent
打印("hello")       == print("hello")
长度("hello")       == len("hello")
排序([3, 1, 2])     == sort([3, 1, 2])
过滤([1,2,3], fn)   == filter([1,2,3], fn)
json解析('{"a":1}') == json_parse('{"a":1}')
读文件("test.txt")  == read_file("test.txt")
```

The full list of Chinese aliases lives in `helen/stdlib/locales/zh.py`.

## Chapter Summary

- The Helen standard library has **364 built-in functions** across **22 modules**
- Import an entire module with `import std.<module>.*`, or import specific functions with `import std.<module>.{fn1, fn2}`
- Most functions have Chinese aliases that can be used interchangeably
- Key modules: `core` (primitives), `str` (strings), `list` / `dict` (collections), `file` (filesystem), `network` (HTTP), `data` (data formats)
- You don't need to memorize everything — look things up as you go

## Further Reading

- [[reference/10-stdlib|Standard Library Reference]] - The complete 364-function reference (351 Chinese aliases), organized into 22 modules with full signatures and examples
- [[reference/13-skills|Skill System]] - How skills provide structured knowledge to agents at runtime

## Next Chapter

[Chapter 10: Testing and Debugging](10-testing.md) ->
