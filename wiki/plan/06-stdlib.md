# M4 — Stdlib: 378 Builtin Functions

**Objective:** Register all **378** `BuiltinFunction`s (verified count in `helen/stdlib/__init__.py`) plus Chinese aliases. Exit criterion: `tests/stdlib/` differential corpus passes per module.

## Architecture

```rust
// crates/helen-stdlib/src/lib.rs
pub struct BuiltinFnDef {
  pub name: &'static str,
  pub description: &'static str,   // used by helen docgen + tool schemas
  pub signature: &'static str,     // e.g. "split(s: str, sep: str) -> list"
  pub category: Category,          // string | collection | data | time | ...
  pub call: NativeFn,              // fn(&mut Interpreter, &[Value], &IndexMap<String,Value>) -> Result<Value, ExceptionValue>
}

pub fn register_all(registry: &mut BuiltinRegistry) {
  string::register(registry); collection::register(registry); /* ... one fn per module */
  zh_aliases::register(registry);   // Chinese alias map
}
```

Module files: `string.rs, collection.rs, data.rs, data_formats.rs, time.rs, math_stats.rs,
file_advanced.rs, system.rs, network.rs, crypto.rs, media.rs, context.rs, quality.rs,
test.rs, tools.rs, transcript.rs, transcript_query.rs, debug.rs, llm_control.rs, mailbox.rs, zh_aliases.rs`.

**Execution order of a builtin call:** resolve name → arity/type check (params schema) → call → convert result. Errors must produce the **same exception class + message** as Python (e.g., `split("")` → ValueError, `int("abc")` → ValueError).

## Task 4.1: Function-signature extraction tooling

Write `scripts/extract_builtins.py` that parses `stdlib/__init__.py` + each `*_contracts.py` and emits `stdlib_catalog.json` (name, module, description, signature, category). This becomes the source of truth for both Rust registration and `helen docgen` parity. Generate once, commit the JSON, regenerate when the Python source changes.

## Task 4.2: Core + string + collection (highest priority)

**Core builtins** (in interpreter or stdlib-category `core`): `print, len, str, int, float, bool, list, dict, abs, min, max, range, type, isinstance, input, multiline_input, read_file`.

**string.rs (25):** `upper, lower, strip, lstrip, rstrip, split, rsplit, join, startswith, endswith, replace, find, rfind, find_from, contains, substring, trim_prefix, trim_suffix, interpolate, regex_match, regex_search, regex_test, regex_replace, regex_split, regex_findall, tokenize, word_count, levenshtein, similarity, remove_punctuation, normalize_whitespace, extract_urls, extract_emails, base64_encode, base64_decode, html_escape, html_unescape`.

> ⚠️ String ops use **native byte-based** semantics (D4): `len()` = byte length; `substring`/`[i]`/`find` operate on byte offsets with UTF-8 boundary checks. `upper/lower` via `char::to_uppercase/to_lowercase` (+ `unicode-normalization` where needed) — verify **ASCII** parity with Python. Non-ASCII (CJK) results deliberately diverge from Python (byte- vs code-point): add ASCII differential tests plus an expected-diff list for non-ASCII fixtures.

**collection.rs (33):** `map, filter, reduce, find, find_if, every, some, sort, unique, flatten, chunk, zip, keys, values, entries, merge, get, set, has, push, pop, shift, unshift, slice, splice, concat, includes, index_of, reverse, fill, min_by, max_by, group_by`.

**Tests per module:** port Python `tests/stdlib/test_string.py` inputs as data tables → Rust table tests, plus differential runs.

## Task 4.3: data + data_formats

**data.rs (16):** `json_parse, json_parse_lenient, json_stringify, json_load, json_save, html_parse, html_text, html_links, html_select, markdown_to_html, markdown_extract_headings, markdown_parse, csv_parse, csv_stringify, csv_load, csv_save`.
**data_formats.rs (15):** `yaml_parse, yaml_stringify, yaml_load, yaml_save, toml_parse, toml_stringify, toml_load, toml_save, xml_parse, xml_stringify, xml_load, xml_save`.

Rust deps: `serde_json` (json), `toml` (toml), `serde_yaml` (yaml), `quick-xml` or `roxmltree` (xml), `csv` (csv), custom small markdown/html parsers (port behavior from Python — verify output formats).

**Compatibility note:** `json_stringify` output formatting (indent, key order, unicode escapes) must match Python `json.dumps` defaults — add snapshot tests.

## Task 4.4: time + math_stats + crypto

