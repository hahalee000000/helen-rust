---
name: debugging
description: "Debugging methodology and language-specific debugger tools. Covers systematic root-cause investigation, Python (pdb/debugpy), Node.js (--inspect/CDP), and Helen AI-native observability (debug/trace_on/:last_error/:llm_log) with cookbook workflows for Helen application development. v1.40 adds advanced AI-native debugging: structured error diagnostics, output contracts, transcript queries, LLM recording/replay, data lineage tracking, and interactive transcript replay."
version: 1.40.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [debugging, troubleshooting, root-cause, python, pdb, debugpy, nodejs, inspect, CDP, helen, observability, debug, trace, llm_log, error-diagnostics, output-contract, transcript-query, recording, replay, data-lineage]
---

# Debugging — Methodology & Tools

Umbrella skill for all debugging work. Covers the systematic investigation methodology plus language-specific debugger tools, including Helen's AI-native observability with a cookbook of 10 common debugging scenarios for Helen application development.

---

## 1. Systematic Debugging Methodology

**Iron Law: NO FIXES WITHOUT ROOT CAUSE INVESTIGATION FIRST.**

### Phase 1: Root Cause Investigation
1. **Read error messages carefully** — stack traces contain the solution
2. **Reproduce consistently** — `pytest tests/test_module.py::test_name -v`
3. **Check recent changes** — `git log --oneline -10`, `git diff`
4. **Gather evidence in multi-component systems** — instrument at each boundary
5. **Trace data flow** — where does the bad value originate?

