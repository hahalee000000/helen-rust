# Tutorial 13: Skill System

> Let Agents work with domain expertise

---

## What Are Skills?

Skills are modular knowledge units that exist as Markdown files. They allow the LLM to load domain-specific knowledge on demand, instead of stuffing everything into the system prompt.

## Agent vs Skill: Fundamental Difference

> **An Agent is "who does it"; a Skill is the knowledge of "how to do it."**

| Dimension | Agent | Skill |
|-----------|-------|-------|
| **Nature** | Runtime entity | Static document |
| **Language level** | First-class citizen (syntax support) | External concept (pure Markdown) |
| **Callable** | Yes — `Agent()` called like a function | No — not callable |
| **Stateful** | Yes — maintains conversation/tool state | No — stateless |
| **Execution logic** | Yes — `main { }` block | No — no execution logic |
| **Purpose** | **Execute** tasks | **Guide** how to execute |

**Agents are executors**: they have model, temperature, tools; they can be called and composed, and they actually perform LLM calls and tool operations. Think of them as **employees**.

**Skills are knowledge bases**: they are pure Markdown documents that provide patterns, best practices, and API usage; agents read them as context. Think of them as **manuals**.

**Use an Agent** when you need to actually perform operations, maintain state, or be called from code.
**Use a Skill** when you need to provide knowledge, document workflows, or share knowledge across multiple agents.

**Practical relationship**: An agent can load a skill as a knowledge source:

```helen
import std.tools.*
agent Developer {
    tools = ["load_skill"]
    main {
        let guide = load_skill("helen-testing")
        return llm act "Follow: " + guide
    }
}
```

## Skill Directory Structure

```
~/.helen/skills/
├── web-search/
│   ├── SKILL.md           # Main file (required)
│   ├── references/        # Reference materials
│   ├── templates/         # Template files
│   └── scripts/           # Helper scripts
└── code-review/
    └── SKILL.md
```

## SKILL.md Format

```markdown
---
name: web-search
description: Search the web for information
version: 1.0.0
author: Your Name
tags: [web, search, research]
---

# Web Search Skill

## When to Use
- User asks for current information
- Need to verify facts

## How to Use
1. Use web_search tool
2. Analyze results
3. Summarize findings
```

## Three-Tier Search Architecture

Skills are searched in priority order from highest to lowest:

| Priority | Directory | Description |
|----------|-----------|-------------|
| 1 (highest) | `<project>/.helen/skills/` | Project-level, shared by the team |
| 2 | `~/.helen/skills/` | User-level, personal global |
| 3 | `<helen>/skills/` | Built-in, distributed with the language (13 skills) |
| Optional | `~/.hermes/skills/` | Hermes fallback (if installed) |

For skills with the same name, higher priority overrides lower priority.

## Two-Tier Disclosure Mechanism

### Tier 1: Skill Index

The name + description + **tags** of all skills are scanned and injected into the system prompt:

```
<available_skills>
Before replying, scan skills below. If a skill matches or is
even partially relevant to your task, you MUST load it with
load_skill and follow its instructions. Err on the side of loading.

  research:
    - web-search: Search the web for information (tags: web, search, research)
  dev:
    - code-review: Review code for quality and security (tags: review, security, quality)
</available_skills>
```

**v1.15 enhancement**: The skill index now includes a **MUST load** mandatory directive, requiring the LLM to load relevant skills instead of treating them as optional. This ensures the agent proactively learns relevant skills before generating code, avoiding guesses about API and syntax.

**The tags field** is key to improving skill hit rates. The LLM matches user intent using keywords in the tags, which is more accurate than relying solely on description text matching. Use a consistent naming convention (lowercase, English keywords).

### Tier 2: On-Demand Loading

After seeing the index, the LLM calls the `load_skill` tool to fetch the full content when needed.

```helen
import std.tools.*
main {
    // Basic loading
    load_skill("helen-testing")

    // Also list reference documents
    load_skill("helen-language-development", include_references=true)
}
```

### Tier 3: Reference Documents

Skills can include a `references/` directory containing in-depth reference documents. The LLM can access them as follows:

```helen
import std.core.*
import std.tools.*
main {
    // List all reference documents (name, path, size, first 3 lines preview)
    list_skill_references("helen-language-development")

    // Use read_file to load a specific reference document
    read_file(".../references/parser-disambiguation.md")
}
```

Reference documents are not loaded automatically — the LLM consults them on demand, saving tokens.

## Skill Awareness in LLM Statements

`llm act` automatically injects the skill index into the system prompt:

```helen
agent Researcher(query) {
    description: "Research topics using web search"
    
    main {
        // The LLM can see all available skills here
        llm act "Research: " + query
    }
}
```

The LLM sees all available skills at execution time and can consult relevant skill knowledge as needed.

## Skill Management Best Practices

### Naming Convention

```
Yes: web-search, code-review, data-analysis
No:  WebSearch, code_review, dataAnalysis
```

### Granularity Control

```
Yes: one skill = one clear task domain
No:  one skill = everything (too broad)
No:  one skill = a single instruction (too granular)
```

### Layered Organization

```
Project-level (.helen/skills/)  -> Project-specific conventions, API patterns
User-level (~/.helen/skills/)   -> Personal preferences, common workflows
Built-in (helen/skills/)        -> General skills, language-related
```

## Built-in Skills

Helen ships with 16 built-in skills, divided into Helen-specific and generic categories:

### Helen-Specific Skills

| Skill | Lines | Description |
|-------|-------|-------------|
| `helen-syntax` | 632 | Complete language syntax reference (89 keywords, types, expressions) |
| `helen-stdlib` | 739 | 203 built-in functions categorized reference with examples |
| `helen-testing` | 705 | Test framework usage, TDD workflow, Agent testing |
| `helen-quality` | 133 | 7-dimension quality assessment guide |
| `helen-agent-patterns` | 815 | Single agent design patterns (7 patterns + best practices) |
| `helen-agent-collaboration` | 545 | Multi-agent collaboration patterns (6 patterns) |
| `helen-language-development` | 674 | Language implementation patterns (AST, parser, interpreter extension) |
| `helen-programming-methodology` | 383 | Contract-driven + TDD + quality assessment workflow |
| `helen-python-bridge` | 576 | Python <-> Helen integration (FFI + Bridge) |
| `hellen-consistency-checker` | 1041 | Design document consistency checking |

### Generic Skills

| Skill | Lines | Description |
|-------|-------|-------------|
| `code-quality` | 402 | 7-dimension scoring, pre-commit verification, parallel cleanup |
| `debugging` | 610 | Systematic debugging methodology + language-specific tools |
| `planning` | 330 | Plan mode + implementation plan writing craft (merged from plan + writing-plans) |
| `test-driven-development` | 354 | Strict TDD enforcement (RED-GREEN-REFACTOR) |
| `subagent-driven-development` | 624 | Execute plans via subagents with 2-stage review |
| `github` | 323 | Complete GitHub workflow (PRs, issues, CI/CD) |

> The skill system has been restructured and optimized, reducing total lines from 10,672 to 9,086 (-15%), significantly lowering context usage.

## Exercises

1. Create a `greeting` skill under `~/.helen/skills/`
2. Create a `.helen/skills/` directory for the current project and add a project-specific skill
3. Use `:ask` in the REPL to test whether the skill is correctly recognized
4. Write a Helen program that uses `llm act` to call an agent that consults skills
