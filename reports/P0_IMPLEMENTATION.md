# P0 Implementation Summary

## Overview
Successfully implemented P0 priority items: `tools.rs` and `debug.rs` stdlib modules.

## ✅ tools.rs — 7 Functions Implemented

All tool functions now wired to `helen_runtime::tools::dispatch_tool`:

| Function | Status | Description |
|----------|--------|-------------|
| `web_search(query, n)` | ✅ | Web search via dispatch_tool |
| `web_fetch(url)` | ✅ | Fetch URL content |
| `shell_exec(cmd, timeout, shell)` | ✅ | Execute shell command |
| `calculate(expr)` | ✅ | Evaluate math expression |
| `patch_file(path, old, new)` | ✅ | Patch file content |
| `load_skill(name)` | ✅ | Load skill documentation |
| `list_skill_references(name)` | ✅ | List skill references |

**Module**: `std.tools`  
**Import**: `import std.tools.*`

## ✅ debug.rs — 19 Functions Implemented

All debug functions now wired to `interpreter.observability`:

| Function | Status | Description |
|----------|--------|-------------|
| `get_llm_log()` | ✅ | Get LLM call audit log |
| `get_call_stack()` | ✅ | Get current call stack |
| `get_last_error()` | ✅ | Get last error snapshot |
| `last_error_detail()` | ✅ | Detailed error with scope/trace |
| `error_category(err)` | ✅ | Classify error semantically |
| `error_suggestion(err)` | ✅ | Actionable fix suggestion |
| `error_data_flow(err)` | ✅ | Data flow tracing |
| `validate_output(out, contract)` | ✅ | Validate LLM output (JSON/text/schema) |
| `record_session(path)` | ✅ | Start LLM recording |
| `stop_recording()` | ✅ | Stop recording |
| `replay_session(path)` | ✅ | Replay from cassette |
| `trace_value_origin(uuid)` | ✅ | Trace data origin |
| `trace_value_consumers(uuid)` | ✅ | Trace data consumers |
| `get_data_lineage()` | ✅ | Full lineage graph |
| `record_data_flow(...)` | ✅ | Manual data flow recording |
| `coverage_on()` | ✅ | Enable coverage tracking |
| `coverage_off()` | ✅ | Disable coverage tracking |
| `coverage_summary()` | ✅ | Coverage report |
| `coverage_report()` | ✅ | Detailed coverage |

**Module**: `std.debug`  
**Import**: `import std.debug.*`

## Usage Example

```helen
import std.core.*
import std.tools.*
import std.debug.*

main {
    // Tool functions
    let calc_result = calculate("2 + 3 * 4")
    print("calculate: " + str(calc_result))
    
    let shell_result = shell_exec("echo hello")
    print("shell_exec: " + str(shell_result))
    
    // Debug functions
    let log = get_llm_log()
    print("get_llm_log: " + str(log))
    
    let stack = get_call_stack()
    print("get_call_stack: " + str(stack))
}
```

## REPL Usage

In the REPL, users must explicitly import modules:

```
>>> import std.tools.*
>>> import std.debug.*
>>> let r = calculate("2 + 3 * 4")
>>> print(str(r))
{"expression":"2 + 3 * 4","result":14}
```

## Implementation Details

### tools.rs
- Delegates to `helen_runtime::tools::dispatch_tool()`
- Converts between `Value` and `serde_json::Value`
- Handles tool-specific argument parsing
- Returns tool results as Helen `Value` types

### debug.rs
- Accesses `interpreter.observability` field
- Uses `ObservabilityManager` for LLM audit, call stack, error tracking
- Implements data flow tracing and coverage tracking
- Validates LLM output against contracts (JSON/text/schema)

## Testing

All functions tested and verified:
- ✅ `calculate("2 + 3 * 4")` → `{"expression":"2 + 3 * 4","result":14}`
- ✅ `shell_exec("echo hello")` → `hello`
- ✅ `get_llm_log()` → `[]`
- ✅ All workspace tests pass

## Files Modified

1. `crates/helen-interpreter/src/tools.rs` — Complete rewrite (7 functions)
2. `crates/helen-interpreter/src/debug.rs` — Complete rewrite (19 functions)
3. `crates/helen-interpreter/src/stdlib.rs` — Removed `read_file` from CORE_EXPORTS
4. `crates/helen-runtime/src/llm.rs` — Added recording methods to LlmRuntime trait

## Next Steps

- **P1**: Implement `transcript.rs` (15 stubs) — wire to SessionManager + TranscriptStore
- **P1**: Implement `context.rs` (29 stubs) — need HistoryManager equivalent
- **P2**: Implement `quality.rs` (4 stubs) — port CodeAnalyzer + SecurityAnalyzer
- **P2**: Implement `llm_control.rs` (3 stubs) — wire to streaming call tracking
