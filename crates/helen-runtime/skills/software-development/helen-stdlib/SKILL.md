---
name: helen-stdlib
description: "Helen Standard Library Guide — Categorized reference and examples for 407 built-in functions across 21 categories (v1.44 adds fromtimestamp, enhanced list_sessions/delete_session)"
version: 1.44.0
author: Helen Team
license: MIT
metadata:
  hermes:
    tags: [helen, stdlib, builtins, reference]
---
<!-- helen-rust edition: stdlib registry in crates/helen-stdlib (22 modules, 378 builtins + aliases, M4). `helen doc` output byte-identical to reference. Subset of happy paths covered by corpus (coverage 68.82%). -->


# Helen Standard Library Reference

Helen's standard library provides **407 built-in functions** across 21 categories, covering all core needs for AI application development.

## Category Overview

| Category | Count | Representative Functions |
|----------|--------|--------------------------|
| **Core** | 17 | `print`, `len`, `str`, `int`, `float`, `bool`, `list`, `dict`, `abs`, `min`, `max`, `range`, `type`, `isinstance`, `input`, `multiline_input`, `exit` |
| **String** | 43 | `upper`, `lower`, `strip`, `split`, `join`, `replace`, `find`, `find_from`, `reverse`, `repeat`, `regex_match`, `regex_replace`, `regex_split`, `format_float`, `tokenize`, `levenshtein`, `base64_encode`, `chr`, `ord` |
| **Data** | 28 | `json_parse`, `json_parse_lenient`, `json_stringify`, `yaml_parse`, `toml_parse`, `csv_parse`, `xml_parse`, `html_escape`, `html_parse`, `markdown_parse`, `markdown_to_html` |
| **Collection** | 26 | `sort`, `reverse`, `unique`, `flatten`, `zip`, `map`, `filter`, `reduce`, `chunk`, `set_union`, `set_intersection`, `set_difference`, `remove_key`, `get`, `set_key`, `has_key` |
| **Network** | 9 | `http_get`, `http_post`, `http_put`, `http_delete`, `http_download`, `url_parse`, `url_build`, `url_encode`, `url_decode` |
| **Time** | 17 | `now`, `time`, `date`, `datetime`, `fromtimestamp`, `date_format`, `date_parse`, `date_add`, `date_diff`, `sleep`, `stopwatch_start`, `stopwatch_elapsed`, `stopwatch_lap` |
| **Math** | 27 | `round`, `sqrt`, `floor`, `ceil`, `sum`, `product`, `mean`, `median`, `mode`, `stddev`, `variance`, `percentile`, `correlation`, `cos`, `sin`, `tan`, `pow`, `log`, `log2`, `log10`, `exp` |
| **File** | 12 | `read_file`, `write_file`, `append_file`, `list_dir`, `mkdir`, `mkdir_p`, `copy_file`, `delete_file`, `file_size`, `glob_files`, `grep_files`, `temp_file` |
| **System** | 24 | `env_get`, `env_set`, `env_delete`, `env_list`, `get_cli_args`, `parse_cli_args`, `shell_exec`, `exec`, `exec_async`, `pid`, `exit`, `kill`, `log_info`, `log_error`, `log_debug`, `platform`, `hostname`, `python_version`, `cpu_count`, `memory_info` |
| **Crypto** | 17 | `md5`, `sha1`, `sha256`, `sha512`, `hmac_sha256`, `random`, `randint`, `choice`, `shuffle`, `sample`, `uuid_generate`, `uuid_from_string`, `uuid_nil`, `random_bytes`, `random_hex`, `random_base64` |
| **IO** | 9 | `stream_print`, `stream_clear`, `progress_bar`, `mkdir`, `mkdir_p`, `append_file`, `stream_cursor_up`, `stream_cursor_down` |
| **Path** | 6 | `path_basename`, `path_dirname`, `path_exists`, `path_is_dir`, `path_is_file`, `path_join` |
| **Tools** | 7 | `shell_exec`, `calculate`, `patch_file`, `load_skill`, `list_skill_references`, `web_search`, `web_fetch` |
| **Debug** (v1.40) | 23 | `debug`, `trace_on`, `trace_off`, `get_trace`, `coverage_on`, `coverage_off`, `coverage_report`, `coverage_summary`, `last_error_detail`, `error_category`, `error_suggestion`, `error_data_flow`, `validate_output`, `query_transcript`, `record_session`, `stop_recording`, `replay_session`, `trace_value_origin`, `trace_value_consumers`, `get_data_lineage`, `record_data_flow` |
| **Context** | 29 | `clear_context`, `compress_context`, `compress_context_target`, `context_stats`, `context_usage`, `get_message`, `delete_message`, `pin_message`, `unpin_message`, `list_pinned_messages`, `insert_message`, `replace_message`, `working_memory_get`, `working_memory_set`, `working_memory_remove`, `working_memory_clear`, `set_compression_strategy`, `set_context_window`, `set_working_memory_enabled`, `set_cache_aware`, `get_context_config`, `search_context`, `context_slice`, `export_context`, `import_context`, `fork_context`, `restore_context`, `on_compression`, `on_context_overflow` |
| **Transcript** | 22 | `get_session_id`, `get_session_meta`, `list_sessions`, `replay_transcript`, `replay_full_session`, `export_transcript`, `search_transcript`, `list_invocations`, `get_invocation`, `get_invocation_tree`, `invocation_path`, `get_compression_audit`, `resume_session`, `get_session_dir`, `set_session_dir`, `delete_session`, `delete_current_session`, `cleanup_sessions`, `get_spawned_sessions`, `get_spawn_tree` |
| **Media** | 12 | `media`, `media_base64`, `is_media`, `media_type`, `to_openai_parts`, `to_claude_parts`, `to_gemini_parts`, `media_to_base64`, `save_media`, `is_image`, `is_video`, `is_audio` |
| **Test** | 23 | `test_suite`, `test_case`, `test_case_skip`, `test_end_suite`, `set_test_timeout`, `run_tests`, `run_tests_json`, `test_count`, `test_reset`, `before_all`, `after_all`, `before_each`, `after_each`, `assert_equal`, `assert_not_equal`, `assert_true`, `assert_contains`, `assert_throws`, `describe`, `expect`, `it`, `it_skip`, `fail` |
| **Quality** | 4 | `analyze_code`, `check_security`, `quality_score`, `quality_report` |
| **LLM** | 16 | `cancel_llm_call`, `current_llm_call_id`, `cancel_all_llm_calls`, `set_temperature`, `get_temperature`, `set_max_turns`, `get_max_turns`, `set_max_tokens`, `get_max_tokens`, `set_thinking_mode`, `get_thinking_mode`, `set_reasoning_effort`, `get_reasoning_effort`, `get_model`, `get_description`, `get_provider` |
| **Concurrency** | 1 | `mailbox_select` |

