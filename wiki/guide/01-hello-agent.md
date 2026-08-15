# Chapter 1: Your First Agent

## What Is an Agent?

The **agent** is the most fundamental concept in Helen. Simply put:

> **Agent = Prompt + Tools + LLM**

When an agent receives a task, it uses the prompt you gave it to understand the task, employs the tools you equipped it with to gather information or perform actions, and ultimately leverages the LLM's capabilities to complete the task and return a result.

Think of an agent as a "digital assistant that can talk and use tools." You tell it what to do (the prompt), give it some tools (like reading files or searching the web), and it figures out how to accomplish the task on its own.

## The Simplest Agent

Let's start with the simplest possible agent:

```helen
import std.core.*

agent HelloAgent {               // Chinese keyword form: 智能体 HelloAgent {
    description "A friendly greeting agent"    // Chinese: 描述
    prompt "You are a friendly assistant."     // Chinese: 提示词

    main {                                     // Chinese: 主函
        return llm act "Please say hello and introduce yourself."  // Chinese: 返回 大模型 执行
    }
}

main {                                         // Chinese: 主函
    let result = HelloAgent()                  // Chinese: 定义 结果 = HelloAgent（）
    print(result)                              // Chinese: 打印（结果）
}
```

### Line-by-Line Breakdown

| Line | Code | Meaning |
|------|------|---------|
| 1 | `import std.core.*` | Import the core standard library (`print`, `len`, etc.) |
| 3 | `agent HelloAgent` | Declare an agent named `HelloAgent` |
| 4 | `description "..."` | A short description of the agent (optional, but recommended) |
| 5 | `prompt "..."` | The agent's system prompt — tells the LLM what role to play |
| 7 | `main { ... }` | The agent's entry point — execution starts here |
| 8 | `llm act "..."` | Ask the LLM to execute a task (Chinese form: `大模型 执行`) |
| 9 | `return ...` | Return the result (Chinese form: `返回`) |
| 13 | `HelloAgent()` | Call the agent, just like calling a function |
| 14 | `print(result)` | Print the result (Chinese form: `打印`) |

### Chinese-English Equivalence

The code above can also be written entirely with Chinese keywords:

```helen
import std.core.*

智能体 HelloAgent {
    描述 "A friendly greeting agent"
    提示词 "You are a friendly assistant."

    主函 {
        返回 大模型 执行 "Please say hello and introduce yourself."
    }
}

主函 {
    定义 result = HelloAgent（）
    打印（result）
}
```

Both versions produce identical results. Helen has **99 bilingual keywords** (48 English + 51 Chinese), and they map one-to-one — you can freely mix and match them. **In this book, we'll use either English or Chinese keywords depending on context, and you're free to choose as well.**

## Running Your Program

1. Save the code above as `hello.helen`
2. Run it in your terminal:

```bash
helen hello.helen
```

You should see the LLM's self-introduction in the output.

> **Tip**: You need to configure an LLM API before running. This is typically done in `~/.helen/config.yaml`. If you haven't configured it yet, refer to the "Environment Setup" appendix.

## Agents with Parameters

Agents can accept parameters, just like functions:

```helen
import std.core.*

// An agent that receives two parameters   (Chinese: // 智能体接收两个参数)
agent Translator(source_text: str, target_lang: str) {    // Chinese: 翻译官（原文：字符串，目标语言：字符串）
    description "Translation agent"                        // Chinese: 描述
    prompt "You are a professional translator. Translate the text into {{target_lang}}."  // Chinese: 提示词
    temperature 0.3                                        // Chinese: 温度

    main {
        return llm act "Please translate this text: " + source_text  // Chinese: 返回 大模型 执行
    }
}

main {
    let chinese_result = Translator("Hello, World!", "Chinese")  // Chinese: 定义 中文结果 = 翻译官（..., "中文"）
    print(chinese_result)

    let english_result = Translator("你好，世界！", "English")   // Chinese: 定义 英文结果 = 翻译官（..., "英文"）
    print(english_result)
}
```

### Parameter Syntax

```helen
agent AgentName(param1: Type, param2: Type) {    // Chinese: 智能体名（参数1：类型，参数2：类型）
    // ...
}
```

- Parameter name comes first, type second, separated by a colon
- Multiple parameters are separated by commas
- Available types include: `str` (string), `int` (integer), `float` (floating-point number), `bool` (boolean), `list` (list), `map` (map), etc.

### Template Variables `{{}}`

Notice the `{{target_lang}}` inside the prompt? That's a **template variable**. Helen automatically substitutes the agent's parameters into the prompt.

```helen
prompt "You are a professional translator. Translate the text into {{target_lang}}."
```

When `Translator("Hello", "Chinese")` is called, `{{target_lang}}` is replaced with `Chinese`, and the prompt the LLM sees becomes:

> You are a professional translator. Translate the text into Chinese.

Template variables are the bridge between "the world of code" and "the world of prompts." We'll cover them in detail in the next chapter.

## Agent Configuration Options

Agents offer many configurable options. Below is a complete example showcasing every configuration field:

