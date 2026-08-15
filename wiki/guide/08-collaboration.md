# Chapter 8: Agent Collaboration

## Why Collaborate?

A single agent has limited capability. Just like human society, complex tasks often require multiple specialized roles working together:

- Requirements analyst → Architect → Developer → Tester
- Data collection → Data cleaning → Data analysis → Report generation
- Multiple competing solutions → Select the best one

Helen makes multi-agent collaboration simple and natural.

## Core Principle: Caller Decides Context

In Helen, agents are **strictly isolated**:

> **Each agent is independent. It cannot see the caller's variables, conversation history, or environment.**

If an agent needs some information, you must **pass it explicitly**.

```
Caller ──parameters──► Agent input
       ──shared state──► Shared data
       ◄──return value─ Agent output
```

### Why Isolate?

Imagine you ask a "code review agent" to review code. If it could access all of the caller's variables, it might accidentally modify data or be distracted by unrelated variables. Isolation keeps each agent focused on its own task, free from outside interference.

### How to Pass Information?

| Method | Use case | Example |
|--------|----------|---------|
| Parameters | One-off input | `Worker(task)` |
| `const` | Read-only config | `const MAX = 100` |
| `shared let` | Cross-agent counter | `shared let count = 0` |
| `shared store` | Complex shared state | `shared store TaskRegistry { ... }` |
| Channel | `spawn` message passing | `mailbox.send(result)` |

### Keyword Quick Reference (English ↔ Chinese)

The code examples in this chapter use English keywords. Each Helen keyword has a Chinese equivalent that means the same thing; inline `// (中文: ...)` comments mark the Chinese form on its first appearance.

| English | Chinese | Notes |
|---------|---------|-------|
| `agent` | `智能体` | Agent declaration |
| `description` | `描述` | Agent description |
| `tools` | `工具` | Tool list |
| `prompt` | `提示词` | Agent system prompt |
| `functions` | `函数区` | Function block inside agent |
| `main` | `主函` | Entry block |
| `fn` | `函数` | Function declaration |
| `return` | `返回` | Function return |
| `let` | `定义` | Mutable variable |
| `const` | `常量` | Constant |
| `shared` | `共享` | Cross-agent visible |
| `store` | `仓库` | Shared store declaration |
| `if` | `如果` | Conditional branch |
| `for ... in` | `对于 ... 属于` | Loop |
| `llm act` | `大模型 执行` | LLM autonomous execution |
| `spawn` | `分生` | Launch concurrent agent |
| `null` | `空` | Null value |
| `print` (stdlib) | `打印` | Print to stdout |
| `str` (stdlib) | `字符串化` | Convert to string |
| `len` (stdlib) | `长度` | Length |
| `time` (stdlib) | `当前时间戳` | Unix timestamp |
| `shell_exec` (stdlib) | `执行命令` | Execute shell command |
| `now` (stdlib) | `当前时间` | Current datetime |

## Pattern 1: Sequential Chain

Multiple agents execute in sequence — each agent's output becomes the next agent's input:

```helen
import std.core.*

// Agent 1: Requirements analysis
agent Analyst(requirement: str) {   // agent = 智能体
    description "Analyze user requirements"   // description = 描述
    main { return llm act "Analyze this requirement: " + requirement }   // main = 主函; llm act = 大模型 执行; return = 返回
}

// Agent 2: Solution design
agent Architect(analysis: str) {
    description "Design technical solution"
    main { return llm act "Design a solution based on this analysis: " + analysis }
}

// Agent 3: Write code
agent Developer(plan: str) {
    description "Write code"
    main { return llm act "Write code according to this plan: " + plan }
}

// Orchestrator: chain the whole pipeline
agent ProjectManager(requirement: str) {
    description "Project orchestrator"

    functions {   // functions = 函数区
        fn execute_project(requirement: str): str {   // fn = 函数
            // Sequential calls — each output feeds the next input
            let analysis = Analyst(requirement)   // let = 定义
            print("✅ Requirements analysis complete")   // print = 打印

            let plan = Architect(analysis)
            print("✅ Solution design complete")

            let code = Developer(plan)
            print("✅ Code writing complete")

            return code
        }
    }

    main {
        return execute_project(requirement)
    }
}

main {
    let result = ProjectManager("Build a todo app")
    print(result)
}
```

