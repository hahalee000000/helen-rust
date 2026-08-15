# Tutorial-Implementation Sync Verification

## Methodology

When verifying tutorial accuracy against the Helen implementation:

1. **Extract all `helen` code blocks** from tutorial markdown files at `~/wiki/helen/tutorial/`
2. **Parse + Analyze + Interpret** each block through the actual compiler pipeline
3. **Skip dangerous patterns**: `while(true)`, `while(1)`, `while` without `break` (infinite loop risk)
4. **Skip runtime-dependent features**: `llm`, `async/await`, `import` (requires file fixtures)
5. **Compare expected vs actual**: blocks with `❌` or `Error:` in comments are marked "expected fail"

## Known Skip Categories (as of 2026-06)

| Category | Reason | Tutorials Affected |
|----------|--------|-------------------|
| `while` without `break` | Infinite loop risk (shadowing bug) | 04-control-flow |
| `llm` statements | Requires real/Mock LLM | 05, 06, 10 |
| `async/await` | Requires async runtime | 07, 10 |
| `import` statements | Requires file fixtures on disk | 08 |
| `try/catch` examples | Reference undeclared functions/agents | 04, 10 |

## Common Syntax Mismatch Patterns Found

| Tutorial Used | Correct | Root Cause |
|--------------|---------|------------|
| `llm if { case "x": }` | `llm if { branch "x" { } }` | Tutorial confused `llm if` with `match` |
| `catch Type(err)` | `catch Type err` | Parser expects type+name, not parenthesized |
| `catch Type as err` | `catch Type err` | `as` keyword not implemented in parser |
| `let x = x + 1` in while | `x = x + 1` in while | `let` creates new scope variable (shadowing) |

## Test Runner Location

`~/helen/tests/tutorial/run_tutorial_tests.py`

Current results: **49 pass, 0 fail, 31 skipped** (80 total blocks across 10 tutorials)

## Pitfall: ❌ in Comments

The test runner's `extract_helen_blocks()` marks any code block containing `❌` or `"Error:"` as "expected fail" — **even in comments**. Keep `❌` only in blocks that genuinely should fail.