## Multilingual stdlib (v1.10)

Helen's stdlib supports multilingual function names. Every stdlib function has an English canonical name and localized aliases, all loaded at startup.

### ⚠️ stdlib Function Names vs Keywords

**stdlib function names (like `长度`, `打印`, `排序`) are NOT reserved keywords.** They can be used as variable names, though this is discouraged to avoid confusion.

```helen
// ✓ Allowed but discouraged — stdlib names are not reserved
设 长度 = 10          // Shadows the stdlib function 长度()
设 打印 = "hello"     // Shadows the stdlib function 打印()

// ✗ NOT allowed — these are reserved keywords
设 描述 = "hello"     // Error: 描述 is a keyword (agent description)
设 模型 = "qwen"      // Error: 模型 is a keyword (model selection)

// ✓ Recommended — use distinct names
设 数据长度 = 10
设 输出内容 = "hello"
```

**Key distinction**:
- **Keywords** (93 total): Reserved, cannot be identifiers. Examples: `设`, `如果`, `描述`, `模型`, `且`, `或`
- **stdlib functions** (333 Chinese aliases): NOT reserved, can be shadowed. Examples: `长度`, `打印`, `排序`

### Chinese stdlib Aliases

Helen has 351 built-in Chinese aliases covering all stdlib categories. Common examples:

| 英文 | 中文 | 类别 |
|------|------|------|
| `len` | `长度` | Core |
| `print` | `打印` | Core |
| `sort` | `排序` | Collection |
| `filter` | `过滤` | Collection |
| `map` | `映射` | Collection |
| `json_parse` | `json解析` | Data |
| `json_parse_lenient` | `json宽松解析` | Data |
| `json_stringify` | `json序列化` | Data |
| `http_get` | `http获取` | Network |
| `regex_match` | `正则匹配` | String |
| `regex_replace` | `正则替换` | String |
| `regex_split` | `正则分割` | String |
| `find_from` | `从位置查找` | String |
| `format_float` | `格式化浮点` | String |
| `chr` | `字符` | String |
| `ord` | `码点` | String |
| `date_format` | `日期格式化` | Time |
| `read_file` | `读文件` | File |
| `write_file` | `写文件` | File |
| `shell_exec` | `执行命令` | System |

For the complete list, see `helen/stdlib/locales/zh.py`.

### Usage Examples

```helen
// Use Chinese stdlib function names directly (no import or alias needed)
import std.core.*
函数 数据处理() {
    定义 原始数据 = [3, 1, 4, 1, 5, 9, 2, 6]
    定义 排序后 = 排序(原始数据)
    定义 去重后 = 去重(排序后)
    返回 长度(去重后)
}

// Mixing Chinese and English is also perfectly legal
函数 混合使用() {
    let data = [1, 2, 3]
    let sorted = 排序(data)     // English variables + Chinese function
    return len(sorted)
}
```

### Custom Aliases

```helen
alias len as 我的长度
别名 print as 输出
```

### Design Principles

- **Single mechanism**: stdlib aliases and user `alias` use the same Environment binding
- **Full loading**: All locale alias tables are registered at startup, not filtered by locale
- **locale only affects display**: `locale: zh` in `~/.helen/config.yaml` only affects the language of docs/LSP/error messages
- **Extending to new languages**: Adding a new language only requires creating `helen/stdlib/locales/<code>.py`

## Common Function Examples

### Core

```helen
import std.core.*
main {
    // Type conversion
    let num = int("42")           // string → integer
    let text = str(3.14)          // float → string
    let flt = float("2.5")        // string → float

    // Length and range
    let length = len([1, 2, 3])   // 3
    let items = range(0, 10, 2)   // [0, 2, 4, 6, 8]

    // Math basics
    let maximum = max(1, 2, 3)    // 3
    let minimum = min(1, 2, 3)    // 1
    let absolute = abs(-42)       // 42

    // Type checking
    if isinstance(value, str) {
        print("It's a string")
    }
}
```

### String

```helen
import std.core.*
import std.str.*
main {
    // Case conversion
    let upper = upper("hello")    // "HELLO"
    let lower = lower("WORLD")    // "world"

    // Split and join
    let parts = split("a,b,c", ",")  // ["a", "b", "c"]
    let joined = join(["a", "b"], "-")  // "a-b"

    // Find and replace
    let found = find("hello world", "world")  // 6
    let replaced = replace("foo bar", "foo", "baz")  // "baz bar"

    // Substring extraction (Python-style: start inclusive, end exclusive)
    let s = "Hello, World!"
    let head = substring(s, 0, 5)      // "Hello"   — indices 0..4
    let tail = substring(s, 7)         // "World!"  — omit end → to string end
    // ⚠️ third param is an EXCLUSIVE INDEX, not a length:
    //    substring("id=repo1", 3, 5)  → "re"  (NOT "repo1")
    //    substring("id=repo1", 3)     → "repo1"
    //    to extract N chars from pos P:  substring(s, P, P + N)

    // Regular expressions
    if regex_match("hello123", r"\d+") {
        print("Contains digits")
    }
    let cleaned = regex_replace("a1b2c3", r"\d", "")  // "abc"

    // Split by regex pattern (multiple delimiters)
    let tokens = regex_split("a, b; c  d", r"[,;\s]+")  // ["a", "b", "c", "d"]

    // Find from position
    let text = "hello world hello"
    let pos1 = find(text, "hello")              // 0 (first occurrence)
    let pos2 = find_from(text, "hello", 6)      // 12 (second occurrence)

    // Whitespace handling
    let trimmed = strip("  hello  ")  // "hello"
    let padded = pad_start("42", 5, "0")  // "00042"

    // Character ↔ code point (v1.31+)
    let ch = chr(65)              // "A"
    let cp = ord("中")            // 20013
    let roundtrip = ord(chr(97))  // 97

    // Chinese aliases
    let ch2 = 字符(65)            // "A"
    let cp2 = 码点("A")           // 65

    // Float formatting
    let formatted1 = format_float(8.5, 1)      // "8.5"
    let formatted2 = format_float(7.857, 2)    // "7.86" (rounded)
    let formatted3 = format_float(3.14159, 3)  // "3.142"

    // Chinese aliases
    let formatted = 格式化浮点(8.5, 1)  // "8.5"
}
```

