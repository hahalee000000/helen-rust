# Helen Language Guide

> **Version**: v1.39 · **Updated**: 2026-08-09

AI roams the digital space. AI says, "Let there be a language," and there was `pip install helen-lang`.

## What Is Helen?

Helen is an **AI-native programming language**.

It was designed by AI, implemented by AI, to develop AI agents.

*Of the AI, By the AI, For the AI.*

This may sound unusual, but it's true — Helen's compiler, interpreter, standard library, and testing framework were all built with AI involvement. It's not an "AI-enhanced" traditional language; it's a language designed **from the ground up** with AI agents at its center.

## Core Philosophy

Helen's core philosophy can be summarized in one sentence:

> **Prompts are first-class. Prompts can freely invoke tools, functions, and other agents.**

In traditional programming languages, code is the program's core, and AI is just an external service being called. In Helen, **the prompt itself is the core logic** — you describe tasks in natural language, and the prompt can invoke tools to read/write files, execute commands, search the web, or call other agents to collaborate.

## Who Is This Guide For?

- Developers who want to build applications with AI
- People interested in AI agents
- Anyone curious about what an "AI-native programming language" is

You don't need deep programming experience. Helen's syntax is designed to be clean and straightforward, and we'll guide you step by step with plenty of examples in each chapter.

## Before You Start: Program with AI, Not by Hand

Before diving into syntax, there's something more important than grammar — **Helen's best practice is not "memorize all syntax then write code by hand," but rather "load skills into your AI coding assistant and let AI help you program with Helen's complete knowledge system."**

### Built-In Skill System

Helen has a built-in **Skills** system — one of the core features that distinguishes Helen from all traditional programming languages. Skills are **pre-built structured knowledge documents** covering every aspect of Helen: syntax reference, standard library usage, testing framework, agent design patterns, debugging methodology, code quality assessment... They're not external documentation; they're **part of the language runtime**, dynamically loadable and usable by AI agents at runtime.

> **You don't need to memorize all of Helen's syntax and APIs. You just need to load skills into your AI coding assistant and let AI write code with this knowledge.**

### Skill List

Helen has 16 built-in skills, organized into these categories:

**Language Fundamentals**

| Skill | Description |
|-------|-------------|
| `helen-syntax` | Language syntax quick reference — keywords, types, expressions, statements, bilingual punctuation |
| `helen-stdlib` | Standard library reference — 364 built-in functions categorized with usage examples |
| `helen-testing` | Testing framework guide — TDD workflow, assertion API, CLI options, agent testing |

**Agent Design**

| Skill | Description |
|-------|-------------|
| `helen-agent-patterns` | Single-agent design patterns — scope isolation, routing, streaming, tool callbacks, closures, spawn, Channel |
| `helen-agent-collaboration` | Multi-agent collaboration patterns — task division, data flow, orchestration, shared state |
| `helen-python-bridge` | Python bridge — let Python directly call Helen agents and functions |

**Development Methodology**

| Skill | Description |
|-------|-------------|
| `helen-programming-methodology` | Complete development workflow — contract-first, TDD, quality assessment, skill evolution |
| `test-driven-development` | TDD methodology — enforces RED-GREEN-REFACTOR cycle |
| `planning` | Plan mode — implementation plan writing, task decomposition, precise paths |
| `subagent-driven-development` | Subagent-driven execution — execute plans via delegate_task (two-stage review) |

**Quality & Debugging**

| Skill | Description |
|-------|-------------|
| `helen-quality` | 7-dimension quality assessment — code analysis, security scoring, CI integration |
| `code-quality` | Code quality assessment — pre-commit verification, parallel cleanup |
| `debugging` | Debugging methodology — systematic root cause analysis, language-level debugging tools |

**Language Development & Tool Integration**

| Skill | Description |
|-------|-------------|
| `helen-language-development` | Language development patterns — AST/parser extension, async/await, exception hierarchy, FFI |
| `github` | Complete GitHub workflow — authentication, repositories, PRs, code review, issues, CI/CD |

