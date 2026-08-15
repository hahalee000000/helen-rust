# Interpreter Sentinel Architecture

## Sentinel Types

| Sentinel | Purpose | Where Produced | Where Consumed |
|----------|---------|----------------|----------------|
| `ReturnSentinel` | Early return from function | `visit_return_stmt` | `_execute_stmts` (propagates), `interpret()` (unwraps value) |
| `BreakSentinel` | Exit nearest loop | `visit_break_stmt` | `visit_while_stmt` / `visit_for_stmt` (break loop, returns `None`) |
| `ContinueSentinel` | Skip to next iteration | `visit_continue_stmt` | `visit_while_stmt` / `visit_for_stmt` (continue loop, returns `None`) |

## Critical Rule: Sentinels Must NOT Leak

**Never return a sentinel from a control-flow handler to a sibling statement level.** The flow:

```
Program
├── let x = 1          → value 1
├── while(...) { break } → BreakSentinel → consumed by while → returns None
├── let y = x + 1      → value 2   ← this MUST still execute!
└── print(y)           → prints 2
```

If `_execute_stmts` returned `BreakSentinel` directly, the loop would stop and `let y` / `print(y)` would never run.

## Double-Execution Bug (Fixed)

**Symptom**: `'builtin_name' is not callable` on valid builtin calls like `len(nums)`.

**Root cause**: `interpret()` ran `visit_program(program)` then re-executed `program.statements` in a second loop. First run defined `let len = len(nums)` correctly (variable `len` gets value `3`). Second run re-defined `len` in the environment, then re-evaluated the call `len(nums)` — but `len` now resolved to the int `3`, not the stdlib function.

**Fix**: Single execution path. `interpret()` calls `visit_program(program)` once, unwraps sentinels at top level, returns.

## `_execute_stmts` Sentinel Handling

```python
def _execute_stmts(self, stmts):
    result = None
    for stmt in stmts:
        step = self._execute(stmt)
        if isinstance(step, ReturnSentinel):
            return step          # Propagate: exit function immediately
        if isinstance(step, (BreakSentinel, ContinueSentinel)):
            return None          # Consume: stop execution, return nothing
        result = step
    return result
```

## `visit_while_stmt` Sentinel Handling

```python
def visit_while_stmt(self, node):
    result = None
    while True:
        condition = node.condition.accept(self)
        if not self._truthy(condition):
            break
        step_result = self._execute(node.body)
        if isinstance(step_result, BreakSentinel):
            break               # Consume and exit loop
        if isinstance(step_result, ContinueSentinel):
            continue            # Consume and restart loop
        if isinstance(step_result, ReturnSentinel):
            return step_result  # Propagate: exit function
        result = step_result     # Track last non-sentinel result
    return result               # Returns None on break, not BreakSentinel
```

## Testing Checklist

When modifying the interpreter:
1. `let x = builtin_func()` — builtins must not be shadowed on first eval
2. `let len = len(list)` — variable shadows builtin on subsequent calls
3. `while(true) { break }; print("after")` — code after loop must execute
4. `fn f() { return 42; print("unreachable") }` — return stops function body
5. `while(cond) { if(x) continue; y = y + 1 }` — continue skips rest of body