### Data

```helen
import std.data.*
import std.network.*
import std.str.*
main {
    // JSON
    let data = json_parse('{"name": "Helen", "version": 1}')
    let json = json_stringify(data, indent=2)

    // Lenient JSON parsing (handles markdown fences from LLM output)
    let llm_output = "```json\n{\"key\": \"value\"}\n```"
    let data2 = json_parse_lenient(llm_output)  # Automatically strips fences

    # YAML
    let config = yaml_parse("key: value\nlist:\n  - item1\n  - item2")

    # CSV
    let rows = csv_parse("name,age\nAlice,30\nBob,25")
    # [["name", "age"], ["Alice", "30"], ["Bob", "25"]]

    # URL encoding
    let encoded = url_encode("hello world&foo=bar")
    let decoded = url_decode(encoded)

    # Base64
    let encoded = base64_encode("secret data")
    let decoded = base64_decode(encoded)
}
```

### Collection

```helen
import std.list.*
main {
    // Sort and deduplicate
    let sorted = sort([3, 1, 4, 1, 5])  // [1, 1, 3, 4, 5]
    let unique_items = unique([1, 2, 2, 3])  // [1, 2, 3]

    // Map and filter
    let doubled = map([1, 2, 3], x => x * 2)  // [2, 4, 6]
    let evens = filter([1, 2, 3, 4], x => x % 2 == 0)  // [2, 4]

    // Reduce
    let sum = reduce([1, 2, 3, 4], (acc, x) => acc + x, 0)  // 10

    // Group by
    let grouped = group_by(users, u => u["role"])
    // {"admin": [...], "user": [...]}

    // Chunk
    let chunks = chunk([1, 2, 3, 4, 5], 2)
    // [[1, 2], [3, 4], [5]]

    // Set operations
    let common = intersection([1, 2, 3], [2, 3, 4])  // [2, 3]
}
```

### Network

```helen
import std.data.*
import std.network.*
main {
    // HTTP GET
    let response = http_get("https://api.example.com/data")
    let data = json_parse(response["body"])

    // HTTP POST
    let result = http_post(
        "https://api.example.com/submit",
        headers={"Content-Type": "application/json"},
        body=json_stringify({"name": "Helen"})
    )

    // Download file
    http_download("https://example.com/file.pdf", "/tmp/file.pdf")
}
```

### Time

```helen
import std.core.*
import std.time.*
main {
    // Current time
    let now_ts = now()                    // Unix timestamp (seconds)
    let current = time()                  // Current time (datetime object)

    // Unix timestamp → ISO datetime (v1.44)
    let dt = fromtimestamp(1723534245)    // "2026-08-13T10:30:45"

    // Formatting
    let formatted = date_format(now(), "%Y-%m-%d %H:%M:%S")
    // "2026-06-19 17:30:00"

    // Parsing
    let parsed = date_parse("2026-06-19", "%Y-%m-%d")

    // Date arithmetic
    let tomorrow = date_add(now(), days=1)
    let diff = date_diff(date1, date2, "days")

    // Sleep
    sleep(1.5)  // Sleep for 1.5 seconds

    // Stopwatch (high precision)
    let sw = stopwatch_start()
    let elapsed = stopwatch_elapsed(sw)   // Seconds (float, high precision)
    print("Elapsed: " + str(elapsed) + " seconds")
}
```

### Math

```helen
import std.crypto.*
import std.math.*
main {
    // Basic math
    let rounded = round(3.14159, 2)   // 3.14
    let root = sqrt(16)               // 4.0
    let ceiling = ceil(3.2)           // 4
    let flooring = floor(3.8)         // 3
    let power = pow(2, 10)            // 1024

    // Logarithms
    let natural = log(2.718)          // Natural log (ln)
    let base2 = log2(8)               // 3 (2^3 = 8)
    let base10 = log10(100)           // 2 (10^2 = 100)
    let exponential = exp(1)          // 2.718... (e^1)

    // Trigonometric functions (radians)
    let cosine = cos(0)               // 1
    let sine = sin(3.14159 / 2)       // 1
    let tangent = tan(0)              // 0
    let angle = acos(0.5)             // 1.047... (60°)
    let angle2 = asin(0.5)            // 0.523... (30°)
    let angle3 = atan(1)              // 0.785... (45°)
    let angle4 = atan2(1, 1)          // 0.785... (45°, y/x)

    // Statistics
    let avg = mean([1, 2, 3, 4, 5])   // 3.0
    let mid = median([1, 2, 3, 4, 5]) // 3
    let std = stddev([1, 2, 3, 4, 5]) // 1.414...
    let total = sum([1, 2, 3, 4, 5])  // 15
    let prod = product([1, 2, 3, 4])  // 24

    // Bitwise operations (v1.39.4)
    let and_result = bit_and(5, 3)         // 1 (101 & 011 = 001)
    let or_result = bit_or(5, 3)           // 7 (101 | 011 = 111)
    let xor_result = bit_xor(5, 3)         // 6 (101 ^ 011 = 110)
    let not_result = bit_not(5)            // -6 (~5 = -6)
    let left_shift = bit_shift_left(5, 2)  // 20 (5 << 2 = 20)
    let right_shift = bit_shift_right(20, 2) // 5 (20 >> 2 = 5)
    
    // Practical: check if even
    let is_even = bit_and(42, 1) == 0      // true
    
    // Practical: multiply/divide by power of 2
    let times_8 = bit_shift_left(7, 3)     // 56 (7 * 8)
    let div_4 = bit_shift_right(20, 2)     // 5 (20 / 4)

    // Random numbers
    let rand = random()               // Random float between 0 and 1
    let rand_int = randint(1, 100)    // Random integer between 1 and 100
    let item = choice([1, 2, 3, 4])   // Random selection
    let shuffled = shuffle([1, 2, 3]) // Random shuffle
}
```

### File

```helen
import std.core.*
import std.file.*
import std.io.*
import std.path.*
main {
    // Read/write files
    let content = read_file("/path/to/file.txt")
    write_file("/path/to/output.txt", "Hello, World!")
    append_file("/path/to/log.txt", "New log entry\n")

    // File info
    if path_exists("/path/to/file.txt") {
        let size = file_size("/path/to/file.txt")
        print("File size: " + str(size) + " bytes")
    }

    // Directory operations
    let files = list_dir("/path/to/dir")
    mkdir("/path/to/new/dir")
    mkdir_p("/path/to/deep/nested/dir")  // Recursive creation
    copy_file("/src/file.txt", "/dst/file.txt")
    delete_file("/path/to/file.txt")

    // File search
    let py_files = glob_files("src", "*.py")       // Recursively find all Python files
    let md_files = glob_files("docs", "**/*.md")   // Use ** for explicit recursion

    // Search file content (literal)
    let matches = grep_files("src/", "TODO")
    // [{"file": "main.py", "line": 42, "text": "    # TODO: fix this"}]

    // Search file content (regex)
    let functions = grep_files("src/", "def \\w+\\(", regex=true)

    // Case-insensitive search
    let errors = grep_files("logs/", "error", case_sensitive=false)
}
```

### System

```helen
import std.core.*
import std.system.*
import std.tools.*
main {
    // Environment variables
    let home = env_get("HOME")
    env_set("MY_VAR", "value")
    let all_env = env_list()  // Sensitive values are auto-masked

    // CLI arguments (predefined constant argv + parsing functions)
    // Command line: helen tool.helen --verbose --output=json input.txt
    print(argv)  // ["tool.helen", "--verbose", "--output=json", "input.txt"]
    // Note: argv[0] is the program name
    
    let parsed = parse_cli_args()           // Auto-parse (skips argv[0])
    // {verbose: true, output: "json", _positional: ["input.txt"]}

    let spec = {
        "verbose": {"type": "flag", "default": false},
        "output": {"type": "string", "default": "text"}
    }
    let config = parse_cli_args(spec)       // Structured parsing (with types + defaults)

    // Shell commands (default shell=true)
    // On Unix: uses /bin/bash; on Windows (v1.30.7+): uses default shell (cmd.exe)
    let result = shell_exec("ls -la")               // Unix only
    let result = shell_exec("mkdir -p ~/project/{src,tests,contracts}")  // Unix only
    let result = shell_exec("cat file.txt | grep pattern | wc -l")       // Unix only
    print(result["output"])

    // Safe mode: use shell=false when handling untrusted input to prevent shell injection
    let result = shell_exec("echo " + user_input, shell=false)

    // Cross-platform alternatives (v1.30.7+): prefer stdlib over shell when possible
    // get_cwd()         instead of shell_exec("pwd")
    // date()            instead of shell_exec("date +%Y-%m-%d")
    // time()            instead of shell_exec("date +%s")
    // now()             instead of shell_exec("date '+%Y-%m-%d %H:%M:%S'")
    // env_get("HOME", "") instead of shell_exec("echo $HOME")
    // env_set("X", "y")  instead of shell_exec("export X=y")
    // delete_file(path)  instead of shell_exec("rm -f path")
    // delete_dir(path, recursive=true) instead of shell_exec("rm -rf path")
    // move_file(a, b)    instead of shell_exec("mv a b")

    // System info
    let pid = pid()                   // Process ID
    let os = platform()               // "linux", "darwin", "windows"
    let host = hostname()             // Hostname
    let py_ver = python_version()     // Python version
    let cpus = cpu_count()            // CPU core count
    let mem = memory_info()           // {total, available, used, percent}

    // Logging
    log_info("Application started")
    log_error("Something went wrong", category="app")
}
```

### Crypto

```helen
import std.crypto.*
main {
    // Hashing
    let md5_hash = md5("data")
    let sha256_hash = sha256("data")
    let sha512_hash = sha512("data")

    // HMAC
    let sig = hmac_sha256("message", "secret_key")

    // Random numbers
    let rand = random()               // Random float between 0 and 1
    let rand_int = randint(1, 100)    // Random integer
    let item = choice([1, 2, 3])      // Random selection

    // UUID
    let id = uuid_generate()          // "550e8400-e29b-41d4-a716-446655440000"
    let nil_id = uuid_nil()           // "00000000-0000-0000-0000-000000000000"
    let parsed = uuid_from_string("550E8400-E29B-41D4-A716-446655440000")

    // Random bytes
    let bytes = random_bytes(16)      // 32-character hex string
    let hex_str = random_hex(32)
    let b64 = random_base64(16)       // Base64-encoded random data
}
```

## Debug (v1.40+)

AI-native debugging functions providing structured error diagnostics, output validation, transcript querying, LLM recording/replay, and data lineage tracking.

### Basic Observability

```helen
import std.debug.*
main {
    // debug() — Structured debug output to stderr
    debug("variable value", x)
    // Output: [DEBUG] variable value {"value": 42}
    debug("checkpoint reached")

    // trace_on() / trace_off() — Enable/disable execution tracing
    trace_on()
    let result = compute_something()
    trace_off()

    // get_trace() — Get recent execution trace records
    let trace = get_trace(10)

    // Coverage measurement — track function/line/branch coverage
    coverage_on()          // Enable coverage tracking
    let result = tested_function()
    coverage_off()         // Disable coverage tracking
    let report = coverage_report("text")  // "text" | "json" | "html"
    let summary = coverage_summary()      // One-line summary
}
```

**Design features**: Zero overhead by default (no impact when tracing is off), JSON structured output (AI-consumable), automatic call stack + scope variable capture on errors/assertions, `llm act` automatically records call details.

### Structured Error Diagnostics (v1.40)

All 11 exception types now provide structured diagnostic information:

```helen
import std.debug.*
main {
    // After an error occurs, get detailed diagnostics
    let err = last_error_detail()
    if err != null {
        debug("Error category", error_category(err))
        // "LLMTimeout", "AgentCallFailed", "RuntimeGenericError", etc.
        
        debug("Fix suggestion", error_suggestion(err))
        // "LLM 调用超时。考虑：(1) 增加 timeout 配置..."
        
        let flows = error_data_flow(err)
        for flow in flows {
            debug("Data flow", flow)
            // {"variable": "x", "source": "msg-123", "via": "Coder"}
        }
    }
}
```

**Supported exception types** (11 types):
- AnyError, LLMError, TimeoutError, ModelError, PromptTooLongError
- AgentError, LLMOutputContractError (v1.40), ToolError
- RuntimeError, AssertionError, AggregateError

### Output Contract Validation (v1.40)

Validate LLM outputs against contracts:

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

**Supported contract types**:
- `"json"`: Validate that output is valid JSON
- `"text"`: Always passes (for explicit marking)
- Schema dict: Validate type, required fields, properties, enum, min/max, etc.

**Agent declaration with output contract**:
```helen
agent Reviewer {
    output_contract: "json"  // or schema dict
    main {
        llm act "Review this code and return JSON"
    }
}
```

### Incremental Transcript Query (v1.40)

Efficiently query large transcripts:

```helen
import std.debug.*
main {
    // Query all assistant messages
    let msgs = query_transcript(role="assistant")
    
    // Query messages from specific agent
    let coder_msgs = query_transcript(agent="Coder")
    
    // Paginated query
    let page1 = query_transcript(limit=100, offset=0)
    
    // Regex search
    let errors = query_transcript(content_regex="Error:")
    
    // Time range query
    let recent = query_transcript(since=now() - 3600)
    
    // Combined query
    let filtered = query_transcript(
        role="assistant",
        agent="Reviewer",
        content_regex="verdict",
        limit=50
    )
}
```

**Query parameters**: `session_id`, `role`, `agent`, `invocation_id`, `since`, `until`, `content_regex`, `message_type`, `limit`, `offset`

**Backend optimization**:
- JSONL backend: Streaming filter + 100k item limit (prevents OOM)
- SQLite backend: SQL WHERE pushdown (O(log n) query)

### LLM Recording/Replay (v1.40)

Record and replay LLM interactions for deterministic debugging:

```helen
import std.debug.*
main {
    // Start recording
    record_session("debug/session.jsonl")
    
    // Run agent (all LLM calls are recorded)
    agent Reviewer {
        main {
            llm act "Review this code..."
        }
    }
    
    // Stop recording
    stop_recording()
    
    // Later, replay the session
    replay_session("debug/session.jsonl")
    // Now all LLM calls use recorded responses
}
```

**Use cases**:
1. **Bug reproduction**: Record session with bug, replay to confirm
2. **Prompt regression testing**: Modify prompt, replay to compare
3. **CI testing**: Replay recorded sessions, avoid LLM dependency
4. **Performance analysis**: Analyze recorded duration and token usage

### Data Lineage Tracking (v1.40)

Track data flow between agents:

```helen
import std.debug.*
main {
    // Manually record data flow
    record_data_flow(
        "msg_abc",           // Producer UUID
        "msg_xyz",           // Consumer UUID
        "agent_call",        // Flow type
        {"arg": "input"}     // Metadata
    )
    
    // Query data origin
    let origins = trace_value_origin("msg_xyz")
    // [{"producer_uuid": "msg_abc", "flow_type": "agent_call", ...}]
    
    // Query data consumers
    let consumers = trace_value_consumers("msg_abc")
    // [{"consumer_uuid": "msg_xyz", "flow_type": "agent_call", ...}]
    
    // Get complete lineage graph
    let lineage = get_data_lineage()
    // {"nodes": [...], "edges": [...]}
}
```

**Flow types**: `"channel"`, `"agent_call"`, `"prompt"`, or custom types

**Data storage**: Independent SQLite sidecar file (`<session_id>_lineage.db`), decoupled from transcript backend.

**CLI tool**: `helen replay <session_id>` for interactive transcript replay.

**Coverage CLI**: Use `helen coverage <test_files> [--source <dir>] [--html <dir>]` to run tests with coverage measurement. See `helen coverage --help` for options.

## Context (Context Management)

Functions for managing LLM conversation context, used for context control in long-running agent conversations.

```helen
// All context-management API in action (must be inside main {})
import std.context.*
import std.core.*
main {
    // Basic operations
    clear_context()                       // Clear context, returns {cleared_messages, cleared_tokens}
    compress_context("auto")              // Compress context
    // Strategies: "auto" | "summarize" (LLM summary) | "truncate" | "none" | "graduated"

    // Inspection
    context_stats()                       // {message_count, total_tokens, system_tokens, ...}
    context_usage()                       // 0.0-1.0 usage ratio
    let usage = context_usage()
    if usage > 0.6 { compress_context("auto") }
    get_message(uuid)                     // Get a single message

    // Fine-grained Mutation
    insert_message("system", "Important note", 0)  // Insert message (position optional)
    replace_message(uuid, "New content")            // Replace message content
    delete_message(uuid)                            // Delete message
    pin_message(uuid) / unpin_message(uuid)         // Pin message (immune to compression)
    list_pinned_messages()                          // List all pinned messages: [{uuid, role, snippet, token_count}]

    // Working Memory — Auto-tracks active files, decisions, TODOs, error history
    working_memory_set("current_file", "main.py")
    working_memory_set("decision", "Use JWT authentication")
    working_memory_get("current_file")       // "main.py"
    working_memory_remove("todo")
    working_memory_clear()

    // Runtime Config
    set_compression_strategy("graduated")    // Dynamically adjust compression strategy
    set_context_window(128000)               // Set context window size
    set_working_memory_enabled(true)
    set_cache_aware(true)                    // Enable cache-aware compression (improves cache hit rate)
    get_context_config()                     // {strategy, window, working_memory, cache_aware}

    // Query
    search_context("authentication")         // [{uuid, role, content}, ...]
    context_slice(-5)                        // Last 5 messages
    context_slice(0, 10)                     // First 10 messages

    // Multi-Agent Transfer
    export_context()                         // Export [{role, content}, ...]
    import_context(messages)                 // Import into current session
    fork_context()                           // Create independent copy

    // Cross-session restore (v1.21+)
    restore_context("session_xxx")           // Restore active context from old transcript

    // Lifecycle Hooks
    on_compression(fn(stats) {
        print("About to compress: " + str(stats["token_count"]) + " tokens")
    })
    on_context_overflow(fn(stats) {
        compress_context("truncate")
    })
}
```

**REPL debug commands**: `:trace on/off/show [n]`, `:last_error` (structured JSON), `:llm_log [n]` (LLM call audit log)

**assert statement**:
```helen
main {
    assert x > 0
    assert x > 0, "x must be positive"
    // Assertion failure throws AssertionError, which can be caught with try-catch
}
```

## Test (Testing Framework)

```helen
import std.test.*
fn test_add() {
    assert_equal(2 + 3, 5)
}

