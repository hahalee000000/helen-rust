<!-- helen-rust edition: crates/helen-runtime mcp (M9) — JSON-RPC over stdio. -->

# MCP (Model Context Protocol) 集成

**版本**: v1.33 (2026-08-05)  
**状态**: ✅ 已实现

## 概述

Helen 从 v1.33 开始支持 MCP（Model Context Protocol）客户端，允许 agent 动态发现和调用外部 MCP 服务器提供的工具。这使得 Helen agent 可以无缝集成外部工具生态（如 codebase-memory-mcp、自定义 MCP 服务器等），扩展能力边界。

## 核心特性

- **懒加载初始化**: 首次访问 MCP 工具时自动初始化，无配置则跳过
- **多服务器支持**: 可同时连接多个 MCP 服务器
- **错误隔离**: MCP 服务器失败不影响 Helen 核心功能
- **OpenAI 兼容**: MCP 工具 schema 自动转换为 OpenAI function calling 格式
- **透明集成**: MCP 工具与内置工具统一通过 `dispatch_tool()` 调用

## 架构

```
┌─────────────────────────────────────────┐
│  Helen Agent (llm act)                  │
│  ┌───────────────────────────────────┐  │
│  │ tools = ["read_file", "echo"]     │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│  tools.dispatch_tool(name, args)        │
│  ┌───────────────────────────────────┐  │
│  │ 1. 检查内置工具 (_tools)          │  │
│  │ 2. 检查 MCP 工具 (_mcp_registry)  │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│  MCPToolRegistry                        │
│  ┌───────────────────────────────────┐  │
│  │ MCPServerManager                  │  │
│  │ ┌─────────┐ ┌─────────┐          │  │
│  │ │ Server1 │ │ Server2 │ ...      │  │
│  │ └─────────┘ └─────────┘          │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│  MCPClient (JSON-RPC over stdio)        │
│  ┌───────────────────────────────────┐  │
│  │ subprocess.Popen(command + args)  │  │
│  │ stdin/stdout 通信                 │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## 配置

在项目根目录创建 `.mcp.json` 文件：

```json
{
  "mcpServers": {
    "codebase-memory": {
      "command": "npx",
      "args": ["-y", "@anthropic-ai/codebase-memory-mcp"],
      "tool_timeout_sec": 60
    },
    "custom-server": {
      "command": "python",
      "args": ["mcp_server.py"],
      "cwd": "./tools",
      "env_vars": {
        "API_KEY": "your-key"
      },
      "tool_timeout_sec": 120
    }
  }
}
```

### 配置字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `command` | string | ✅ | 启动服务器的命令 |
| `args` | string[] | ❌ | 命令参数 |
| `cwd` | string | ❌ | 工作目录（相对路径相对于配置文件） |
| `env_vars` | object | ❌ | 环境变量 |
| `tool_timeout_sec` | int | ❌ | 工具调用超时（默认 60 秒） |

## 使用示例

### 示例 1: Agent 使用 MCP 工具

```helen
agent CodeAnalyzer {
    description = "使用 codebase-memory 分析代码"
    
    // MCP 工具会自动发现并可用
    tools = ["search_code", "get_code_snippet", "trace_path"]
    
    main {
        let result = llm act "使用 search_code 查找所有包含 'authentication' 的函数"
        print(result)
    }
}
```

### 示例 2: 动态调用 MCP 工具

```helen
main {
    // MCP 工具与内置工具统一调用
    let result = llm act "使用 echo 工具（来自 MCP）测试连接"
    
    // 或直接通过 stdlib 调用（如果注册为 stdlib 函数）
    // let tools = list_mcp_tools()
    // print(tools)
}
```

### 示例 3: 多 MCP 服务器

```json
{
  "mcpServers": {
    "memory": {
      "command": "npx",
      "args": ["-y", "@anthropic-ai/codebase-memory-mcp"]
    },
    "search": {
      "command": "npx",
      "args": ["-y", "@anthropic-ai/search-mcp"]
    },
    "custom": {
      "command": "python",
      "args": ["./custom_mcp.py"]
    }
  }
}
```

所有服务器的工具会自动合并，可在 `tools = [...]` 中统一引用。

## 实现细节

### 模块结构

```
helen/runtime/mcp/
├── __init__.py          # 包导出
├── exceptions.py        # MCP 异常定义
├── config.py            # .mcp.json 配置加载
├── client.py            # MCP 客户端（JSON-RPC over stdio）
├── server_manager.py    # 多服务器管理
└── registry.py          # 工具注册和分发
```

### 核心类

#### MCPClient

与单个 MCP 服务器通信的客户端。

```python
from helen.runtime.mcp import MCPClient

client = MCPClient(
    name="test",
    command="python",
    args=["server.py"],
)

client.start()
tools = client.list_tools()
result = client.call_tool("echo", {"message": "Hello"})
client.shutdown()
```

#### MCPToolRegistry

集成 MCP 工具到 Helen 工具系统。

```python
from helen.runtime.mcp import MCPToolRegistry
from pathlib import Path

registry = MCPToolRegistry()
registry.initialize(Path(".mcp.json"))

# 获取工具 schema
tools = registry.get_tool_schemas()

# 分发工具调用
result = registry.dispatch("echo", {"message": "Hello"})

# 清理
registry.shutdown()
```

### 懒加载机制

MCP 在首次访问时自动初始化：

```python
# helen/runtime/tools.py

def _ensure_mcp_initialized() -> None:
    """确保 MCP 已初始化（懒加载）"""
    global _mcp_registry
    if _mcp_registry is None:
        config_path = Path.cwd() / ".mcp.json"
        if config_path.exists():
            initialize_mcp(config_path)

