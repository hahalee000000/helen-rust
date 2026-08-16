<!-- helen-rust edition: stdlib registry in crates/helen-stdlib — 22 modules, 378+ builtins + aliases. `helen doc` output byte-identical to reference. -->

# Standard Library (Stdlib)

> Crate: `crates/helen-stdlib` | **378+ builtins** | Tests: `tests/stdlib/`

---

## Registry

The stdlib registry is implemented in Rust (`helen-stdlib` crate). Each builtin function is registered with:

- **name**: Function name
- **description**: Description
- **signature**: Signature
- **category**: Category (core/string/math/data/collection/network/time/file/system/crypto/io/test/quality)

---

## Function Category Summary

| Category | Count | Crate Module |
|----------|-------|-------------|
| **Core** | 11 | `core` |
| **String** | 40 | `string` |
| **Data** | 26 | `data` |
| **Collection** | 22 | `collection` |
| **Network** | 9 | `network` |
| **Time** | 14 | `time` |
| **Math** | 15 | `math_stats` |
| **File** | 18 | `file` |
| **System** | 18 | `system` |
| **Crypto** | 11 | `crypto` |
| **Test** | 14 | `test` |
| **Quality** | 4 | `quality` |
| **IO** | 5 | `io` |
| **Observability** | 8 | `observability` |
| **Context** | 29 | `context` |
| **Transcript** | 11 | `transcript` |
| **Media** | 12 | `media` |
| **Tools** | 24 | `tools` |
| **Total** | **378+** | - |

---

## Core (11)

| Function | Signature | Description |
|---|---|---|
| `print` | `print(*args)` → str | Print value, returns string representation |
| `len` | `len(value)` → int | String length / list length |
| `str` | `str(value)` → str | Convert to string |
| `int` | `int(value)` → int | Convert to integer |
| `float` | `float(value)` → float | Convert to float |
| `abs` | `abs(value)` → float | Absolute value |
| `min` | `min(*args)` → Any | Minimum value |
| `max` | `max(*args)` → Any | Maximum value |
| `range` | `range(start, stop?, step?)` → list[int] | Generate integer list |
| `type` | `type(value)` → str | Return type name |
| `isinstance` | `isinstance(value, type_name)` → bool | Type check |

---

## String (40)

### Basic Operations (15)

| Function | Signature | Description |
|---|---|---|
| `upper` | `upper(s)` → str | Convert to uppercase |
| `lower` | `lower(s)` → str | Convert to lowercase |
| `strip` | `strip(s)` → str | Strip whitespace from both ends |
| `trim_prefix` | `trim_prefix(s, prefix)` → str | Remove prefix |
| `trim_suffix` | `trim_suffix(s, suffix)` → str | Remove suffix |
| `split` | `split(s, sep?)` → list[str] | Split string |
| `join` | `join(sep, items)` → str | Join string list |
| `startswith` | `startswith(s, prefix)` → bool | Prefix check |
| `endswith` | `endswith(s, suffix)` → bool | Suffix check |
| `replace` | `replace(s, old, new)` → str | Replace substring |
| `find` | `find(s, sub)` → int | Find substring position |
| `find_from` | `find_from(s, sub, start)` → int | Find substring from position (**v1.26 new**) |
| `substring` | `substring(s, start, end?)` → str | Extract `s[start:end]` — *end* is an **exclusive index**, not a length. Omit *end* to slice to string end. |
| `chr` | `chr(code)` → str | Convert Unicode code point to character (**v1.31 new**) |
| `ord` | `ord(char)` → int | Convert character to Unicode code point (**v1.31 new**) |

### Regular Expressions (5)

| Function | Signature | Description |
|---|---|---|
| `regex_match` | `regex_match(pattern, s)` → dict? | Match at beginning |
| `regex_search` | `regex_search(pattern, s)` → dict? | Search anywhere |
| `regex_replace` | `regex_replace(pattern, s, replacement)` → str | Replace |
| `regex_split` | `regex_split(pattern, s)` → list[str] | Split |
| `regex_findall` | `regex_findall(pattern, s)` → list[str] | Find all matches |

### Text Analysis (8)

