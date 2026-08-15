# Helen 设计哲学

> "Prompt-first" — 用自然语言定义意图，用结构化代码约束行为。

---

## 为什么需要 Agent 编程语言？

传统编程语言（Python/Java/Go）的核心抽象是 **数据 + 算法**：

```python
def classify_email(text):
    # 硬编码规则
    if "urgent" in text.lower():
        return "priority"
    elif "meeting" in text.lower():
        return "calendar"
    return "general"
```

Agent 场景的核心抽象是 **意图 + 模型**：

```helen
agent EmailClassifier {
    description "Classify emails into categories"
    prompt """
    Analyze the email and classify into one of:
    - priority: Urgent, requires immediate action
    - calendar: Meeting or event scheduling
    - general: Everything else
    """
}

let result = call EmailClassifier(email_text)
```

**Helen 的核心洞察**：LLM 不是另一个函数调用——它是一个 **非确定性计算原语**，需要全新的语言级别支持。

---

## 设计原则

### 1. Prompt-first

Helen 程序中，Agent 的 `description` 和 `prompt` 是 **一等公民**，不是注释或字符串常量。编译器理解它们的语义：

- 在 System Prompt 中自动组装
- 在 LSP 中提供语义补全
- 在 `helen doc` 中自动提取文档

### 2. LLM 是语言原语

`llm act`、`llm if` 是 Helen 的 **关键字级语句**，不是库函数：

| 语句 | 语义 | 类比 |
|---|---|---|
| `llm act "desc"` | 让 LLM 自主执行任务 | `print` — 基础 I/O |
| `llm if "desc" { branch }` | LLM 分类路由 | `if/switch` — 分支控制 |

### 3. 确定性 + 非确定性混合

Helen 程序 = **确定性代码**（变量/循环/函数） + **非确定性代码**（LLM 调用）：

```helen
// 确定性：传统控制流
let thresholds = [0.3, 0.7]

// 非确定性：LLM 判断
llm if "Classify sentiment" {
    case "positive": let sentiment = "😊"
    case "negative": let sentiment = "😞"
    default: let sentiment = "😐"
}

// 确定性：使用 LLM 结果
if sentiment == "😊" {
    call HappyAgent()
}
```

### 4. 两层渐进式披露

Agent 的 prompt 采用 **Skill Index → 按需加载** 策略：

- **Tier 1**：System Prompt 中注入轻量级 Skill Index（名称+描述）
- **Tier 2**：Agent 运行时通过 `load_skill` 工具按需加载完整内容

减少 System Prompt 开销，同时保持完整上下文可访问性。

### 5. 三层架构

```
┌─────────────────────────────────────────┐
│           Toolchain (工具链)              │
│  CLI · LSP · VS Code · Stdlib · DocGen  │
├─────────────────────────────────────────┤
│           Runtime (运行时)               │
│  LLM · Prompt · Memory · History        │
├─────────────────────────────────────────┤
│           Core (核心编译器)              │
│  Lexer → Parser → AST → Semantic → Eval  │
└─────────────────────────────────────────┘
```

| 层 | 职责 | 模块 |
|---|---|---|
| **Core** | 词法分析 → 语法分析 → 语义分析 → AST | M1-M4, M9-M10 |
| **Runtime** | LLM 抽象接口、Prompt 构建、记忆管理 | M5-M8, M16-M17 |
| **Toolchain** | CLI、LSP、IDE、标准库、文档生成 | M11-M15 |

---

## 与其他 Agent 框架的比较

| 特性 | Helen | LangChain | AutoGen | CrewAI |
|---|---|---|---|---|
| **语言级别 LLM 语句** | ✅ 关键字 | ❌ 库函数 | ❌ 库函数 | ❌ 库函数 |
| **编译时类型检查** | ✅ | ❌ | ❌ | ❌ |
| **语法高亮/IDE** | ✅ LSP | ❌ | ❌ | ❌ |
| **Agent 声明式语法** | ✅ `agent {}` | ❌ 代码构造 | ❌ 代码构造 | ❌ 代码构造 |
| **沙箱隔离** | ✅ Environment | ⚠️ 部分 | ⚠️ 部分 | ⚠️ 部分 |
| **零 Python 依赖** | ✅ 自解释器 | ❌ 需 Python | ❌ 需 Python | ❌ 需 Python |

Helen 的定位：**不是另一个 Agent 框架，而是一个专门为 Agent 编程设计的语言**。
