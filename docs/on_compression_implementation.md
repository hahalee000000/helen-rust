# on_compression Callback 完整实现

## 概述

完成了 `on_compression` callback 的完整实现（方案2），包括注册和调用功能。

## 实现内容

### 1. Interpreter 结构体扩展

**文件**: `crates/helen-interpreter/src/interpreter.rs`

```rust
pub struct Interpreter {
    // ... existing fields ...
    
    /// P1: Compression callback (invoked after each context compression).
    pub compression_callback: Option<Value>,
}
```

在 `Interpreter::new()` 中初始化：
```rust
compression_callback: None,
```

### 2. on_compression 函数实现

**文件**: `crates/helen-interpreter/src/context.rs`

```rust
pub fn context_on_compression(
    interp: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let callback = args.first().cloned();
    let previous = interp.compression_callback.take();

    // Store the new callback (None clears it)
    interp.compression_callback = match &callback {
        Some(Value::Null) => None,
        Some(v) => Some(v.clone()),
        None => None,
    };

    let mut result = make_ok_map();
    result.insert(
        Value::Str(Rc::from("previous")),
        previous.unwrap_or(Value::Null),
    );
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}
```

**功能**:
- 接受一个 callback 参数（闭包或 null）
- 返回 `{"status": "ok", "previous": <previous_callback>}`
- 支持清除 callback（传入 null）

### 3. compress_context 调用 Callback

**文件**: `crates/helen-interpreter/src/context.rs`

在 `context_compress_context` 函数中添加 callback 调用：

```rust
// 成功压缩后调用
if let Some(callback) = &interp.compression_callback {
    let stats = Value::Map(Rc::new(RefCell::new(result.clone())));
    let _ = interp.call_value(callback.clone(), vec![stats]);
}

// 错误情况下也调用（提供默认 stats）
None => {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    result.insert(Value::Str(Rc::from("original_messages")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("compressed_messages")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("original_tokens")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("compressed_tokens")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("strategy")), Value::Str(Rc::from(strategy.as_str())));
    
    if let Some(callback) = &interp.compression_callback {
        let stats = Value::Map(Rc::new(RefCell::new(result.clone())));
        let _ = interp.call_value(callback.clone(), vec![stats]);
    }
    
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}
```

**Stats 结构**:
```helen
{
    "status": "ok" | "error",
    "original_messages": int,
    "compressed_messages": int,
    "original_tokens": int,
    "compressed_tokens": int,
    "strategy": str
}
```

### 4. 测试修复

**文件**: `crates/helen-interpreter/tests/on_compression_callback_tests.rs`

修复了所有5个测试的导入问题：
```helen
import std.core.*
import std.context.*  // 添加这行
```

**测试覆盖**:
1. ✅ `on_compression_registers_callback` - 注册 callback
2. ✅ `on_compression_returns_previous_callback` - 返回前一个 callback
3. ✅ `on_compression_with_none_clears_callback` - 清除 callback
4. ✅ `on_compression_callback_invoked_on_compression` - callback 被调用
5. ✅ `on_compression_callback_receives_stats` - callback 接收 stats

## 使用示例

```helen
import std.core.*
import std.context.*

main {
    // 注册 compression callback
    let callback = fn(stats) {
        print("Compression occurred!")
        print("Original tokens: " + str(stats["original_tokens"]))
        print("Compressed tokens: " + str(stats["compressed_tokens"]))
        print("Strategy: " + stats["strategy"])
    }
    
    let result = on_compression(callback)
    print("Registration: " + str(result))
    
    // 添加消息
    for i in range(0, 20) {
        insert_message("user", "message " + str(i))
    }
    
    // 触发压缩（会调用 callback）
    compress_context("truncate")
    
    // 清除 callback
    on_compression(null)
}
```

## 与 Python 实现的对比

| 功能 | Python | Rust | 状态 |
|------|--------|------|------|
| 注册 callback | ✅ | ✅ | 完整实现 |
| 返回 previous | ✅ | ✅ | 完整实现 |
| 清除 callback | ✅ | ✅ | 完整实现 |
| 调用 callback | ❌ (未实现) | ✅ | **超越 Python** |
| 错误时调用 | ❌ | ✅ | **超越 Python** |

**注意**: Python 实现只注册 callback 但从未调用，Rust 实现完整支持注册和调用。

## 技术细节

### Callback 调用机制

使用 `Interpreter::call_value` 方法调用 callback：
```rust
interp.call_value(callback.clone(), vec![stats])
```

这个方法支持：
- `Value::UserFn` - 用户定义函数
- `Value::Closure` - 闭包
- `Value::BuiltinFn` - 内置函数
- `Value::Agent` - Agent 调用

### 错误处理

- Callback 调用失败不会中断压缩流程（使用 `let _ =` 忽略错误）
- 即使没有 TranscriptStore，也会调用 callback（提供默认 stats）
- 保证 callback 在所有情况下都被调用（成功/失败）

## 测试验证

```bash
# 运行 on_compression 测试
cargo test --package helen-interpreter --test on_compression_callback_tests

# 结果
running 5 tests
test on_compression_registers_callback ... ok
test on_compression_returns_previous_callback ... ok
test on_compression_with_none_clears_callback ... ok
test on_compression_callback_invoked_on_compression ... ok
test on_compression_callback_receives_stats ... ok

test result: ok. 5 passed; 0 failed
```

## 构建验证

```bash
# Release 构建
cargo build --release

# 结果
Finished `release` profile [optimized] target(s) in 22.93s
```

## 总结

完整实现了 `on_compression` callback 功能，包括：
1. ✅ Callback 注册和存储
2. ✅ 返回前一个 callback
3. ✅ 支持清除 callback
4. ✅ 压缩后自动调用 callback
5. ✅ 传递完整的压缩统计信息
6. ✅ 错误情况下也调用 callback
7. ✅ 所有测试通过

实现超越了 Python 参考实现，提供了更完整的生命周期钩子功能。