| Function | Signature | Description |
|---|---|---|
| `tokenize` | `tokenize(text)` → list[str] | Tokenize |
| `word_count` | `word_count(text)` → dict | Word frequency count |
| `levenshtein` | `levenshtein(s1, s2)` → int | Edit distance |
| `similarity` | `similarity(s1, s2)` → float | Similarity |
| `remove_punctuation` | `remove_punctuation(text)` → str | Remove punctuation |
| `normalize_whitespace` | `normalize_whitespace(text)` → str | Normalize whitespace |
| `extract_urls` | `extract_urls(text)` → list[str] | Extract URLs |
| `extract_emails` | `extract_emails(text)` → list[str] | Extract emails |

### Encoding (4)

| Function | Signature | Description |
|---|---|---|
| `base64_encode` | `base64_encode(s)` → str | Base64 encode |
| `base64_decode` | `base64_decode(s)` → str | Base64 decode |
| `html_escape` | `html_escape(s)` → str | HTML escape |
| `html_unescape` | `html_unescape(s)` → str | HTML unescape |

### String Operations (7)

| Function | Signature | Description |
|---|---|---|
| `repeat` | `repeat(s, n)` → str | Repeat |
| `reverse` | `reverse(s)` → str | Reverse |
| `pad_left` | `pad_left(s, width, char?)` → str | Pad left |
| `pad_right` | `pad_right(s, width, char?)` → str | Pad right |
| `center` | `center(s, width, char?)` → str | Center |
| `count` | `count(s, sub)` → int | Count occurrences |
| `index` | `index(s, sub)` → int | Find index |

---

## Data (26)

### JSON (5)

| Function | Signature | Description |
|---|---|---|
| `json_parse` | `json_parse(text)` → Any | Parse JSON |
| `json_parse_lenient` | `json_parse_lenient(text)` → Any | Parse JSON, auto-strip markdown fences (**v1.26 new**) |
| `json_stringify` | `json_stringify(value, indent?)` → str | Generate JSON |
| `json_load` | `json_load(path)` → Any | Load from file |
| `json_save` | `json_save(path, value, indent?)` → str | Save to file |

### HTML (3)

| Function | Signature | Description |
|---|---|---|
| `html_parse` | `html_parse(text)` → dict | Parse HTML |
| `html_text` | `html_text(html)` → str | Extract text |
| `html_links` | `html_links(html)` → list[str] | Extract links |

### Markdown (2)

| Function | Signature | Description |
|---|---|---|
| `markdown_to_html` | `markdown_to_html(text)` → str | Convert to HTML |
| `markdown_extract_headings` | `markdown_extract_headings(text)` → list[dict] | Extract headings |

### CSV (4)

| Function | Signature | Description |
|---|---|---|
| `csv_parse` | `csv_parse(text, delimiter?)` → list[list[str]] | Parse CSV |
| `csv_stringify` | `csv_stringify(rows, delimiter?)` → str | Generate CSV |
| `csv_load` | `csv_load(path, delimiter?)` → list[list[str]] | Load from file |
| `csv_save` | `csv_save(path, rows, delimiter?)` → str | Save to file |

### YAML (4)

| Function | Signature | Description |
|---|---|---|
| `yaml_parse` | `yaml_parse(text)` → Any | Parse YAML |
| `yaml_stringify` | `yaml_stringify(value)` → str | Generate YAML |
| `yaml_load` | `yaml_load(path)` → Any | Load from file |
| `yaml_save` | `yaml_save(path, value)` → str | Save to file |

### TOML (4)

| Function | Signature | Description |
|---|---|---|
| `toml_parse` | `toml_parse(text)` → dict | Parse TOML |
| `toml_stringify` | `toml_stringify(value)` → str | Generate TOML |
| `toml_load` | `toml_load(path)` → dict | Load from file |
| `toml_save` | `toml_save(path, value)` → str | Save to file |

### XML (4)

| Function | Signature | Description |
|---|---|---|
| `xml_parse` | `xml_parse(text)` → dict | Parse XML |
| `xml_stringify` | `xml_stringify(value, root?)` → str | Generate XML |
| `xml_load` | `xml_load(path)` → dict | Load from file |
| `xml_save` | `xml_save(path, value, root?)` → str | Save to file |

---

## Collection (22)

### List Operations (12)

| Function | Signature | Description |
|---|---|---|
| `map` | `map(lst, fn)` → list | Map |
| `filter` | `filter(lst, fn)` → list | Filter |
| `reduce` | `reduce(lst, fn, initial?)` → Any | Reduce |
| `find_if` | `find_if(lst, fn)` → Any? | Find |
| `every` | `every(lst, fn)` → bool | All satisfy |
| `some` | `some(lst, fn)` → bool | Some satisfy |
| `sort` | `sort(lst, compare?)` → list | Sort |
| `unique` | `unique(lst)` → list | Deduplicate |
| `flatten` | `flatten(lst)` → list | Flatten |
| `chunk` | `chunk(lst, size)` → list[list] | Chunk |
| `zip` | `zip(*lists)` → list[tuple] | Zip |

