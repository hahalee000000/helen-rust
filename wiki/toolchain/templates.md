# 内置模板库

Helen 提供一组内置模板，涵盖常见 agent 模式。每个模板都是**完整可运行的示例代码**，并附有详细注释。

## 使用方法

```bash
# 列出所有模板
helen template --list

# 查看模板内容
helen template <name>

# 复制模板到当前目录
helen template <name> --copy
helen template <name> --copy <output_file>
```

## 可用模板

| 模板 | 描述 | 用途 |
|------|------|------|
| `simple_agent` | 最简单的 agent 调用模式 | 单 agent 处理单任务 |
| `spawn_channel` | spawn + Channel 基础并发模式 | 后台执行，Channel 接收结果 |
| `spawn_with_transcript` | spawn + transcript 继承模式 | 子 agent 访问父对话历史 |
| `shared_store` | SharedStore 跨 agent 数据交换 | 多 agent 共享可变数据 |
| `context_object` | Context 对象聚合参数模式 | 多参数场景简化 |
| `pipeline` | Agent 管道模式（顺序处理） | 多步骤数据处理流水线 |

## 示例

### 1. 简单 Agent (`simple_agent`)

最基础的 agent 调用模式——通过参数显式传递上下文。

```bash
helen template simple_agent --copy my_agent.helen
```

关键点：
- 所有需要的信息都通过参数传入
- 模块级 `let` 在 agent main 中不可见
- 如需只读共享，用 `const`
- 如需跨 agent 共享，用 `shared let` 或 `shared store`

### 2. Spawn + Channel (`spawn_channel`)

后台执行 agent，通过 Channel 接收结果。

```bash
helen template spawn_channel --copy my_worker.helen
```

关键点：
- `spawn` 立即返回 Channel，不阻塞
- spawn 创建的 agent 运行在独立的 Interpreter 实例中
- 独立 transcript、独立 working memory、独立 session_id
- 所有上下文必须通过参数传递

### 3. Spawn + Transcript 继承 (`spawn_with_transcript`)

子 agent 需要访问父 agent 的对话历史时使用。

```bash
helen template spawn_with_transcript --copy my_inheritor.helen
```

关键点：
- spawn 默认不继承 transcript
- 如需继承：显式传递 `parent_sid` + `resume_session`
- 这是"调用者决定上下文"原则的体现

### 4. SharedStore 数据交换 (`shared_store`)

多个 agent 需要共享和修改同一份数据时使用。

```bash
helen template shared_store --copy my_collab.helen
```

关键点：
- `shared store` 是显式的跨 agent 共享机制
- 支持任意类型（dict、list、自定义类型）
- 线程安全（内部用 RLock 保护）
- 下划线前缀字段是私有的

### 5. Context 对象 (`context_object`)

agent 需要多个相关参数时，用 Context 对象简化。

```bash
helen template context_object --copy my_processor.helen
```

关键点：
- Context 对象聚合相关参数，减少参数列表长度
- 仍然是显式传递（不违反隔离原则）
- 适用于参数超过 3-4 个的场景
- Context 对象可序列化，便于跨 agent/跨进程传递

### 6. 管道模式 (`pipeline`)

多个 agent 顺序处理，前一个的输出是后一个的输入。

```bash
helen template pipeline --copy my_flow.helen
```

关键点：
- 每一步都是显式的，数据流清晰可见
- 每一步都可以独立测试和调试
- 可以插入条件分支和错误处理
- 如需并行，用 spawn + Channel

## 设计原则

所有模板都遵循 **"调用者决定上下文"** 原则：

1. **显式传递** — agent 的所有上下文都通过参数传入
2. **隔离优先** — agent 不自动继承外层变量
3. **数据流清晰** — 每一步的输入输出都可见

## 相关文档

- [[reference/05-agents]] — Agent 编程教程（包含模板库使用指南）
- **`helen-agent-collaboration` skill** — Agent 协作模式详解
- **`helen-programming-methodology` skill §5** — 上下文接力模式
- **`helen-agent-patterns` skill** — Agent 设计模式

---

**最后更新**: 2026-07-20
