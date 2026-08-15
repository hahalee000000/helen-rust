<!-- helen-rust edition: lexer in crates/helen-core (M1) — 36/36 token-stream differential. -->

# 词法分析 (Lexer)

> 模块 M1 | `helen/core/lexer.py` | 测试: `tests/lexer/test_lexer.py`

---

## 概述

Helen 使用**手写递归扫描器**（遵循 Crafting Interpreters 模式），将源代码转换为 Token 流。

```
"agent Translator {\n  description \"Translate text\"\n}"
         ↓ Scanner.scan_all()
[AGENT, IDENTIFIER(Translator), LEFT_BRACE, DESCRIPTION, STRING_DQ(...), RIGHT_BRACE, EOF]
```

---

## Token 结构

```python
@dataclass(frozen=True)
class Token:
    type: TokenType        # Token 类型 (77种)
    lexeme: str            # 原始文本
    literal: LiteralValue  # 解析后的值 (str/int/float/bool/None)
    line: int              # 行号 (1-based)
    column: int            # 列号 (1-based)
    span: SourceSpan       # 源码位置区间
```

---

## Token 类型分类

### 字面量 (5)
| Token | 示例 | literal 值 |
|---|---|---|
| `NUMBER` | `42`, `3.14` | `int` / `float` |
| `STRING` | `"hello"`, `'world'` | `str` |
| `TRIPLE_QUOTE_STRING` | `"""multi\nline"""` | `str` |
| `TRUE` | `true` | `True` |
| `FALSE` | `false` | `False` |

### 括号 (6)
`LEFT_PAREN` `RIGHT_PAREN` `LEFT_BRACE` `RIGHT_BRACE` `LEFT_BRACKET` `RIGHT_BRACKET`

### 分隔符 (8)
`DOT` `COMMA` `COLON` `SEMICOLON` `QUESTION` `PIPE` `IDENTIFIER` `EOF`

### 模板 (2)
`TEMPLATE_OPEN` (`{{`) `TEMPLATE_CLOSE` (`}}`)

### 运算符 (17)
| Token | 符号 | Token | 符号 |
|---|---|---|---|
| `PLUS` | `+` | `ARROW` | `->` |
| `MINUS` | `-` | `ASSIGN` | `=` |
| `STAR` | `*` | `EQUAL_EQUAL` | `==` |
| `SLASH` | `/` | `BANG_EQUAL` | `!=` |
| `PERCENT` | `%` | `GREATER` | `>` |
| `BANG` | `!` | `GREATER_EQUAL` | `>=` |
| `LESS` | `<` | `LESS_EQUAL` | `<=` |
| `AND` | `&&` | `OR` | `\|\|` |

### 关键字 (90: 45 英文 + 45 中文)
见 [[overview/language-spec#关键字一览-89|关键字参考]]。Helen 支持中英双语关键字，中文关键字与英文关键字映射到相同 TokenType，解析器和解释器无需任何改动。注意：`true`/`false` 同时是字面量和关键字。

---

## 扫描策略

### Maximal Munch（最长匹配）

当多个 Token 规则可能匹配时，选择**最长的匹配**：

```helen
"abc"    // → STRING_DQ (不是 IDENTIFIER)
"""abc""" // → TRIPLE_DOUBLE (不是三个 STRING_DQ)
=>       // → ARROW (不是 = 后跟 >)
```

### 连字符关键字消歧

Helen 的 `IDENTIFIER` 允许连字符 `-`，但部分关键字本身包含连字符：

```python
# 扫描器优先级：关键字先于标识符匹配
"sub-agents"  # → SUB_AGENTS (不是 sub - agents)
"max-turns"   # → MAX_TURNS
"my-agent"    # → IDENTIFIER (不是关键字)
```

实现策略：在扫描关键字时使用精确匹配表，`sub-agents` 和 `max-turns` 优先于 `IDENTIFIER` 规则。

### 字符串处理

| 类型 | 开始 | 结束 | 转义支持 |
|---|---|---|---|
| 双引号 | `"` | `"` | `\n`, `\t`, `\\`, `\"` |
| 单引号 | `'` | `'` | 同上 |
| 三引号 | `"""` | `"""` | 支持多行 |

### 注释

```helen
// 行注释：到行尾结束

/*
   块注释：跨多行，可嵌套（简化版不嵌套）
*/
```

### SourceSpan 全链路

每个 Token 携带 `SourceSpan`：

```python
@dataclass(frozen=True)
class SourceSpan:
    filename: str   # 文件名
    start_line: int # 起始行 (1-based)
    start_col: int  # 起始列 (1-based)
    end_line: int   # 结束行
    end_col: int    # 结束列
```

贯穿全链路：Lexer → Parser → AST → SemanticAnalyzer → ErrorFormatter

---

## 错误处理

| ErrorCode | 触发条件 | 示例 |
|---|---|---|
| `SCANNER_ERROR` (E0300) | 非法字符 | `@#$%` |
| `UNTERMINATED_STRING` (E0306) | 字符串未闭合 | `"hello` |
| `INVALID_ESCAPE` (E0305) | 无效转义 | `"\q"` |
| `INVALID_IDENTIFIER` (E0307) | 标识符以数字开头 | `1abc` |

---

## Scanner 类 API

```python
class Scanner:
    def __init__(self, source: str, filename: str = "<unknown>")
    def scan_all(self) -> list[Token]     # 扫描全部 → Token 列表
    def scan_token(self) -> Token          # 扫描单个 Token
    def _scan_number(self) -> Token        # 内部：扫描数字
    def _scan_string(self, quote: str)     # 内部：扫描字符串
    def _scan_triple_string(self)          # 内部：扫描三引号
    def _add_token(self, type, literal=None) # 内部：添加 Token
```

---

## 测试覆盖

- ✅ 基本 Token 识别（关键字、标识符、数字）
- ✅ 字符串（双引号/单引号/三引号）
- ✅ 运算符和分隔符
- ✅ 注释（行/块）
- ✅ 连字符关键字消歧
- ✅ 错误处理（未终止字符串、非法字符）
- ✅ SourceSpan 精度
