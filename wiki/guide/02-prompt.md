# Chapter 2: Prompts — The Soul of the Agent

## What Is a Prompt?

If an agent is a person, then the prompt is its "personality manual." The prompt tells the LLM:

- **Who you are** (role definition)
- **What you should do** (task definition)
- **How you should behave** (behavioral guidelines)
- **What you need to know** (background information)

In Helen, a prompt is not just static text — it is a **dynamic template that can embed variables**. This is the key feature that sets Helen apart from simple API calls.

> **Note:** Helen has **99 bilingual keywords** (48 English + 51 Chinese). Every English keyword below has a Chinese counterpart, so you can write Helen programs entirely in either language (or mix both). The first occurrence of each code sample will show the Chinese equivalent in a comment.

## Basic Usage

```helen
agent Assistant {  // Chinese: 智能体 Assistant {
    prompt "You are a helpful assistant. Answer questions in English."  // Chinese: 提示词 "..."

    main {  // Chinese: 主函 {
        return llm act "What is 1+1?"  // Chinese: 返回 大模型 执行 "..."
    }
}
```

`prompt` (Chinese: `提示词`) defines the system prompt, which stays active throughout the conversation. The string after `llm act` (Chinese: `大模型 执行`) is the user message — the concrete task for this turn.

## Template Variables `{{}}`

### Basic Substitution

An agent's parameters can be referenced directly inside the prompt via `{{param_name}}`:

```helen
agent CodeReviewer(code: str, language: str) {  // Chinese: 智能体 代码审查员(代码内容：字符串，语言：字符串) {
    prompt """
    You are a senior code review expert.
    Review the following {{language}} code and point out potential issues.
    Code:
    {{code}}
    """

    main {
        return llm act "Please start the review."
    }
}
```

Calling it:

```helen
let result = CodeReviewer("fn add(a, b) { return a + b }", "Helen")  // Chinese: 定义 结果 = 代码审查员("...", "Helen")
```

The actual prompt the LLM sees becomes:

```
You are a senior code review expert.
Review the following Helen code and point out potential issues.
Code:
fn add(a, b) { return a + b }
```

### Multi-Line Prompts

Use triple quotes `"""` to write multi-line prompts:

```helen
agent Writer(topic: str, style: str, word_count: int) {  // Chinese: 智能体 写手(主题：字符串，风格：字符串，字数：整数) {
    prompt """
    You are a writer with a {{style}} style.

    Task: Write an article about "{{topic}}".
    Requirements:
    - Approximately {{word_count}} words
    - Elegant and fluent prose
    - Opinionated and with depth
    """

    main {
        return llm act "Please start writing."
    }
}
```

Multi-line prompts are especially well suited for complex role definitions and task descriptions.

## Injecting Real Information (Key Concept)

This is one of the most important concepts in Helen: **the LLM does not know what it does not know.**

The LLM does not know the current time, your working directory, or the state of your project. If you don't tell it, it will **make something up** — and do so with complete confidence.

### The Wrong Way

```helen
// ❌ The LLM will invent the current time and environment info
agent DevAssistant {
    prompt "You are a senior engineer. Please help review the current project's code."
    main { return llm act "Review the code" }
}
```

The LLM has no idea what "the current project" is. It will guess — and it will very likely guess wrong.

### The Right Way

```helen
import std.system.*
import std.time.*

// ✅ Inject real information into the prompt via parameters
agent DevAssistant(work_dir: str, current_time: str, system: str) {  // Chinese: 智能体 开发助手(工作目录：字符串，当前时间：字符串，系统：字符串) {
    prompt """
    You are a senior engineer.
    Current working directory: {{work_dir}}
    Current time: {{current_time}}
    Operating system: {{system}}

    Please review the project code based on the above real information.
    If information is missing, state that honestly — do not guess.
    """

    main {
        return llm act "Review the current project's code"
    }
}

main {
    let dir = shell_exec("pwd")        // Chinese: 定义 目录 = 执行命令("pwd")
    let time = now()                   // Chinese: 定义 时间 = 当前时间戳()
    let system = platform()            // Chinese: 定义 系统 = 操作系统()
    let result = DevAssistant(dir, time, system)
    print(result)                      // Chinese: 打印(结果)
}
```