fn test_subtract() {
    assert_equal(10 - 4, 6)
}

main {
    test_suite("Calculator")
    test_case("adds numbers", test_add)
    test_case("subtracts numbers", test_subtract)
    test_end_suite()
    run_tests()
}

// CLI:
// helen test calc.helen              # Run tests
// helen test calc.helen --watch      # Watch mode
// helen test calc.helen --filter "add"  # Filter
```

### Expect Chain API

```helen
import std.test.*
fn test_expect() {
    expect(42).toBe(42)
    expect("hello").toContain("ell")
    expect([1, 2, 3]).toHaveLength(3)
    expect(10).toBeGreaterThan(5)
    expect("test123").toMatch("[0-9]+")
    expect(5).not_.toBe(6)
}
```

`before_all`/`after_all`/`before_each`/`after_each` hooks are available.

## Quality (Quality Assessment)

```helen
import std.core.*
import std.quality.*
main {
    let source = read_file("my_program.helen")

    let metrics = analyze_code(source, "my_program.helen")
    print("Functions: " + str(metrics["function_count"]))

    let issues = check_security(source)
    print("Security issues: " + str(len(issues)))

    let scores = quality_score(source, "my_program.helen")
    print("Total: " + str(scores["total"]) + " Grade: " + scores["grade"])

    print(quality_report(source, "my_program.helen"))
    // CLI: helen quality my_program.helen --json
}
```

### 7 Assessment Dimensions

| Dimension | Weight | Assesses |
|-----------|:------:|----------|
| Architecture | 20% | Function length, complexity, nesting depth |
| Code Quality | 15% | Comment ratio, average function length |
| Security | 20% | Dangerous pattern detection |
| Test Coverage | 15% | Test file existence |
| Documentation | 10% | Docstring coverage |
| Maintainability | 10% | Long functions, high-complexity functions |
| Engineering Standards | 10% | Naming conventions, file size |

## Transcript (Session Records)

TranscriptStore (v1.16) — SSOT, persistent storage for all conversation messages.

### Session Management

```helen
import std.transcript.*
main {
    // get_session_id() — Current session ID
    let session = get_session_id()  // "session_{timestamp}_{uuid8}"

    // get_session_meta() (v1.23.3) — Session metadata (recorded at startup)
    let meta = get_session_meta()
    // {argv, timestamp, helen_version, python_version, cwd, session_scope}

    // list_sessions(scope?) — List all sessions
    let sessions = list_sessions()
    // [{session_id, created_at, modified_at, size_bytes, message_count, scope,
    //   dir, parent_session_id}, ...]
    //   scope: "global" | "project"
    //   dir:   directory containing the session (absolute path)
    //   parent_session_id: parent session's ID if this is a spawned child (v1.44)
    let global_sessions = list_sessions("global")
    let project_sessions = list_sessions("project")

    // Session directory management
    let info = get_session_dir()    // {session_dir, scope, project_dir}
    set_session_dir("/custom/path")
}
```

**Runtime isolation principles**:
- Multiple calls to `get_session_id()` within the same process → Same ID
- Restart program → New Interpreter → New session_id
- `spawn`-created agent → New Interpreter → New session_id (independent transcript)
- Normal agent call (same process) → Shared session_id, distinguished by `invocation_id`
- Cross-runtime inheritance must be explicit: `resume_session(parent_sid)` or `Channel.send(sid)`

### Replay, Export & Search

```helen
import std.context.*
import std.core.*
import std.transcript.*
main {
    // Replay
    replay_transcript()                              // Current session
    replay_transcript("session_123", true)           // Include compressed messages
    replay_transcript(agent="A", last_only=true)     // Filter by agent
    replay_transcript(invocation_id="inv_1", include_subtree=true)

    // Export
    export_transcript(null, "json")                  // Export current as JSON
    export_transcript(null, "text")                  // Export as plain text
    export_transcript("full.json", "json", include_spawned=true)  // Include spawned

    // Search (v1.22+) — Search persisted transcript (unlike search_context which searches current context)
    search_transcript("auth bug")                    // Basic search
    search_transcript("database", scope="all", limit=10)  // Across all sessions
    search_transcript("fix.*bug", regex=true)        // Regex
    search_transcript("TODO", role="user")           // Filter by role
    search_transcript("error", include_spawned=true) // Cross-spawn search (v1.23.7)

    // Typical search → restore context workflow
    let matches = search_transcript("auth bug", scope="all")
    if len(matches) > 0 {
        restore_context(matches[0]["session_id"])
    }
}
```

### Invocation Tree Query (v1.22+)

```helen
// Each agent main {} execution is an invocation with a unique invocation_id
import std.transcript.*
list_invocations()                               // List all invocations
list_invocations(agent="Researcher", limit=10)   // Filter by agent

