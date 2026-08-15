# Interpreter Execution Patterns — Hellen Phase 3 Learnings

## Function Call Resolution Order

When implementing `visit_call` in an interpreter, check the function registry BEFORE evaluating `node.callee.accept(self)`:

```python
def visit_call(self, node: CallNode) -> object:
    callee_name = node.callee.name if isinstance(node.callee, VariableNode) else None

    # FIRST: Check registered functions (not in environment)
    if callee_name is not None and callee_name in self._functions:
        func = self._functions[callee_name]
        args = [arg.value.accept(self) for arg in node.arguments]
        return self._call_function(func, args)

    # THEN: Evaluate callee as expression (for higher-order functions)
    callee = node.callee.accept(self)
    args = [arg.value.accept(self) for arg in node.arguments]

    if isinstance(callee, FunctionDeclNode):
        return self._call_function(callee, args)

    self._runtime_error(node.span, f"'{callee_name}' is not callable")
    return None
```

**Pitfall**: If you call `node.callee.accept(self)` first for a `VariableNode("sum")`, it returns `None` because functions are stored in `_functions`, not the `Environment`. The variable lookup fails silently.

## Control Flow via Sentinels (Not Exceptions)

Use return-value sentinels for break/continue/return — NOT Python exceptions:

```python
@dataclass
class BreakSentinel:
    span: SourceSpan | None = None

@dataclass
class ContinueSentinel:
    span: SourceSpan | None = None

@dataclass
class ReturnSentinel:
    value: Any = None
```

Loop implementation checks for sentinel types:

```python
def visit_for_stmt(self, node: ForStmtNode) -> object:
    for item in iterable:
        self.environment = self.environment.enter_scope()
        try:
            self.environment.define(node.iterator.name, item)
            result = self._execute(node.body)
            if isinstance(result, BreakSentinel):
                break
            if isinstance(result, ContinueSentinel):
                continue
            if isinstance(result, ReturnSentinel):
                return result
        finally:
            self.environment = old_env
```

**Why not exceptions**: Break/continue are normal control flow, not error conditions. Using exceptions would require catching them in every loop, making the code noisy and slow.

## Environment Chain Pattern

Scope management with `enter_scope`/`exit_scope` and `try/finally`:

```python
old_env = self.environment
self.environment = self.environment.enter_scope()
try:
    # ... execute in new scope ...
finally:
    self.environment = old_env
```

Key properties:
- `enter_scope()` creates a child with `parent=self`
- `lookup(name)` walks up the chain
- `assign(name, value)` finds the defining scope and updates there
- `define(name, value, is_const)` always targets the innermost scope
- `assign` checks `is_const` in the defining scope (not current scope)

## Const Runtime Protection (Two Layers)

1. **AST level**: `visit_binary_op` detects ASSIGN operator + VariableNode left-hand side → checks `environment.is_const()`
2. **Environment level**: `environment.assign()` checks `_consts` set before updating

Both layers are needed: AST-level catches semantic analysis phase, environment-level catches runtime.

## Type Checking at Runtime (v1 Strategy)

- `_truthy(value)`: None/False/0/""/[]/{} → False, everything else → True
- `_equal(a, b)`: None-safe equality
- `_check_number(op, *values)`: Raises HellenRuntimeError if any value isn't int/float
- `_add(left, right)`: Supports string concatenation, numeric addition, AND list concatenation (auto-convert non-strings)
- `_stringify(value)`: Human-readable representation for all Hellen types
