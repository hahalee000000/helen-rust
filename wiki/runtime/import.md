# 模块系统 (Import)

> 模块 M8 | `helen/runtime/import_resolver.py` | 测试: `tests/runtime/test_import_resolver.py`

---

## 概述

Helen 支持从外部文件导入 Agent、函数和数据。

```helen
import "./utils.helen"
import "./config.json" as cfg
import "./prompt.md"
```

---

## 多格式支持

| 扩展名 | 加载方式 | 返回内容 |
|---|---|---|
| `.helen` | 递归解析 | 注册 Agent/Function，不执行 main |
| `.json` | `json.loads()` | dict / list |
| `.yaml` / `.yml` | `yaml.safe_load()` | dict / list |
| `.md` / `.txt` | 纯文本读取 | str |

---

## 安全机制

### 路径安全检查

```python
def _is_safe_path(self, base_dir: str, resolved: str) -> bool:
    abs_base = os.path.abspath(base_dir)
    abs_resolved = os.path.abspath(resolved)
    return abs_resolved.startswith(abs_base + os.sep) or abs_resolved == abs_base
```

**拦截 `../` 遍历**：

```helen
import "../../secrets.helen"  # ❌ 路径越界
import "./utils.helen"        # ✅ 在当前目录或子目录
```

### 循环导入检测

```python
def resolve(self, path: str, from_file: str) -> ImportResult:
    resolved = self._resolve_path(path, from_file)
    if resolved in self._loaded:
        return None  # 循环导入，静默跳过
    self._loaded.add(resolved)
    # ... 加载文件
```

---

## Import 不执行 main

导入 `.helen` 文件时：

```python
def _parse_helen(self, path: str) -> ImportResult:
    tokens = self._lexer.scan_all()
    program = self._parser.parse(tokens)
    # 只注册 Agent 和 Function，不执行 main 块
    for stmt in program.statements:
        if isinstance(stmt, AgentDeclNode):
            self.agents[stmt.name] = stmt
        elif isinstance(stmt, FunctionDeclNode):
            self.functions[stmt.name] = stmt
```

**安全保证**：被导入文件的 `main { ... }` 不会自动执行。

---

## 相对路径解析

```python
def _resolve_path(self, path: str, from_file: str) -> str:
    base_dir = os.path.dirname(os.path.abspath(from_file))
    return os.path.normpath(os.path.join(base_dir, path))
```

基于**导入者文件所在目录**解析相对路径。

---

## Interpreter 集成

```python
def visit_import_stmt(self, node):
    result = self.import_resolver.resolve(node.path, self._current_file)
    # 合并导入的 Agent 和函数到本地注册表
    for name, agent in self.import_resolver.agents.items():
        if name not in self._agents:
            self._agents[name] = agent
```

导入的 Agent 和函数可以在当前文件中通过 `call` 使用。

---

## v1.10 shared let 导入跟踪

### 概述

v1.10 添加了 `shared let` 关键字，用于声明跨 agent 可见的可变变量。导入的 `shared let` 被正确跟踪，可以在导入模块的 agent 中访问和修改。

### 导入行为

导入 `.helen` 文件时，`shared let` 变量会被跟踪并导入：

```python
def _parse_helen(self, path: str) -> ImportResult:
    tokens = self._lexer.scan_all()
    program = self._parser.parse(tokens)
    
    # 注册 Agent 和 Function
    for stmt in program.statements:
        if isinstance(stmt, AgentDeclNode):
            self.agents[stmt.name] = stmt
        elif isinstance(stmt, FunctionDeclNode):
            self.functions[stmt.name] = stmt
        elif isinstance(stmt, VarDeclNode) and stmt.is_shared:
            # v1.10: 跟踪 shared let
            self.shared_vars[stmt.name] = {
                'value': self._evaluate_const(stmt.value),
                'mutable': True,
                'source': path
            }
```

### 示例

