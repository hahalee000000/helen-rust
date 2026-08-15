# Working Memory

> v1.25: System prompt-based working memory — the LLM maintains working memory
> via a `<working_memory>` block in its response, parsed by the runtime.

## 概述

Working Memory 是 Helen 的三通道上下文之一（system 15% / working memory 50% / history 35%），
用于让 LLM 在跨 invocation、跨重启的持续对话中保持上下文连续性。

Helen v1.25 采用 **system prompt-based** 方案：通过 system prompt 指导 LLM 在每次任务
结束时主动填写 working memory，runtime 解析后更新 working memory store。

## 工作机制

### 三步流程

```
┌─────────────────────────────────────────────────────────────┐
│ 1. System Prompt 注入指令                                   │
│    llm_mixin._build_working_memory_instructions()            │
│    → 告诉 LLM 在回复末尾包含 <working_memory> 块            │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ 2. LLM 回复包含结构化更新                                   │
│    <working_memory>                                         │
│    active_files: [src/auth.py, tests/test_auth.py]          │
│    decisions: [Use JWT tokens for cross-device support]     │
│    todos: [Add password strength validation]                │
│    errors: [Fixed token expiration - missing refresh logic] │
│    </working_memory>                                        │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ 3. Runtime 解析并更新                                        │
│    llm_mixin._extract_working_memory_update(response)        │
│    → agent_context.update_from_llm_summary(summary)         │
│    → working memory store 更新                              │
└─────────────────────────────────────────────────────────────┘
```

### `<working_memory>` 块格式

LLM 在回复末尾包含的结构化更新：

```
<working_memory>
active_files: [文件1, 文件2]
decisions: [决策1, 决策2]
todos: [待办1, 待办2]
errors: [错误1, 错误2]
</working_memory>
```

**字段说明**：

| 字段 | 更新策略 | 说明 |
|------|---------|------|
| `active_files` | 替换 | 当前活跃文件（反映当前状态，非历史） |
| `decisions` | 追加（保留最近 10 条） | 关键决策及原因（历史上下文） |
| `todos` | 替换 | 当前待办事项 |
| `errors` | 追加（保留最近 5 条） | 遇到的错误及解决方法 |

**规则**：
- 只有有意义的字段才需要包含
- 每项保持简洁（一行一项）
- 如无更新，省略整个 `<working_memory>` 块

## 与旧方案对比

### v1.15: 正则启发式（旧）

```
LLM 回复 → 正则匹配 "I'll use X" / "Let me try X" → 提取决策
         → 正则匹配 TODO/FIXME/[ ] → 提取待办
         → 正则匹配文件路径模式 → 提取活跃文件
```

**局限**：
- 只识别英文句式（`I'll`, `Let me`），中文 agent 完全失效
- 无语义理解，无法提取用户偏好、关键约定
- 只能匹配结构化模式（TODO/FIXME）

### v1.25: System Prompt-Based（新）

```
LLM 回复 → 解析 <working_memory> 块 → 更新 working memory store
```

**优势**：
- **语义理解**：LLM 自己决定什么重要，比正则更智能
- **多语言**：中文、英文、混合场景同样工作
- **零额外成本**：不需要额外的 LLM 调用（复用主调用的输出）
- **实现简单**：~100 行代码，无新基础设施

## 实现细节

### 关键文件

| 文件 | 方法 | 作用 |
|------|------|------|
| `helen/interpreter/llm_mixin.py` | `_build_working_memory_instructions()` | 构建 system prompt 指令 |
| `helen/interpreter/llm_mixin.py` | `_extract_working_memory_update(response)` | 从回复中解析 `<working_memory>` 块 |
| `helen/interpreter/llm_mixin.py` | `_apply_working_memory_update(response)` | 解析并应用更新的便捷封装 |
| `helen/interpreter/agent_context.py` | `update_from_llm_summary(summary)` | 将解析结果合并到 working memory store |

### 触发位置

在 `visit_llm_act_expr()` 中，system prompt 组装时注入指令：

