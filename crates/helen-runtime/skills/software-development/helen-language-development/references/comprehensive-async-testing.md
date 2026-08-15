# Comprehensive Async/Await Testing Patterns

Session: 2026-06-17  
Test file: `tests/execution/test_async_comprehensive.py`  
Result: 32 new tests, all passing

## Overview

This document captures the comprehensive testing approach for Helen's async/await implementation, covering both regular functions and agents, statement and expression forms, concurrent execution, error handling, and integration with AsyncLLMInterpreter.

## Test Structure

### 1. Statement Form Tests (Immediate Execution)

```python
def test_async_stmt_returns_completed_task(self):
    """async Agent() as statement returns Task.completed."""
    source = """
agent Worker() {
    main {
        return "done"
    }
}

main {
    async Worker()
}
"""
    result, interp = parse_and_run(source)
    assert isinstance(result, Task)
    assert result.is_done, "Statement form should be immediately completed"
    assert result.result() == "done"
```

**Key points:**
- Statement form executes immediately
- Returns `Task.completed` (not pending)
- Can be used as last expression in main block

### 2. Expression Form Tests (Deferred Execution)

```python
def test_async_expr_creates_pending_task(self):
    """let task = async Agent() creates a pending Task."""
    source = """
agent Worker() {
    main {
        return "done"
    }
}

main {
    let task = async Worker()
    task
}
"""
    result, interp = parse_and_run(source)
    assert isinstance(result, Task)
    assert result.is_pending, "Expression form should be pending"
    assert not result.is_done
```

**Key points:**
- Expression form creates `Task.pending`
- Execution deferred until `await`
- Must use `await [task]` to execute and get result

### 3. Regular Function Async Calls

```python
def test_async_function_expr_and_await(self):
    """let task = async fn() and await [task]."""
    source = """
fn compute(x: num) {
    return x * x
}

main {
    let t = async compute(7)
    let results = await [t]
    results[0]
}
"""
    result, interp = parse_and_run(source)
    assert result == 49
```

**Key points:**
- Works with both `agent` and `fn` declarations
- Same semantics for both
- Multiple functions can be called concurrently

### 4. Concurrent Execution Timing

```python
def test_async_llm_calls_concurrent_timing(self):
    """AsyncLLMInterpreter: LLM calls run concurrently."""
    
    class TimingLLMRuntime(LLMRuntime):
        def __init__(self, delay=0.1):
            self.delay = delay
            self.call_count = 0
        
        async def act_async(self, prompt, **kwargs):
            await asyncio.sleep(self.delay)
            self.call_count += 1
            return LLMResponse(text=f"response_{self.call_count}", model="mock")
        
        # ... other methods
    
    runtime = TimingLLMRuntime(delay=0.1)
    interp = AsyncLLMInterpreter(llm_runtime=runtime)
    
    # Create 3 LLM act expressions
    nodes = [
        LlmActExprNode(prompt=LiteralNode(value=f"task_{i}", span=span), span=span)
        for i in range(3)
    ]
    
    # Create pending tasks
    tasks = [
        Task.pending(node, interp, interp.environment.snapshot())
        for node in nodes
    ]
    
    # Execute concurrently
    start = time.time()
    results = interp._await_tasks(tasks)
    elapsed = time.time() - start
    
    # Should complete in ~0.1s (concurrent), not ~0.3s (sequential)
    assert elapsed < 0.25, f"Expected concurrent (<0.25s), got {elapsed:.2f}s"
```

**Key points:**
- Use timing to verify true concurrency
- 3 × 0.1s delays should complete in ~0.1s total
- Custom runtime with `asyncio.sleep()` for predictable timing

### 5. Error Handling Tests

#### Single Task Error

```python
def test_single_task_error_raises_on_await(self):
    """Awaiting a failed task raises its exception."""
    source = """
agent Failer() {
    main {
        throw RuntimeError("task failed")
    }
}

main {
    let t = async Failer()
    await [t]
}
"""
    with pytest.raises(Exception):
        parse_and_run(source)
```

#### Multiple Failures → AggregateError

```python
def test_multiple_task_errors_raise_aggregate(self):
    """Multiple failed tasks raise AggregateError."""
    source = """
agent Failer(msg: str) {
    main {
        throw RuntimeError(msg)
    }
}

main {
    let t1 = async Failer("error1")
    let t2 = async Failer("error2")
    await [t1, t2]
}
"""
    with pytest.raises(AggregateError) as exc_info:
        parse_and_run(source)
    assert len(exc_info.value.errors) == 2
```

#### Try-Catch AggregateError

```python
def test_try_catch_aggregate_error(self):
    """try-catch can catch AggregateError."""
    source = """
agent Failer() {
    main {
        throw RuntimeError("oops")
    }
}

main {
    let t1 = async Failer()
    let t2 = async Failer()
    try {
        await [t1, t2]
    } catch AggregateError err {
        "caught"
    }
}
"""
    result, interp = parse_and_run(source)
    assert result == "caught"
```

**Key points:**
- `throw` requires exception type name (not bare string)
- Multiple failures aggregate into `AggregateError`
- `try-catch` can handle `AggregateError`