**module_a.helen**:
```helen
shared let counter = 0
shared let config = {
  "debug": true,
  "max_retries": 3
}

agent Worker {
  main {
    counter += 1
    print("Counter: " + str(counter))
  }
}
```

**module_b.helen**:
```helen
import "./module_a.helen"

agent Manager {
  main {
    // 可以访问导入的 shared let
    counter += 10  // ✅ 合法
    config["debug"] = false  // ✅ 合法
    
    // 调用导入的 agent
    call Worker()
  }
}
```

### 作用域规则

导入的 `shared let` 遵循以下规则：

| 变量类型 | 在导入模块中可见？ | 可修改？ |
|---------|------------------|---------|
| 模块级 `let` | ❌ 不可见 | - |
| 模块级 `const` | ✅ 可见 | ❌ 只读 |
| `shared let` | ✅ 可见 | ✅ 可读写 |

### 模块函数作用域解析 (v1.10)

导入模块的函数在调用时，可以访问其**自身模块**的 `const` 和 `shared let`，无论是否使用别名导入。

**非别名导入**：函数和变量直接注入全局命名空间。

```helen
// module.helen
const MAX = 100
shared let counter = 0

fn inc() { counter = counter + 1 }
fn total(): int { return MAX + counter }

// main.helen
import "./module.helen"
main {
    inc()
    print(total())       // ✅ 101 — 函数可见模块的 const 和 shared let
    print(counter)       // ✅ 1 — shared let 直接可见
    print(MAX)           // ✅ 100 — const 直接可见
}
```

**别名导入**：通过模块对象访问。

```helen
// output.helen
const OUTPUT_NORMAL = 1
shared let _use_colors = true

fn _colorize(text: str): str {
    if _use_colors {
        return "[C]" + text + "[/C]"
    }
    return text
}

fn set_use_colors(enabled: bool) {
    _use_colors = enabled
}

// main.helen
import "./output.helen" as output
main {
    print(output.OUTPUT_NORMAL)          // ✅ 1 — const 通过别名访问
    print(output._use_colors)            // ✅ true — shared let 通过别名访问
    print(output._colorize("hello"))     // ✅ "[C]hello[/C]" — 函数可见模块变量
    output.set_use_colors(false)
    print(output._colorize("hello"))     // ✅ "hello" — shared let 已修改
}
```

**实现原理**：导入时为每个模块创建独立的模块级 `Environment`，其中定义了该模块的 `const` 和 `shared let`。调用模块函数时，该 Environment 作为父作用域传入，确保函数能看到模块级变量。

### 循环导入处理

如果存在循环导入，`shared let` 的处理与普通变量一致：

```python
def resolve(self, path: str, from_file: str) -> ImportResult:
    resolved = self._resolve_path(path, from_file)
    if resolved in self._loaded:
        return None  # 循环导入，跳过
    self._loaded.add(resolved)
    # ... 加载并跟踪 shared let
```

### 错误处理

导入不存在的 `shared let` 会报错：

```helen
import "./module_a.helen"

agent MyAgent {
  main {
    let x = nonexistent_var  // ❌ E0333 UNDECLARED_VARIABLE
  }
}
```

---

## 缓存机制（开发者必读）

### 内存级缓存

`ImportResolver` 使用**内存级缓存**（非磁盘缓存）来加速重复导入：

```python
class ImportResolver:
    def __init__(self):
        self._cached_results: dict[str, ImportResult] = {}
        # ...
```

**缓存行为**：

| 场景 | 缓存命中？ | 说明 |
|------|----------|------|
| 同一进程内多次 import 同一文件 | ✅ 命中 | 返回缓存的 `ImportResult` |
| 进程重启后 import 同一文件 | ❌ 未命中 | 缓存已清空，重新从磁盘读取 |
| 修改 .helen 文件后在同一进程内 import | ❌ 未命中但... | 缓存不会自动失效！ |

