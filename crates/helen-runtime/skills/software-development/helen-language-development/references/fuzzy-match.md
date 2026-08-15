# Fuzzy Match Engine

Source: `helen/runtime/fuzzy_match.py` (860 lines, copied from Hermes `hermes-agent/tools/fuzzy_match.py`)

## Purpose

Robustly find and replace text in files, accommodating variations in whitespace, indentation, and escaping common in LLM-generated code. Used by the `patch_file` built-in tool.

## Main Entry Point

```python
from helen.runtime.fuzzy_match import fuzzy_find_and_replace, format_no_match_hint

new_content, match_count, strategy, error = fuzzy_find_and_replace(
    content="def foo():\n    pass",
    old_string="def foo():",
    new_string="def bar():",
    replace_all=False
)
# Returns: (modified_content, num_replacements, strategy_name, error_message)
# Success: (new_content, 1, "exact", None)
# Failure: (original_content, 0, None, "error description")
```

## 9 Matching Strategies (tried in order)

| # | Name | Function | What it handles |
|---|------|----------|-----------------|
| 1 | exact | `_strategy_exact` | Direct `str.find()` — no normalization |
| 2 | line_trimmed | `_strategy_line_trimmed` | Strip leading/trailing whitespace per line |
| 3 | whitespace_normalized | `_strategy_whitespace_normalized` | Collapse `[ \t]+` → single space |
| 4 | indentation_flexible | `_strategy_indentation_flexible` | Strip all leading whitespace (`lstrip()`) |
| 5 | escape_normalized | `_strategy_escape_normalized` | `\\n` → newline, `\\t` → tab, `\\r` → CR |
| 6 | trimmed_boundary | `_strategy_trimmed_boundary` | Trim first/last line whitespace only |
| 7 | unicode_normalized | `_strategy_unicode_normalized` | Smart quotes → ASCII, em-dash → `--`, etc. |
| 8 | block_anchor | `_strategy_block_anchor` | Match first+last lines, SequenceMatcher for middle (0.50 unique / 0.70 multiple) |
| 9 | context_aware | `_strategy_context_aware` | Line-by-line: ≥80% similarity, ≥50% lines must match |

## Key Helper Functions

- `_detect_escape_drift()` — Guards against `\'` or `\"` serialization artifacts from JSON tool calls
- `_reindent_replacement()` — Adjusts new_string indentation to match the file's actual indent when fuzzy-matched
- `_maybe_unescape_new_string()` — Conditionally converts `\\t`/`\\r` to real tab/CR only when file region contains them
- `_apply_replacements()` — Applies replacements end-to-start to preserve positions
- `find_closest_lines()` — "Did you mean?" feedback using SequenceMatcher
- `format_no_match_hint()` — Appends helpful hints when no match found

## Unicode Normalization Map

```python
UNICODE_MAP = {
    "\u201c": '"', "\u201d": '"',   # smart double quotes
    "\u2018": "'", "\u2019": "'",   # smart single quotes
    "\u2014": "--", "\u2013": "-",  # em/en dashes
    "\u2026": "...", "\u00a0": " ", # ellipsis and non-breaking space
}
```

## Testing All Strategies

```python
from helen.runtime.fuzzy_match import fuzzy_find_and_replace

# 1. Exact
fuzzy_find_and_replace('def foo():\n    pass', 'def foo():', 'def bar():')
# → strategy="exact"

# 2. Line-trimmed (tab vs space)
fuzzy_find_and_replace('def foo():\n\tpass', 'def foo():\n    pass', 'def bar():\n    pass')
# → strategy="line_trimmed"

# 3. Whitespace-normalized (double spaces)
fuzzy_find_and_replace('def  foo():\n\tpass', 'def foo():\n    pass', 'def bar():\n    pass')
# → strategy="whitespace_normalized"

# 7. Unicode-normalized (em-dash)
fuzzy_find_and_replace('text\u2014more', 'text--more', 'text--changed')
# → strategy="unicode_normalized"
```

## Integration with patch_file Tool

In `helen/runtime/tools.py`, `_patch_file()` calls:
```python
from helen.runtime.fuzzy_match import fuzzy_find_and_replace, format_no_match_hint

new_content, match_count, strategy, error = fuzzy_find_and_replace(
    content, old_string, new_string, replace_all=replace_all
)
```

Then generates a unified diff via `difflib.unified_diff()` for LLM feedback.