```python
# 5. Working Memory Instructions (v1.25)
wm_instructions = self._build_working_memory_instructions()
if wm_instructions:
    system_prompt_parts.append(wm_instructions)
```

在 `_visit_llm_act_sync()` 和 `_visit_llm_act_streaming()` 中，回复返回前解析：

```python
# v1.25: Extract and apply working memory update from response
if response_text:
    self._apply_working_memory_update(response_text)
return response_text
```

### 字段更新策略

```python
def update_from_llm_summary(self, summary):
    wm = self.working_memory

    # active_files: 替换（当前状态）
    if "active_files" in summary:
        wm.active_files = summary["active_files"]

    # decisions: 追加（历史），保留最近 10 条
    if "decisions" in summary:
        wm.recent_decisions.extend(summary["decisions"])
        wm.recent_decisions = wm.recent_decisions[-10:]

    # todos: 替换（当前任务列表）
    if "todos" in summary:
        wm.pending_todos = summary["todos"]

    # errors: 追加（近期），保留最近 5 条
    if "errors" in summary:
        wm.error_history.extend(...)
        wm.error_history = wm.error_history[-5:]
```

## 配置

Working memory 默认启用，可通过 agent 配置或 stdlib 控制：

### Agent 配置

```helen
agent ChatBot(query: str) {
    description "持续对话助手"
    context {
        working-memory true              // 启用（默认）
        working-memory-tokens 5000       // token 预算
    }
    main {
        return llm act query
    }
}
```

### Stdlib 函数

| 函数 | 作用 |
|------|------|
| `working_memory_get(key?)` | 获取 working memory（全部或指定字段） |
| `working_memory_set(key, value)` | 手动设置字段 |
| `working_memory_remove(key, item?)` | 移除字段或字段中的项 |
| `working_memory_clear()` | 清空所有字段 |
| `set_working_memory_enabled(enabled)` | 运行时开关 |

**禁用 working memory**：

```helen
set_working_memory_enabled(false)
// 之后 llm act 不再注入指令，也不解析 <working_memory> 块
```

## 使用示例

### 持续对话助手

```helen
agent ChatBot(user_msg: str) {
    description "持续对话助手"
    context {
        working-memory true
    }
    main {
        return llm act user_msg
    }
}

main {
    // 第一次对话
    let r1 = ChatBot("我叫小王，帮我修复 issue #19")
    // LLM 回复中包含 <working_memory> 块，runtime 自动提取并存储
    // active_files: [src/auth.py], decisions: [使用 dataclasses.replace()], ...

    // 第二次对话 — working memory 已注入 context
    let r2 = ChatBot("继续上次的工作")
    // LLM 通过 working memory 知道用户是小王，任务是 issue #19
}
```

### 跨重启恢复

```bash
# 第一次运行
helen chat_assistant.helen
# working memory 存储在 transcript 中

# 重启后恢复
helen --resume-latest chat_assistant.helen
# working memory 从 transcript 加载，agent "记得" 之前的对话
```

## 设计原则

1. **LLM 是主动参与者**：不是被动等待正则匹配，而是 LLM 主动总结并填写
2. **零额外成本**：复用主 LLM 调用的输出，不需要单独的摘要调用
3. **优雅降级**：LLM 不包含 `<working_memory>` 块时，working memory 保持不变
4. **向后兼容**：现有 stdlib 函数（`working_memory_get/set`）继续工作
5. **可关闭**：通过 `working_memory_enabled` 标志控制

## 与 Claude Code 的对比

Claude Code 采用相同的方案：通过 system prompt 指导 LLM 维护上下文，LLM 在回复中
包含结构化的 working memory 更新，runtime 解析后存储。Helen v1.25 的实现与这一最佳
实践一致。

## 相关文档

- [[runtime/context-management|上下文管理架构]] — 三通道上下文、渐进压缩
- [[runtime/transcript-store|TranscriptStore SSOT]] — 消息持久化
- `reports/semantic-working-memory-proposal.md` — 设计方案（已废弃，被本方案替代）
- `tests/interpreter/test_working_memory_extraction.py` — 实现测试
