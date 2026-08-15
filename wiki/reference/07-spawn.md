# Tutorial 07: Concurrent Programming (spawn)

> spawn / Channel message queue / mailbox_select / explicit sharing / fire-and-forget / error handling

---

## Overview

Helen v1.18 uses `spawn` + Channel message queues for concurrency. `spawn Agent(...)` returns a Channel (mailbox) for bidirectional communication.

**Core principles**:
- One concurrency primitive (`spawn`) + one communication mechanism (Channel message queue)
- **Isolation first**: snapshot deep-copies everything; agents are fully isolated from the outer environment by default
- **Explicit context passing**: a spawned agent **cannot see** the parent agent's variables, transcript, or working memory at all — it can only see the arguments passed in and the Channel. **You must explicitly decide what context to pass before spawning**
- Sharing is explicit: pass SharedStore references through Channels

> 💡 This principle applies to all agent calls, not just spawn. See [Tutorial 05: Core Design Principles](05-agents.md#core-design-principle-caller-decides-context).

---

## Basic spawn Usage

```helen
import std.core.*
agent Worker(task: str, reply: Channel) {
    description "Background worker agent"
    main {
        let result = "Processing complete: " + task
        reply.send(result)
        // Function ends → reply auto-closes → main thread receive() gets null
    }
}

main {
    let mailbox = spawn Worker("data analysis")
    let result = mailbox.receive()
    print(result)  // "Processing complete: data analysis"
}
```

**Key points**:
- `spawn` returns a `Channel` type (mailbox)
- The spawned agent's **last parameter** receives the communication channel (auto-injected by spawn)
- `reply.send(msg)` sends a message to the main thread
- `mailbox.receive()` blocks waiting for a message

---

## Channel Message Queue

### send / receive

```helen
main {
    // Send a message
    mailbox.send("hello")

    // Blocking receive
    let msg = mailbox.receive()

    // Receive with timeout
    let msg_with_timeout = mailbox.receive(5.0)  // 5 second timeout, returns null
}
```

### try_receive (non-blocking)

```helen
import std.core.*
main {
    let msg = mailbox.try_receive()
    if msg == null {
        print("No messages yet")
    }
}
```

### cancel

```helen
main {
    mailbox.cancel()  // Send cancel signal + close channel
}
```

The spawned agent can check the cancel signal via `reply.cancel_event`:

```helen
import std.core.*
agent LongTask(reply: Channel) {
    main {
        for i in range(100) {
            if reply.cancel_event.is_set() { break }
            // Do work...
            reply.send("Progress: " + str(i))
        }
    }
}
```

### Streaming Interrupt (cancel_event + on_chunk returning false)

When a spawned agent is streaming LLM output, the main thread can send a cancel signal via `mailbox.cancel()`. The spawned agent can respond in two ways:

1. **Check `reply.cancel_event`**: detect the cancel signal inside a loop
2. **`on_chunk` callback returns `false`**: immediately interrupt LLM streaming

```helen
import std.core.*
agent StreamWorker(prompt: str, reply: Channel) {
    main {
        let result = llm act prompt on_chunk fn(chunk: str) {
            // Check cancel signal: if main thread has cancelled, return false to stop streaming
            if reply.cancel_event.is_set() {
                return false  // Immediately stop LLM streaming
            }
            reply.send({type: "chunk", data: chunk})
            return true  // Continue receiving next chunk
        }
        reply.send({type: "done", data: result})
    }
}

main {
    let mailbox = spawn StreamWorker("Write a long essay")
    // Cancel after reading the first 3 chunks
    let count = 0
    loop {
        let msg = mailbox.receive()
        if msg == null { break }
        print(msg["data"])
        count = count + 1
        if count >= 3 {
            mailbox.cancel()  // Send cancel signal
            break
        }
    }
}
```

### close

```helen
import std.core.*
agent LongTask(prompt: str, reply: Channel) {
    main {
        let result = llm act prompt on_chunk fn(chunk: str) {
            reply.send({type: "progress", data: chunk})
        }
        reply.send({type: "done", data: result})
    }
}

main {
    let mailbox = spawn LongTask("Write an AI research paper")
    loop {
        let msg = mailbox.receive()
        if msg == null { break }
        if msg["type"] == "progress" {
            print(msg["data"])
        } else {
            print("Done: " + msg["data"])
        }
    }
}
```

---

## Racing Pattern (mailbox_select)

`mailbox_select([m1, m2, ...])` receives the first message that arrives from multiple channels:

```helen
import std.concurrency.*
import std.core.*
agent StrategyA(problem: str, reply: Channel) {
    main {
        let result = llm act "Strategy A solves: " + problem
        reply.send(result)
    }
}

agent StrategyB(problem: str, reply: Channel) {
    main {
        let result = llm act "Strategy B solves: " + problem
        reply.send(result)
    }
}

main {
    let m1 = spawn StrategyA("complex problem")
    let m2 = spawn StrategyB("complex problem")

    let result = mailbox_select([m1, m2])
    print("Fastest result: " + result["message"])

    // Cancel the other one
    if result["endpoint"] == m1 {
        m2.cancel()
    } else {
        m1.cancel()
    }
}
```

---

## Explicit Sharing (Passing SharedStore via Channel)

Spawned agents are fully isolated from the outer environment by default. To access the main thread's shared store, explicitly pass the reference through the channel:

```helen
import std.core.*
import std.dict.*
shared store Counter {
    let count: int = 0
    fn inc() { count = count + 1 }
    fn get(): int { return count }
}

agent Worker(reply: Channel) {
    main {
        // Receive shared store reference from channel
        let store = reply.receive()
        store.inc()
        store.inc()
        reply.send("done")
    }
}

main {
    let mailbox = spawn Worker()
    mailbox.send(Counter)           // Explicitly pass reference
    print(mailbox.receive())        // "done"
    print(Counter.get())            // 2 — same object, modifications are visible
}
```

**Why explicit passing?**
- Snapshots deep-copy everything; spawned agents get an independent copy of SharedStore by default
- Passing references through channels is **intentional** sharing — the developer explicitly knows which state is shared

---

## Fire-and-Forget

When you don't need the result, ignore spawn's return value:

```helen
import std.core.*
main {
    spawn Logger("System startup log")
    spawn Monitor("Health check")
    print("System started")  // Executes immediately, doesn't wait for spawned agents
}
```

---

## Bidirectional Communication

Channels support bidirectional communication — the main thread and spawned agent can send messages to each other:

```helen
import std.core.*
import std.tools.*
agent Calculator(reply: Channel) {
    main {
        loop {
            let cmd = reply.receive()
            if cmd == null || cmd == "quit" { break }
            reply.send(calculate(cmd))
        }
    }
}

main {
    let calc = spawn Calculator()
    calc.send("2 + 3")
    print(calc.receive())     // 5
    calc.send("10 * 20")
    print(calc.receive())     // 200
    calc.send("quit")
}
```

---

## Error Handling

Uncaught exceptions in spawned agents propagate through the channel:

```helen
import std.core.*
agent RiskyTask(reply: Channel) {
    main {
        let result = 100 / 0  // Throws exception
        reply.send(result)
    }
}

main {
    let mailbox = spawn RiskyTask()
    let msg = mailbox.receive()
    if msg != null && msg["__error__"] == true {
        print("Spawned agent error: " + msg["message"])
    }
}
```

**Lifecycle**:

| Event | Behavior |
|-------|----------|
| Spawned agent finishes normally | `reply.close()` → main thread `receive()` returns null |
| Spawned agent throws exception | `reply.send({__error__: true, message: ...})` + `reply.close()` |
| Main thread `mailbox.cancel()` | cancel_event.set() → spawned agent can check cancel signal |
| Main thread process exits | Daemon threads die with it |

---

## Chinese Aliases

All keywords and methods have Chinese aliases:

| English | Chinese |
|---------|---------|
| `spawn` | `分生` |
| `mailbox.send()` | `邮箱.发送()` |
| `mailbox.receive()` | `邮箱.接收()` |
| `mailbox.try_receive()` | `邮箱.尝试接收()` |
| `mailbox.cancel()` | `邮箱.取消()` |
| `mailbox.close()` | `邮箱.关闭()` |
| `mailbox_select([...])` | `邮箱选择([...])` |

Chinese example:

```helen
智能体 工作者(任务: str, 回复: 消息通道) {
    主函 {
        回复.发送("完成: " + 任务)
    }
}

主函 {
    设 邮箱 = 分生 工作者("数据分析")
    设 结果 = 邮箱.接收()
    打印(结果)
}
```

---

## Comparison with Old async/await

| Old async/await | New spawn |
|-----------------|-----------|
| `let t = async Agent(...)` | `let m = spawn Agent(...)` |
| `result = await t` | `result = m.receive()` |
| `await [t1, t2]` | `[m1.receive(), m2.receive()]` |
| `detach Agent(...)` | `spawn Agent(...)` (ignore return value) |

**Why the replacement?**
- `async/await` was backed by a thread pool, with no essential difference from `threading.Thread`
- spawn + channel covers all scenarios and is better for streaming, cancellation, and racing patterns
- One concurrency primitive is clearer than two

---

## Important Notes

| Rule | Description |
|------|-------------|
| `spawn` argument | Must be an agent call |
| Last parameter | Must be `Channel` type (auto-injected by spawn) |
| Return type | `Channel` (main thread endpoint) |
| Isolation | Snapshot deep-copies everything, no exceptions |
| Sharing | Explicitly pass references through channels |
| Daemon threads | Spawned agents run in daemon threads |

---

## Spawn Transcript Management (v1.23.7+)

Each spawned agent has its own transcript session. v1.23.7 introduces spawn relationship tracking and cascading management.

### Querying Spawn Relationships

```helen
import std.core.*
import std.transcript.*
main {
    // Get all direct child sessions of the current session
    let children = get_spawned_sessions()
    for child in children {
        print("Spawned: " + child["session_id"])
        print("  Agent: " + child["agent_name"])
    }

    // Get the full spawn tree (including nested spawns)
    let tree = get_spawn_tree()
    print("Root: " + tree["session_id"])
    for child in tree["children"] {
        print("  Child: " + child["session_id"])
    }
}
```

### Aggregate View

```helen
import std.core.*
import std.transcript.*
main {
    // View the main session + all spawned complete execution flow
    let all_messages = replay_full_session()
    for msg in all_messages {
        print("[" + msg["session_id"] + "] " + msg["role"] + ": " + msg["content"][:50])
    }

    // Cross-spawn search
    let errors = search_transcript("error", include_spawned=true)
}
```

### Cascading Deletion

When deleting a session, all spawned child sessions are cascade-deleted by default to avoid orphan transcripts:

```helen
import std.transcript.*
main {
    // Delete session and all its spawns (default)
    delete_session("session_abc123")

    // Delete only the specified session, keep spawns
    delete_session("session_abc123", cascade=false)

    // Clean up old sessions (cascade-deletes spawns)
    cleanup_sessions(keep_count=10)  // Keep the most recent 10, cascade-delete spawns
}
```

**Design rationale**:
- Spawns are child tasks; their lifecycle should be bound to the main session
- Avoids orphan transcripts (spawns lose context after the main session is deleted)
- Simplifies cleanup — no need to manually find and delete all spawns

> **Detailed docs**: See the "Transcript Functions" section in `10-stdlib.md`

---

## Session Recovery and Debugging (v1.24+)

### Recovering Sessions at Startup

v1.24 supports specifying a historical session to recover at startup, convenient for debugging and continuing work:

```bash
# Recover a specific session
helen --session=session_xxx file.helen
helen repl --session=session_xxx

# Automatically recover the most recent session
helen --resume-latest file.helen
helen repl --resume-latest
helen repl -r  # Shorthand
```

### Python API Session Recovery

```python
from helen.interpreter import Interpreter

# Recover a specific session
interp = Interpreter(session_id="session_xxx")

# Recover the most recent session
from helen.runtime.session_manager import SessionManager
manager = SessionManager()
sessions = manager.list_sessions()
if sessions:
    latest_sid = sessions[0]["session_id"]
    interp = Interpreter(session_id=latest_sid)
```

### Debugging Spawn Execution Flow

Combining session recovery and spawn tracking, you can fully debug multi-agent collaboration:

```bash
# 1. Run the program, note the session_id
helen my_agent.helen
# Output: Current session: session_abc123

# 2. Recover the session, view the full spawn tree
helen --session=session_abc123 debug.helen
```

```helen
// debug.helen: Analyze the previous execution flow
import std.core.*
import std.transcript.*
main {
    // View spawn tree
    let tree = get_spawn_tree()
    print("Spawn tree:")
    for child in tree["children"] {
        print("  - " + child["session_id"])
    }

    // Aggregate view of all messages
    let all = replay_full_session()
    for msg in all {
        print("[" + msg["session_id"] + "] " + msg["role"] + ": " + msg["content"][:50])
    }
}
```

---

## Resuming a Spawned Session (v1.27+)

By default, `spawn` starts the child agent in a **fresh** transcript session. v1.27 adds an optional `resume("<session_id>")` clause so a spawned agent **continues a previously saved child-session conversation** instead of starting from scratch:

```helen
import std.context.*
import std.core.*
agent Worker(task: str, reply: Channel) {
    main {
        // The resumed transcript is already loaded; the LLM "remembers"
        // the previous conversation. Only program-state variables reset.
        let history = search_context(task)
        reply.send({"matches": history["total_matches"]})
        reply.close()
    }
}

main {
    // Save the child session_id your application got from a prior run:
    let saved_child_sid = "session_1783492628_d9d9c0aa"

    // Resume it: the spawned agent loads that transcript's history.
    let mb = spawn Worker("deploy") resume(saved_child_sid)
    let r = mb.receive()
    print(r["matches"])
}
```

Chinese alias: `恢复会话("<session_id>")`.

```helen
import std.transcript.*
main {
    设 邮箱 = 分生 工作者("部署") 恢复会话(已存子会话id)

}```

### How it works

| Aspect | Behavior |
|--------|----------|
| `resume("<sid>")` clause | Optional; parsed as an identifier clause (not a keyword), so the bilingual keyword count stays at 89 |
| Existing session | The spawned `Interpreter` is constructed with `session_id=<sid>` and **loads** that transcript's messages (true resumption, continues appending to the same file) |
| Non-existent session | A fresh session is created with that id (graceful fallback, no crash) |
| Argument type | Must evaluate to a `str`; any other type raises a runtime error |
| Locking | A cross-process lock is acquired on resume to prevent two processes from corrupting the same transcript (see below) |
| LLM-visible context | Resumed messages are auto-exposed to each new agent invocation, so `llm act` inside agent `main {}` sees the prior conversation (not just the transcript file). New messages created during the run still respect per-invocation isolation. |

### Resume vs. `resume_session()` vs. startup `--session`

Helen offers three distinct mechanisms - pick by intent:

| Mechanism | Where | What it does |
|-----------|-------|--------------|
| `spawn A(...) resume("sid")` | spawn site (v1.27) | The spawned agent **is** session `sid`; continues appending to its transcript |
| `Interpreter(session_id="sid")` / `helen --session=sid` | startup | The main interpreter **is** session `sid`; continues its transcript |
| `resume_session("sid")` | runtime stdlib | Imports `sid`'s messages **into the current new** session (copy-on-resume, not true continuation) |

Use `resume(...)` (spawn) or `--session` (startup) when you want to **continue** a session. Use `resume_session()` when you want to **reference** another session's history from a new one.

> **v1.27 context restore**: both `spawn ... resume("sid")` and `Interpreter(session_id="sid")` not only continue the transcript file but also make the prior conversation visible to `llm act` inside agent `main {}`. At each invocation entry, the new invocation_id is added to every resumed message's `visible_to_invocation_ids`, bypassing the per-invocation isolation (v1.22) for resumed history only. Messages created during the new run are **not** tagged, so they stay scoped to their own invocation - agent B does not see agent A's new messages, but both see the resumed history.

### Important: transcript ≠ program state

Resuming a transcript restores the **LLM's conversation memory** (what was said and done) - the LLM sees the prior exchange in `llm act`. It does **not** restore runtime variable state - task lists, SharedStore contents, and in-memory caches reset to their initial values. In practice the LLM can re-derive state by reading the transcript, but for true stateful continuation, persist critical state separately (e.g. to `.helen/agent_state.json`) and reload it on startup.

### Cross-process locking

When a session is resumed, Helen writes `<session_dir>/<session_id>/session.lock` containing the current PID. If another **live** process already holds that session, resume is refused with a clear error rather than corrupting the transcript:

```
E0351 at <unknown>: Cannot resume session 'session_xxx': it is locked by
another running process (PID 12345). Stop that process first, or start a
fresh session.
```

Stale locks (holder PID no longer alive) and same-process reuse (e.g. spawning a sibling that resumes an earlier child session) are reclaimed automatically. The lock guards **cross-process** conflicts only.

### End-to-end example: pause and resume a long-running agent

```helen
// Run 1: do work, then persist the child session_id for next time.
import std.context.*
import std.core.*
import std.io.*
import std.transcript.*
agent Worker(reply: Channel) {
    main {
        insert_message("user", "checkpoint: phase 1 done, secret=BANANA_42")
        reply.send(get_session_id())   // report child session_id
        reply.close()
    }
}

main {
    let mb = spawn Worker()
    let child_sid = mb.receive()
    write_file(".helen/last_child_session", child_sid)   // persist for next run
}

// Run 2 (later): resume the child session and continue.
main {
    let child_sid = read_file(".helen/last_child_session")
    let mb = spawn Worker() resume(child_sid)
    // The spawned agent's transcript already contains the checkpoint message.
}
```

---

## Exercises

1. Create two concurrent agents handling different tasks, use `mailbox.receive()` to get results
2. Implement a racing pattern: two agents solve the same problem, use `mailbox_select` to take the fastest result
3. Implement bidirectional communication: spawned agent receives instructions and returns computation results
4. Use shared store + channel to implement an explicitly-shared counter

---

**Last updated**: 2026-07-26  
**Version**: v1.27
