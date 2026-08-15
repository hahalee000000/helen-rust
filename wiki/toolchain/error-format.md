# 错误格式化

> 模块 M10 | `helen/core/errors.py` + `helen/cli/formatter.py` | HLD 3.11.2

---

## HelenError 结构

```python
@dataclass
class HelenError(Exception):
    code: ErrorCode         # 错误码枚举
    message: str            # 人类可读消息
    span: SourceSpan | None # 源码位置
```

---

## format_error() 输出格式

```
Error: [E0311] at main.helen:5:10
  5 | let x = y
    |         ^
Undefined variable 'y'

Code: E0311 — UNDEFINED_VARIABLE
```

### 组成部分

1. **标题行**：`Error: [E{code:04d}] at {file}:{line}:{col}`
2. **缩进**：`  {line_num} | {source_line}`
3. **定位符**：`    | {" " * (col-1)}{"^" * len(lexeme)}`
4. **消息**：错误描述
5. **空行**
6. **代码说明**：`Code: E{code} — {ERROR_NAME}`

---

## 多错误输出

```
Error: [E0311] at main.helen:3:5
  3 | let x = y
    |         ^
Undefined variable 'y'

Code: E0311 — UNDEFINED_VARIABLE

Error: [E0313] at main.helen:5:5
  5 | let x = 1
    |     ^
Duplicate declaration 'x'

Code: E0313 — DUPLICATE_DECLARATION

2 errors found.
```

---

## 警告格式

```python
@dataclass
class HelenWarning:
    code: str               # 如 "W001"
    message: str
    span: SourceSpan | None
```

```
Warning: [W001] at main.helen:10:1
  10 | let unused = 42
     |     ^^^^^^
Variable 'unused' is declared but never used
```

---

## ErrorReporter 收集器

```python
class ErrorReporter:
    def __init__(self):
        self._errors: list[HelenError] = []

    def error(self, span: SourceSpan | None, message: str, code: ErrorCode):
        self._errors.append(HelenError(code, message, span))

    def has_errors(self) -> bool:
        return len(self._errors) > 0

    def get_errors(self) -> list[HelenError]:
        return list(self._errors)
```

编译阶段通过 `ErrorReporter` 收集所有错误（不中断分析），最终一次性输出。