**Best for**: Tasks with clearly defined stages, where each stage depends on the previous stage's output.

## Pattern 2: Parallel Fan-out

Multiple agents execute different tasks simultaneously, then results are aggregated:

```helen
import std.core.*

agent FileAnalyzer(file_path: str) {
    description "Analyze a single file"
    main { return llm act "Analyze file: " + file_path }
}

agent ResultAggregator(results: list) {
    description "Aggregate analysis results"
    main { return llm act "Aggregate these analysis results: " + str(results) }
}

agent ParallelOrchestrator(file_list: list) {
    description "Analyze multiple files in parallel"

    functions {
        fn analyze_parallel(paths: list): str {
            // spawn starts concurrent agents, returns a Channel
            let mailboxes = []
            for path in paths {
                let mb = spawn FileAnalyzer(path)
                mailboxes.append(mb)
            }

            // Collect all results
            let results = []
            for mb in mailboxes {
                results.append(mb.receive())
            }

            // Aggregate
            return ResultAggregator(results)
        }
    }

    main {
        return analyze_parallel(file_list)
    }
}

main {
    let files = ["a.py", "b.py", "c.py"]
    let summary = ParallelOrchestrator(files)
    print(summary)
}
```

**Best for**: Multiple independent subtasks that can run concurrently.

## Pattern 3: Pipeline

Multiple agents form an assembly line — data flows through each stage like water:

```helen
import std.core.*

agent DataCollection(source: str) {
    description "Collect data"
    tools = ["web_search", "web_fetch"]   // tools = 工具
    main { return llm act "Collect data from {{source}}" }
}

agent DataCleaning(raw_data: str) {
    description "Clean data"
    main { return llm act "Clean this data: " + raw_data }
}

agent DataAnalysis(clean_data: str) {
    description "Analyze data"
    main { return llm act "Analyze this data: " + clean_data }
}

agent ReportGeneration(analysis_result: str) {
    description "Generate report"
    main { return llm act "Generate a report based on the analysis: " + analysis_result }
}

// Pipeline orchestration
agent DataPipeline(data_source: str) {
    main {
        // Like an assembly line, data flows through four stages
        let collected = DataCollection(data_source)
        let cleaned = DataCleaning(collected)
        let analyzed = DataAnalysis(cleaned)
        let final_report = ReportGeneration(analyzed)
        return final_report
    }
}
```

## Pattern 4: Competition — Best Wins

Multiple agents solve the same problem with different strategies, and the best one is chosen:

```helen
import std.core.*

shared let best_solution = null   // shared = 共享; null = 空
shared let best_score = 0

agent SolutionGenerator(problem: str, strategy: str) {
    description "Generate solution using specified strategy"

    main {
        let solution = llm act "Use " + strategy + " strategy to solve: " + problem
        let score = llm act "Score this solution (0-100): " + solution

        if score > best_score {   // if = 如果
            best_score = score
            best_solution = solution
        }

        return solution
    }
}

agent SolutionSelector(problem: str) {
    main {
        let strategies = ["divide_and_conquer", "dynamic_programming", "greedy", "backtracking"]
        let mailboxes = []

        // Try multiple strategies in parallel
        for strategy in strategies {   // for ... in = 对于 ... 属于
            let mb = spawn SolutionGenerator(problem, strategy)   // spawn = 分生
            mailboxes.append(mb)
        }

        // Wait for all to finish
        for mb in mailboxes {
            mb.receive()
        }

        return best_solution
    }
}
```

## spawn and Channel

`spawn` starts a concurrently running agent and returns a Channel. Use the Channel to send and receive messages:

```helen
import std.core.*

agent Sender(output: Channel) {
    main {
        output.send("Hello!")
        output.send("Second message")
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
    // spawn returns a Channel
    let mb = spawn Sender(null)
    let result = mb.receive()  // Wait for and receive the result
    print(result)
}
```

### Channel Core Methods

| Method | Description |
|--------|-------------|
| `channel.send(message)` | Send a message |
| `channel.receive()` | Receive a message (blocks until available) |
| `mailbox_select([mb1, mb2])` | Multi-way select: handle whichever arrives first |

## Shared State

### shared let: Simple Shared Variables

```helen
import std.core.*

shared let completion_count = 0  // Visible and modifiable by all agents

agent Worker(task: str) {
    main {
        completion_count = completion_count + 1
        return "Completed: " + task
    }
}

main {
    Worker("task1")
    Worker("task2")
    print(completion_count)  // 2
}
```

> **Limitation**: `shared let` only supports value types (`int`, `float`, `str`, `bool`). It does not support lists or maps.

### shared store: Complex Shared State

When you need to share complex data structures, use `shared store`:

```helen
import std.core.*

shared store TaskRegistry {   // shared store = 共享 仓库
    task_map: map = {}
    counter: int = 0

    fn register(name: str, data: any) {
        counter = counter + 1
        task_map[name] = data
    }

    fn get(name: str): any {
        return task_map[name]
    }

    fn total(): int {
        return len(task_map)
    }
}

agent Producer(registry: TaskRegistry) {
    main {
        registry.register("task1", {status: "pending"})
        return "registered"
    }
}

agent Consumer(registry: TaskRegistry) {
    main {
        let task = registry.get("task1")
        return task
    }
}

main {
    let registry = TaskRegistry
    spawn Producer(registry)
    spawn Consumer(registry)
}
```

### const: Read-Only Sharing

`const` constants are automatically visible (read-only) in all agents:

```helen
import std.core.*

const MAX_RETRIES = 3   // const = 常量; visible to all agents
const TIMEOUT_SECONDS = 30

agent Worker {
    main {
        print(MAX_RETRIES)  // ✅ Accessible
        return llm act "Working"
    }
}
```

## Passing Real Information to Downstream Agents

When an orchestrator calls downstream agents, it should pass along the real information it has gathered:

```helen
import std.system.*
import std.time.*

agent Orchestrator(task: str) {
    main {
        // Gather real information at the orchestrator level
        let cwd = shell_exec("pwd")   // shell_exec = 执行命令
        let current_time = time()   // time = 当前时间戳

        // Pass real information to the downstream agent
        return Worker(task, cwd, current_time)
    }
}

agent Worker(task: str, work_dir: str, current_time: str) {
    prompt """   // prompt = 提示词
    Task: {{task}}
    Working directory: {{work_dir}}
    Current time: {{current_time}}
    """
    main { return llm act }
}
```

> **Principle**: Whoever holds the information is responsible for passing it down. Don't ask a downstream agent to guess what it doesn't know.

## Chapter Summary

- Agents are strictly isolated — all information must be passed explicitly
- **Sequential chain**: each agent's output is the next agent's input
- **Parallel fan-out**: `spawn` for concurrency, `Channel` for collecting results
- **Pipeline**: data flows through multiple processing stages
- **Competition**: multiple strategies run in parallel; pick the best
- `shared let` for sharing simple values, `shared store` for sharing complex state
- `const` constants are automatically visible in all agents
- Passing real information downstream is the orchestrator's responsibility

## Further Reading

- [[reference/11-building-agents|Building Multi-Agent Systems]] - Complete multi-agent case study with context flow diagrams
- [[reference/07-spawn|Concurrent Programming]] - `spawn` + Channel reference: `send`/`receive`/`try_receive`/`cancel`/`close`, `mailbox_select`, `spawn resume` (v1.27), cross-process session lock
- [[reference/05-agents|Agent Programming]] - Shared store (`shared store`) and isolation levels (`@open`/`@strict`/`@sandbox`)

## Next Chapter

[Chapter 9: Standard Library at a Glance](09-stdlib.md) ->