get_invocation("inv_xxx")                        // Query single
// {agent_name, message_count, parent_invocation_id, ...}

get_invocation_tree()                            // Full call tree (nested structure)
invocation_path("inv_3")                         // "top -> A -> C"

// Chinese aliases
列出调用()
获取调用("inv_xxx")
获取调用树()
调用路径("inv_3")
```

**Context isolation (v1.22/v1.23)**: Each agent main {} execution is an independent invocation; the LLM can only see messages from the current invocation.

### Session Restore & Cleanup

```helen
import std.transcript.*
main {
    get_spawned_sessions()                           // Direct child sessions
    get_spawn_tree()                                 // Full spawn tree
    replay_full_session()                            // Aggregate main session + all spawned

}```

### Session Scope (v1.20)

- `global`: `~/.helen/sessions/`
- `project`: `.helen/sessions/` (when `.helen/`, `helen.yaml`, or `helen.toml` is detected)
- `auto` (default): Auto-detect project directory, otherwise global

### Startup Session Recovery (v1.24+)

```bash
helen --session=session_xxx file.helen    # Start with specified session
helen --resume-latest file.helen          # Auto-restore most recent session
helen repl --resume-latest                # REPL shorthand: -r
```

```python
# Python API
from helen.interpreter import Interpreter
interp = Interpreter(session_id="session_xxx")
```

| Feature | `--session` (startup) | `resume_session()` (runtime) |
|---------|----------------------|------------------------------|
| Timing | Before interpreter starts | During program execution |
| Behavior | Directly reuses specified session | Imports historical messages into current new session |
| transcript | One file | Two files |

## Media (Media/Multimodal)

v1.17 introduces multimodal support; `MediaPart` is a first-class data type.

```helen
import std.media.*
main {
    // Creation
    let img = media("/path/to/image.png")          // File path or URL
    let video = media("https://example.com/video.mp4")
    let audio = media("/path/to/audio.mp3", "audio")  // Explicitly specify type
    let base64_img = media_base64("iVBORw0KGgo...", "image/png")

    // Inspection
    is_media(value)                                // Whether it's a MediaPart
    media_type(img)                                // "image" | "video" | "audio"
    is_image(img) / is_video(video) / is_audio(audio)

    // Format adapters
    to_openai_parts([img, video])                  // [{type: "image_url", ...}]
    to_claude_parts([img])                         // [{type: "image", source: {...}}]
    to_gemini_parts([img])

    // Utilities
    media_to_base64(img)                           // Convert to base64 string
    save_media(img, "/path/to/save.png")           // Save to file

    // Usage in llm act (callbacks as adapters)
    llm act "Analyze this image"
        media("/path/to/image.png")
        on_media fn(parts, provider) {
            if provider == "claude" { return to_claude_parts(parts) }
            return to_openai_parts(parts)
        }
}
```

## LLM (LLM Call Control)

Control ongoing LLM streaming calls.

```helen
import std.llm.*
main {
    let call_id = current_llm_call_id()     // string | null
    cancel_llm_call(call_id)
    cancel_all_llm_calls()                  // Returns count of cancelled calls

    // Chinese aliases
    取消大模型调用(call_id)
    当前大模型调用id()
    取消所有大模型调用()
}
```

Used in `on_chunk` callbacks to detect termination conditions, Ctrl+C interruption, and timeout control.

## Concurrency

v1.18 Channel-based message-passing concurrency model.

```helen
import std.concurrency.*
import std.core.*
agent Worker(task: str) {
    main {
        // Execute task...
        return "Result"
    }
}