```helen
agent FullyConfiguredAgent {             // Chinese: 全配置智能体
    // === Identity ===                   // Chinese: 身份
    description "Showcases all configuration options"      // Chinese: 描述
    prompt "You are a professional assistant."             // Chinese: 提示词

    // === Model & Behavior ===           // Chinese: 模型与行为
    model "qwen-plus"                    // Chinese: 模型 — which LLM to use
    temperature 0.7                      // Chinese: 温度 — creativity (0.0–2.0, higher = more creative)
    max-turns 10                         // Chinese: 最大轮次 — max rounds of tool calling
    max-tokens 4096                      // Chinese: 最大tokens — max output tokens

    // === Advanced Features ===          // Chinese: 高级功能
    streaming true                       // Chinese: 流式输出 — enable streaming (token-by-token)
    thinking-mode true                   // Chinese: 思考模式 — enable reasoning/thinking (v1.36)
    reasoning-effort "high"              // Chinese: 推理强度 — reasoning depth: low/medium/high/max (v1.36)
    provider "deepseek"                  // Chinese: 提供商 — explicit API provider (v1.36)
    transcript "persistent"              // Chinese: 记录 — persistence: none/memory/persistent (v1.29)

    // === Tools & Authorization ===      // Chinese: 工具与授权
    tools = ["read_file", "web_search"]  // Chinese: 工具 — list of available tools

    // === Functions Block ===            // Chinese: 函数区
    functions {                          // Chinese: 函数区
        fn helper_fn(): str {            // Chinese: 辅助函数
            return "I am a helper function"   // Chinese: 返回 "我是辅助函数"
        }
    }

    // === Entry Point ===                // Chinese: 入口
    main {                               // Chinese: 主函
        return llm act "Do something complex"   // Chinese: 返回 大模型 执行
    }
}
```

### Full Configuration Reference

| Setting | English Keyword | Chinese Keyword | Type | Description |
|---------|----------------|-----------------|------|-------------|
| Description | `description` | `描述` | string | Short description of the agent |
| Prompt | `prompt` | `提示词` | string | System prompt — defines role and behavior (does NOT support `=` syntax) |
| Model | `model` | `模型` | string | Which LLM to use |
| Temperature | `temperature` | `温度` | float | 0.0 = most deterministic, 2.0 = most creative |
| Max Turns | `max-turns` | `最大轮次` | int | Limits the number of tool-calling rounds |
| Max Tokens | `max-tokens` | `最大tokens` | int | Limits output length |
| Streaming | `streaming` | `流式输出` | bool | Stream results token by token |
| Thinking Mode | `thinking-mode` | `思考模式` | bool | Let the model think before answering (v1.36) |
| Reasoning Effort | `reasoning-effort` | `推理强度` | string | Reasoning depth: `low`/`medium`/`high`/`max` (v1.36) |
| Provider | `provider` | `提供商` | string | Explicit API provider (v1.36) |
| Transcript | `transcript` | `记录` | string | Conversation persistence: `none`/`memory`/`persistent` (v1.29) |
| Tools | `tools` | `工具` | list | Whitelist of tools the agent can use |
| Functions | `functions` | `函数区` | block | Define helper functions (must also be listed in `tools` to be callable by the LLM) |
| Context | `context` | `上下文` | block | Context management config (compression strategy, working memory, etc. — advanced) |
| Entry Point | `main` | `主函` | block | The agent's execution entry point (**required**) |

> **Syntax note**: All configuration fields support two syntaxes — with `=` and without `=` — with identical effect. The sole exception is `prompt` (`提示词`), which must be written as `prompt "..."` because it needs to process `{{variable}}` template substitution.

> **Beginner tip**: When you're just starting out, you only need `description`, `prompt`, and `main`. Everything else is optional — add them when you need them. `thinking-mode`, `reasoning-effort`, `provider`, `transcript`, and `context` are advanced features covered in later chapters.

## The Essence of an Agent

Let's wrap up with an analogy:

| Traditional Programming | Helen Agent |
|------------------------|-------------|
| Function definition | Agent declaration (`agent`) |
| Function parameters | Agent parameters |
| Function body (code logic) | Prompt + LLM (natural-language logic) |
| Library function calls | Tool calls |
| Function invocation | Agent invocation (exactly the same) |

The key difference: in traditional programming, you use code to describe **how to do it**; in Helen, you use prompts to describe **what to do**, then let the LLM figure out **how to do it**.

## Chapter Summary

- The agent is Helen's core concept: `Prompt + Tools + LLM`
- Use `agent` to declare an agent, and `main` to declare its entry point
- `llm act` asks the LLM to handle a task (Chinese form: `大模型 执行`)
- Agents can accept parameters; reference them in prompts with `{{param_name}}`
- English and Chinese keywords can be freely mixed
- Agents are called exactly like ordinary functions

## Further Reading

For the complete agent declaration syntax (all fields, isolation levels, transcript control), see the Language Reference:

- [[reference/05-agents|Agent Programming]] - Full `agent` declaration reference: `description`, `prompt`, `model`, `temperature`, `tools`, `functions {}`, isolation levels (`@open`/`@strict`/`@sandbox`), `transcript` control

## Next Chapter

[Chapter 2: Prompts — The Soul of an Agent](02-prompt.md) →