### ⚠️ 开发时的重要陷阱

**问题**：在同一个 Python 进程内（如 REPL、Web 服务器、长时间运行的 agent），修改 .helen 文件后再次 import，**仍然会使用旧的缓存**！

```python
# 场景 1: Python REPL 或 Jupyter
from helen.interpreter import Interpreter
interp = Interpreter()
interp.execute_file("agent.helen")  # 加载 v1

# 修改 agent.helen（添加新功能）

interp.execute_file("agent.helen")  # ❌ 仍然是 v1！缓存未失效
```

```python
# 场景 2: Web 服务器（Flask/FastAPI）
@app.post("/run")
def run_agent():
    interp = Interpreter()  # 每次请求创建新实例
    return interp.execute_file("agent.helen")  # ✅ 每次都重新加载
```

### 解决方案

#### 方案 1: 每次创建新的 Interpreter 实例（推荐）

```python
def execute_helen(file_path: str):
    # 每次创建新的 ImportResolver，缓存自动清空
    interp = Interpreter()
    return interp.execute_file(file_path)
```

#### 方案 2: 手动清除缓存

```python
interp = Interpreter()

# 执行一次
interp.execute_file("agent.helen")

# 修改文件后，手动清除缓存
interp.import_resolver._cached_results.clear()
interp.import_resolver._loaded.clear()

# 重新执行
interp.execute_file("agent.helen")  # ✅ 使用新代码
```

#### 方案 3: 使用文件修改时间（mtime）检查

```python
import os

class SmartInterpreter:
    def __init__(self):
        self.interp = Interpreter()
        self._file_mtimes = {}
    
    def execute_if_changed(self, file_path: str):
        current_mtime = os.path.getmtime(file_path)
        cached_mtime = self._file_mtimes.get(file_path)
        
        if cached_mtime is None or current_mtime > cached_mtime:
            # 文件已修改，清除缓存并重新加载
            self.interp.import_resolver._cached_results.clear()
            self._file_mtimes[file_path] = current_mtime
            return self.interp.execute_file(file_path)
        else:
            # 文件未修改，使用缓存
            return self.interp.execute_file(file_path)
```

### Helen vs Python 缓存对比

| 特性 | Helen ImportResolver | Python `__pycache__` |
|------|---------------------|---------------------|
| 缓存位置 | 内存（进程内） | 磁盘（`.pyc` 文件） |
| 跨进程持久化 | ❌ 否 | ✅ 是 |
| 进程重启后 | 自动清空 | 保留（除非删除） |
| 文件修改后 | 需手动清除或重启进程 | 自动失效（基于 mtime） |
| 性能影响 | 微小（内存查找） | 首次加载稍慢 |

### 最佳实践

1. **开发环境**：
   - 使用 `helen` CLI 命令（每次都是新进程，自动重新加载）
   - 或在 Web 服务器中每次请求创建新的 `Interpreter` 实例

2. **生产环境**：
   - 长时间运行的服务应实现 mtime 检查或提供热重载 API
   - 考虑添加 `--no-cache` 选项或 `clear_cache()` API

3. **调试技巧**：
   ```python
   # 检查缓存状态
   print(f"Cached files: {len(interp.import_resolver._cached_results)}")
   print(f"Loaded files: {interp.import_resolver._loaded}")
   
   # 强制清除所有缓存
   interp.import_resolver._cached_results.clear()
   interp.import_resolver._loaded.clear()
   ```

---

## 标准库模块化导入 (v1.34+, v1.38 扩展)

Helen v1.34 引入标准库的模块化导入,v1.38 扩展到 **22 个模块**覆盖所有标准库类别。

### 三种导入形式

```helen
// 1. 通配符导入(最常用)
import std.core.*
import std.str.*
import std.list.*

// 2. 选择性导入
import std.data.{json_parse, yaml_parse}
import std.path.{path_join, path_exists}

// 3. 命名空间导入(避免冲突)
import std.dict as Dict
import std.list as List

main {
    let keys = Dict.keys({"a": 1, "b": 2})
    let sorted = List.sort([3, 1, 2])
}
```

