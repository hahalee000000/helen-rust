---
name: helen-agent-collaboration
description: "Helen Agent Collaboration Patterns — Multi-agent collaboration, orchestration, task division, data flow, shared state, scope isolation"
version: 1.19.0
author: Helen Team
license: MIT
tags: [helen, agent, collaboration, orchestration, workflow, multi-agent, shared-let, scope-isolation, v1.12, read-only-params, ground-truth-injection, v1.17, spawn, channel, v1.18]
---
<!-- helen-rust edition: shared let / Channel / SharedStore in helen-runtime + helen-interpreter (M7). -->


# Helen Agent Collaboration Patterns

This skill describes patterns and best practices for multi-agent collaboration in the Helen language (v1.18 update: spawn + Channel replaces async/await).

## Core Concepts

> 💡 **For single-agent design patterns, scope isolation, and context management, see `helen-agent-patterns`**

### 🎯 Caller Decides Context

Agents are strictly isolated — each call/spawn creates a brand-new execution environment and **does not automatically inherit** the caller's variables, history, or LLM context. Before calling, you must explicitly consider what context to provide:

```
Caller ──parameters──► Agent Input
       ──SharedStore──► Shared State
       ◄──return value/Channel── Agent Output
```

```helen
// ❌ Wrong: assuming module variables are auto-visible
import std.core.*
let user_name = "Alice"
agent Greeter { main { print("Hello " + user_name) } }  // Compile error!

// ✅ Right: pass explicitly
agent Greeter(user_name: str) { main { print("Hello " + user_name) } }
main { Greeter("Alice") }
```

| Scenario | Recommended Approach |
|----------|---------------------|
| One-off input | Parameters `Agent(x, y)` |
| Read-only config | `const` (auto-visible) |
| Cross-agent mutable state | `shared store` |
| Spawned sub-agent output | Channel `ch.send(result)` |
| Resume a saved child session (true continuation) | `spawn A(...) resume("sid")` (v1.27) |
| Import another session's history into current | `resume_session(sid)` |

> **v1.27 spawn resume**: `spawn Agent(...) resume("<session_id>")` (Chinese `恢复会话(...)`) makes the spawned agent continue a previously saved child-session transcript instead of starting fresh. This is **true resumption** - the spawned interpreter appends to that session's transcript. Use it to pause/resume long-running multi-agent workflows: persist the child session_id, then on the next run `spawn ... resume(saved_sid)`. Resuming restores LLM conversation memory, not runtime variable state; persist critical state separately. A cross-process lock prevents two processes from corrupting the same transcript. See [Tutorial 07: spawn › Resuming a Spawned Session](../../../../wiki/reference/07-spawn.md#resuming-a-spawned-session-v127).

## Collaboration Patterns

### Pattern 1: Sequential Chain

Multiple agents execute in sequence; each agent's output becomes the next agent's input.

```helen
import std.core.*
import std.io.*
agent WorkflowOrchestrator(requirement: str) {
    description "Workflow orchestrator - sequential chain pattern"
    prompt """
    Requirement: {{requirement}}

    Workflow:
    1. Call ContractDesigner to design the interface
    2. Call TestBuilder to generate tests
    3. Call Implementer to write the implementation
    4. Call QualityChecker to assess quality
    """

    functions {
        fn run_workflow(req: str): map {
            // Step 1: Contract design
            let contract = ContractDesigner(req)
            print("✅ Step 1: Contract design complete")

            // Step 2: Test generation
            let tests = TestBuilder(contract)
            write_file("tests/generated.helen", tests)
            print("✅ Step 2: Test generation complete")

            // Step 3: Implementation
            let impl = Implementer(contract, tests)
            write_file("src/implementation.helen", impl)
            print("✅ Step 3: Implementation complete")

            // Step 4: Quality assessment
            let quality = QualityChecker("src/implementation.helen")
            print("✅ Step 4: Quality assessment complete")

            return {
                "contract": contract,
                "tests": tests,
                "implementation": impl,
                "quality": quality
            }
        }
    }

    main {
        let result = run_workflow(requirement)
        print("Workflow complete")
    }
}
```

**When to use**:
- The task has clearly defined phases
- Each phase depends on the previous phase's output
- Strict execution order is required