### Dict Operations (6)

| Function | Signature | Description |
|---|---|---|
| `keys` | `keys(dict)` → list | All keys |
| `values` | `values(dict)` → list | All values |
| `entries` | `entries(dict)` → list[tuple] | All key-value pairs |
| `merge` | `merge(*dicts)` → dict | Merge |
| `pick` | `pick(dict, keys)` → dict | Select keys |
| `omit` | `omit(dict, keys)` → dict | Exclude keys |

### Set Operations (5)

| Function | Signature | Description |
|---|---|---|
| `make_set` | `make_set(items)` → set | Create set |
| `set_union` | `set_union(s1, s2)` → set | Union |
| `set_intersection` | `set_intersection(s1, s2)` → set | Intersection |
| `set_difference` | `set_difference(s1, s2)` → set | Difference |
| `set_has` | `set_has(set, item)` → bool | Contains check |

---

## Network (9)

### HTTP Requests (5)

| Function | Signature | Description |
|---|---|---|
| `http_get` | `http_get(url, headers?)` → dict | GET request |
| `http_post` | `http_post(url, data?, headers?)` → dict | POST request |
| `http_put` | `http_put(url, data?, headers?)` → dict | PUT request |
| `http_delete` | `http_delete(url, headers?)` → dict | DELETE request |
| `http_download` | `http_download(url, path)` → str | Download file |

### URL Handling (4)

| Function | Signature | Description |
|---|---|---|
| `url_parse` | `url_parse(url)` → dict | Parse URL |
| `url_build` | `url_build(scheme, host, path?, query?)` → str | Build URL |
| `url_encode` | `url_encode(s)` → str | URL encode |
| `url_decode` | `url_decode(s)` → str | URL decode |

---

## Time (14)

### Time Retrieval (4)

| Function | Signature | Description |
|---|---|---|
| `now` | `now()` → str | Current date and time |
| `time` | `time()` → float | Unix timestamp |
| `fromtimestamp` | `fromtimestamp(timestamp)` → str | Unix timestamp to ISO datetime (v1.44) |
| `sleep` | `sleep(seconds)` → None | Pause execution |

### Date Operations (10)

| Function | Signature | Description |
|---|---|---|
| `date` | `date(year?, month?, day?)` → str | Create/get date |
| `datetime` | `datetime(...)` → str | Create/get date-time |
| `date_format` | `date_format(date_str, format_str)` → str | Format date |
| `date_parse` | `date_parse(date_str, format_str)` → str | Parse date |
| `date_add` | `date_add(date_str, days?, hours?, ...)` → str | Date addition |
| `date_diff` | `date_diff(date1, date2, unit?)` → float | Date difference |
| `date_year` | `date_year(date_str)` → int | Extract year |
| `date_month` | `date_month(date_str)` → int | Extract month |
| `date_day` | `date_day(date_str)` → int | Extract day |
| `date_weekday` | `date_weekday(date_str)` → int | Get day of week |

---

## Math (15)

### Basic Math (4)

| Function | Signature | Description |
|---|---|---|
| `round` | `round(value, ndigits=0)` → float | Round |
| `sqrt` | `sqrt(value)` → float | Square root |
| `floor` | `floor(value)` → int | Floor |
| `ceil` | `ceil(value)` → int | Ceiling |

### Statistical Analysis (11)

| Function | Signature | Description |
|---|---|---|
| `mean` | `mean(numbers)` → float | Arithmetic mean |
| `median` | `median(numbers)` → float | Median |
| `mode` | `mode(numbers)` → list | Mode |
| `variance` | `variance(numbers, population?)` → float | Variance |
| `stddev` | `stddev(numbers, population?)` → float | Standard deviation |
| `correlation` | `correlation(x, y)` → float | Correlation coefficient |
| `percentile` | `percentile(numbers, p)` → float | Percentile |
| `sum` | `sum(numbers)` → float | Sum |
| `product` | `product(numbers)` → float | Product |
| `stats_min` | `stats_min(numbers)` → float | Minimum |
| `stats_max` | `stats_max(numbers)` → float | Maximum |

---

## File (18)