### What Information Should Be Injected?

| Scenario | Information to Inject |
|----------|----------------------|
| Programming assistant | Working directory, OS, Git branch |
| Time-sensitive tasks | Current time, timezone |
| File operations | Directory path, file listing |
| Data analysis | Data size, field names, sample data |
| Multi-agent collaboration | Output from upstream agents, shared state |

> **Core principle:** Any information the LLM cannot obtain on its own must be injected via `{{}}`. This is called **Ground Truth Injection**.

## Relationship Between Prompt and User Message

Many people confuse the prompt with the user message. Let's clarify the relationship:

```
┌─────────────────────────────────────┐
│  System Prompt (prompt)             │
│  "You are a translator. Translate   │  ← Always active; defines the role
│   to Chinese."                      │
├─────────────────────────────────────┤
│  User Message (string after llm act)│
│  "Hello, World!"                    │  ← The concrete task; may differ each call
├─────────────────────────────────────┤
│  LLM Reply                          │
│  "你好，世界！"                      │
└─────────────────────────────────────┘
```

```helen
agent Translator(target_lang: str) {  // Chinese: 智能体 翻译官(目标语言：字符串) {
    prompt "You are a translator. Translate text into {{target_lang}}."  // system prompt

    main {
        // "Hello" is the user message — the concrete content to translate
        return llm act "Hello"
    }
}
```

## Best Practices for Prompts

### 1. Be Specific About the Role

```helen
// ✅ Good: Clear role and scope of expertise
prompt "You are a pediatrician, specializing in health questions for children aged 0-12. For questions outside that scope, advise seeking medical care."

// ❌ Bad: Too broad
prompt "You are an assistant."
```

### 2. Be Specific About Behavior

```helen
// ✅ Good: Tells the LLM *how* to do the job
prompt """
You are a code reviewer.
Review steps:
1. Check for syntax errors
2. Check for logic errors
3. Check for security issues
4. Provide improvement suggestions
Output format: list the problems first, then give suggestions.
"""

// ❌ Bad: Too vague
prompt "Review the code."
```

### 3. Never Invent Facts in the Prompt

```helen
// ❌ Wrong: these "facts" are made up
prompt "You are in /home/user/project, on the main branch."

// ✅ Right: use template variables to inject real values
prompt "You are in {{work_dir}}, on the {{branch}} branch."
```

### 4. Pair Prompts with the Temperature Parameter

Temperature (`temperature`) controls the LLM's creativity:

| Temperature | Suitable Scenarios | Prompt Style |
|-------------|--------------------|--------------|
| 0.0–0.3 | Translation, classification, extraction | Precise, strict instructions |
| 0.4–0.7 | Conversation, writing, summarization | Balanced instructions |
| 0.8–1.0 | Creative writing, brainstorming | Open, encourages creativity |

## Chapter Summary

- `prompt` (Chinese: `提示词`) defines an agent's role and behavior — it is the "personality manual."
- Embed variables in the prompt with `{{param_name}}`.
- Write multi-line prompts with triple quotes `"""`.
- **Whatever the LLM cannot know on its own must be injected** — this is Helen's core principle.
- The system prompt defines the role; the user message (the string after `llm act`) defines the concrete task.
- Helen's 99 bilingual keywords mean you can write every construct above in either English or Chinese — the code behaves identically either way.

## Further Reading

- [[reference/05-agents|Agent Programming]] - Complete prompt field syntax, including multi-line `"""..."""` templates, `{{var}}` interpolation rules, and the `context {}` block for runtime context injection
- [[reference/agent-system-prompt-guide|Agent Prompt Engineering Complete Guide]] - Deep dive into prompt structure, writing principles, and anti-patterns (reverse-engineered from Claude Code)

## Next Chapter

[Chapter 3: Talking to the LLM](03-llm-statements.md) ->
