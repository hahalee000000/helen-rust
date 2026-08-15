# Session Scoping

> **v1.29 New** — Project-local vs global transcript sessions, `.helen/` marker auto-creation

**Since**: v1.29 (with refinements in v1.29.15, v1.29.16, v1.29.17)

Helen transcripts can live in one of two places:

- **Project-local**: `<your-project>/.helen/sessions/` — sessions belong to a specific Helen project
- **Global**: `~/.helen/sessions/` — sessions belong to Helen as a whole (REPL experiments, one-off scripts)

The scope is resolved at `TranscriptStore` initialization time. This document explains the resolution order, the `.helen/` project marker, and the memento format used by `helen agent`.

---

## 📋 Table of Contents

- [Why Two Scopes?](#why-two-scopes)
- [Resolution Order](#resolution-order)
- [Configuration](#configuration)
- [Project Marker Auto-Creation (v1.29.16+)](#project-marker-auto-creation)
- [HELEN_SESSION_DIR Override](#helen_session_dir-override)
- [Memento File Format](#memento-file-format)
- [The Three-Layer Bug (v1.29.15)](#the-three-layer-bug)
- [Troubleshooting](#troubleshooting)
- [See Also](#see-also)

---

## Why Two Scopes?

Different Helen programs have different session semantics:

| Use Case | Desired Scope |
|---|---|
| `helen agent` (programming assistant) | Project-local — every project has its own assistant history |
| `helen app.helen` (long-running service in a repo) | Project-local — sessions belong to that service |
| `helen scratch.helen` (one-off experiment) | Global — no project to attach to |
| `helen repl` (interactive exploration) | Global — the REPL isn't tied to a project |
| Helen's own test suite | Global — tests run outside any project |

The `auto` scope (default) picks project-local when a project marker exists, and falls back to global otherwise. This matches user intuition: if you're *in* a Helen project, sessions should live with the project.

---

## Resolution Order

When `TranscriptStore` initializes, the directory is resolved in this priority order:

```
1. HELEN_SESSION_DIR env var          (highest — explicit override)
      ↓ if not set
2. --transcript-log CLI flag          (per-invocation override)
      ↓ if not set
3. config.yaml transcript.session_dir (user preference)
      ↓ if not set
4. session_scope resolution:
      ├── scope == "global"  → ~/.helen/sessions/
      ├── scope == "project" → <project>/.helen/sessions/
      └── scope == "auto"    → try project, fall back to global
```

The `auto` scope is the default and what most users should leave it at.

### Step-by-step: `auto` scope

```
1. Read session_scope from config (default: "auto")
2. If scope is "auto" or "project":
   a. Walk from cwd upward looking for .helen/ directory
   b. If found: use <project>/.helen/sessions/
   c. If not found:
      - Since v1.29.16: create .helen/ in cwd and use it (unless cwd == ~/.helen)
      - Before v1.29.16: fall back to ~/.helen/sessions/
3. If scope is "global":
   Use ~/.helen/sessions/ directly
```

---

## Configuration

In `~/.helen/config.yaml`:

```yaml
transcript:
  enabled: true
  backend: "sqlite"                 # or "jsonl"
  session_scope: "auto"             # "auto" | "global" | "project"
  session_dir: "~/.helen/sessions"  # global default
  project_session_dir: ".helen/sessions"  # project-relative path
  max_memory_items: 1000
```

| Key | Default | Meaning |
|---|---|---|
| `session_scope` | `"auto"` | Scope resolution strategy |
| `session_dir` | `"~/.helen/sessions"` | Global fallback directory |
| `project_session_dir` | `".helen/sessions"` | Project-relative directory (appended to detected project root) |

---

## Project Marker Auto-Creation

**Since v1.29.16**, when `session_scope` is `auto` or `project` and no `.helen/` marker is found in `cwd` or its ancestors, Helen **creates `.helen/` in `cwd`** automatically.

This is the interpreter's responsibility — it happens inside `AgentContextManager._init_transcript_store()` in `helen/interpreter/agent_context.py`. Any Helen program with transcript enabled (`helen app.helen`, `helen agent`, `helen repl --transcript`, etc.) will establish `cwd` as a Helen project on first run.

**Exception**: if `cwd` is itself `~/.helen` (you're running Helen from its own home), no marker is created — that would create a recursive reference.

### Why This Was Added

Before v1.29.16, if you ran `helen agent` in a fresh directory:

1. No `.helen/` existed
2. `detect_project_dir()` returned `None`
3. Sessions silently went to `~/.helen/sessions/`
4. The user expected them to be in `<cwd>/.helen/sessions/`
5. No error, no warning — just wrong

The auto-creation eliminates this class of silent misrouting. It also makes the project marker *discoverable*: after the first run, `ls -la` shows `.helen/` and the user knows they're in a Helen project.

---

## HELEN_SESSION_DIR Override

The `HELEN_SESSION_DIR` environment variable **always wins** — it bypasses scope resolution entirely.

```bash
export HELEN_SESSION_DIR=/tmp/helen-sessions
helen agent             # sessions go to /tmp/helen-sessions, no .helen/ created
helen app.helen         # same
```

This is useful for:

- **CI**: isolate test sessions from real project sessions
- **Demos**: write sessions to a throwaway directory
- **Docker**: point sessions to a mounted volume

---

## Memento File Format

`helen agent` saves the active session ID to `.helen/current_session_id` so subsequent runs can resume. This file is read by the Python bridge import hook (`helen/python_bridge/import_hook.py`) and by `helen agent`'s own startup logic.

### Current Format (v1.29.15+, JSON)

```json
{
  "main": "session_1783492628_d9d9c0aa",
  "child": "session_1783492629_abc123"
}
```

| Field | Meaning |
|---|---|
| `main` | The `ChatSessionActor`'s session ID (the long-lived actor itself) |
| `child` | Any spawned sub-session (e.g. `:ask` side-conversations) |

### Legacy Format (pre-v1.29.15, plain string)

```
session_1783492628_d9d9c0aa
```

Just the session ID, nothing else. This is still accepted — `_detect_session_id()` checks if the first character is `{` to pick JSON parsing vs plain-string reading.

### Why the Change

The original single-ID format was fine when `helen agent` had one session. When `:ask` side-conversations were added (REPL sub-sessions), there was ambiguity: the memento should point to the *main* session, not a transient side-conversation. JSON with explicit `main` / `child` fields resolves this unambiguously.

---

## The Three-Layer Bug

The v1.29.15 → v1.29.17 releases fixed a particularly insidious three-layer bug around session scope. The diagnosis is worth recording because it illustrates how lazy initialization can silently bake in wrong state.

### Layer 1: Lazy init order (v1.29.15)

`_init_transcript_store()` was calling `resolve_session_dir()` directly. If any code path triggered `TranscriptStore` initialization *before* `set_session_dir()` had been called with the project path, the global path would be **permanently baked in** — `TranscriptStore` is initialized once, and re-initializing is a no-op.

**Fix**: before calling `resolve_session_dir()`, do an explicit `detect_project_dir(os.getcwd())` when `HELEN_SESSION_DIR` is unset and scope is `auto`/`project`.

### Layer 2: Missing project marker (v1.29.16)

Even with Layer 1 fixed, `detect_project_dir()` returns `None` if no `.helen/` exists. So on a cold start in a fresh directory, Layer 1's fix still fell through to global.

**Fix**: auto-create `.helen/` in `cwd` when no marker is found (see [Project Marker Auto-Creation](#project-marker-auto-creation)).

### Layer 3: Shared store initialization flag (v1.29.15)

`ContextManager` is a `shared store` — its `initialized` flag is shared across spawned interpreters. If one interpreter had run `init()` successfully (setting `initialized = true`) but failed to set `session_dir_path`, a second interpreter would see `initialized = true` and skip `init()` entirely — leaving `session_dir_path` empty forever.

**Fix**: the `init()` guard now checks both: `if initialized && session_dir_path != ""`. If `session_dir_path` is empty, re-run setup even if the flag is set.

### The Takeaway

When using lazy initialization with shared state, check that **every** piece of state is consistent before bailing out. A single boolean `initialized` flag can hide partial-initialization bugs that surface only under specific call orders.

---

## Troubleshooting

### "Sessions are going to `~/.helen/sessions/` instead of my project"

1. Check `echo $HELEN_SESSION_DIR` — if set, it overrides everything
2. Check `cat ~/.helen/config.yaml | grep session_scope` — if `"global"`, that's why
3. Run `ls -la .helen` — if missing, this is a pre-v1.29.16 install; upgrade or `mkdir .helen`
4. Upgrade to v1.29.17+ which auto-creates `.helen/`

### "helen agent doesn't resume my previous session"

1. Check `.helen/current_session_id` exists and has content
2. If it's a JSON object, verify the `main` field is populated (empty `main` → treated as no session)
3. Check the session file is actually in `.helen/sessions/`, not `~/.helen/sessions/`

### "I want a clean slate without losing old sessions"

```bash
rm -rf .helen/sessions/*     # wipe project sessions
rm .helen/current_session_id # forget the active session
helen agent                  # starts fresh
```

Old sessions in `~/.helen/sessions/` (global) are untouched.

---

## See Also

- [TranscriptStore SSOT](transcript-store.md) — the storage layer this scope resolution feeds into
- [Tutorial 18: Helen Programming Agent](../tutorial/18-helen-agent.md) — how `helen agent` uses session scoping
- [CLI: helen agent](../toolchain/cli.md#helen-agent) — the subcommand reference
- [Context Management](context-management.md) — how transcripts integrate with the four-layer context architecture