### Basic File Operations (5)

| Function | Signature | Description |
|---|---|---|
| `read_file` | `read_file(path)` → str | Read file |
| `write_file` | `write_file(path, content)` → str | Write file |
| `append_file` | `append_file(path, content)` → str | Append to file |
| `mkdir` | `mkdir(path)` → str | Create directory |
| `mkdir_p` | `mkdir_p(path)` → str | Create directory recursively |

### Path Operations (6)

| Function | Signature | Description |
|---|---|---|
| `path_join` | `path_join(*parts)` → str | Join path |
| `path_dirname` | `path_dirname(path)` → str | Get directory name |
| `path_basename` | `path_basename(path)` → str | Get filename |
| `path_exists` | `path_exists(path)` → bool | Check existence |
| `path_is_file` | `path_is_file(path)` → bool | Check if file |
| `path_is_dir` | `path_is_dir(path)` → bool | Check if directory |

### Advanced File Operations (5)

| Function | Signature | Description |
|---|---|---|
| `file_size` | `file_size(path)` → int | File size |
| `file_modified` | `file_modified(path)` → str | Modification time |
| `list_dir` | `list_dir(path, pattern?)` → list[str] | List directory |
| `walk_dir` | `walk_dir(path)` → list[tuple] | Walk directory tree |
| `copy_file` | `copy_file(src, dst)` → str | Copy file |
| `move_file` | `move_file(src, dst)` → str | Move file |
| `delete_file` | `delete_file(path)` → str | Delete file |
| `delete_dir` | `delete_dir(path, recursive?)` → str | Delete directory |

### File Search (2) (v1.15)

| Function | Signature | Description |
|---|---|---|
| `glob_files` | `glob_files(path, pattern?)` → list[str] | Find files recursively (glob pattern) |
| `grep_files` | `grep_files(path, pattern, regex?, case_sensitive?, max_results?)` → list[map] | Search file contents |
| `temp_file` | `temp_file(suffix?, prefix?, dir?)` → str | Create temporary file |
| `temp_dir` | `temp_dir(suffix?, prefix?, dir?)` → str | Create temporary directory |

---

## System (18)

### Environment Variables (4)

| Function | Signature | Description |
|---|---|---|
| `env_get` | `env_get(key, default?)` → str? | Get environment variable |
| `env_set` | `env_set(key, value)` → str | Set environment variable |
| `env_list` | `env_list()` → dict | List all |
| `env_delete` | `env_delete(key)` → str | Delete environment variable |

### CLI Arguments (2)

| Function | Signature | Description |
|---|---|---|
| `get_cli_args` | `get_cli_args()` → list[str] | Get command-line arguments (same as `argv`) |
| `parse_cli_args` | `parse_cli_args(spec?)` → map | Structured CLI argument parsing |

`parse_cli_args()` supports two modes:

- **Auto mode** (no arguments): Automatically recognizes `--flag`, `--key=value`, `--key value`, `-v` short flags, positional arguments (collected into `_positional` key)
- **Spec mode** (pass a spec map): Converts by type (`flag`/`string`/`int`/`float`) and applies defaults

> See also: `argv` predefined constant ([[toolchain/cli|CLI Documentation]]).

### Process Management (5)

| Function | Signature | Description |
|---|---|---|
| `exec` | `exec(command, shell?, timeout?)` → dict | Execute command |
| `exec_async` | `exec_async(command, shell?)` → int | Execute asynchronously |
| `pid` | `pid()` → int | Current process ID |
| `exit` | `exit(code?)` → None | Exit program |
| `kill` | `kill(pid, signal?)` → str | Send signal |

### Logging System (7)

| Function | Signature | Description |
|---|---|---|
| `log_debug` | `log_debug(message)` → str | Debug log |
| `log_info` | `log_info(message)` → str | Info log |
| `log_warn` | `log_warn(message)` → str | Warning log |
| `log_error` | `log_error(message)` → str | Error log |
| `log_critical` | `log_critical(message)` → str | Critical log |
| `log_set_level` | `log_set_level(level)` → str | Set level |
| `log_to_file` | `log_to_file(path)` → str | Output to file |

---

## Crypto (11)

### Hash Functions (6)

| Function | Signature | Description |
|---|---|---|
| `md5` | `md5(text)` → str | MD5 hash |
| `sha1` | `sha1(text)` → str | SHA1 hash |
| `sha256` | `sha256(text)` → str | SHA256 hash |
| `sha512` | `sha512(text)` → str | SHA512 hash |
| `hmac_sha256` | `hmac_sha256(key, message)` → str | HMAC-SHA256 |
| `hash_file` | `hash_file(path, algorithm?)` → str | File hash |