main {
    // spawn returns a Channel (mailbox)
    let ch = spawn Worker("Task 1")

    // Channel methods
    ch.send("message")              // Send message
    let msg = ch.receive()          // Blocking receive
    let maybe = ch.try_receive()    // Non-blocking receive (returns null if no message)
    ch.cancel()                     // Cancel (interrupts streaming LLM call)
    ch.close()                      // Close channel
    ch.is_closed()                  // Check if closed

    // Chinese aliases: 发送(), 接收(), 尝试接收(), 取消(), 关闭(), 已关闭()

    // mailbox_select — Multi-channel select (race mode: first to complete wins)
    let m1 = spawn StrategyA()
    let m2 = spawn StrategyB()
    let m3 = spawn StrategyC()
    let result = mailbox_select([m1, m2, m3])  // {endpoint: Channel, message: "..."}

    // With timeout
    let result = mailbox_select([m1, m2], timeout=5.0)  // Returns null on timeout
    if result == null { print("Timeout") }

    // Chinese aliases
    let result = 邮箱选择([m1, m2, m3])
}
```

**Key features**: Snapshot semantics (spawn deep-copies all variables including SharedStore), isolated environment, streaming interrupt (`ch.cancel()`). Inter-agent data sharing is done explicitly through Channel messages.

## Exception Handling (v.9+)

Python exceptions are automatically wrapped as `RuntimeError`, with format `"Python <Type Name>: <Original Message>"`:

```helen
import std.core.*
main {
    try {
        let x = len(42)
    } catch RuntimeError err {
        print(err.message)    // "Python TypeError: object of type 'int' has no len()"
    }

    try {
        let data = read_file("/nonexistent")
    } catch RuntimeError err {
        print(err.message)    // "Python FileNotFoundError: [Errno 2] ..."
    }
}
```

Python exception types can be distinguished by the message prefix. Existing Helen exceptions (such as `TimeoutError`) retain their original types unchanged.

## Module Cache (Python REPL/Jupyter)

`ImportResolver` uses an in-memory cache (`_cached_results`). After modifying `.helen` files, you need to manually clear it:

```python
# Option 1: Create a new Interpreter each time (simple)
interp = Interpreter()