### 22 个标准库模块

| 模块 | 内容 | 函数数 |
|------|------|:------:|
| `std.core` | `len`, `str`, `int`, `print`, `abs`, `min`, `max`, `range`, `type` | 17 |
| `std.str` | `upper`, `lower`, `split`, `join`, `replace`, `regex_match`, `base64_encode` | 43 |
| `std.list` | `map`, `filter`, `reduce`, `sort`, `unique`, `flatten`, `chunk`, `zip` | 11 |
| `std.dict` | `keys`, `values`, `entries`, `get`, `set_key`, `has_key`, `merge`, `pick` | 10 |
| `std.math` | `round`, `floor`, `ceil`, `sum`, `pow`, `sqrt`, `log`, `sin`, `cos` | 27 |
| `std.time` | `now`, `date`, `date_format`, `date_parse`, `date_add`, `sleep` | 16 |
| `std.file` | `read_file`, `write_file`, `append_file`, `delete_file`, `list_dir` | 12 |
| `std.io` | `progress_bar`, `stream_print`, `stream_clear`, `mkdir` | 9 |
| `std.system` | `env_get`, `env_set`, `shell_exec`, `platform`, `cpu_count`, `pid` | 24 |
| `std.path` | `path_basename`, `path_dirname`, `path_exists`, `path_join` | 6 |
| `std.data` | `json_parse`, `json_stringify`, `yaml_parse`, `toml_parse`, `csv_parse` | 28 |
| `std.network` | `http_get`, `http_post`, `http_put`, `http_delete`, `http_download` | 9 |
| `std.tools` | `shell_exec`, `calculate`, `patch_file`, `load_skill`, `web_search` | 7 |
| `std.debug` | `debug`, `trace_on`, `trace_off`, `coverage_on`, `coverage_report` | 11 |
| `std.context` | `clear_context`, `compress_context`, `context_stats`, `working_memory_set` | 29 |
| `std.transcript` | `get_session_id`, `list_sessions`, `replay_transcript`, `resume_session` | 21 |
| `std.media` | `media`, `media_base64`, `is_media`, `to_openai_parts`, `save_media` | 12 |
| `std.test` | `test_suite`, `test_case`, `assert_equal`, `assert_true`, `expect` | 23 |
| `std.quality` | `analyze_code`, `check_security`, `quality_score`, `quality_report` | 4 |
| `std.llm` | `cancel_llm_call`, `current_llm_call_id`, `cancel_all_llm_calls` | 3 |
| `std.crypto` | `md5`, `sha256`, `hmac_sha256`, `random`, `randint`, `uuid_generate` | 17 |
| `std.concurrency` | `mailbox_select` | 1 |

注:`std.tools`、`std.transcript`、`std.llm` 使用关键字作为模块名。parser 在 `std.` 后接受这些关键字 token。

### 推荐用法

v1.38 起,**显式导入是推荐风格**:

```helen
// 推荐:显式声明依赖
import std.core.*
import std.str.*
import std.path.*

main {
    let content = read_file("data.txt")  // from std.core
    let name = upper("helen")            // from std.str
    let exists = path_exists("data.txt") // from std.path
}

// 也允许:不导入直接使用(向后兼容,但不推荐)
main {
    print(len("hello"))  // 全局 stdlib 仍可用
}
```

### 设计原则

- **显式优于隐式**:代码依赖一目了然
- **按类别分组**:每个标准库类别对应一个模块
- **避免命名冲突**:命名空间导入 (`import std.dict as Dict`) 可隔离同名函数
- **向后兼容**:全局 stdlib 仍可用,显式导入是可选的最佳实践

---

**最后更新**: 2026-08-06
**版本**: v1.38