### Random Functions (5)

| Function | Signature | Description |
|---|---|---|
| `random` | `random()` → float | Random float |
| `randint` | `randint(min, max)` → int | Random integer |
| `choice` | `choice(items)` → Any | Random selection |
| `shuffle` | `shuffle(items)` → list | Random shuffle |
| `sample` | `sample(items, k)` → list | Random sample |

---

## IO (5)

| Function | Signature | Description |
|---|---|---|
| `stream_print` | `stream_print(text)` → str | Print without newline |
| `stream_clear` | `stream_clear()` → str | Clear current line |
| `progress_bar` | `progress_bar(current, total, width?)` → str | Progress bar |
| `stream_cursor_up` | `stream_cursor_up(n?)` → str | Move cursor up |
| `stream_cursor_down` | `stream_cursor_down(n?)` → str | Move cursor down |

---

## Media (12) (v1.17)

| Function | Signature | Description |
|---|---|---|
| `media` | `media(source, type?)` → MediaPart | Create media from file path or URL |
| `media_base64` | `media_base64(data, mime, type?)` → MediaPart | Create media from base64 data |
| `is_media` | `is_media(value)` → bool | Check if value is MediaPart |
| `media_type` | `media_type(value)` → str? | Get media type |
| `to_openai_parts` | `to_openai_parts(parts)` → list[map] | Convert to OpenAI content_parts format |
| `to_claude_parts` | `to_claude_parts(parts)` → list[map] | Convert to Claude content_blocks format |
| `to_gemini_parts` | `to_gemini_parts(parts)` → list[map] | Convert to Gemini inline_data format |
| `media_to_base64` | `media_to_base64(part)` → str | Any source → pure base64 |
| `save_media` | `save_media(part, path?)` → str | Save media to file |
| `is_image` | `is_image(value)` → bool | Whether image MediaPart |
| `is_video` | `is_video(value)` → bool | Whether video MediaPart |
| `is_audio` | `is_audio(value)` → bool | Whether audio MediaPart |

---

## Registration

All builtin functions are registered at compile time in the `helen-stdlib` crate. Each module (core, string, data, etc.) registers its functions with the central registry.

---

## Implementation Notes

### Core Features (Zero Dependencies)

All core features are implemented in Rust with no external dependencies:
- Core, String, Collection, Network, Time, Math, File, System, Crypto, IO

### Data Format Support

The following data formats are supported:
- **JSON**: Built-in (serde_json)
- **YAML**: Built-in (serde_yaml)
- **TOML**: Built-in (toml crate)
- **XML**: Built-in (quick-xml)
- **CSV**: Built-in (csv crate)

---

## Usage Examples

```helen
import std.core.*
main {
    // Core
    let len = len("hello")          // 5
    let nums = range(1, 5)          # [1, 2, 3, 4]
    let t = type(42)                # "int"

    // String
    let upper = upper("hello")      # "HELLO"
    let parts = split("a,b,c", ",") # ["a", "b", "c"]
    let joined = join("-", parts)   # "a-b-c"
    
    // Regex
    let matches = regex_findall(r"\d+", "a1b2c3")  # ["1", "2", "3"]

    // Data
    let data = json_parse('{"name": "Alice"}')
    let yaml_data = yaml_load("config.yaml")

    // Collection
    let doubled = map([1, 2, 3], x => x * 2)  # [2, 4, 6]
    let filtered = filter([1, 2, 3, 4], x => x > 2)  # [3, 4]

    // Network
    let response = http_get("https://api.example.com")
    
    // Time
    let today = date()              # "2026-06-18"
    let tomorrow = date_add(today, days=1)

    // Math
    let avg = mean([1, 2, 3, 4, 5])  # 3.0
    
    // File
    let content = read_file("data.txt")
    let files = list_dir(".", pattern="*.helen")
    
    // System
    let path = env_get("PATH")
    log_info("Processing started")
    
    // CLI arguments
    let args = argv                       // Predefined const list<str>
    let parsed = parse_cli_args()         // Auto-parse
    // Or with spec: parse_cli_args({"verbose": {"type": "flag", "default": false} })
    
    // Crypto
    let hash = sha256("hello")
    let rand = randint(1, 100)
}
```