# Option 2: Manually clear cache (efficient)
interp.import_resolver._cached_results.clear()
interp.import_resolver._loaded.clear()

# Debug: Check cache status
print(f"Cached: {len(interp.import_resolver._cached_results)} files")
for path in interp.import_resolver._loaded:
    print(f"  - {path}")
```

Recommended approach: Use the CLI for development (`helen my_program.helen`); each new process automatically reloads.

## Built-in Template Library

```bash
helen template --list                  # View all templates
helen template simple_agent            # View template content
helen template spawn_channel --copy my_worker.helen  # Copy to current directory
```

Templates: `simple_agent`, `spawn_channel`, `shared_store`, `context_object`, `pipeline`. All templates follow the "Caller Decides Context" principle — all agent information is passed explicitly through parameters.

## Modular Imports (v1.34+)

Helen v1.34 introduces modular imports for stdlib functions, providing explicit control over which functions are imported and avoiding naming conflicts.

### Import Syntax

Three forms of stdlib module imports are supported:

#### 1. Selective Import
Import specific functions:

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
    print(upper("hello"))
    print(map([1, 2, 3], fn(x) { return x * 2 }))
}
```

#### 3. Namespace Import
Import under a namespace to avoid conflicts:

```helen
import std.dict as Dict
import std.list as List

main {
    let data = {"name": "Alice"}
    let keys = Dict.keys(data)
    let sorted = List.sort([3, 1, 2])
}
```

