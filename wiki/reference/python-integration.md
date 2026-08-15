# Helen ↔ Python 双向集成

> 一篇文档看清全貌：Helen 调用 Python / Python 调用 Helen / 混合使用

---

## 全景图

```
┌─────────────────────────────────────────────────────────────┐
│                    混合项目架构                              │
│                                                             │
│   ┌─────────────┐         ┌─────────────────┐              │
│   │ Helen 程序  │ ──FFI──▶│ Python 库/模块  │              │
│   │ (.helen)    │         │ (numpy, rich…)  │              │
│   └─────────────┘         └─────────────────┘              │
│          ▲                                                  │
│          │ Bridge                                           │
│          ▼                                                  │
│   ┌─────────────┐                                          │
│   │ Python 程序 │                                          │
│   │ (.py)       │                                          │
│   └─────────────┘                                          │
└─────────────────────────────────────────────────────────────┘
```

| 方向 | 名称 | 用途 | 详细文档 |
|------|------|------|----------|
| Helen → Python | **Python FFI** | 在 Helen 中使用 Python 库 | [[reference/09-python-ffi]] |
| Python → Helen | **Python Bridge** | 在 Python 中使用 Helen Agent | [[reference/15-python-bridge]] |

---

## Helen → Python（FFI）

### 一句话

用 `import` 导入 Python 模块，像调用 Helen 函数一样调用 Python。

### 核心语法

```helen
// 1. 导入 Python 模块（无扩展名 = Python 模块）
import "math" as math
import "json" as json
import "mylib.greeter" as PyGreeter

// 2. 调用 Python 函数
let s = json.dumps({"name": "Alice"})

// 3. 访问 Python 常量
let pi = math.pi

// 4. 实例化 Python 类
let encoder = json.JSONEncoder()
let result = encoder.encode({"x": 1})        // 自然方法调用
let result2 = encoder.call("encode", {"x": 1})  // 按方法名调用（动态场景）
```

### 类型自动转换

| Helen | Python | 方向 |
|-------|--------|------|
| `int/float/str/bool` | `int/float/str/bool` | 双向 |
| `list` | `list` | 双向 |
| `map` | `dict` | 双向 |
| `null` | `None` | 双向 |
| 复杂对象 | `WrappedPythonObject` | Python → Helen |

### 嵌套导入

Python 模块可以在被导入的 `.helen` 模块中使用，跨模块正常工作：

```helen
// bridge.helen
import "mylib.renderer" as PyRenderer
shared let _renderer = null
fn init() { _renderer = PyRenderer.Renderer() }

// main.helen
import "bridge.helen"
main { init() }    // ✅ PyRenderer 在 bridge 中可用
```

### 详细文档

→ [[reference/09-python-ffi|Python FFI 完整教程]]

---

## Python → Helen（Bridge）

### 一句话

在 Python 中 `import helen`，像使用普通 Python 类一样使用 Helen Agent。

### 核心语法

```python
from helen.bridge import HelenRuntime

# 1. 创建运行时（加载 Helen 程序）
runtime = HelenRuntime("my_agents.helen")

# 2. 调用 Helen 函数
result = runtime.call("my_function", arg1, arg2)

# 3. 调用 Helen Agent
agent = runtime.get_agent("Translator")
result = agent("Hello", target_language="Chinese")

# 4. 异步调用
result = await agent.async_call("Hello")
```

### 典型场景

```python
from helen.bridge import HelenRuntime

runtime = HelenRuntime("support_agent.helen")
support = runtime.get_agent("CustomerSupport")

# Python Web 应用中嵌入 Helen Agent
@app.post("/support")
async def handle_support(query: str):
    response = await support.async_call(query)
    return {"response": response}
```

### 会话管理 (v1.24+)

在 Python 服务中持久化对话，可以恢复历史 session：

```python
from helen.interpreter import Interpreter

# 恢复指定 session
interp = Interpreter(session_id="session_xxx")

# 恢复最近的 session
from helen.runtime.session_manager import SessionManager
manager = SessionManager()
sessions = manager.list_sessions()
if sessions:
    latest_sid = sessions[0]["session_id"]
    interp = Interpreter(session_id=latest_sid)

# 默认行为：创建新 session
interp = Interpreter()
```

**Web 服务中的典型用法**：

```python
from helen.bridge import HelenRuntime
from helen.interpreter import Interpreter

class ChatService:
    def __init__(self, user_id: str, session_id: str | None = None):
        self.user_id = user_id
        # 恢复之前的对话，或创建新对话
        self.interp = Interpreter(session_id=session_id)
        self.runtime = HelenRuntime("chat.helen", interpreter=self.interp)
        self.agent = self.runtime.get_agent("ChatBot")

    async def chat(self, message: str) -> str:
        return await self.agent.async_call(message)

# Web 路由
@app.post("/chat")
async def chat(user_id: str, message: str, session_id: str | None = None):
    service = ChatService(user_id, session_id)
    response = await service.chat(message)
    return {
        "response": response,
        "session_id": service.interp._agent_context.session_id  # 下次恢复用
    }
```

### 详细文档

→ [[reference/15-python-bridge|Python Bridge 完整教程]]

---

## 混合使用：同一项目中双向调用

最常见的项目架构：Python 主程序 + Helen Agent + Python 工具库。

### 示例架构

```
my_project/
├── main.py              # Python 入口（Web 服务/CLI）
├── agents.helen         # Helen Agent 定义
├── tools/
│   ├── data_loader.py   # Python 数据处理
│   └── formatter.py     # Python 格式化
└── requirements.txt
```

### agents.helen — 使用 Python 工具

```helen
import "tools.data_loader" as PyLoader
import "tools.formatter" as PyFormat

agent DataAnalyst(query: str) {
    description "数据分析助手"
    prompt "分析用户查询: {{query}}"

    functions {
        fn load_data(path: str): list {
            // 调用 Python 数据处理库
            return PyLoader.load_csv(path)
        }

        fn format_report(data: list): str {
            // 调用 Python 格式化库
            return PyFormat.to_markdown(data)
        }
    }

    main {
        let data = load_data("sales.csv")
        let report = format_report(data)
        return report
    }
}
```

### main.py — 使用 Helen Agent

```python
from helen.bridge import HelenRuntime

runtime = HelenRuntime("agents.helen")
analyst = runtime.get_agent("DataAnalyst")

# Python 主程序调用 Helen Agent
report = analyst("分析 Q3 销售趋势")
print(report)
```

### 数据流

```
main.py (Python)
  └─ analyst("查询")         → Helen Agent
       ├─ PyLoader.load_csv  → Python 工具库
       ├─ LLM 推理           → 大模型
       └─ PyFormat.to_md     → Python 工具库
  ◄─ 返回报告                 → Python 主程序
```

---

## 选择指南

| 你的场景 | 选择 |
|----------|------|
| Helen 程序需要 Python 生态（numpy/requests/rich 等） | FFI |
| Python 程序需要 Helen 的 LLM Agent 能力 | Bridge |
| 同时需要两者 | 混合（FFI + Bridge） |
| 纯 Helen 程序，无需与 Python 互操作 | 都不需要 |

---

## 相关文档

- [[reference/09-python-ffi|Python FFI 详细教程]]（Helen → Python）
- [[reference/15-python-bridge|Python Bridge 详细教程]]（Python → Helen）
- [[syntax/grammar|Helen 语法参考]]
- [[toolchain/stdlib|标准库参考]]
