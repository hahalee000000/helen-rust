# Tutorial 18: Helen Programming Agent

> The interactive self-evolving programming assistant, built entirely in Helen.

**Since**: v1.26 (integrated) → v1.29 (matured)

Helen ships with an interactive programming assistant — a long-lived agent that runs inside a Web UI (or future TUI), accepts natural language requests, edits code, runs tests, checks quality, and **self-evolves** by creating new skills and accumulating memory across sessions. Unlike one-shot `helen program.helen` invocations, this is a **persistent, self-improving coding partner** that keeps context across turns and *grows its knowledge base* as it works.

Best of all: the entire assistant is written in Helen itself. The kernel (CLI launcher + Web UI backend) is Python; every piece of agent *behavior* — tooling, slash commands, context management, memory, integrity defense, skill creation, self-reflection — lives in `.helen` files under `helen/agent/`.

---

## 📋 Table of Contents

- [Quick Start](#quick-start)
- [Architecture Overview](#architecture-overview)
- [The Single-Agent Pattern](#the-single-agent-pattern)
- [Tool Set](#tool-set)
- [Slash Commands](#slash-commands)
- [Project-Level Skills](#project-level-skills)
- [Self-Evolution: Skills and Memory](#self-evolution-skills-and-memory)
  - [Skill Evolution](#skill-evolution)
  - [Memory Evolution](#memory-evolution)
  - [The Self-Evolution Loop](#the-self-evolution-loop)
  - [Security Guardrails](#security-guardrails)
- [Three-Layer Integrity Defense](#three-layer-integrity-defense)
- [Hooks Mechanism](#hooks-mechanism)
- [Context and Memory](#context-and-memory)
- [Web UI](#web-ui)
- [Session and Transcript Control](#session-and-transcript-control)
- [How Helen Agent Differs From Regular Agents](#how-helen-agent-differs-from-regular-agents)
- [Further Reading](#further-reading)

---

## Quick Start

### Launch the Web UI

```bash
# Recommended: cross-platform Python launcher (Windows/macOS/Linux)
python helen/agent/webui/start_webui.py
```

Or from CLI:

```bash
helen agent             # starts the programming assistant (launches Web UI)
```

Open the URL shown (default `http://localhost:5173`). The Web UI talks to a FastAPI backend which spawns `helen agent` under the hood.

### Launch Directly (CLI)

```bash
helen agent             # starts the programming assistant (currently launches Web UI)
```

The CLI subcommand `helen agent` is defined in `helen/cli/__main__.py` and delegates to `helen/cli/agent_launcher.py`. The launcher is responsible for:

1. Checking Node.js and Python dependencies
2. Setting the `HELEN_WEBUI_CWD` environment variable (cross-platform working directory)
3. Spawning `start_webui.py` (cross-platform Python launcher)
4. Forwarding signals (Ctrl+C) gracefully to child processes

#### Cross-Platform Support (v1.30.7+)

The agent works on **Windows, macOS, and Linux** without bash:

- **`start_webui.py`**: Single Python launcher replaces platform-specific bash scripts
- **`get_cwd()`**: Cross-platform working directory detection (uses `HELEN_WEBUI_CWD` env var with platform-specific fallback)
- **stdlib over shell**: Agent code uses stdlib functions (`time()`, `date()`, `env_get()`, `delete_file()`, `move_file()`) instead of Unix shell commands

To enable debug output (hidden by default since v1.30.7):

```bash
HELEN_DEBUG=1 helen agent          # Unix
set HELEN_DEBUG=1 && helen agent   # Windows
```

See [CLI documentation](../toolchain/cli.md#helen-agent) for the full subcommand reference.

---

## Architecture Overview

The v1.0 architecture consolidates all *specialist* agents into a single long-lived agent, `ChatSessionActor`, with the LLM calling every tool directly.

```
┌─────────────────────────────────────────────────────────┐
│ Web UI (webui/) — React + TypeScript + Tailwind          │
│  ↕ WebSocket                                             │
│ FastAPI backend (chat_tui_web.py)                        │
│  ↕                                                       │
│ chat_tui.helen — Actor lifecycle management              │
│  ↕ Channel mailbox                                       │
│ ChatSessionActor — single long-lived agent               │
│   ├── File ops : read_file / write_file / patch_file     │
│   ├── Quality  : run_helen_check / get_scores / metrics  │
│   ├── Testing  : run_helen_tests / verify_after_change   │
│   ├── Skills   : save_new_skill / update_existing_skill  │
│   │            / list_existing_skills / load_skill        │
│   │            / refresh_skill_index                      │
│   ├── Memory   : update_memory / update_user_preference   │
│   └── Hooks    : save_code_file / patch_code_file        │
│                  / pre_exit_check (self-evolution nudge)  │
└─────────────────────────────────────────────────────────┘
```

### Why One Agent, Not Many?

Earlier versions used a constellation of specialist agents (a *Contractor*, a *Tester*, a *QualityChecker*, …). That design paid a heavy LLM round-trip cost for orchestration — five tool calls to do what one can do.

The v1.0 insight: **let the LLM call every tool directly, and load *methodology* on demand as skills.** A single agent with 15 tools beats five agents with 3 tools each, because:

- One LLM call instead of five
- Context accumulates naturally in the transcript store
- No inter-agent coordination protocol to debug
- Skills provide the same domain knowledge the specialists did, on demand

---

## The Single-Agent Pattern

`ChatSessionActor` is the only `agent` declaration in the project. It has:

- A `description` explaining its role
- A `prompt` template that injects the working directory, environment, and memory context
- A `main {}` loop that runs indefinitely, pulling requests from a Channel mailbox
- A `tools` list of every tool it can call (see [Tool Set](#tool-set))

```helen
agent ChatSessionActor(cwd: str, session_id: str, env_context: str, reply: Channel) {
    description "Helen Programming Agent — self-evolving coding assistant"
    model "qwen3.7-plus"
    tools CHAT_TOOLS
    transcript "persistent"        // full audit trail
    
    main {
        _init_actor_output()
        try { ContextManager.init(cwd) } catch {}
        try { SessionStats.init() } catch {}
        
        // save main/child session IDs to memento
        _save_memento()
        
        // long-lived loop: wait for requests, dispatch, reply
        while true {
            let req = reply.receive()
            // ... LLM act with tool loop
        }
    }
}
```

Note three things:

1. **`transcript "persistent"`** — the actor's conversation is written to `.helen/sessions/`. This is what lets the user `resume_session()` later, and what the `/stats` and `/clear` commands operate on.
2. **`tools CHAT_TOOLS`** — the tool list is a `const` reference (v1.11 feature), making it easy to see and modify the tool set in one place.
3. **`reply: Channel`** — the caller sends requests through a Channel mailbox. The actor is `spawn`-ed once at startup, then driven via messages.

---

## Tool Set

`CHAT_TOOLS` is defined in `helen/agent/contracts/contracts.helen`. It falls into six groups:

| Group | Tools | Purpose |
|---|---|---|
| **File I/O** | `read_file`, `write_file`, `patch_file`, `path_exists`, `mkdir_p`, `list_dir` | Read, write, and patch source files |
| **Execution** | `shell_exec`, `calculate` | Run arbitrary commands; arithmetic |
| **Discovery** | `web_search`, `web_fetch`, `find_files`, `search_files` | Look up docs, search the codebase |
| **Quality** | `run_helen_check`, `get_scores`, `get_metrics`, `run_helen_tests` | Static check, quality scoring, test runs |
| **Skill Evolution** | `load_skill`, `list_skill_references`, `save_new_skill`, `update_existing_skill`, `refresh_skill_index` | On-demand methodology loading + growing the skill set |
| **Memory Evolution** | `update_memory`, `update_user_preference` | Accumulate project facts and user preferences across sessions |

All of these are either built-in tools (v1.1+) or stdlib functions exposed as tools via the agent's `tools` list. No Python-side custom tooling is needed. The Skill/Memory Evolution groups are what make the agent *self-evolving* — see [Self-Evolution: Skills and Memory](#self-evolution-skills-and-memory) for the full picture.

---

## Slash Commands

The agent understands Claude-Code-style slash commands. They are parsed in `helen/agent/commands.helen` and intercepted before the message reaches the LLM.

| Command | Effect |
|---|---|
| `/help` | Display available commands |
| `/clear` | Clear conversation context (inserts `BoundaryMarker`, keeps session entity) |
| `/clear-session [<sid>]` | Delete the entire session (cascades to spawned transcripts) |
| `/compress` | LLM-driven semantic context compression |
| `/stats` | Show session statistics (turns, tokens, tool calls, cost estimate) |
| `/memory` | Show memory system state |
| `/working-memory` | Show the `<working_memory>` block contents |
| `/mode` | Switch output mode (e.g. streaming vs. batched) |
| `/dir [path]` | Show or change the working directory (tilde `~` expansion supported) |
| `/session list` | List all sessions across global + project scopes (v1.44) |
| `/session delete <id>` | Delete a session by ID (fuzzy match; refuses if referenced by memento or has children) (v1.44) |
| `/session view <id>` | Show the user messages of a session, walking up the parent chain (v1.44) |

**Parsing details**: `parse_command()` strips whitespace, normalizes fullwidth slash `／` → `/`, and returns `{"is_command": bool, "command": str, "args": list<str>}`. Non-commands pass through to the LLM.

---

## Project-Level Skills

Helen Agent ships with **6 project-level skills** under `helen/agent/.helen/skills/`:

```
helen/agent/.helen/skills/
├── architecture/
│   └── helen-contractor-design/SKILL.md
├── testing/
│   ├── helen-test-patterns/SKILL.md
│   └── helen-tdd-methodology/SKILL.md
└── code-quality/
    ├── helen-quality-rubrics/SKILL.md
    └── helen-code-integrity/SKILL.md
```

These are **not** globally installed skills — they live next to the agent code and are loaded on demand via `load_skill("helen-code-integrity")` etc. The LLM decides when to call `load_skill` based on the task.

The skill set encodes the project's methodology:

- **`helen-contractor-design`** — architectural patterns (contract-first, interface segregation)
- **`helen-test-patterns`** — how to write Helen tests (built-in test framework + pytest)
- **`helen-tdd-methodology`** — strict RED-GREEN-REFACTOR workflow
- **`helen-quality-rubrics`** — the 7-dimension scoring rubric
- **`helen-code-integrity`** — completeness checks (no half-written features)

These six skills are the *seed* knowledge. The agent is not limited to them — it can **grow** its own skill set and memory across sessions. This is the self-evolution capability, covered in the next section.

---

## Self-Evolution: Skills and Memory

Most AI assistants forget everything between sessions. Helen Agent is designed to *accumulate knowledge* — both as reusable methodology (skills) and as episodic facts (memory). Two evolution channels, three LLM-callable tools per channel, two files on disk, and a prompt that nudges the LLM to use them.

### Skill Evolution

Three tools are exposed to the LLM via `CHAT_TOOLS` (defined in `helen/agent/contracts/contracts.helen`):

```helen
// Create a new skill at .helen/skills/<category>/<name>/SKILL.md
fn save_new_skill(name: str, category: str, tags: str, content: str): map

// Append to an existing skill (path safety enforced — see below)
fn update_existing_skill(skill_path: str, addition: str): map

// Rebuild the SKILL_INDEX.md from all SKILL.md files
fn refresh_skill_index(): str
```

**How `save_new_skill` works**:

1. Validates `name` and `category` against path traversal (`validate_path`)
2. Creates the directory `.helen/skills/<category>/<name>/`
3. Writes `SKILL.md` with YAML frontmatter:
   ```yaml
   ---
   name: <name>
   description: "<first 80 chars of content>"
   tags: <tags>
   version: 1.0.0
   ---
   ```
4. Appends the full `content` below the frontmatter
5. Automatically calls `refresh_skill_index()` so `load_skill()` can find it immediately

**How `update_existing_skill` works**:

1. Validates the path
2. Rejects paths outside `.helen/skills/` (defense in depth — see [Security Guardrails](#security-guardrails))
3. Reads the existing `SKILL.md`
4. Appends `\n\n` + `addition`
5. Writes back

**How `refresh_skill_index` works**:

Runs `find .helen/skills -name 'SKILL.md'`, extracts the top 6 lines (frontmatter) from each, and atomically writes `.helen/skills/SKILL_INDEX.md` (via a `.tmp` file + `mv` to avoid half-written reads).

**Example**: After debugging a subtle scope bug, the LLM might decide this pattern is worth capturing:

```helen
main {
    save_new_skill(
        "scope-pitfalls",
        "debugging",
        "scope,isolation,shared_let",
        "# Scope Pitfalls\n\n## shared let Write-Back\n..."
    )

}```

Next session, `load_skill("scope-pitfalls")` retrieves it. Or the LLM can simply see it listed in `SKILL_INDEX.md`.

### Memory Evolution

Skills are for **methodology** (reusable patterns). Memory is for **facts** (specific discoveries about this project, its bugs, its user's preferences). Two tools, two files:

```helen
// Append a fact to .helen/MEMORY.md under a category
fn update_memory(category: str, key: str, value: str): map

// Append a preference to .helen/USER.md under a category
fn update_user_preference(category: str, preference: str): map
```

**MEMORY.md format** — grouped by `## category`, each entry is `- **key**: value`:

```markdown
# HelenAgent Unified Context

> Agent 启动时加载的统一上下文入口。

## error_patterns
- **shared_let_write_back**: Agent 内修改 shared let 后，agent 返回前会自动写回
- **transcript_lazy_init**: 当 TranscriptStore 延迟初始化发生在 set_session_dir 之前，路径会被永久烤错

## project_facts
- **default_test_runner**: pytest (not helen test) for integration tests
```

**USER.md format** — user preferences (output language, verbosity, etc.):

```markdown
# User Preferences

## style
- **verbosity**: minimal — one-line status per phase
- **language**: match user's language (Chinese input → Chinese output)
```

**At startup**, `load_memory()`, `load_user_preferences()`, and `build_memory_context()` (in `helen/agent/memory_utils.helen`) read both files and inject their contents into the agent's `{{env_context}}` template. The LLM sees them as part of its initial context, so memories from prior sessions influence current behavior without any explicit retrieval.

### The Self-Evolution Loop

The agent's system prompt explicitly drives the loop:

```
## Core Workflow
You operate in an implicit agentic loop. For each user request:

1. Understand intent — read relevant files if needed
2. Plan approach — choose the simplest tool path that works
3. Execute — use tools directly for all tasks
4. Verify — run tests / helen check after changes
5. Learn — save non-obvious discoveries to memory

IMPORTANT: Proactively save non-obvious discoveries to memory using update_memory().
```

Combined with the `pre_exit_check` hook (see [Hooks Mechanism](#hooks-mechanism)), which returns a reminder:

```json
{"reminder": "如本次会话有重要学习，请调用 update_memory 保存"}
```

The full loop looks like this:

```
       ┌─────────────────────────────────────────┐
       │                                         │
       ▼                                         │
Agent encounters new problem / pattern           │
       │                                         │
       ▼                                         │
Solve + verify (helen check, tests)              │
       │                                         │
       ▼                                         │
LLM judges: is this reusable?                    │
       │                                         │
       ├── yes, reusable methodology ──────────┐ │
       │                                       │ │
       │   save_new_skill()                    │ │
       │   or update_existing_skill()          │ │
       │        │                              │ │
       │        ▼                              │ │
       │   Auto: refresh_skill_index()         │ │
       │        │                              │ │
       │        ▼                              │ │
       │   .helen/skills/...                   │ │
       │                                       │ │
       └── yes, project-specific fact ───────┐ │ │
                                             │ │ │
         update_memory()                     │ │ │
         or update_user_preference()         │ │ │
              │                              │ │ │
              ▼                              │ │ │
         .helen/MEMORY.md                    │ │ │
         or .helen/USER.md                   │ │ │
                                             │ │ │
              │                              │ │ │
              └──────────────────────────────┘ │ │
                                               │ │
       Next session startup:                   │ │
         load_memory() + load_user_preferences()│ │
         → injected into {{env_context}}       │ │
                                               │ │
       Next skill use:                         │ │
         load_skill("...") or SKILL_INDEX.md ←─┘ ┘
```

The two channels are complementary:

| Channel | File | Content kind | Retrieval |
|---|---|---|---|
| **Skills** | `.helen/skills/**/SKILL.md` | Methodology, patterns, how-to | `load_skill()` (on demand) |
| **Memory** | `.helen/MEMORY.md` | Facts, error patterns, project truths | Auto-injected at startup |
| **User prefs** | `.helen/USER.md` | Style, language, verbosity | Auto-injected at startup |

### Security Guardrails

Self-evolution touches the filesystem, so it needs guardrails:

1. **Path validation**: `save_new_skill` and `update_existing_skill` both run `validate_path(name)` / `validate_path(skill_path)` which rejects `..`, null bytes, and absolute paths.

2. **Directory confinement**: `update_existing_skill` has an explicit `startswith(skill_path, ".helen/skills/")` check — even if path validation passes, an update can only target the project's skill directory.

3. **Atomic index writes**: `refresh_skill_index` writes to `.tmp` then `mv`, preventing a concurrent `load_skill` from reading a half-written index.

4. **No system file writes**: `update_memory` and `update_user_preference` only write to `.helen/MEMORY.md` and `.helen/USER.md` in the project directory. They never touch `~/.helen/` or system paths.

5. **Opt-in retrieval**: Skills are only loaded when the LLM explicitly calls `load_skill()`. Memory is auto-injected, but only from the project's own `.helen/MEMORY.md` — a malicious project cannot inject memory into another project's agent.

---

## Three-Layer Integrity Defense

Helen Agent refuses to let bad code stand. Every edit passes through three gates:

```
Layer 1 — LLM self-check
  └─ Prompt includes "verify your change with helen check before reporting done"

Layer 2 — Test coverage
  └─ After edit: run_helen_tests (or user's pytest)
  └─ If tests fail: LLM must fix before reporting done

Layer 3 — Code integrity skill
  └─ load_skill("helen-code-integrity") — LLM audits for half-written features,
     dead code, TODOs without follow-through
```

The `pre_exit_check` hook runs Layer 1 and 2 automatically before the actor accepts a `/clear` or exit request.

---

## Hooks Mechanism

Helen supports `on_*` callbacks on agent tool lifecycle (v1.21+). Helen Agent uses these to enforce invariants:

| Hook | Behavior |
|---|---|
| `on_tool_end save_code_file` | After any `save_code_file` call, automatically runs `helen check` on the modified file and reports errors back to the LLM |
| `on_tool_end patch_code_file` | Same, for patches |
| `on_tool_end` (generic) | Records the tool call for `/stats` |
| `pre_exit_check` (explicit tool) | Returns `{"reminder": "如本次会话有重要学习，请调用 update_memory 保存"}` — drives the [self-evolution loop](#the-self-evolution-loop) by nudging the LLM to save learnings before exit |

This is what makes the agent *self-healing*: a bad `write_file` is immediately caught by `helen check`, and the LLM sees the errors in its next tool-result turn.

The `pre_exit_check` hook is the *self-evolution* counterpart to the self-healing `save_code_file` hook: one catches code mistakes, the other catches missed learning opportunities.

See [Tutorial 05: Agent Programming](05-agents.md) and [Tutorial 06: LLM Statements](06-llm-statements.md) for the underlying callback mechanism.

---

## Context and Memory

The agent uses three distinct context layers:

1. **Transcript (SSOT)** — full conversation history, persisted via `TranscriptStore`. Managed by stdlib functions `get_session_id()`, `list_sessions()`, `replay_transcript()`.

2. **Working memory** — a `<working_memory>` block in the system prompt that the LLM proactively maintains across turns (v1.25 feature). Updated by the agent itself via ordinary prompt instructions.

3. **Long-term memory** — `helen/agent/memory_utils.helen` wraps the `FileMemoryProvider` (v1.16+) for cross-session recall.

The `ContextManager` shared store coordinates context decisions:

```helen
shared store ContextManager {
    let initialized: bool = false
    let pinned_uuids: list = []        // messages pinned across compression
    let session_dir_path: str = ""     // current session directory
    
    fn init(cwd: str = "") {
        // idempotent; forces re-setup if session_dir_path is empty
        if initialized && session_dir_path != "" { return }
        initialized = true
        cached_version = detect_helen_version()
        session_dir_path = _setup_session_scope(cwd)
        _register_hooks()
    }
}
```

The `init(cwd)` parameter was added in v1.29.15 to avoid `shell_exec("pwd")` returning the wrong path inside a spawned actor. Since v1.30.7, the agent uses a cross-platform `get_cwd()` helper (defined in `utils.helen`) that reads `HELEN_WEBUI_CWD` env var and falls back to platform-specific commands (`cd` on Windows, `pwd` on Unix). See [Session Scoping](../runtime/session-scoping.md) for the full story.

---

## Web UI

The Web UI is a FastAPI + React app under `helen/agent/webui/`:

```
webui/
├── backend/         # FastAPI + WebSocket
│   └── app/
│       ├── main.py
│       ├── routers/
│       ├── services/
│       └── websocket/
├── frontend/        # React + TypeScript + Tailwind
│   └── src/
│       ├── components/
│       ├── pages/
│       ├── hooks/
│       ├── services/
│       └── stores/
├── start_webui.py   # Cross-platform launcher (Windows/macOS/Linux)
├── start-all.sh     # Unix legacy launcher
├── start-backend.sh
├── start-frontend.sh
└── stop-all.sh
```/
│       └── stores/
├── start-all.sh
├── start-backend.sh
├── start-frontend.sh
└── stop-all.sh
```

Features:

- **Real-time streaming** — WebSocket delivers LLM tokens as they arrive
- **Session management** — create, switch, delete sessions from the sidebar
- **Slash commands** — same as TUI; sent as regular messages starting with `/`
- **Responsive** — works on desktop and mobile
- **Agent status** — live indicator of what the agent is doing (thinking, tool call, etc.)

### Stop Button

The stop button sends a cancel signal via WebSocket. Since v1.39.7, cancel checks are placed at all key points in the agentic loop:

| Phase | Cancel responsive? |
|---|---|
| LLM streaming (text tokens) | ✅ Immediate |
| Between LLM turns | ✅ Immediate |
| Between tool calls (sequential) | ✅ At next tool boundary |
| Between tool completions (concurrent) | ✅ Cancels remaining futures |
| During a single long tool call | ⚠️ After current tool completes |

The cancel signal propagates: `stream_emitter.request_cancel()` → actor `on_chunk`/`on_tool_end` polls via FFI → `cancel_all_llm_calls()` → interpreter `cancel_event` → HTTP SSE loop breaks.

### Backend ↔ Helen Bridge

The backend doesn't call the LLM directly — it spawns `chat_tui.helen` as an actor and communicates via a Channel. This keeps the "Helen code in Helen" invariant: the Python side only does I/O bridging, not agent logic.

---

## Session and Transcript Control

Each `helen agent` run is a session. Sessions live in the **project-local** `.helen/sessions/` directory (since v1.29.16 — see [Session Scoping](../runtime/session-scoping.md) for details).

**Memento file**: `.helen/current_session_id` stores the active session ID so that subsequent `helen agent` runs can resume. Since v1.29.15, the memento file is a JSON object:

```json
{
  "main": "session_1783492628_d9d9c0aa",
  "child": "session_1783492629_abc123"
}
```

The `main` field is the `ChatSessionActor`'s session; `child` is any spawned sub-session (e.g. a `:ask` side-conversation). Older mementos are plain strings (a single session ID) — both formats are supported.

**Transcript level**: The agent uses `transcript "persistent"` so every turn is written to `.helen/sessions/<sid>/`. Use `/clear` to insert a `BoundaryMarker` (starts a fresh logical conversation but keeps the audit trail), or `/clear-session` to delete the session entirely.

---

## How Helen Agent Differs From Regular Agents

| | Regular `agent Foo {}` | Helen Programming Agent |
|---|---|---|
| Lifetime | One invocation | Long-lived, loop in `main {}` |
| Driven by | Caller | Channel mailbox + user input |
| Tools | `tools` list chosen per-agent | Full `CHAT_TOOLS` set |
| Skills | Can `load_skill()` on demand | Same, plus can `save_new_skill()` |
| Transcript | Defaults to `none` | Defaults to `persistent` |
| Hooks | None typically | `save_code_file` auto-runs `helen check` |
| Context | Per-invocation fresh | Accumulates across turns |

When you write your own long-lived agent, the Helen Programming Agent is a good reference implementation. Look at `helen/agent/chat_session_actor.helen` for the structure.

---

## Further Reading

- [Tutorial 05: Agent Programming](05-agents.md) — agent declarations, tools, transcript control
- [Tutorial 07: Concurrent Programming](07-spawn.md) — `spawn`, Channel, `mailbox_select`
- [Tutorial 13: Skill System](13-skills.md) — three-layer search, two-layer disclosure
- [Runtime: Session Scoping](../runtime/session-scoping.md) — how `.helen/` markers and session directories are resolved
- [Runtime: TranscriptStore SSOT](../runtime/transcript-store.md) — persistent sessions, LRU cache
- [Runtime: Context Management](../runtime/context-management.md) — the four-layer context architecture
- [Agent README](https://github.com/hahalee000000/helen/tree/master/helen/agent) — source-level documentation
