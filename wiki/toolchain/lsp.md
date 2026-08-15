<!-- helen-rust edition: LSP ported in M12 (crates/helen-lsp, JSON-RPC 2.0). -->

# 语言服务器协议 (LSP)

> 模块 M12 | `helen/lsp/server.py` | 测试: `tests/lsp/test_server.py`

---

## 概述

Helen LSP Server 实现 JSON-RPC 2.0 over stdio，为 IDE 提供：
- 实时诊断 (Diagnostics)
- 自动补全 (Completion)
- 跳转定义 (Go-to-Definition)

---

## 启动 LSP 服务器

### CLI 命令

```bash
$ helen lsp
```

启动 Helen Language Server，通过 stdin/stdout 进行 JSON-RPC 2.0 通信。

### 与 VS Code 集成

安装 [Helen VS Code 扩展](vscode.md) 后，LSP 服务器会自动启动。

### 手动测试

```bash
# 发送 initialize 请求
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}' | helen lsp
```

---

## 传输协议

```
Content-Length: <N>\r\n
\r\n
{JSON-RPC message}
```

### 客户端 → 服务器

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}
{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.helen","text":"..."}}}
{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"contentChanges":[{"text":"new content"}]}}
```

### 服务器 → 客户端

```json
{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"textDocumentSync":1,"completionProvider":{},"definitionProvider":{}}}}
{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":"file:///main.helen","diagnostics":[{"range":{...},"severity":1,"message":"..."}]}}
```

---

## 诊断 (Diagnostics)

```python
def _analyze(self, text: str) -> list[Diagnostic]:
    """执行 Lex→Parse→Analyze，返回 LSP Diagnostic 列表。"""
    tokens = self.lexer.scan_all()
    ast = self.parser.parse(tokens)
    self.analyzer.analyze(ast)

    diagnostics = []
    for error in self.analyzer.errors:
        diagnostics.append(Diagnostic(
            range=Range(
                start=Position(error.span.start_line - 1, error.span.start_col - 1),
                end=Position(error.span.end_line - 1, error.span.end_col - 1),
            ),
            severity=1,  # Error
            message=error.message,
            code=f"E{error.code.value:04d}",
        ))
    return diagnostics
```

**SourceSpan 转换**：
- SourceSpan 使用 1-based 行列号
- LSP Position 使用 0-based 行列号
- 转换：`Position(line - 1, col - 1)`

---

## 自动补全 (Completion)

```python
def _completion(self, position: Position) -> list[CompletionItem]:
    """返回关键字 + 类型 + stdlib builtins 补全。"""
    items = []

    # 关键字补全
    for kw in KEYWORDS:
        items.append(CompletionItem(label=kw, kind=14))  # Keyword

    # stdlib 补全
    for name in StdlibRegistry.names():
        items.append(CompletionItem(label=name, kind=3))  # Function

    return items
```

### 补全内容

| 类型 | 示例 |
|------|------|
| 关键字 | `agent`, `fn`, `let`, `const`, `if`, `else`, `match`, `case`... |
| 类型 | `string`, `int`, `float`, `bool`, `list`, `dict`, `any` |
| stdlib 函数 | `print`, `len`, `str_upper`, `str_lower`, `regex_match`... |

---

## 跳转定义 (Go-to-Definition)

```python
def _find_definition_at(self, text: str, position: Position) -> Location | None:
    """正则匹配 agent/fn/let 声明位置。"""
    patterns = [
        r'agent\s+(\w+)',
        r'fn\s+(\w+)',
        r'(?:let|const)\s+(\w+)',
    ]
    # 查找匹配位置的声明
    # 返回 Location(uri, Range)
```

支持跳转到：
- Agent 声明
- 函数声明
- 变量声明（let/const）

---

## Capabilities

```python
capabilities = {
    "textDocumentSync": 1,           # 增量同步
    "completionProvider": {},        # 自动补全
    "definitionProvider": {},        # 跳转定义
    "diagnosticProvider": {},        # 诊断
}
```

---

## 文档同步

| 方法 | 行为 |
|---|---|
| `textDocument/didOpen` | 存储文档，发布初始诊断 |
| `textDocument/didChange` | 更新文档，发布增量诊断 |
| `textDocument/didClose` | 清除文档，清除诊断 |

---

## 配置

### VS Code 设置

```json
{
  "helen.lsp.path": "helen",
  "helen.lsp.args": ["lsp"],
  "helen.lsp.enabled": true
}
```

| 设置 | 说明 | 默认值 |
|------|------|--------|
| `helen.lsp.path` | LSP 服务器可执行文件路径 | `"helen"` |
| `helen.lsp.args` | LSP 服务器参数 | `["lsp"]` |
| `helen.lsp.enabled` | 启用/禁用 LSP | `true` |

### 自定义路径

如果 Helen 安装在非标准位置：

```json
{
  "helen.lsp.path": "/home/user/helen/venv/bin/helen"
}
```