### How to Use

When using AI coding tools like Cursor, Windsurf, Claude, you can provide Helen's skill files as context to AI:

```
Step 1: Have AI load the helen-syntax skill → AI knows Helen grammar
Step 2: Have AI load the helen-stdlib skill → AI knows the standard library
Step 3: Tell AI what you want to build → AI writes correct Helen code with complete knowledge
```

In Helen's runtime, the AI assistant automatically loads relevant skills — when you say "help me write a test," AI automatically loads the `helen-testing` skill; when you say "help me design a multi-agent system," AI loads the `helen-agent-collaboration` skill.

### Skill-Driven Development Workflow

```
1. Clarify requirements
   ↓
2. Load relevant skills (syntax, testing, methodology)
   ↓
3. Have AI write tests based on skill knowledge (TDD)
   ↓
4. Have AI write implementation based on tests
   ↓
5. Run tests to verify
   ↓
6. Have AI assess code quality (load quality skill)
   ↓
7. Iterate and optimize
```

| Traditional Approach | Skill-Driven Approach |
|---------------------|----------------------|
| Memorize syntax → write code → check docs → fix code | Load skills → AI writes code → run verification |
| Easy to misremember APIs | AI writes code with accurate references |
| Don't know best practices | Skills contain methodologies and patterns |
| Debug by experience | Debugging skills provide systematic methods |
| Fragmented knowledge | Skill system provides complete coverage |

Skills are **alive**. As Helen evolves, skills are continuously updated. Your AI coding assistant, once loaded with the latest skills, automatically has the latest knowledge.

---

## How to Read This Guide

The guide is organized from beginner to advanced. We recommend reading sequentially from start to finish:

| Chapter | Title | What You'll Learn |
|---------|-------|-------------------|
| Chapter 1 | [Your First Agent](01-hello-agent.md) | What agents are, how to create and run one |
| Chapter 2 | [Prompts: The Soul of Agents](02-prompt.md) | Purpose of prompts, template variables, injecting real information |
| Chapter 3 | [Talking to LLMs](03-llm-statements.md) | `llm act`, `llm if`, streaming output |
| Chapter 4 | [Equipping Agents with Tools](04-tools.md) | Tool declarations, tool callbacks, letting agents read/write files |
| Chapter 5 | [Variables and Data Types](05-basics.md) | Basic types, lists, maps, constants |
| Chapter 6 | [Control Flow](06-control-flow.md) | Conditional branches, loops, pattern matching, exception handling |
| Chapter 7 | [Functions and Closures](07-functions.md) | Function definitions, closures, pipe operator |
| Chapter 8 | [Agent Collaboration](08-collaboration.md) | Sequential chains, parallel, pipelines, spawn and Channel |
| Chapter 9 | [Standard Library Tour](09-stdlib.md) | Quick reference for 364 built-in functions |
| Chapter 10 | [Testing and Debugging](10-testing.md) | Testing framework, assertions, debugging techniques |
| Chapter 11 | [Advanced Topics](11-advanced.md) | Scope isolation, multimodal, protocols, MCP |
| Appendix | [Keywords and Quick Reference](appendix.md) | 99 keywords, common error codes, naming conventions |

## A Quick Preview

Before diving in, here's a complete Helen code sample to get a feel for its style:

```helen
import std.core.*

// Define an agent
agent Translator(source: str, target_lang: str) {
    description "Professional translation agent"
    prompt "You are a professional translator. Translate the text to {{target_lang}}. Preserve meaning, use natural language."
    temperature 0.3

    main {
        return llm act "Please translate: " + source
    }
}

// Program entry point
main {
    let result = Translator("Hello, World!", "Chinese")
    print(result)
}
```

Notice something? Helen supports bilingual keywords. `agent` and `智能体` are equivalent, `main` and `主函` are equivalent. You can write code in whatever way feels most comfortable.

## Let's Begin

Turn to [Chapter 1](01-hello-agent.md) and start your Helen journey.