### Phase 2: Pattern Analysis
- Find working examples in the same codebase
- Compare against references (read COMPLETELY, don't skim)
- Identify every difference, however small

### Phase 3: Hypothesis and Testing
- Form ONE hypothesis: "I think X is the root cause because Y"
- Make the SMALLEST possible change to test it
- One variable at a time

### Phase 4: Implementation
- Create failing test case FIRST
- Implement single fix addressing root cause
- Verify: `pytest tests/test_regression -v && pytest tests/ -q`
- **Rule of Three**: If 3+ fixes failed, STOP and question the architecture

### Red Flags — STOP and Return to Phase 1
- "Quick fix for now, investigate later"
- "Just try changing X and see if it works"
- "I don't fully understand but this might work"
- "One more fix attempt" (when already tried 2+)

---

## 2. Python Debugging (pdb + debugpy)

### Tool Selection

| Tool | When |
|------|------|
| `breakpoint()` + pdb | Local, interactive, simplest |
| `python -m pdb script.py` | Launch under pdb without source edits |
| `debugpy` | Remote/headless/attach to running process |

### pdb Quick Reference

| Command | Action |
|---------|--------|
| `n` | next line (step over) |
| `s` | step into |
| `r` | return from function |
| `c` | continue |
| `l` / `ll` | list source / full function |
| `w` | where (stack trace) |
| `u` / `d` | up / down in stack |
| `p expr` | print expression |
| `b file:line` | set breakpoint |
| `!stmt` | execute arbitrary Python |
| `interact` | full Python REPL in current scope |

### Recipe: Local breakpoint
```python
def compute(x, y):
    breakpoint()  # drops into pdb here
    return result
```
Remember to remove before committing: `rg -n 'breakpoint\(\)' --type py`

### Recipe: pytest debugging
```bash
pytest tests/foo.py::test_bar --pdb -p no:xdist  # xdist breaks pdb!
pytest tests/foo.py::test_bar --showlocals --tb=long  # without pdb
```

### Recipe: Remote debug with debugpy
```bash
# Launch with debugpy
python -m debugpy --listen 127.0.0.1:5678 --wait-for-client script.py

# Or attach to running process
python -m debugpy --listen 127.0.0.1:5678 --pid <PID>
```

### Recipe: remote-pdb (simplest for terminal agents)
```bash
pip install remote-pdb
```
```python
from remote_pdb import set_trace
set_trace(host="127.0.0.1", port=4444)  # blocks until connection
```
Then: `nc 127.0.0.1 4444` → get a (Pdb) prompt

### Common Pitfalls
- pdb under pytest-xdist silently does nothing → use `-p no:xdist`
- `breakpoint()` in CI/non-TTY hangs → never commit it
- `PYTHONBREAKPOINT=0` disables all breakpoints
- `debugpy.listen` doesn't block without `wait_for_client()`
- ptrace may fail on hardened kernels → `echo 0 > /proc/sys/kernel/yama/ptrace_scope`
- **Stale `.pyc` cache**: Code changes don't seem to take effect → `find . -name "*.pyc" -delete && find . -name "__pycache__" -type d -exec rm -rf {} +`. Always clear cache after modifying source if behavior seems unchanged.
- **`asyncio.get_running_loop()` is unreliable for "am I in a loop?" checks**: It raises `RuntimeError` when no loop is running, but in some contexts (mixed sync/async code, REPL environments, nested interpreters) the `except RuntimeError` clause may not catch it as expected — the exception propagates despite the try/except. **Use `asyncio.get_event_loop()` + `.is_running()` instead**, which is safer across all contexts:
  ```python
  # ❌ Unreliable — may not catch properly in all contexts
  try:
      asyncio.get_running_loop()
      in_event_loop = True
  except RuntimeError:
      in_event_loop = False
  
  # ✅ Reliable — works in REPL, scripts, tests, nested contexts
  in_event_loop = False
  try:
      _loop = asyncio.get_event_loop()
      if _loop.is_running():
          in_event_loop = True
  except Exception:
      in_event_loop = False
  ```
  **Symptom**: Code works in standalone scripts but fails in REPL/interactive environments with `RuntimeError: no running event loop` even though you have a try/except around `get_running_loop()`.

---

## 3. Node.js Debugging (--inspect + CDP)

### Tool Selection
- **`node inspect`** — built-in, zero install, CLI REPL. Best for quick poking.
- **CDP via `chrome-remote-interface`** — scriptable, automate many breakpoints.

### Launch with Inspector
```bash
node --inspect script.js           # listen on 127.0.0.1:9229
node --inspect-brk script.js       # listen AND pause on first line
node --inspect=0.0.0.0:9230 script.js  # custom port
```

### Attach to Running Process
```bash
kill -SIGUSR1 <pid>    # enables inspector on existing process
node inspect -p <pid>  # attach debugger CLI
```

### `node inspect` REPL Commands

| Command | Action |
|---------|--------|
| `c` / `cont` | continue |
| `n` / `next` | step over |
| `s` / `step` | step into |
| `o` / `out` | step out |
| `sb('file.js', 42)` | set breakpoint |
| `cb('file.js', 42)` | clear breakpoint |
| `bt` | backtrace |
| `repl` | drop into REPL in current scope |
| `watch('expr')` | evaluate on every pause |

### Programmatic CDP (automation)
```javascript
const CDP = require('chrome-remote-interface');
const client = await CDP({ port: 9229 });
const { Debugger, Runtime } = client;
// Set breakpoints, capture scope, evaluate expressions...
```

### Common Pitfalls
- Wrong line numbers in TS → break in built `dist/*.js` or enable sourcemaps
- `--inspect` vs `--inspect-brk` — the latter pauses on first line
- Port collisions → use `--inspect=0` (random port), check `/json/list`
- Child processes not inherited → use `NODE_OPTIONS='--inspect-brk'`
- Background kills: `Ctrl+C` out of inspect leaves target paused → `cont` first
- Always bind to `127.0.0.1` (security)

### Heap Snapshots & CPU Profiles
```javascript
// CPU profile
await client.Profiler.enable();
await client.Profiler.start();
await new Promise(r => setTimeout(r, 5000));
const { profile } = await client.Profiler.stop();
```

---

## 4. Helen AI-Native Observability

Helen provides **AI-native observability** instead of traditional interactive debuggers. AI agents need structured, machine-consumable context — not breakpoints and single-stepping.

### Core Concepts

| Traditional Debugger | Helen Observability |
|---------------------|---------------------|
| Breakpoints | `assert` statements |
| Single-step execution | Execution tracing (`trace_on/off`) |
| Variable watch | `debug()` structured output |
| Call stack panel | Programmatic call stack tracking |
| No LLM logging | LLM call audit log |

### assert Statement

```helen
import std.core.*
main {
    // Runtime assertion with optional message
    assert x > 0, "x must be positive"

    // Catchable — throws AssertionError
    try {
        assert false, "test"
    } catch AssertionError e {
        print("Caught: " + e.message)
    }
}
```

### debug() Function

```helen
import std.debug.*
main {
    // Structured debug output to stderr (JSON format)
    let x = 42
    debug("variable value", x)
    // Output: [DEBUG] variable value {"value": 42}
}
```

### Execution Tracing

```helen
import std.debug.*
main {
    // Programmatic control
    trace_on()
    let result = compute()
    trace_off()
    let trace = get_trace(10)
}
```

**REPL commands**:
```
:trace on          # Enable tracing
:trace off         # Disable tracing
:trace show [n]    # Show last n trace entries
:last_error        # Show structured error context (human-readable)
:last_error -v     # Verbose: includes execution trace
:llm_log [n]       # Show LLM call audit log (compact)
:llm_log [n] -v    # Verbose: shows all audit fields
```

> **Note**: Call stack and execution tracing are enabled by default in REPL — no need for `:trace on`.

### Error Snapshot Format (JSON)

```json
{
  "error": {"type": "RuntimeError", "message": "...", "location": "..."},
  "call_stack": [{"function": "...", "args": {...}}],
  "scope": {"var": "value"},
  "trace": [...],
  "timestamp": 1718812800.0
}
```

### LLM Audit Log

All `llm act` calls are automatically logged:
- timestamp, call_type, agent_name, model
- prompt, response, tokens_in/out, duration_ms
- tool_calls list (for stream mode)
- error (if any)

Compact mode shows one-line summary with model name and tool call count. Verbose mode (`-v`) shows all fields.

### Key Design Decisions

1. **Zero overhead when disabled**: Tracing is opt-in (but enabled by default in REPL)
2. **Ring buffers**: Trace (10000), LLM log (1000), call stack (100)
3. **JSON structured**: AI can parse directly
4. **Auto-capture**: Errors/assertions capture full context

---

## 5. Helen 应用开发调试工作流（Cookbook）

> **给 Helen 应用开发者的实战指南**：什么时候用什么工具，怎么写可观测的 Agent 代码。
>
> **核心心智模型**：`helen check` + `pytest` 是**质量门禁**，`debug()` / `trace_on` / `:last_error` / `:llm_log` 是**手术刀**。前者告诉你"坏没坏"，后者告诉你"坏在哪里、为什么"。

### 5.1 决策树：遇到问题先用哪个工具

```
Helen 程序出问题了吗？
│
├─ 没运行就跑不起来 → helen check <file.helen>
│     └─ 看错误位置，修复语法/语义错误
│
├─ 运行起来但结果错 → 用哪个工具取决于症状
│     │
│     ├─ 报错/异常 → REPL 中跑 → :last_error
│     │     └─ 看 error/call_stack/scope
│     │           ├─ scope 里变量值不对 → 在赋值前加 debug()
│     │           └─ call_stack 太深 → 看哪个函数出问题
│     │
│     ├─ 没报错但 LLM 行为奇怪 → :llm_log -v
│     │     └─ 看 prompt/response/tokens/duration
│     │           ├─ prompt 不对 → 检查 prompt 模板
│     │           ├─ response 被截断 → 看 max_tokens/timeout
│     │           └─ tool_calls 异常 → 看 tools 注册
│     │
│     ├─ 流程看不懂 → 在可疑块外 trace_on()/trace_off()
│     │     └─ get_trace(50) 看执行轨迹
│     │
│     └─ 变量值不符合预期 → 在关键点加 debug()
│           └─ debug("label", {"x": x, "state": state})
│
└─ 性能问题 → context_usage() + context_stats()
      └─ 看 token 占用和压缩情况
```

### 5.2 在 Agent 代码中布局可观测性

**❌ 无可观测性的 Agent**（出问题时无从下手）：

```helen
import std.tools.*
agent Researcher(topic: str) {
    main {
        let plan = llm act "Plan research on " + topic
        let results = web_search(plan)
        let report = llm act "Write report from " + results
        return report
    }
}
```

**✅ 带可观测性的 Agent**（出问题时有迹可循）：

```helen
import std.core.*
import std.debug.*
import std.tools.*
agent Researcher(topic: str) {
    main {
        debug("Researcher 启动", {"topic": topic})
        
        let plan = llm act "Plan research on " + topic
        debug("计划阶段完成", {"plan_length": len(plan)})
        
        trace_on()
        let results = web_search(plan)
        trace_off()
        debug("搜索完成", {"results_count": len(results)})
        
        assert len(results) > 0, "搜索没有返回结果"
        
        let report = llm act "Write report from " + results
        debug("报告生成完成", {"report_length": len(report)})
        
        return report
    }
}
```

**布局原则**：

| 位置 | 用什么 | 目的 |
|------|--------|------|
| Agent 入口 | `debug("agent-name", {"arg": arg})` | 记录输入参数 |
| 每次 `llm act` 后 | `debug("llm 结果", {"len": len(result)})` | 追踪 LLM 行为 |
| 工具调用前后 | `trace_on()` / `trace_off()` | 追踪工具执行流程 |
| 分支/循环入口 | `debug("branch", {"i": i})` | 追踪控制流 |
| 关键断言 | `assert cond, "msg"` | 提前捕获错误 |
| Agent 出口 | `debug("agent-name 完成", {...})` | 记录输出 |

### 5.3 常见调试场景 Cookbook（10 例）

#### 场景 1：Agent 给出错误答案

**症状**：用户问 A，Agent 回答 B。

```bash
# 在 REPL 中运行同一程序
helen repl
> :llm_log -v
```

看 LLM 实际收到的 prompt 和返回的 response，定位是 prompt 问题还是模型问题。

#### 场景 2：工具调用死循环

**症状**：Agent 反复调用同一个工具不前进。

```helen
import std.core.*
import std.debug.*
main {
    debug("tool loop iter", {"i": i, "history_len": len(history)})
    llm act "continue task"
}
```

看每次迭代的 `history_len` 是否增长。如果不增长，说明 LLM 没把历史带进来。

#### 场景 3：上下文被意外压缩

**症状**：Agent 突然"忘记"之前的对话。

```helen
import std.context.*
import std.debug.*
main {
    let stats = context_stats()
    debug("上下文状态", {
        "usage_ratio": stats["usage_ratio"],
        "compressed_count": stats["compressed_count"]
    })
    if stats["usage_ratio"] > 0.8 {
        debug("⚠️ 上下文快满了", {})
    }
}
```

或者直接 `pin_message(uuid)` 钉住关键消息。

#### 场景 4：spawn 后子 agent 行为异常

**症状**：主 Agent 正常，spawn 的子 Agent 出错。

```helen
import std.core.*
import std.debug.*
agent Worker(task: str) {
    main {
        debug("Worker 启动", {"task": task, "spawned_from": "MainAgent"})
        // ... 子 Agent 逻辑
        debug("Worker 完成", {})
    }
}

main {
    let ch = spawn Worker("do task")
    debug("spawn 返回 channel", {"channel": str(ch)})
}
```

子 Agent 入口的 debug 能告诉你它收到了什么参数。

#### 场景 5：闭包捕获到意外的值

**症状**：闭包里的变量值和预期不一样。

```helen
import std.debug.*
main {
    let callbacks = []
    for i in range(5) {
        callbacks.append(fn() {
            debug("闭包执行", {"i": i})   // 看捕获到的 i 是什么
            return i * 2
        })
    }
}
```

Helen 的闭包是**值捕获**（深拷贝），所以 i 应该都是不同值。如果都是同一个值，就是 bug。

#### 场景 6：LLM 流式输出中断

**症状**：`llm act ... on_chunk fn(c) { print(c) }` 流式输出中途停了。

```helen
import std.core.*
import std.debug.*
main {
    let chunks = []
    llm act "long response" on_chunk fn(c: str) {
        chunks.append(c)
        debug("chunk 收到", {"len": len(c), "total": len(chunks)})
        return true   // 注意：返回 false 会停止流式
    }
    debug("流式结束", {"total_chunks": len(chunks)})
}
```

看是 LLM 没继续返回，还是 callback 返回了 false 主动停止。

**v1.39.7 Web UI 停止按钮修复**：停止按钮现在在工具执行期间也能响应。cancel 检查点覆盖 LLM 流式输出、turn 间、工具 dispatch 前后、`on_tool_end` 回调。如果停止按钮仍然无反应，检查是否卡在单个长时间工具调用中（如 `shell_exec` timeout=120s）— cancel 在当前工具完成后才检测。

#### 场景 7：多 Agent 协作数据错乱

**症状**：Agent A 把数据发给 Agent B，B 收到的数据不对。

```helen
import std.debug.*
main {
    // 发送端
    let payload = {"key": "value"}
    debug("发送 payload", payload)
    channel.send(payload)

    // 接收端（在另一个 agent 里）
    let received = channel.receive()
    debug("收到 payload", received)
    assert received["key"] == "value", "数据错乱"
}
```

两端加 debug 对比，看数据在哪一步被篡改。

#### 场景 8：import 失败

**症状**：`import "other.helen"` 报错。

```helen
import std.core.*
import std.debug.*
import std.system.*
main {
    debug("当前工作目录", {"cwd": env_get("PWD")})
    try {
        import "other.helen"
    } catch e {
        debug("import 失败", {"error": str(e), "type": type(e)})
        throw e
    }
}
```

#### 场景 9：stdlib 函数返回值不符合预期

**症状**：`json_parse(text)` 解析失败。

```helen
import std.core.*
import std.data.*
import std.debug.*
main {
    let text = response_body
    debug("要解析的文本", {"text": text, "len": len(text)})
    assert text[0] == "{", "不是 JSON 对象"
    let parsed = json_parse(text)
    debug("解析结果", parsed)
}
```

#### 场景 10：性能分析——为什么这么慢

**症状**：Agent 响应时间长。

```helen
import std.debug.*
import std.time.*
main {
    let t0 = stopwatch_start()
    let r1 = llm act "step 1"
    debug("step 1 耗时", {"ms": stopwatch_elapsed(t0)})

    let t1 = stopwatch_start()
    let r2 = llm act "step 2"
    debug("step 2 耗时", {"ms": stopwatch_elapsed(t1)})

    // 或者看 :llm_log 的 duration_ms 字段
}
```

### 5.4 与 pytest 的协作

**什么时候用 pytest，什么时候用 Helen 自带工具？**

| 场景 | 用什么 | 理由 |
|------|--------|------|
| 回归测试（改动是否破坏旧功能） | `pytest` | 自动化、可重复、CI 友好 |
| 验证 stdlib 函数行为 | `pytest`（Python 单元测试） | Python 层可以直接 assert |
| 验证新 agent 行为 | `helen <agent.helen>` + `:llm_log` | 需要真实 LLM 调用链路 |
| 复现用户报告的 bug | REPL + `:last_error` | 需要交互式调试 |
| 追踪解释器执行流程 | `trace_on()` + `get_trace()` | 能看到 Python 单元测试看不到的 |
| LLM 集成测试 | `helen <file.helen>` + `debug()` | 验证真实 LLM 行为 |

**最佳实践**：先用 pytest 保证基本正确性，再用 Helen 自带工具验证 LLM 集成和运行时行为。

### 5.5 一个完整的带可观测性的 Agent 示例

```helen
// translator.helen — 带完整可观测性的翻译 Agent
import std.core.*
import std.debug.*
import std.str.*
agent Translator(text: str, target: str) {
    description "Translate text with observability"
    
    main {
        // 入口打桩：记录输入
        debug("Translator 启动", {
            "text_len": len(text),
            "text_preview": substring(text, 0, 50),
            "target": target
        })
        
        // 前置断言
        assert len(text) > 0, "text 不能为空"
        assert len(target) > 0, "target 不能为空"
        
        // LLM 调用 + 追踪
        trace_on()
        let prompt = "Translate to " + target + ":\n\n" + text
        let translated = llm act prompt
        trace_off()
        
        // 出口打桩：记录输出
        debug("Translator 完成", {
            "translated_len": len(translated),
            "translated_preview": substring(translated, 0, 50)
        })
        
        // 结果验证（可选）
        assert len(translated) > 0, "翻译结果为空"
        
        return translated
    }
}

// 使用方法：
//   helen translator.helen
//   如果出错，进 REPL 用 :last_error 看结构化错误
//   想看 LLM 调用，用 :llm_log -v
//   想看执行流程，看 debug 输出
```

---

## 6. Helen v1.40 AI-Native 高级调试工具集

> **v1.40 新增**：六个阶段的高级调试功能，让 AI 调试者能够：
> 1. 从结构化错误诊断中直接获取根因分析
> 2. 使用 output contract 确保 LLM 输出格式正确
> 3. 高效查询大型 transcript
> 4. 录制和重放 LLM 交互，实现确定性调试
> 5. 追踪跨 agent 的数据流动
> 6. 交互式回放 transcript 会话

### 6.1 结构化错误诊断（Structured Error Diagnostics）

**问题**：传统错误信息只告诉你"出错了"，不告诉你"为什么错"和"怎么修"。

**解决方案**：v1.40 为所有 11 种异常类型提供结构化诊断信息。

```helen
import std.debug.*
main {
    // 发生错误后，获取详细诊断信息
    let err = last_error_detail()
    if err != null {
        debug("错误类别", error_category(err))
        debug("修复建议", error_suggestion(err))
        debug("数据流", error_data_flow(err))
    }
}
```

**错误快照新增字段**：
- `diagnostic_category`: 语义分类（如 "LLMTimeout", "AgentCallFailed"）
- `suggestion`: 具体的修复建议（基于规则引擎生成，零 LLM 调用）
- `data_flow`: 数据流追踪信息（显示值从哪里来）

**REPL 命令**：
```
:last_error        # 现在包含 Suggestion 和 Data Flow 字段
```

**支持的异常类型**（11 种）：
- AnyError, LLMError, TimeoutError, ModelError
- PromptTooLongError, AgentError, LLMOutputContractError (v1.40)
- ToolError, RuntimeError, AssertionError, AggregateError

**RuntimeError 规则匹配**：
- 除零错误 → "在除法前检查分母是否为 0"
- 类型错误 → "检查函数返回值类型是否符合预期"
- 未定义变量 → "检查变量是否已声明，或作用域是否正确"
- 索引越界 → "检查数组/列表长度，确保索引在有效范围内"
- 键不存在 → "检查键名是否正确，或用 get() 方法提供默认值"

### 6.2 Output Contract 验证

**问题**：LLM 输出格式不符合预期，导致下游代码崩溃。

**解决方案**：在 agent 声明中定义 output contract，运行时自动验证。

```helen
agent Reviewer {
    model: "qwen3.7-plus"
    
    // 简单 contract：期望合法 JSON
    output_contract: "json"
    
    main {
        llm act "Review this code and return JSON"
    }
}

agent Validator {
    // 详细 schema contract
    output_contract: {
        type: "object",
        required: ["verdict", "confidence"],
        properties: {
            verdict: {type: "string", enum: ["pass", "fail"]},
            confidence: {type: "number", min: 0, max: 1}
        }
    }
    main {
        llm act "Validate this input"
    }
}
```

**验证失败时**：
- 抛出 `LLMOutputContractError`（继承自 `LLMError`）
- 错误信息包含具体的 violation 描述
- 可通过 `last_error_detail()` 获取诊断信息

**手动验证**：
```helen
import std.debug.*
main {
    let output = llm act "Return JSON"
    let result = validate_output(output, "json")
    if !result.valid {
        debug("验证失败", result.violation)
    }
}
```

**支持的 contract 类型**：
- `"json"`: 验证是否为合法 JSON
- `"text"`: 总是通过（用于明确标记）
- Schema dict: 验证类型、必需字段、属性约束
  - `type`: "string", "number", "integer", "boolean", "array", "object"
  - `required`: 必需字段列表
  - `properties`: 属性 schema（支持 type, enum, min/max, minLength/maxLength）

### 6.3 增量 Transcript 查询

**问题**：`replay_transcript()` 加载整个 transcript，大型会话会爆内存。

**解决方案**：`query_transcript()` 支持过滤和分页，高效查询大型 transcript。

```helen
import std.debug.*
main {
    // 查询当前会话的所有 assistant 消息
    let msgs = query_transcript(role="assistant")
    
    // 查询特定 agent 的消息
    let coder_msgs = query_transcript(agent="Coder")
    
    // 分页查询
    let page1 = query_transcript(limit=100, offset=0)
    let page2 = query_transcript(limit=100, offset=100)
    
    // 正则搜索
    let errors = query_transcript(content_regex="Error:")
    
    // 时间范围查询
    let recent = query_transcript(since=now() - 3600)  // 最近1小时
    
    // 组合查询
    let filtered = query_transcript(
        role="assistant",
        agent="Reviewer",
        content_regex="verdict",
        limit=50
    )
}
```

**查询参数**：
- `session_id`: 会话 ID（空=当前会话）
- `role`: 角色过滤（"user", "assistant", "tool"）
- `agent`: Agent 名称过滤
- `invocation_id`: 调用 ID 过滤
- `since`, `until`: 时间戳范围
- `content_regex`: 内容正则匹配
- `message_type`: 消息类型过滤
- `limit`, `offset`: 分页控制

**后端优化**：
- JSONL 后端：流式过滤 + 10 万条上限（防止 OOM）
- SQLite 后端：SQL WHERE 下推（O(log n) 查询）

### 6.4 LLM 录制/重放

**问题**：LLM 是非确定性的，同一个 bug 无法复现。

**解决方案**：录制 LLM 交互到 cassette 文件，后续可以确定性重放。

```helen
import std.debug.*
main {
    // 开始录制
    let result = record_session("debug/session.jsonl")
    // result: {"status": "recording", "cassette_path": "debug/session.jsonl"}
    
    // 运行 agent（所有 LLM 调用都会被录制）
    agent Reviewer {
        main {
            llm act "Review this code..."
        }
    }
    
    // 停止录制
    let result = stop_recording()
    // result: {"status": "stopped"}
}

// 后续可以重放
main {
    let result = replay_session("debug/session.jsonl")
    // result: {"status": "replaying", "entry_count": 5}
    
    // 现在所有 LLM 调用都使用录制的响应
    agent Reviewer {
        main {
            llm act "Review this code..."  // 返回录制的响应
        }
    }
}
```

**Cassette 文件格式**（JSONL）：
```json
{
  "type": "llm_call",
  "seq": 0,
  "timestamp": 1234567890.123,
  "agent_name": "Reviewer",
  "model": "qwen3.7-plus",
  "request": {"messages": [...], "tools": [...]},
  "response": {"content": "...", "tool_calls": [...]},
  "usage": {"prompt_tokens": 100, "completion_tokens": 50},
  "duration_ms": 1234.5
}
```

**使用场景**：
1. **Bug 复现**：录制出现 bug 的会话，重放确认问题
2. **Prompt 回归测试**：修改 prompt 后重放，对比行为变化
3. **CI 测试**：在 CI 中重放录制的会话，避免依赖真实 LLM
4. **性能分析**：分析录制的 duration_ms 和 token 使用

**注意事项**：
- 录制会捕获完整的 messages 数组和 response
- 重放时按顺序返回录制的响应
- Cassette 文件可以手动编辑（JSONL 格式）
- 性能开销：< 1ms per LLM call

### 6.5 跨 Agent 数据血缘追踪

**问题**：多 agent 系统中，数据在 agent 之间流动，出问题后难以追踪来源。

**解决方案**：使用 data lineage tracker 记录数据流动，支持查询起源和消费者。

```helen
import std.debug.*
main {
    // 手动记录数据流（自动追踪将在后续版本实现）
    record_data_flow(
        "msg_abc",           // 生产者 UUID
        "msg_xyz",           // 消费者 UUID
        "agent_call",        // 流类型
        {"arg": "input"}     // 元数据
    )
    
    // 查询数据起源
    let origins = trace_value_origin("msg_xyz")
    // origins: [
    //   {"producer_uuid": "msg_abc", "flow_type": "agent_call", ...}
    // ]
    
    // 查询数据消费者
    let consumers = trace_value_consumers("msg_abc")
    // consumers: [
    //   {"consumer_uuid": "msg_xyz", "flow_type": "agent_call", ...}
    // ]
    
    // 获取完整血缘图
    let lineage = get_data_lineage()
    // lineage: {
    //   "nodes": ["msg_abc", "msg_xyz", ...],
    //   "edges": [
    //     {"source": "msg_abc", "target": "msg_xyz", "flow_type": "agent_call", ...}
    //   ]
    // }
}
```

**数据存储**：
- 使用独立的 SQLite sidecar 文件（`<session_id>_lineage.db`）
- 与 transcript backend（JSONL/SQLite）解耦
- 支持 JOIN 查询和索引优化

**流类型**：
- `"channel"`: 通过 Channel 传递
- `"agent_call"`: 通过 agent 调用参数
- `"prompt"`: 通过 prompt 注入
- 自定义类型

**使用场景**：
1. **错误追踪**：找到错误值的原始来源
2. **依赖分析**：理解 agent 之间的数据依赖
3. **影响分析**：修改一个 agent 后，看哪些 agent 会受影响

**注意事项**：
- 数据血缘追踪是 opt-in，默认不启用
- 需要手动调用 `record_data_flow()` 记录
- 自动追踪（Channel send/receive）将在后续版本实现

### 6.6 Transcript 事后回放

**问题**：需要逐步查看 transcript 会话，理解 agent 交互过程。

**解决方案**：使用 `helen replay` CLI 命令进行交互式回放。

**CLI 使用**：
```bash
# 查看 session 摘要
$ helen replay abc123 --summary
Session: abc123
Total messages: 150
Roles: {'user': 50, 'assistant': 100}
Agents: {'Reviewer': 30, 'Coder': 70}

# 交互式回放
$ helen replay abc123
Transcript Replay - Session: abc123
Total messages: 150

Commands:
  n, next      - Next message
  p, prev      - Previous message
  j <n>        - Jump to message n
  f, first     - First message
  l, last      - Last message
  s <query>    - Search for query
  summary      - Show summary
  q, quit      - Exit replay mode

[0/150] user: Hello, please review this code...

replay> n
[1/150] [Reviewer] assistant: I'll review the code...

replay> s error
Found 3 matches at indices: [15, 42, 89]

replay> j 42
[42/150] [Reviewer] assistant: I found an error in line 10...
```

**Python API**：
```python
from helen.runtime.transcript_replay import TranscriptReplay

with TranscriptReplay("abc123") as replay:
    # 导航
    replay.next()
    replay.prev()
    replay.jump(42)
    replay.first()
    replay.last()
    
    # 搜索
    results = replay.search("error")
    
    # 获取摘要
    summary = replay.get_summary()
    
    # 获取当前消息
    msg = replay.current_message
    formatted = replay.format_message(msg)
```

**使用场景**：
1. **事后分析**：理解复杂的多 agent 交互
2. **教学演示**：逐步展示 agent 行为
3. **问题复现**：配合 recording/replay 使用

### 6.7 v1.40 调试工作流 Cookbook

#### 场景 1：LLM 输出格式错误

**症状**：Agent 返回的 JSON 解析失败。

```helen
import std.debug.*
agent Parser {
    output_contract: "json"  // 自动验证
    main {
        llm act "Return JSON"
    }
}

main {
    try {
        let result = Parser()
    } catch LLMOutputContractError e {
        let err = last_error_detail()
        debug("Contract 违反", error_suggestion(err))
        // 输出：LLM 返回纯文本而非 JSON。在 agent prompt 里显式要求 '返回严格的 JSON 格式'。
    }
}
```

#### 场景 2：非确定性 Bug 复现

**症状**：同一个输入，有时成功有时失败。

```helen
import std.debug.*
main {
    // 第一次运行：录制
    record_session("debug/bug_session.jsonl")
    let result = MyAgent()  // 这次失败了
    stop_recording()
    
    // 后续运行：重放
    replay_session("debug/bug_session.jsonl")
    let result = MyAgent()  // 确定性复现失败
}
```

#### 场景 3：多 Agent 数据流追踪

**症状**：Agent B 收到了错误的数据，不知道是哪里来的。

```helen
import std.debug.*
main {
    // 假设已经记录了数据流
    let origins = trace_value_origin("msg_error")
    for origin in origins {
        debug("错误数据来源", {
            "producer": origin.producer_uuid,
            "flow_type": origin.flow_type
        })
    }
    
    // 获取完整血缘图
    let lineage = get_data_lineage()
    debug("血缘图", {
        "nodes": len(lineage.nodes),
        "edges": len(lineage.edges)
    })
}
```

#### 场景 4：大型 Transcript 分析

**症状**：Session 有 10000+ 条消息，需要找到特定的 LLM 调用。

```helen
import std.debug.*
main {
    // 使用 query_transcript 高效查询
    let llm_calls = query_transcript(
        role="assistant",
        content_regex="verdict",
        limit=50
    )
    
    for msg in llm_calls {
        debug("找到 verdict", {
            "uuid": msg.uuid,
            "agent": msg.agent_name,
            "preview": substring(msg.content, 0, 100)
        })
    }
}
```

#### 场景 5：交互式 Transcript 回放

**症状**：需要逐步查看 150 条消息的交互过程。

```bash
$ helen replay session_abc
# 使用 n/p/j/s 命令导航
# 使用 summary 查看统计信息
# 使用 search 搜索关键字
```

### 6.8 v1.40 调试工具决策树

```
Helen 程序出问题了吗？
│
├─ 报错/异常 → :last_error（现在包含 Suggestion）
│     └─ 看 diagnostic_category + suggestion
│           ├─ LLMOutputContractError → 检查 output_contract 定义
│           ├─ LLMTimeout → 增加 timeout 或减小 prompt
│           └─ AgentCallFailed → 检查 agent 参数和内部逻辑
│
├─ LLM 输出格式不对 → output_contract（自动验证）
│     └─ 验证失败 → last_error_detail() 看具体 violation
│
├─ 非确定性 Bug → record_session() + replay_session()
│     └─ 录制 → 重放 → 确定性复现
│
├─ 多 Agent 数据流问题 → trace_value_origin() / trace_value_consumers()
│     └─ 追踪数据来源和消费者
│
├─ 需要分析大型 Transcript → query_transcript()
│     └─ 使用过滤和分页高效查询
│
└─ 需要逐步查看交互 → helen replay <session_id>
      └─ 交互式导航和搜索
```

---

## Quick Decision Guide

| Situation | Use |
|-----------|-----|
| Test fails, need to see intermediate state | `breakpoint()` (Python) or `--inspect-brk` (Node) |
| Long-running process misbehaving | `remote-pdb` (Python) or `kill -SIGUSR1` (Node) |
| Need to understand WHY something fails | Systematic debugging Phase 1-3 first |
| 3+ fixes failed | Question the architecture (Phase 4, step 5) |
| Need to automate many breakpoints | CDP driver (Node) or debugpy (Python) |