### 6. Mixed Sync/Async Execution

```python
def test_sync_before_async(self):
    """Sync code before async calls."""
    source = """
fn compute() {
    return 10
}

agent Worker(x: num) {
    main {
        return x + 1
    }
}

main {
    let base = compute()
    let t = async Worker(base)
    let results = await [t]
    results[0]
}
"""
    result, interp = parse_and_run(source)
    assert result == 11
```

**Key points:**
- Sync and async code can be mixed freely
- Sync results can be passed to async calls
- Async results can be used in sync processing

### 7. AsyncLLMInterpreter Integration

```python
def test_async_interpreter_multiple_agents(self):
    """AsyncLLMInterpreter with multiple concurrent agents."""
    source = """
agent A() { main { return "a" } }
agent B() { main { return "b" } }
agent C() { main { return "c" } }

main {
    let t1 = async A()
    let t2 = async B()
    let t3 = async C()
    let results = await [t1, t2, t3]
    results[0] + results[1] + results[2]
}
"""
    interp = make_async_interpreter()
    result, _ = parse_and_run(source, interpreter=interp)
    assert result == "abc"
```

**Key points:**
- AsyncLLMInterpreter handles regular agents correctly
- Multiple agents execute concurrently
- Results collected in order

### 8. Edge Cases

#### No Return Value

```python
def test_async_with_no_return(self):
    """Agent with no return statement returns None."""
    source = """
agent Silent() {
    main {
    }
}

main {
    let t = async Silent()
    let results = await [t]
    results[0]
}
"""
    result, interp = parse_and_run(source)
    assert result is None
```

#### Complex Return Expressions

```python
def test_async_with_complex_return(self):
    """Agent returning complex expression."""
    source = """
agent Calculator(a: num, b: num) {
    main {
        let sum = a + b
        let product = a * b
        return sum + product
    }
}

main {
    let t = async Calculator(3, 4)
    let results = await [t]
    results[0]
}
"""
    result, interp = parse_and_run(source)
    assert result == 19  # 7 + 12
```

#### Conditionals and Loops

```python
def test_async_with_conditionals(self):
    """Agent with conditional logic."""
    source = """
agent Classifier(x: num) {
    main {
        if (x > 0) {
            return "positive"
        } else {
            return "non-positive"
        }
    }
}

main {
    let t1 = async Classifier(5)
    let t2 = async Classifier(-3)
    let results = await [t1, t2]
    results[0] + "," + results[1]
}
"""
    result, interp = parse_and_run(source)
    assert result == "positive,non-positive"
```

### 9. LLM Async Call Verification

```python
def test_llm_act_async_execution(self):
    """llm act expression uses async execution path."""
    
    class TrackingAsyncRuntime(LLMRuntime):
        def __init__(self):
            self.async_calls = 0
            self.sync_calls = 0
        
        def act(self, prompt, **kwargs):
            self.sync_calls += 1
            return LLMResponse(text="sync_response", model="mock")
        
        async def act_async(self, prompt, **kwargs):
            self.async_calls += 1
            return LLMResponse(text="async_response", model="mock")
    
    runtime = TrackingAsyncRuntime()
    interp = AsyncLLMInterpreter(llm_runtime=runtime)
    
    node = LlmActExprNode(
        prompt=LiteralNode(value="test prompt", span=span),
        span=span
    )
    
    task = Task.pending(node, interp, interp.environment.snapshot())
    asyncio.run(task.execute_async())
    
    assert task.is_done
    assert runtime.async_calls == 1
    assert runtime.sync_calls == 0  # Should use async, not sync
    assert task.result() == "async_response"
```

**Key points:**
- Verify async path is actually used (not sync fallback)
- Track call counts to ensure correct method invoked
- MockLLMRuntime doesn't override async methods - need custom runtime

## Test Results

**Total: 32 tests, all passing**

| Category | Count | Status |
|----------|-------|--------|
| Statement form | 3 | ✅ |
| Expression form | 3 | ✅ |
| Function calls | 3 | ✅ |
| Concurrent timing | 3 | ✅ |
| Error handling | 4 | ✅ |
| Mixed sync/async | 3 | ✅ |
| AsyncLLMInterpreter | 2 | ✅ |
| Edge cases | 6 | ✅ |
| Task state transitions | 3 | ✅ |
| LLM async calls | 2 | ✅ |

## Lessons Learned

1. **Test both forms**: Statement (immediate) and expression (deferred) have different semantics
2. **Verify concurrency with timing**: Use measurable delays to prove concurrent execution
3. **Test error aggregation**: Multiple failures should aggregate, not just raise first error
4. **Custom runtime for async**: MockLLMRuntime doesn't override async methods
5. **Edge cases matter**: No return, conditionals, loops, nested calls all need testing
6. **End-to-end is essential**: Unit tests miss integration issues
7. **throw syntax**: Must use exception type names, not bare strings
8. **pytest.mark.asyncio**: Required for all async tests in strict mode

## References

- Test file: `~/helen/tests/execution/test_async_comprehensive.py`
- Commit: `c971085` - "Add comprehensive async/await tests (32 new tests, fix e2e)"
- Total test count: 933 (888 → 933, +45)