### Pattern 2: Parallel Fan-out

Call multiple agents concurrently to process different sub-tasks, then aggregate the results.

```helen
// v1.12: Results passed via return values, not shared let
import std.core.*
shared let completed_count = 0

agent CodeAnalyzer(path: str) {
    description "Analyze a code file"
    main {
        completed_count = completed_count + 1
        return { "path": path, "status": "ok", "issues": 0 }
    }
}

agent ResultSummarizer(results: list) {
    description "Summarize results"
    main {
        return { "total": len(results), "status": "done" }
    }
}

agent ParallelOrchestrator(file_paths: list) {
    description "Parallel orchestrator - fan-out pattern"
    prompt """
    File list: {{file_paths}}

    Analyze each file in parallel, then aggregate the results.
    """

    functions {
        fn analyze_files_parallel(paths: list): map {
            // Launch parallel analysis (v1.18: spawn)
            let mailboxes = []
            for path in paths {
                let mailbox = spawn CodeAnalyzer(path)
                mailboxes.append(mailbox)
            }

            // Collect results one by one
            let results = []
            for mailbox in mailboxes {
                results.append(mailbox.receive())
            }

            // Summarize results
            let summary = ResultSummarizer(results)
            return {
                "individual": results,
                "summary": summary
            }
        }
    }

    main {
        let result = analyze_files_parallel(file_paths)
        print("Analysis complete, " + str(len(result["individual"])) + " files total")
        print("Completed count: " + str(completed_count))
    }
}
```

**When to use**:
- Multiple independent sub-tasks can run in parallel
- Results from multiple agents need to be aggregated
- Higher throughput is desired

### Pattern 3: Pipeline

Multiple agents form a processing pipeline; each stage handles a specific aspect.

```helen
// v1.12: Using value-type counters to track progress
import std.core.*
shared let pipeline_stage = 0

agent DataCollector(source: str) {
    description "Stage 1: Data collection"
    tools = ["web_search", "web_fetch"]

    main {
        let raw_data = llm act "Collect data from: " + source
        pipeline_stage = 1
        print("✅ Data collection complete")
        return raw_data  // Pass via return value
    }
}

agent DataCleaner(data: str) {
    description "Stage 2: Data cleaning"

    main {
        let cleaned = llm act "Clean this data: " + data
        pipeline_stage = 2
        print("✅ Data cleaning complete")
        return cleaned
    }
}

agent DataAnalyzer(data: str) {
    description "Stage 3: Data analysis"

    main {
        let analysis = llm act "Analyze: " + data
        pipeline_stage = 3
        print("✅ Data analysis complete")
        return analysis
    }
}

agent DataReporter(analysis: str) {
    description "Stage 4: Report generation"

    main {
        let report = llm act "Generate report from: " + analysis
        pipeline_stage = 4
        print("✅ Report generation complete")
        return report
    }
}

// Pipeline execution
agent DataPipeline(source: str) {
    description "Complete data processing pipeline"

    main {
        let raw = DataCollector(source)
        let cleaned = DataCleaner(raw)
        let analysis = DataAnalyzer(cleaned)
        let report = DataReporter(analysis)

        print("Pipeline execution complete")
        return report
    }
}
```

### Pattern 4: Router

Route input to different specialized agents based on content.

```helen
agent TechSupport(query: str) {
    description "Technical support"
    prompt "You are a technical support expert."
    main {
        return llm act "Answer the technical question: " + query
    }
}

agent BillingSupport(query: str) {
    description "Billing support"
    prompt "You are a billing support expert."
    main {
        return llm act "Answer the billing question: " + query
    }
}

agent SalesSupport(query: str) {
    description "Sales support"
    prompt "You are a sales consultant."
    main {
        return llm act "Answer the sales question: " + query
    }
}

// Router agent
agent SupportRouter(query: str) {
    description "Intelligent router for customer queries"

    functions {
        fn classify(query: str): str {
            // Use LLM to classify
            let category = llm act "Classify query into: tech, billing, sales. Query: " + query
            return category
        }
    }

    main {
        let category = classify(query)

        if category == "tech" {
            return TechSupport(query)
        } else if category == "billing" {
            return BillingSupport(query)
        } else if category == "sales" {
            return SalesSupport(query)
        } else {
            return llm act "Generic reply: " + query
        }
    }
}
```