**time.rs (20):** `now, time_func, sleep, date, datetime, fromtimestamp, date_format, date_parse, date_add, date_diff, date_year, date_month, date_day, date_weekday, stopwatch_start, stopwatch_elapsed, stopwatch_lap`. Use `chrono`; `date_format` must match Python `strftime` directives → write a `strftime` compatibility layer (or use `chrono`'s formatting with a directive mapping + `%Z`/`%z` handling).

**math_stats.rs (33):** `mean, median, mode, variance, stddev, correlation, percentile, sum, product, min, max, cos, sin, tan, acos, asin, atan, atan2, sqrt, pow, floor, ceil, round, log, log2, log10, exp, abs, clamp, lerp, degrees, radians, gcd, lcm, is_prime, factorial, random, randint, choice, shuffle, sample, seed`.
**crypto.rs (22):** `md5, sha1, sha256, sha512, hmac_sha256, hash_file, random, randint, choice, shuffle, sample, uuid_generate, uuid_from_string, uuid_nil, random_bytes`. Use `md-5`, `sha1`, `sha2`, `hmac`, `uuid` crates; output hex lowercase like Python `hashlib`.

## Task 4.5: file_advanced + system + network

**file_advanced.rs (20):** `file_size, file_modified, list_dir, walk_dir, copy_file, move_file, delete_file, delete_dir, temp_file, temp_dir, glob_files, grep_files, read_json, write_json, read_lines, write_lines, append_file, ensure_dir, touch`.
**system.rs (31):** `env_get, env_set, env_list, env_delete, get_cli_args, parse_cli_args, exec, exec_async, pid, exit, kill, log_debug, log_info, log_warn, log_error`. `exec` uses `std::process::Command` (mirror `shell_exec` tool defaults: `shell=false`, timeout, PID/signal validation — port the security checks from `runtime/system.py`).
**network.rs (9):** `http_get, http_post, http_put, http_delete, http_download, url_parse, url_build, url_encode, url_decode` (via `ureq`; port URL-validation and size-limit constants from `runtime/network.py`).

## Task 4.6: context + quality + debug + test + tools + llm_control + media + transcript

These need the runtime (M5/M6/M8) — **implement in M5+ as features land**, but register names + stubs returning the documented error in M4 so `helen check` and `docgen` are complete early.

- **context.rs (50):** `clear_context, compress_context, context_stats, context_usage, get_message, delete_message, pin_message, unpin_message, list_pinned_messages, …`
- **quality.rs (4):** `analyze_code, check_security, quality_score, quality_report` (call the code-quality engine — port 7-dimension scoring from `stdlib/quality.py`).
- **test.rs (23):** `describe, it, it_skip, assert_true, assert_equal, assert_not_equal, assert_contains, assert_throws, expect, before_each, after_each, before_all, after_all, run_tests, run_tests_json, test_reset, test_count, test_suite, test_case, test_case_skip` → feeds M12 `helen test`.
- **tools.rs (9):** `web_search, web_fetch, shell_exec, calculate, patch_file, load_skill, list_skill_references` → wrappers over `helen-runtime` tools (M6).
- **llm_control.rs (17):** `cancel_llm_call, current_llm_call_id, cancel_all_llm_calls, set_temperature, get_temperature, set_max_turns, get_max_turns, set_max_tokens, get_max_tokens, set_thinking_mode, get_thinking_mode, set_reasoning_effort, get_reasoning_effort, get_model, get_description, get_provider`.
- **media.rs (12):** `media, media_base64, is_media, media_type, to_openai_parts, to_claude_parts, to_gemini_parts, media_to_base64, save_media, is_image, is_video, is_audio`.
- **transcript.rs (51) + transcript_query.rs (2):** port against the TranscriptStore (M8).
- **mailbox.rs (1):** `mailbox_select` (M7).

## Task 4.7: Chinese aliases

Port `stdlib/locales/zh.py`: alias map `"字符串转大写" → "upper"` etc. Register as secondary names in the registry. Add a test: every alias resolves to the same function object and `helen docgen --zh` lists them.

## Task 4.8: Stdlib differential sweeps

For each module: `scripts/diff.sh --suite stdlib/<module>` with a dedicated corpus. Add edge-case programs (empty inputs, unicode, negative indices, NaN/Inf) to `tests/programs/stdlib/` — for string modules assert **ASCII** parity and add non-ASCII cases to `tests/conformance/expected-diffs.md` (D4).

## Definition of Done — M4

- [ ] All 378 names registered; `helen docgen` output matches Python's docgen for the full function table.
- [ ] Chinese aliases registered and tested.
- [ ] Differential pass per module ≥ reference parity on corpus (string/collection/data/time first).
- [ ] Runtime-dependent modules (context/quality/test/tools/llm_control/media/transcript/mailbox) return correct results once their runtime lands in M5–M8.