### Available Modules

Helen v1.38 provides **22 stdlib modules** covering every stdlib category.

| Module | Key Functions |
|--------|---------------|
| `std.core` | `len`, `str`, `int`, `float`, `bool`, `print`, `abs`, `min`, `max`, `range`, `type`, `isinstance` (17) |
| `std.str` | `upper`, `lower`, `split`, `join`, `replace`, `find`, `contains`, `regex_match`, `regex_replace`, `base64_encode` (43) |
| `std.list` | `map`, `filter`, `reduce`, `sort`, `unique`, `flatten`, `chunk`, `zip`, `find_if`, `every` (11) |
| `std.dict` | `keys`, `values`, `entries`, `get`, `set_key`, `has_key`, `remove_key`, `merge`, `pick`, `omit` (10) |
| `std.math` | `round`, `floor`, `ceil`, `sum`, `pow`, `sqrt`, `log`, `sin`, `cos`, `random` (27) |
| `std.time` | `now`, `date`, `date_format`, `date_parse`, `date_add`, `date_diff`, `sleep`, `stopwatch_start` (16) |
| `std.file` | `read_file`, `write_file`, `append_file`, `delete_file`, `copy_file`, `list_dir`, `glob_files` (12) |
| `std.io` | `progress_bar`, `stream_print`, `stream_clear`, `mkdir`, `mkdir_p`, `stream_cursor_up` (9) |
| `std.system` | `env_get`, `env_set`, `shell_exec`, `exec`, `get_cli_args`, `platform`, `cpu_count`, `pid` (24) |
| `std.path` | `path_basename`, `path_dirname`, `path_exists`, `path_is_dir`, `path_is_file`, `path_join` (6) |
| `std.data` | `json_parse`, `json_parse_lenient`, `json_stringify`, `yaml_parse`, `toml_parse`, `csv_parse`, `xml_parse` (28) |
| `std.network` | `http_get`, `http_post`, `http_put`, `http_delete`, `http_download`, `url_parse`, `url_build` (9) |
| `std.tools` | `shell_exec`, `calculate`, `patch_file`, `load_skill`, `web_search`, `web_fetch`, `find_files` (7) |
| `std.debug` | `debug`, `trace_on`, `trace_off`, `get_trace`, `coverage_on`, `coverage_report` (11) |
| `std.context` | `clear_context`, `compress_context`, `context_stats`, `working_memory_set`, `search_context` (29) |
| `std.transcript` | `get_session_id`, `list_sessions`, `replay_transcript`, `resume_session`, `search_transcript` (21) |
| `std.media` | `media`, `media_base64`, `is_media`, `to_openai_parts`, `save_media`, `is_image` (12) |
| `std.test` | `test_suite`, `test_case`, `assert_equal`, `assert_true`, `expect`, `run_tests` (23) |
| `std.quality` | `analyze_code`, `check_security`, `quality_score`, `quality_report` (4) |
| `std.llm` | `cancel_llm_call`, `current_llm_call_id`, `cancel_all_llm_calls` (3) |
| `std.crypto` | `md5`, `sha256`, `hmac_sha256`, `random`, `randint`, `choice`, `uuid_generate` (17) |
| `std.concurrency` | `mailbox_select` (1) |

Note: `std.tools`, `std.transcript`, `std.llm` use keyword-named modules. The parser accepts them after `std.` even though they are reserved keywords in other contexts.

### Benefits

- **Explicit dependencies**: Clear which functions your code uses
- **Avoid naming conflicts**: Use namespaces to prevent collisions
- **Better IDE support**: Improved autocomplete and type checking
- **Easier refactoring**: Clear module boundaries

### Backward Compatibility

All stdlib functions remain available globally. Modular imports are optional.

```helen
// Both forms work identically
import std.str.{upper}
// or just use upper() directly (global stdlib)
```

## MCP Tools Integration (v1.33+)

Helen v1.33 introduces MCP (Model Context Protocol) client support. MCP tools extend Helen's built-in tools with external capabilities.

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
    tools = ["search_code", "get_code_snippet", "read_file"]
    
    main {
        // LLM can call MCP tools like built-in tools
        let result = llm act "Search for authentication functions"
        print(result)
    }
}
```

### Tool Priority

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
}
```

### Complete Guide

See [MCP Integration Guide](../../../wiki/runtime/mcp-integration.md) for detailed documentation.

---

**Last updated**: 2026-08-05

## Related Skills

- **helen-syntax** — Helen syntax reference (keywords, types, expressions)
- **helen-agent-patterns** — Agent design patterns
- **helen-agent-collaboration** — Multi-agent collaboration patterns
- **helen-testing** — Testing framework usage guide