### Pattern 5: Hierarchical

A lead agent coordinates multiple sub-agents; sub-agents can further decompose tasks.

```helen
// v1.12: Using value types to track state
shared let project_phase = "init"
shared let project_progress = 0

agent ProjectManager(requirement: str) {
    description "Project manager - overall coordination"

    main {
        project_phase = "planning"

        // Phase 1: Requirement analysis
        let analysis = RequirementAnalyst(requirement)
        project_progress = 25

        // Phase 2: Architecture design
        let architecture = Architect(analysis)
        project_progress = 50

        // Phase 3: Parallel development (v1.18: spawn + Channel)
        let frontend_mb = spawn FrontendDev(architecture)
        let backend_mb = spawn BackendDev(architecture)
        let dev_results = [frontend_mb.receive(), backend_mb.receive()]
        project_progress = 80

        // Phase 4: Integration testing
        let integration = IntegrationTester(dev_results)
        project_progress = 100
        project_phase = "complete"

        return integration
    }
}

agent RequirementAnalyst(req: str) {
    description "Requirement analyst"
    main {
        return llm act "Analyze requirements: " + req
    }
}

agent Architect(analysis: str) {
    description "Architect"
    main {
        return llm act "Design architecture: " + analysis
    }
}

agent FrontendDev(arch: str) {
    description "Frontend developer"
    tools = ["write_file"]
    main {
        return llm act "Implement frontend: " + arch
    }
}

agent BackendDev(arch: str) {
    description "Backend developer"
    tools = ["write_file"]
    main {
        return llm act "Implement backend: " + arch
    }
}

agent IntegrationTester(results: list) {
    description "Integration tester"
    main {
        return llm act "Run integration tests"
    }
}
```

### Pattern 6: Compete & Select

Multiple agents compete to solve the same problem; select the best result.

```helen
shared let best_solution = null
shared let best_score = 0

agent SolutionGenerator(problem: str, strategy: str) {
    description "Generate a solution"

    main {
        let solution = llm act "Solve using " + strategy + " strategy: " + problem

        // Self-evaluation
        let score = llm act "Evaluate solution quality (0-100): " + solution

        // Update best solution (needs concurrency safety)
        if score > best_score {
            best_score = score
            best_solution = {
                "strategy": strategy,
                "solution": solution,
                "score": score
            }
        }

        return solution
    }
}

agent SolutionSelector(problem: str) {
    description "Compete to select the best solution"

    main {
        // Parallel competition with multiple strategies (v1.18: spawn)
        let strategies = ["divide-and-conquer", "dynamic-programming", "greedy", "backtracking"]
        let mailboxes = []

        for strategy in strategies {
            let mailbox = spawn SolutionGenerator(problem, strategy)
            mailboxes.append(mailbox)
        }

        // Collect all results
        for mailbox in mailboxes {
            mailbox.receive()
        }

        // Return the best solution
        return best_solution
    }
}
```

## Shared State Best Practices

> 💡 For detailed examples and anti-patterns, see `helen-agent-patterns` § Scope Isolation / § Best Practice 6

How to choose the right sharing mechanism for collaboration:

| Mechanism | When to Use | Constraints |
|-----------|-------------|-------------|
| `shared let` | Cross-agent value-type counters/flags | v1.12: only int/float/str/bool |
| `const` | Read-only config (auto-visible in agents) | Immutable |
| Parameter passing | Reference types (list/dict) | Auto-wrapped as read-only; use `list(x)` to create a mutable copy |
| `shared store` | Complex mutable shared state | RLock thread-safe, `_` prefix for private fields |
| Channel | Message/result passing between agents | spawn returns a Channel |

**Key rules**: `shared let` forbids reference types; module-level `let` is not visible inside agent main (compile error).

Shared Store quick example:

```helen
import std.core.*
import std.dict.*
shared store TaskRegistry {
    tasks: dict = {}
    _counter: int = 0
    fn register(name: str, data: any) { _counter += 1; tasks[name] = data }
    fn get(name: str): any { return tasks[name] }
    fn size(): int { return len(tasks) }
}

agent Producer(r: TaskRegistry) { main { r.register("t1", {status: "pending"}) } }
agent Consumer(r: TaskRegistry) { main { let t = r.get("t1") } }
main { let r = TaskRegistry; spawn Producer(r); spawn Consumer(r) }
```

### Using Channel Message Queues for Inter-Agent Communication (v1.18)

v1.18 introduces `spawn` + Channel message queues, replacing the old async/await concurrency model:

```helen
// spawn returns a Channel (mailbox)
import std.core.*
agent Sender(output: Channel) {
    main {
        output.send("Hello from sender")
        output.send("Another message")
        return "done"
    }
}

agent Receiver(input: Channel) {
    main {
        let msg1 = input.receive()
        let msg2 = input.receive()
        print("Received: " + msg1 + ", " + msg2)
        return msg1 + msg2
    }
}

main {
    // Create channel to connect sender and receiver
    // Launch concurrent agent via spawn
    let mb1 = spawn Sender(null)
    let result = mb1.receive()
}
```

**v1.18 Channel Message Queue vs Shared Store**:

| Scenario | Recommendation |
|----------|---------------|
| Multiple agents reading/writing the same state | Shared Store |
| Passing messages/task results between agents | spawn + Channel |
| Multiplexed selection | mailbox_select([m1, m2, ...]) |
| Thread-safe field access required | Shared Store |

### Propagating Ground Truth to Downstream Agents (v1.17)

The most common orchestrator mistake: **holding ground-truth facts but not injecting them into the prompt for downstream agents**. The LLM cannot see the runtime environment — whatever is missing, it will fabricate.

```helen
// ✅ Orchestrator resolves ground truth once, fans out via {{}}
import std.time.*
import std.tools.*
agent Orchestrator(task: str) {
    main {
        // Cross-platform (v1.30.7+): use get_cwd() from utils.helen
        let cwd = get_cwd()
        let git_branch = shell_exec("git branch --show-current")
        let now = now()
        return Worker(task, cwd, git_branch, now)
    }
}

agent Worker(task: str, cwd: str, branch: str, now: str) {
    prompt """
    Task: {{task}}
    Working directory: {{cwd}}
    Git branch: {{branch}}
    Current time: {{now}}
    """
    main { return llm act }
}
```

> **Cross-platform note (v1.30.7+):** `get_cwd()` is defined in `utils.helen` and works on Windows, macOS, and Linux. It reads `HELEN_WEBUI_CWD` env var first, then falls back to `cd` (Windows) or `pwd` (Unix). Avoid `shell_exec("pwd")` directly — it fails on Windows.

Principle: **Whoever owns the facts is responsible for injecting them**. A shared `shared store` works for mutable state, but immutable facts like time/OS/path are cheapest and least error-prone passed directly into the prompt via `{{}}`. See **helen-agent-patterns § Best Practice 7** for details.

## Error Handling & Performance

**Concurrent error handling**: Combine spawn + Channel with try/catch AggregateError:

```helen
import std.core.*
agent RobustOrchestrator(tasks: list) {
    main {
        let mailboxes = []
        for task in tasks { mailboxes.append(spawn TaskWorker(task)) }
        try {
            let results = []
            for mb in mailboxes { results.append(mb.receive()) }
            return results
        } catch AggregateError e {
            print("Some tasks failed: " + str(len(e.errors)))
            return []  // Handle the failure case
        }
    }
}
```

**Performance tips**:
- **Batched concurrency**: `for i in range(0, len(items), MAX_CONCURRENT)` to control how many agents to spawn per batch
- **Caching**: Pass a cache map via parameters; return the updated copy; use `shared let` to track hit statistics

## Related Skills

- **helen-agent-patterns** — Detailed agent design patterns
- **helen-syntax** — Helen language syntax (shared let, const, agent main, etc.)
- **subagent-driven-development** — Sub-agent-driven development workflow

## Further Reading

- **[[The Complete Guide to Agent Prompt Engineering]]** (`wiki/reference/agent-system-prompt-guide.md`) — Agent prompt design methodology; orchestrator agents especially should follow "principles over procedures" and "inject ground-truth facts".