def get_mcp_tool_schemas() -> list[dict]:
    _ensure_mcp_initialized()
    if _mcp_registry is None:
        return []
    return _mcp_registry.get_tool_schemas()
```

### 工具分发优先级

1. **内置工具** (`_tools` 注册表) - 最高优先级
2. **Agent 函数** (`functions {}` 块) - 第二优先级
3. **MCP 工具** (`_mcp_registry`) - 第三优先级

```python
def dispatch_tool(name: str, args: dict) -> str:
    # 1. 检查内置工具
    tool = _tools.get(name)
    if tool is not None:
        return tool.handler(**args)
    
    # 2. 检查 MCP 工具
    return dispatch_mcp_tool(name, args)
```

## 错误处理

MCP 设计原则：**MCP 失败不应影响 Helen 核心功能**。

### 错误类型

| 错误 | 处理策略 |
|------|----------|
| 配置文件不存在 | 静默跳过，不初始化 MCP |
| JSON 解析失败 | 记录警告，返回空配置 |
| 服务器启动失败 | 记录错误，跳过该服务器 |
| 通信超时 | 返回错误 JSON 给 LLM |
| 工具执行失败 | 返回错误 JSON 给 LLM |
| 服务器崩溃 | 后台线程退出，pending 请求超时 |

### 错误示例

```python
# MCP 工具调用失败时返回错误 JSON
{
    "error": "MCP tool 'unknown_tool' failed: Unknown tool"
}
```

LLM 会收到错误信息并可以相应处理。

## 最佳实践

### 1. 合理设置超时

```json
{
  "mcpServers": {
    "slow-server": {
      "command": "python",
      "args": ["slow_server.py"],
      "tool_timeout_sec": 300  // 5 分钟
    }
  }
}
```

### 2. 工具名称冲突

如果多个 MCP 服务器提供同名工具，**第一个服务器的工具胜出**，并记录警告：

```
WARNING - MCP tool 'search' from server 'server2' conflicts with server 'server1' (first server wins)
```

### 3. 环境变量传递

```json
{
  "mcpServers": {
    "api-server": {
      "command": "python",
      "args": ["server.py"],
      "env_vars": {
        "API_KEY": "${API_KEY}",
        "DEBUG": "true"
      }
    }
  }
}
```

### 4. 调试 MCP 问题

启用详细日志：

```bash
export HELEN_LOG_LEVEL=DEBUG
helen agent
```

查看 MCP 相关日志：

```bash
tail -f ~/.helen/helen.log | grep -i mcp
```

## 限制和注意事项

### 性能考虑

- **启动延迟**: 每个 MCP 服务器需要启动子进程（约 100-500ms）
- **工具发现**: 需要向每个服务器发送 `tools/list` 请求
- **调用延迟**: 每次工具调用需要 JSON-RPC 往返（约 10-100ms）

### 资源管理

- **进程数**: 每个 MCP 服务器占用一个子进程
- **内存**: 每个服务器约 10-50MB（取决于实现）
- **清理**: 使用 `atexit` 确保退出时清理所有进程

### 兼容性

- **MCP 协议版本**: 2024-11-05
- **传输方式**: 仅支持 stdio（不支持 HTTP/SSE）
- **Python 版本**: 3.12+

## 测试

### 运行 MCP 测试

```bash
# 运行所有 MCP 测试
pytest tests/runtime/test_mcp_*.py -v

# 运行配置测试
pytest tests/runtime/test_mcp_config.py -v

# 运行集成测试
pytest tests/runtime/test_mcp_integration.py -v
```

### Mock MCP 服务器

使用 `tests/runtime/mock_mcp_server.py` 进行测试：

```python
import sys
from pathlib import Path
from helen.runtime.mcp import MCPClient

client = MCPClient(
    name="test",
    command=sys.executable,
    args=[str(Path("tests/runtime/mock_mcp_server.py"))],
)

client.start()
result = client.call_tool("echo", {"message": "Hello"})
print(result)  # {"output": "Echo: Hello"}
client.shutdown()
```

## 故障排除

### 问题 1: MCP 工具未被发现

**症状**: `tools = ["mcp_tool"]` 但 LLM 说工具不存在

**解决**:
1. 检查 `.mcp.json` 是否存在于项目根目录
2. 启用 DEBUG 日志查看 MCP 初始化过程
3. 确认 MCP 服务器正常启动（检查进程）
4. 手动运行 MCP 服务器测试 `tools/list` 响应

### 问题 2: MCP 工具调用超时

**症状**: 工具调用卡住，最终超时

**解决**:
1. 增加 `tool_timeout_sec` 配置
2. 检查 MCP 服务器是否响应（手动测试）
3. 查看 MCP 服务器日志（stderr 输出）
4. 确认工具参数正确

### 问题 3: MCP 服务器启动失败

**症状**: 日志显示 "Failed to start MCP server"

**解决**:
1. 确认 `command` 在 PATH 中可用
2. 检查 `args` 是否正确
3. 手动运行命令测试：`command args...`
4. 检查权限和依赖

## 未来计划

- [ ] 支持 HTTP/SSE 传输（除 stdio 外）
- [ ] MCP 服务器健康检查和自动重启
- [ ] 工具调用缓存和去重
- [ ] MCP 工具调用统计和监控
- [ ] 动态加载/卸载 MCP 服务器
- [ ] MCP 工具权限控制

## 参考资料

- [MCP 官方文档](https://modelcontextprotocol.io/)
- [MCP 协议规范](https://spec.modelcontextprotocol.io/)
- [Helen 工具系统](./toolchain/stdlib.md)
- [Agent 编程](../tutorial/05-agents.md)
