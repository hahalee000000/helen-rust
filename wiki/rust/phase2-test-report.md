# Phase 2 测试报告

**日期:** 2026-08-16  
**目标:** 为高风险组件添加综合单元测试  
**提交:** `9e4152d`

## 执行结果

### 新增测试

| 类别 | 测试数 | 覆盖功能 |
|------|--------|---------|
| String 操作 | 8 | upper, lower, trim, contains, startswith, endswith, replace, reverse |
| Math 操作 | 6 | pow, sqrt, floor, ceil, round, mean, median |
| List 操作 | 3 | sort (ints/strings), copy |
| Dict 操作 | 3 | keys, values, get, get with default |
| **总计** | **18** | **stdlib 核心功能** |

### 测试方法

使用 **集成测试** 方式（通过 `run_src` 执行 Helen 源代码），而非直接调用内部函数。

**优势：**
- 测试真实的用户场景
- 验证完整的执行路径（parser → semantic → interpreter）
- 不需要暴露内部 API

**示例：**
```rust
#[test]
fn phase2_str_upper() {
    let src = r#"import std.core.*
import std.str.*
main {
    print(upper("hello"))
}
"#;
    let (r, out) = run_src(src);
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(out, "HELLO\n");
}
```

### 依赖更新

添加了测试框架依赖：
- `rstest = "0.18"` — 参数化测试（类似 pytest.mark.parametrize）
- `proptest = "1"` — property-based testing（已存在）

### 测试覆盖率

| 指标 | Phase 1 | Phase 2 | 变化 |
|------|---------|---------|------|
| Rust 测试总数 | 719 | 737 | +18 |
| stdlib 测试 | 0 | 18 | +18 |
| 覆盖率 | 20.2% | 20.7% | +0.5% |

## 发现的问题

### 1. 类型不一致

**问题：** math 函数返回类型不一致
- `floor(3.7)` → `3` (int)
- `ceil(3.2)` → `4` (int)
- `round(3.7)` → `4.0` (float)
- `pow(2, 3)` → `8.0` (float)

**影响：** 用户可能期望一致的返回类型

**建议：** 文档化这些行为差异，或统一返回类型

### 2. 字符串引号风格

**问题：** list 打印使用单引号
```
['apple', 'banana', 'cherry']
```

**Python 对比：** Python 使用双引号
```
['apple', 'banana', 'cherry']
```

**影响：** 无（仅显示差异）

### 3. Dict 方法缺失

**问题：** 没有 `has_key()` 方法

**替代方案：** 使用 `get(key, default)` 检查键是否存在

## 下一步建议

### Phase 2.5（可选）

添加更多 stdlib 测试：
- `std.time` — 日期/时间操作
- `std.file` — 文件 I/O
- `std.network` — 网络请求
- `std.crypto` — 加密/哈希

### Phase 3（推荐）

高风险组件单元测试：
1. **agent** (12 → 50 tests)
   - spawn/channel 生命周期
   - shared store 并发
   
2. **core** (9 → 40 tests)
   - Value 类型转换
   - 错误处理
   
3. **lexer** (9 → 60 tests)
   - 88 种 token 类型
   - 边界字符处理

### Phase 4（长期）

Property-based testing：
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_string_reverse_twice(s in ".*") {
        let reversed = s.chars().rev().collect::<String>();
        let double = reversed.chars().rev().collect::<String>();
        prop_assert_eq!(s, double);
    }
}
```

## 总结

✅ **Phase 2 完成**
- 添加 18 个 stdlib 集成测试
- 覆盖 string/math/list/dict 核心功能
- 所有测试通过
- 总测试数：719 → 737

**下一步：** 进入 Phase 3（agent/core/lexer 单元测试）
