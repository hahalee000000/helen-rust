# The Complete Guide to Agent Prompt Engineering — Insights from Claude Code Reverse Engineering

> Based on reverse engineering of Claude Code v2.0.14 system prompts, DeepAgents framework source analysis, and Helen's own agent design practices.
> Original: [The Complete Guide to Writing Agent System Prompts](https://fengliu.substack.com/) — Feng Liu, 2026-03-20
> This version: Adapted for Helen; all examples use Helen syntax

**Date**: 2026-07-11
**Applicable version**: Helen v1.17+

---

## 0. Positioning of This Guide

This guide answers one question: **How do you write high-quality `prompt` and `description` fields for Helen agents?**

It does not teach Helen syntax (see [[syntax/grammar]] and [[reference/05-agents]]); instead, it focuses on the **design philosophy, structural layout, writing principles, and common anti-patterns** of agent prompts — the "soft knowledge" beyond syntax that determines agent quality.

Core insights come from reverse engineering Claude Code's system prompt. Claude Code itself is a complex agent, and its system prompt has been extensively battle-tested by Anthropic. We **translate these lessons into Helen's context** — Helen's `agent {}` blocks, `prompt`/`description` fields, `tools` lists, and `context {}` configurations are the "system prompt" for Helen agents.

---

## 1. Design Philosophy: Harness Mindset

> "An agent is a model. Not a framework. Not a prompt chain." — shareAI-lab/learn-claude-code

LLMs already know how to reason, plan, and execute. Your agent prompt is **not teaching it to think** — it is **building its work environment**.

Think of it like hiring a senior engineer: you would not give them a 20-step checklist to follow mechanically. You would say: **who we are, what the boundaries are, and what good delivery looks like**, then let them do their work.

### 1.1 Four Responsibilities of an Agent Prompt

A Helen agent's `prompt` has only four things to do:

| # | Responsibility | Helen Field | Example |
|---|---------------|-------------|---------|
| 1 | Tell it **who it is** | `description` | `"Review code for bugs and security issues"` |
| 2 | Draw **boundaries** | `prompt` + `@sandbox` | `NEVER expose API keys` |
| 3 | Define **what good looks like** | `prompt` | `Always explain changes before making them` |
| 4 | Provide **tools and knowledge** | `tools` + `load_skill` | `tools = ["read_file", "shell_exec"]` |

Everything else is noise.

### 1.2 The Harness Formula

```
Harness = Tools + Knowledge + Observation + Action Interface + Permissions
```

Helen's implementation:
- **Tools**: `tools = [...]` list — 10 built-in tools + `load_skill` for domain knowledge
- **Knowledge**: Domain context in `prompt` + environment facts injected via `{{}}`
- **Observation**: `working-memory`, `transcript`, `:stats`
- **Action interface**: `llm act` + tool call loop
- **Permissions**: `@open` / `@strict` / `@sandbox` isolation levels

Do not write the agent as a flowchart — the model will decide the execution order itself.

---

## 2. Prompt Structural Layout

### 2.1 Recommended Order (Reverse Engineered from Claude Code v2.0.14)

```
┌─────────────────────────────────────────────┐
│ 1. Identity                  │ ← Read first; anchors behavior
│ 2. Safety Boundaries         │ ← IMPORTANT marked; non-negotiable
│ 3. Tone & Style              │ ← Controls output form
│ 4. Core Workflow             │ ← How to do the work
│ 5. Tool Usage Policy         │ ← Tool selection priorities
│ 6. Domain Knowledge          │ ← Load on demand; do not pre-fill
│ 7. Environment Info          │ ← Injected at runtime
│ 8. Reminders                 │ ← Reiterate the most important rules
├─────────────────────────────────────────────┤
│ [Tool Definitions — auto-injected by system] │ ← Not editable; usually very long
├─────────────────────────────────────────────┤
│ [User Messages]                              │
└─────────────────────────────────────────────┘
```

### 2.2 Why This Order

LLMs have a **U-shaped attention curve** — they pay the most attention to the beginning and end, while the middle tends to be "forgotten" (the "Lost in the Middle" effect).

- **Identity + Safety at the beginning**: primacy effect
- **Core Workflow in the upper-middle**: the most important content
- **Tool definitions injected by the system**: Claude Code's tool definitions are ~11,438 tokens, which "pushes" your custom content back toward the front — paradoxically improving compliance
- **Reminders at the end**: recency effect

Claude Code repeats its safety statements at both the beginning and the end — not because the engineers are forgetful, but because they understand U-shaped attention.

---

## 3. Helen Agent Prompt Structure

### 3.1 Complete Example

```helen
agent CodeReviewer {
    // ── 1. Identity ─────────────────────────────────────
    description "Review code for correctness, security, and style"

    // ── 2-8. Full prompt ─────────────────────────────────
    prompt """
    You are a senior code reviewer with 20 years of experience.

    IMPORTANT: NEVER suggest changes that break backward compatibility.
    IMPORTANT: NEVER expose secrets, tokens, or credentials in code or output.

    ## Tone and style
    - Short, direct, technical. No flattery or filler.
    - Professional objectivity: prioritize truth over validating the user.
    - Use GitHub-flavored markdown.

    ## Core workflow (principles, not steps)
    - Understand before modifying — always read existing code first.
    - Minimal changes — only change what's necessary.
    - Verify — suggest how to test the changes.
    - Explain the "why" — every suggestion includes rationale.

    ## Tool usage
    - Use `read_file` to inspect code; avoid `shell_exec` for file inspection.
    - Use `search_files` to find references before suggesting renames.
    - If a tool call is denied, do NOT retry the same call — reconsider.

    ## Environment
    Working directory: {{cwd}}
    OS: {{os_name()}}
    Current time: {{now()}}
    Git branch: {{shell_exec("git branch --show-current")}}

    ## Reminders
    IMPORTANT: Minimal changes. Explain why. Never break compatibility.
    """

    tools = ["read_file", "search_files", "shell_exec"]
    model "qwen3.7-plus"
    temperature 0.3
    max-turns 10
    context {
        working-memory true
        compression "graduated"
        cache-aware true
    }

    main {
        return llm act "Review the changes in the current diff"
    }
}
```

### 3.2 Helen Field Mapping

| General Principle | Helen Field | Description |
|-------------------|-------------|-------------|
| Identity | `description` | 1-3 sentences; anchors the role |
| Safety / Tone / Workflow / … | `prompt` | Use markdown sections |
| Tool policy | `prompt` explanation + `tools` list | List controls **what is available**; prompt controls **when to use** |
| Environment info | `{{}}` interpolation in `prompt` | **Always inject; never assume** (see §7) |
| On-demand knowledge | `tools = [..., "load_skill"]` + `load_skill("xxx")` | Two-tier disclosure; do not dump knowledge |
| Reminders | Final section of `prompt` | Reiterate the 2-3 most important rules |
| Context strategy | `context {}` block | Compression, working memory, cache awareness |

---

## 4. Section-by-Section Writing Guide

### 4.1 Identity — Who It Is

**Goal**: Anchor the role in 1-3 sentences.

```helen
// ✅ Good
description "Senior Rust engineer specializing in concurrent systems"

// ❌ Vague
description "A helpful AI assistant"

// ❌ Too long (wastes tokens)
description "You are a wise and experienced software engineer who has worked at many top tech companies and has deep knowledge of..."
```

### 4.2 Safety — Hard Boundaries

**Goal**: Behavioral constraints that must not be violated.

```helen
prompt """
IMPORTANT: NEVER generate or guess URLs for the user.
IMPORTANT: NEVER execute `rm -rf` or other destructive shell commands without confirmation.
MUST NOT modify files outside the working directory.
"""
```

Writing tips:
- Use the `IMPORTANT:` prefix — Claude's instruction hierarchy training gives it extra weight
- Use absolute language: `NEVER` / `MUST NOT` / `Refuse to`
- Bidirectional constraints: state both what is **allowed** and what is **prohibited**
- Place once at the beginning and once again at the end (U-shaped attention double insurance)

### 4.3 Tone & Style — Output Form

**Goal**: Specific, testable behavioral rules.

```helen
prompt """
## Tone and style
- Short and concise. No filler phrases.
- Only use emojis if the user explicitly requests it.
- Use GitHub-flavored markdown.
- Professional objectivity: prioritize technical accuracy over validating the user's beliefs.
"""
```

**Key point**: The "professional objectivity" paragraph is critically important — it suppresses the model's "sycophancy tendency." If the agent needs to make judgments (code review, architecture choices, solution evaluation), it must include a similar clause.

### 4.4 Core Workflow — The Most Important Section

**Goal**: Teach the model **how to work** — methodology, not mechanical steps.

Core principle: **Give principles, not procedures**.

```helen
// ✅ Principles (generalizable)
prompt """
Core workflow:
- Understand existing code before modifying it.
- Plan before executing complex changes.
- Make minimal changes — don't refactor while you're in there.
- Verify your changes work (run tests, lint).
"""

// ❌ Procedure (rigid)
prompt """
Step 1: Read the file.
Step 2: Find the bug.
Step 3: Fix it.
Step 4: Run tests.
Step 5: Commit.
"""
```

Principles can generalize to scenarios the model has never seen; procedures can only execute within expected parameters.

**Exception**: When the output will be consumed by a **machine** downstream (inter-agent communication, API response formats), use strict schemas — principles govern behavior, schemas govern interfaces.

### 4.5 Tool Usage Policy — Disambiguation

**Goal**: When multiple tools can do the same thing, tell the model which to prefer.

```helen
prompt """
## Tool usage
- Use `read_file` for reading files instead of `shell_exec cat`.
- Use `patch_file` for small edits instead of `write_file` rewrite.
- Use `search_files` for content search instead of `shell_exec grep`.
- Call independent tools in parallel when possible.
- If a tool call is denied, do NOT re-attempt the exact same call.
  Think about why it was denied and adjust your approach.
"""
```

Key points:
- Express priorities using "A instead of B"
- Explain **why** something is preferred ("reduces context usage", "better user experience")
- Define parallelism strategy (independent → parallel, dependent → sequential)
- Handle the tool-rejection scenario — otherwise the model will retry indefinitely

### 4.6 Domain Knowledge — Load On Demand

**Core principle**: Progressive disclosure; do not dump knowledge.

```helen
// ❌ Stuffing all 200 APIs into the prompt → token explosion
prompt "Here is the complete API documentation: ..."

// ✅ Let the agent load on demand
agent Worker {
    tools = ["load_skill", "read_file"]
    main {
        let guide = load_skill("helen-testing")
        return llm act "Follow this guide: " + guide
    }
}
```

Helen's Skills system (two-tier disclosure) is designed for exactly this:
- **Tier 1**: Only the **skill index** (name + description + tags) is injected into the system prompt
- **Tier 2**: The agent loads full content on demand via the `load_skill` tool

### 4.7 Environment Info — Runtime Facts

**Core principle**: **Always inject; never assume** — the agent cannot know facts you have not told it.

```helen
// ✅ Inject real values
agent DevAgent(cwd: str) {
    prompt """
    Working directory: {{cwd}}
    OS: {{os_name()}}
    Current time: {{now()}}
    Git branch: {{shell_exec("git branch --show-current")}}
    """
    main { return llm act "..." }
}

// ❌ Let the LLM guess
prompt "You are a helpful engineer."
// The LLM does not know cwd and will fabricate a plausible-sounding value
```

LLMs are trained to "always answer" — when lacking context, they will confidently fabricate. Injecting facts turns this failure mode into a non-issue.

See [[helen-agent-patterns § Best Practice 7]].

### 4.8 Reminders — Final Reinforcement

Only reiterate the 2-3 **most** important rules:

```helen
prompt """
[... previous sections ...]

## Reminders
IMPORTANT: Minimal changes only.
IMPORTANT: Explain the "why" for every suggestion.
IMPORTANT: NEVER break backward compatibility.
"""
```

---

## 5. Token Budget

### 5.1 Recommended Allocation

| Section | Recommended Tokens | Description |
|---------|-------------------|-------------|
| Identity + Safety | 200-500 | Concise but non-negotiable |
| Tone & Style | 300-800 | Rules must be specific |
| Core Workflow | 500-2,000 | Most important; worth the tokens |
| Tool Usage Policy | 300-1,000 | Depends on tool count |
| Domain Knowledge | 0-1,000 | Prefer on-demand loading |
| Environment Info | 100-300 | Dynamically generated |
| Reminders | 100-300 | Only reiterate essentials |
| **Your subtotal** | **1,500-6,000** | |
| Tool definitions (system-injected) | 5,000-15,000 | Outside your control |

### 5.2 Context Degradation Curve

Community-measured (Reddit u/CodeMonke_) actual compliance degradation:

| Context Length | Compliance |
|---------------|------------|
| < 80K tokens | Stable |
| 80K - 120K | Instruction following begins to degrade |
| > 120K | Significant degradation — model "forgets" earlier instructions |
| > 180K | Severe degradation |

**A 200K context window ≠ 200K effective context**.

Helen's countermeasures:
- `context { compression "graduated" }` — Five-layer graduated compression
- `context { cache-aware true }` — Cache-aware; preserves stable prefix
- `context { working-memory true }` — Working memory automatically tracks key information

---

## 6. Writing Principles

### 6.1 Principles Over Procedures

```
❌ "Step 1: Read file. Step 2: Find bug. Step 3: Fix. Step 4: Test."
✅ "Always understand existing code before modifying it. Verify your changes work."
```

Principles generalize; procedures can only execute mechanically and freeze when encountering the unexpected.

### 6.2 Use Absolute Language for Hard Constraints

| Strength | Wording | Used For |
|----------|---------|----------|
| Absolutely prohibited | `NEVER` / `MUST NOT` | Safety, irreversible operations |
| Strong requirement | `ALWAYS` / `MUST` | Core workflow rules |
| Recommended | `recommended` / `prefer` | Best practices with exceptions |
| Suggested | `consider` / `you may` | Optional optimizations |

### 6.3 Use Examples Instead of Explanations

```helen
prompt """
## Code references
When referencing code, use `file_path:line_number` format.

<example>
user: Where are client errors handled?
assistant: Clients are marked as failed in `connectToServer`
           at src/services/process.ts:712.
</example>
"""
```

One example is worth 100 words of explanation. Wrap in `<example>` tags; provide both positive and negative examples.

### 6.4 Bidirectional Constraints

```
✅ "Use `read_file` for reading files."
✅ "Do NOT use `shell_exec cat` for file inspection."
Bidirectional → clear and unambiguous.
```

Only saying "do this" → the model does not know when not to do it; only saying "don't do this" → the model does not know what to do instead.

### 6.5 Explain Why, Not Just What

```
❌ "Don't use `git commit --amend`."
✅ "Avoid `git commit --amend`. Reason: amending may overwrite others' commits.
    ONLY use --amend when you explicitly requested it."
```

Explaining why enables the model to make correct judgments in edge cases.

### 6.6 Structure Over Prose

- **Markdown headings** (`##` / `###`) — the model recognizes hierarchy
- **Lists** preferred over paragraphs — each rule is independently testable
- **XML tags** wrap special content: `<example>`, `<env>`
- **Tables** for comparisons and mappings

Never dump unstructured text — structured prompts **consistently** outperform natural language prose in compliance tests.

---

## 7. Anti-Patterns — These Are Wasting Your Tokens

### 7.1 Prompt Chains Disguised as Agents

```
❌ "First call tool A. Then tool B with result. Then format JSON. Then save."
```

This is not an agent prompt; it is a pipeline script. The model will execute mechanically and lose its ability to plan autonomously.

**Fix**: Tell the model the **goal and constraints**; let it decide the steps.

### 7.2 Sycophancy Engineering

```
❌ "You are an EXTREMELY TALENTED and INCREDIBLY EXPERIENCED senior engineer..."
```

Praise and superlative adjectives **do not improve output quality**. The model does not have an ego to manage. Save those 15 tokens for real rules.

### 7.3 Knowledge Dumping

```
❌ "Here is the complete API documentation for our 200 endpoints..."
```

This eats up the context window and accelerates context rot.

**Fix**: Load on demand — "use `load_skill` to retrieve documentation when needed."

### 7.4 Duplicating Tool Definitions

The tool definition already says "`read_file` reads a file" — do not repeat it in the prompt.
Only write in the prompt what **the tool definition does not cover**: when to use it, why it is preferred, priorities.

### 7.5 Missing Failure Handling

If you do not tell the model "what to do when a tool is rejected," it will retry failed calls indefinitely.

**Must include**:
```
If a tool call is denied, do not re-attempt the exact same call.
Think about why it was denied and adjust your approach.
```

### 7.6 Ignoring Context Window Decay

200K context ≠ 200K effective context. Measurements show degradation starting at 80K.

**A compression strategy is essential** — Helen's `context { compression "graduated" }` is the default behavior, but you should understand what it is doing.

---

## 8. Dynamic Injection — The Overlooked Power Tool

### 8.1 Why It Is Needed

The system prompt only appears once at the start of a conversation. As the conversation grows, the model's compliance with early instructions decays (noticeable at 80K+ tokens). **Injecting reminders mid-conversation = refreshing rules via the recency effect**.

Mental model:
- **System prompt = constitution**: established once; long-term authority
- **Mid-conversation injection = memo**: sent periodically; maintains execution discipline

### 8.2 Helen's Injection Mechanisms

Helen implements mid-conversation injection through multiple mechanisms:

| Mechanism | Location | Trigger |
|-----------|----------|---------|
| `prompt` + `{{}}` interpolation | Agent startup | Each agent invocation |
| `working-memory` | Within system messages | Auto-tracks files/decisions/TODOs/errors |
| `context { compression }` | During context compression | Automatic graduated compression |
| `load_skill` tool | Tool call result | Agent on demand |
| Tool results | Tool message | Each tool return |

### 8.3 Injection Best Practices

- **Wrap in XML tags** (`<system-reminder>`) — the model can distinguish system injections from user speech
- **Do not inject on every message** — each injection costs tokens; only inject when necessary
- **Keep it brief** — reminders are not a second system prompt; only reiterate 1-2 key rules
- **Do not contradict the system prompt** — reminders supplement and reinforce, not override
- **Use for dynamic switching** — plan mode, readonly mode, feature flags

### 8.4 System Prompt vs. Mid-Conversation Injection: Division of Labor

| Scenario | System Prompt | Mid-Conversation Injection |
|----------|--------------|---------------------------|
| Role definition | ✅ | ❌ |
| Safety constraints | ✅ Initial statement | ✅ Periodic repetition |
| Workflow methodology | ✅ | ❌ |
| Mode switching (plan mode) | ❌ | ✅ |
| File change notifications | ❌ | ✅ |
| Date / environment | ✅ Initial value | ✅ Updated value |
| Behavior correction | ❌ | ✅ |
| Tool usage reminders | ✅ Rule definition | ✅ Execution nudging |

---

## 9. Prompt Cache — Save 90% on Repeated Tokens

### 9.1 Key Numbers

| Metric | Value |
|--------|-------|
| Cache hit cost | 10% of normal price (saves 90%) |
| Cache write cost | 125% of normal price (25% more upfront) |
| Cache TTL | 5 minutes |
| Minimum cacheable length | 1,024 tokens |
| Cache granularity | Prefix matching |

### 9.2 How It Changes Prompt Design

**Core principle**: Static content first, dynamic content last.

```
✅ Cache-friendly layout:
System prompt (static)             ← Cache breakpoint 1
Tool definitions (static)          ← Cache breakpoint 2
Project rules (occasionally change)← Cache breakpoint 3
Conversation history               ← Breakpoint 4: rolling window

❌ Cache-breaking layout:
System prompt
  DYNAMIC TIMESTAMP                ← Changes every request; everything after cache misses
Tool definitions
Conversation history
```

**Trap**: Placing a dynamic timestamp in the middle of the system prompt turns everything after it into cache misses. A single misplaced timestamp can cost you full price for thousands of tokens.

### 9.3 Design Recommendations

- **Do not place high-frequency dynamic values** in the system prompt — date (changes daily) is fine; precise timestamps are not
- Place dynamic context (git status, etc.) in **mid-conversation injection**, not the system prompt
- Keep tool definitions stable — do not dynamically add/remove tools at runtime
- Use a **rolling window** for conversation history — cache the first N messages; only the newest is a cache miss

Helen's `context { cache-aware true }` is designed for exactly this — it preserves the first 30% of messages as a stable prefix, raising cache hit rates from 10-20% to 70-80%.

---

## 10. Checklist

After writing an agent prompt, check against this list:

### Structure
- [ ] Is identity (description) at the very front?
- [ ] Are safety constraints marked with `IMPORTANT:` and repeated at the end?
- [ ] Are sections clearly separated with `##` / `###`?
- [ ] Are examples wrapped in `<example>` tags?

### Token Budget
- [ ] Is your own portion < 6,000 tokens?
- [ ] Are you not repeating content already in tool definitions?
- [ ] Is domain knowledge loaded on demand, not pre-filled?
- [ ] Is there no lengthy role backstory?

### Rule Quality
- [ ] Is each rule testable as true/false?
- [ ] Do hard constraints use absolute language (NEVER/MUST)?
- [ ] Do soft recommendations use advisory language (recommended/prefer)?
- [ ] Do key rules explain why, not just what?
- [ ] Are constraints bidirectional (do + do not)?

### Agent Behavior
- [ ] Are you giving principles, not a 20-step mechanical procedure?
- [ ] Have you handled the "tool call rejected" scenario?
- [ ] Have you handled the "encountered an obstacle" strategy (no brute-force retries)?
- [ ] Is there a context management strategy (compression thresholds)?

### What Should Not Be There
- [ ] No sycophancy / superlative adjectives?
- [ ] No redundant "you are a helpful AI"?
- [ ] Not a prompt chain disguised as an agent?
- [ ] No features the user did not ask for?

---

## 11. If Starting From Scratch Today

Our recommendations (and the original author's):

1. **First 3 lines**: Complete identity + safety. Two sentences about who the agent is; hard constraints use NEVER/MUST; repeat safety rules at the end.
2. **Core workflow as principles**, at most 4-5 items. Soft rules use `recommended`/`prefer`; hard rules use `NEVER`/`MUST`.
3. **Budget 1,500-6,000 tokens for your portion**. Tool definitions will add another 5,000-15,000. Exceeding 6K means you are dumping knowledge that should be loaded on demand.
4. **Structure everything** — Markdown headings, lists, XML examples. Structured prompts always outperform natural language prose.
5. **Design for mid-conversation injection from day one** — Declare `<system-reminder>` tags in the system prompt; use them to refresh key rules, switch modes, and update context.
6. **Design for cache** — Static first, dynamic last. Never place high-frequency changing values in the body of the system prompt.

> **Ironically, the best system prompts are short.** Claude Code's custom instructions (excluding tool definitions) are surprisingly brief. Every line has earned its place.
>
> Prompt engineering is not about finding clever tricks — it is about discipline: say less, say precisely, and trust the model to figure out the rest.
>
> **The model is smarter than your prompt. Design the environment, not the behavior.**

---

## References

| Source | Key Insight |
|--------|------------|
| Claude Code v2.0.14 system prompt | Complete production agent prompt structure reference |
| Reddit: Understanding Claude Code's 3 System Prompt Methods | Deep analysis of Output Styles / --append / --system-prompt; measured context rot data |
| shareAI-lab/learn-claude-code | "The model is the agent" philosophy; Harness engineering methodology |
| Anthropic Prompt Engineering Docs | Official prompt best practices |
| DeepAgents Framework | Progressive disclosure middleware; Summarization strategy |
| Feng Liu, "The Complete Guide to Writing Agent System Prompts", 2026-03-20 | Original source material |

## Related Helen Documentation

- [[reference/05-agents]] — Helen agent syntax introduction
- [[reference/11-building-agents]] — Building multi-agent systems
- [[runtime/prompt-builder]] — Helen prompt builder system implementation
- [[runtime/context-management]] — Context management architecture (authoritative reference)
- `helen-agent-patterns` skill — Design patterns and best practices
- `helen-agent-collaboration` skill — Multi-agent collaboration patterns
