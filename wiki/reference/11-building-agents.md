# Tutorial 11: Building Multi-Agent Systems

> Complete case study: from requirements to implementation

---

## ⚠️ Design Principle: Caller Decides Context

Before diving into the case study, keep the core design principle of Helen's multi-agent system in mind:

> **Agents are strictly isolated — you must explicitly consider what context to provide to an agent before calling it.**

The most common mistake in multi-agent systems is **assuming agents will automatically see outer variables or history**. In Helen, this never happens.

When designing multi-agent systems, start by drawing a **context flow diagram**:

```
[Caller]
  │
  ├─── Arguments ──────────► [Inputs Agent A needs]
  │
  ├─── SharedStore reference ► [Shared state Agent A needs to access]
  │
  └─── Channel ─────────────► [Where Agent A's output goes]
```

**Checklist**:
- What is the **minimum** information each agent needs to complete its task?
- Is all this information explicitly passed through arguments or shared state?
- Is context that different agents **shouldn't** know about actually isolated?

> 💡 See [Tutorial 05: Core Design Principles](05-agents.md#core-design-principle-caller-decides-context)

---

## Agent Configuration Reference

Before building agents, understand the configuration options available in agent declarations.

**Note:** Configuration elements support two equivalent syntaxes — with or without `=`. Both `description "..."` and `description = "..."` are valid. The `=` is optional for all configuration elements except `prompt`, which always uses `prompt "..."` (literal string template).

```helen
agent MyAgent {
    // === Identity ===
    description "What this agent does"  // Shown to LLM as system context
    model "gpt-4"                        // LLM model to use (optional, uses default)
    
    // === Behavior ===
    temperature 0.7     // Sampling temperature (0.0-2.0)
                        // Lower = more deterministic, higher = more creative
    max-turns 10        // Max LLM interaction turns for tool-calling loops
    max-tokens 4096     // Max output tokens per response (v1.31.2)
                        // Controls response length limit
                        // Default: API provider's default (usually 4096)
                        // Set higher for long-form content (e.g., 131072 for 128k)
    
    // === Tools ===
    tools ["read_file", "write_file"]  // List of tools agent can use
    // or
    tools = MY_TOOLS_CONST              // Reference to const list
    
    // === Transcript ===
    transcript "persistent"  // "none" (default), "memory", or "persistent"
    
    // === Context Management ===
    context {
        compression "graduated"      // "none", "traditional", or "graduated"
        cache-aware true             // Enable cache-aware compression
        working-memory true          // Enable working memory tracking
        working-memory-tokens 5000   // Token budget for working memory
    }
    
    // === Prompt Template ===
    prompt """
    You are a helpful assistant.
    {{user_input}}
    """
    
    main {
        // Agent logic
    }
}
```

### Configuration Guidelines

| Setting | Purpose | When to Use | Example Values |
|---------|---------|-------------|----------------|
| `description` | Agent role description | Always provide context to LLM | "Senior code reviewer" |
| `model` | LLM model selection | Override default model | "gpt-4", "qwen3.7-plus" |
| `temperature` | Control creativity vs determinism | Balance creativity/precision | 0.1 (factual), 0.7 (balanced), 1.2 (creative) |
| `max-turns` | Limit tool-calling iterations | Prevent infinite loops | 1 (single response), 10 (moderate), 50 (complex tasks) |
| `max-tokens` | Control response length | Limit output size | 100 (short), 4096 (default), 131072 (long-form) |
| `streaming` | Enable streaming response | Real-time output, long content | true, false |
| `transcript` | Control conversation persistence | Session management, debugging | "none" (ephemeral), "memory" (session), "persistent" (disk) |
| `tools` | LLM-visible tool whitelist | Control agent capabilities | ["read_file", "web_search"] |
| `thinking-mode` | Enable reasoning mode | Complex reasoning tasks | true, false |
| `reasoning-effort` | Control reasoning depth | Balance speed/quality | "low", "medium", "high", "max" |
| `provider` | Explicit provider override | Multi-provider setups | "openai", "anthropic" |

**Note**: All configuration elements support optional `=` syntax (e.g., both `description "..."` and `description = "..."` are valid), except `prompt` which always uses `prompt "..."` for template interpolation.

**Runtime adjustment (v1.39.8)**: Use stdlib functions to override these settings at runtime without changing prompt content (preserves cache):

```helen
import std.llm.*
set_temperature(0.3)       // Override agent's default
set_max_turns(10)
set_thinking_mode(true)
```

---

## Case Study: Intelligent Customer Service System

### Requirements

Build an intelligent customer service system that can:
1. Understand user questions
2. Classify question types
3. Route to different specialized Agents based on type
4. Generate satisfactory responses

---

## Step 1: Define Agents

```helen
// Question classifier
agent QuestionClassifier {
    description "Classify customer questions into categories"
    model "gpt-4"
    temperature 0.1
    prompt """
    Classify the question into one of:
    - product: Questions about products or features
    - billing: Questions about pricing, invoices, payments
    - technical: Technical issues, bugs, errors
    - account: Account management, login, settings
    - general: Everything else
    """
}

// Product expert
agent ProductExpert {
    description "Answer product-related questions"
    model "gpt-4"
    temperature 0.3
    prompt """
    You are a product expert. Answer questions about our products
    clearly and helpfully. If unsure, say so honestly.
    """
}

// Billing expert
agent BillingExpert {
    description "Handle billing inquiries"
    model "gpt-4"
    temperature 0.1
    prompt """
    You are a billing expert. Help customers with pricing, invoices,
    and payment issues. Be precise with numbers.
    """
}

// Technical support
agent TechSupport {
    description "Provide technical support"
    model "gpt-4"
    temperature 0.2
    prompt """
    You are a technical support engineer. Help users resolve technical
    issues step by step. Ask clarifying questions if needed.
    """
}

// Response polisher
agent ResponsePolisher {
    description "Polish responses to be friendly and professional"
    temperature 0.5
    prompt """
    Rewrite the response to be warm, professional, and helpful.
    Keep the technical accuracy but improve the tone.
    """
}
```

---

## Step 2: Implement Routing Logic

```helen
import std.core.*
main {
    let customer_question = "How do I reset my password?"

    // Step 1: Classify
    llm if "Classify customer question" {
        branch "product" {
            print("📦 Product question")
            let answer = ProductExpert(customer_question)
        }
        branch "billing" {
            print("💰 Billing question")
            let answer = BillingExpert(customer_question)
        }
        branch "technical" {
            print("🔧 Technical question")
            let answer = TechSupport(customer_question)
        }
        branch "account" {
            print("👤 Account question")
            let answer = TechSupport(customer_question)
        }
        default {
            print("📋 General question")
            let answer = "Thank you for your question. Let me help you."
        }
    }

    // Step 3: Polish the response
    let polished = ResponsePolisher(answer)

    // Step 4: Output
    print("\n--- Response to Customer ---")
    print(polished)
}
```

---

## Step 3: Add Concurrency Optimization

```helen
// Knowledge base query agent — receives reply Channel to return results
import std.core.*
agent KnowledgeBase(query: str, reply: Channel) {
    description "Search knowledge base"
    prompt "Search knowledge base for: {{query}}"
    main {
        let result = llm act "Search knowledge base: " + query
        reply.send(result)
    }
}

// History lookup agent — receives reply Channel to return results
agent HistoryLookup(topic: str, reply: Channel) {
    description "Lookup relevant history"
    prompt "Find relevant history for: {{topic}}"
    main {
        let result = llm act "Find relevant history: " + topic
        reply.send(result)
    }
}

// Optimized version: concurrent knowledge base queries (v1.18+ spawn pattern)
main {
    let question = "How do I reset my password?"

    // Concurrently fetch context: spawn returns Channel
    let kb_mailbox = spawn KnowledgeBase(question)
    let history_mailbox = spawn HistoryLookup("password reset")

    // Receive results from Channels
    let kb_result = kb_mailbox.receive()
    let history_result = history_mailbox.receive()
    let full_context = kb_result + "\n" + history_result

    // Classify first (serial, needs result for routing)
    llm if "Classify customer question" {
        branch "technical" {
            let answer = TechSupport(question + "\nContext: " + full_context)
        }
        default {
            let answer = "I'll help you with that."
        }
    }

    let polished = ResponsePolisher(answer)
    print(polished)
}
```

---

## Step 4: Add Error Handling

```helen
import std.core.*
main {
    let question = "How do I reset my password?"

    try {
        llm if "Classify customer question" {
            branch "technical" {
                let answer = TechSupport(question)
                let polished = ResponsePolisher(answer)
                print(polished)
            }
            default {
                print("I'll help you with that.")
            }
        }
    } catch TimeoutError err {
        print("⏱️ The service is taking too long. Please try again.")
    } catch RuntimeError err {
        print("⚠️ Something went wrong: " + str(err))
        print("A human agent will contact you shortly.")
    } catch {
        print("❌ An unexpected error occurred.")
        print("Please try again or contact support@company.com")
    }
}
```

---

## Step 5: Optimize Context Management (v1.15+)

Helen v1.15 introduces comprehensive context management enhancements, configurable independently for each agent:

```helen
// Technical support agent: optimized context management
agent TechSupport {
    description "Provide technical support"
    model "gpt-4"
    
    // Context configuration
    context {
        compression "graduated"      // Graduated compression
        cache-aware true             // Cache-aware
        working-memory true          // Working memory
        working-memory-tokens 8000   // Larger working memory
    }
    
    tools ["read_file", "web_search"]
    
    prompt """
    You are a technical support engineer. Help users resolve technical
    issues step by step.
    """
}

// Product expert: simple context configuration
agent ProductExpert {
    description "Answer product questions"
    
    context {
        compression "none"           // No compression (short conversations)
        working-memory false         // Disable working memory
    }
    
    prompt """
    You are a product expert.
    """
}
```

### Context Management Best Practices

| Agent Type | Recommended Config | Description |
|-----------|-------------------|-------------|
| Research Agent | `compression "graduated"` + `working-memory true` | Long conversations, needs file tracking |
| Quick Response Agent | `compression "none"` + `working-memory false` | Short conversations, fast responses |
| Multi-turn Agent | `cache-aware true` + `working-memory-tokens 8000` | Improve cache hit rate |

---

## Step 6: Using Working Memory (v1.15+)

Working memory automatically tracks key information during agent execution:

```helen
// Helper function: fix code
import std.core.*
import std.io.*
fn fix_code(code: str): str {
    // Actual code repair logic
    return code  // Simplified example
}

agent CodeReviewer {
    description "Review code changes"
    
    context {
        working-memory true  // Auto-track file operations
    }
    
    tools ["read_file", "write_file", "patch_file"]
    
    functions {
        fn fix_code(code: str): str {
            // Actual code repair logic
            return code  // Simplified example
        }
    }
    
    main {
        // Auto-tracked: files read
        let code = read_file("src/main.py")
        
        // Auto-tracked: files modified
        let fixed = fix_code(code)
        write_file("src/main.py", fixed)
        
        // LLM now knows which files were modified
        return llm act "Review the changes"
        // Working memory contains:
        // - Active files: src/main.py
        // - Recent decisions: Modified src/main.py
    }
}
```

---

## Step 7: Monitoring Context Usage (v1.15+)

Use `:stats` in the REPL to view context usage:

```
> :stats
╔══════════════════════════════════════╗
║       Context Usage Statistics        ║
╠══════════════════════════════════════╣
║ ✅ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  12.3%            ║
║ Tokens:   15,984 /  131,072              ║
║ Model:  qwen3.7-plus                  ║
║ Messages: 8                           ║
║                                       ║
║ Working Memory:                       ║
║   Active Files: 3                     ║
║   Recent Decisions: 5                 ║
║   Pending TODOs: 2                    ║
║   Error History: 1                    ║
╚══════════════════════════════════════╝
```

---

## Step 8: Multi-Agent Collaboration Patterns (v1.18+)

In real applications, multiple Agents often need to **share state** or **communicate with each other**. Helen provides two mechanisms: `shared store` (shared state container) and **spawn + Channel** (message communication).

### Sharing State with Shared Store

Suppose our customer service system needs to track statistics across all sessions:

```helen
import std.core.*
shared store SessionStats {
    let totalSessions: int = 0
    let resolvedSessions: int = 0
    let activeSessions: list = []
    
    fn startSession(sessionId: str) {
        totalSessions = totalSessions + 1
        activeSessions.append(sessionId)
    }
    
    fn endSession(sessionId: str) {
        resolvedSessions = resolvedSessions + 1
        activeSessions.remove(sessionId)
    }
    
    fn getResolutionRate(): str {
        if (totalSessions == 0) {
            return "0%"
        }
        let rate = resolvedSessions * 100 / totalSessions
        return str(rate) + "%"
    }
}

// Agent receives reply Channel and sessionId parameters
agent CustomerService(sessionId: str, question: str, reply: Channel) {
    description "Handle customer session"
    main {
        SessionStats.startSession(sessionId)
        
        // Handle customer question...
        let response = llm act Assistant "Question: " + question
        
        SessionStats.endSession(sessionId)
        reply.send(response)
    }
}

main {
    // Multiple agents running concurrently (v1.18+ spawn pattern)
    let mb1 = spawn CustomerService("session-1", "How to reset password?")
    let mb2 = spawn CustomerService("session-2", "Billing issue")
    let mb3 = spawn CustomerService("session-3", "Technical support")

    // Wait for all sessions to complete
    let r1 = mb1.receive()
    let r2 = mb2.receive()
    let r3 = mb3.receive()

    print("Resolution rate: " + SessionStats.getResolutionRate())
}
```

### Passing Messages with Channels

Suppose we need a background task processing queue:

```helen
// Producer Agent: generates tasks and sends via Channel
import std.core.*
agent TaskProducer(reply: Channel) {
    description "Produce tasks"
    main {
        reply.send("send-email-1")
        reply.send("send-email-2")
        reply.send("send-email-3")
        reply.send("done")  // Completion signal
    }
}

// Consumer Agent: receives from Channel and processes tasks
agent TaskConsumer(task: str, reply: Channel) {
    description "Consume tasks"
    main {
        print("Processing: " + task)
        // Process the task...
        reply.send("completed: " + task)
    }
}

main {
    // Producer runs concurrently
    let producer_mb = spawn TaskProducer()

    // Consume all tasks
    let task = producer_mb.receive()
    while (task != "done") {
        let consumer_mb = spawn TaskConsumer(task)
        let result = consumer_mb.receive()
        print(result)
        task = producer_mb.receive()
    }
    producer_mb.close()
}
```

### Choosing a Collaboration Pattern

| Pattern | Suitable Scenario | Examples |
|---------|------------------|----------|
| **Shared Store** | Multiple Agents read/write the same data | Statistics counters, caches, configuration |
| **Channel (spawn)** | Passing messages/events between Agents | Task queues, result reporting, signals |

**Best practices**:
- ✅ Use `shared store` to manage **global state** (statistics, configuration, caches)
- ✅ Use `spawn` + Channel for **message communication** (queues, events, signals)
- ✅ Use `mailbox_select` to listen for results from multiple Channels
- ✅ Channel is auto-injected as the Agent's last parameter — no manual passing needed

---

## Project Structure

```
customer-service/
├── main.helen
├── agents/
│   ├── classifier.helen
│   ├── product_expert.helen
│   ├── billing_expert.helen
│   ├── tech_support.helen
│   └── polisher.helen
├── utils/
│   └── formatting.helen
└── config.json
```

---

## Running and Verification

```bash
# Verify
$ helen check customer-service/main.helen
✓ customer-service/main.helen: OK

# Run
$ helen customer-service/main.helen
🔧 Technical question


--- Response to Customer ---
To reset your password, please follow these steps...

# Generate documentation
$ helen doc customer-service/main.helen --format markdown
```

---

## Summary

Through this case study, you learned how to:
1. ✅ Declare multiple Agents with their configurations
2. ✅ Use `llm if` for intelligent routing
3. ✅ Use `spawn` + Channel to concurrently fetch context
4. ✅ Use `try-catch` to handle LLM exceptions
5. ✅ Organize multi-file project structures

---

## Next Steps

- Explore LSP completion and diagnostics in your IDE
- Use `helen repl` for rapid prototyping
- Read [[../reference/agent-system-prompt-guide|Agent Prompt Engineering Complete Guide]] — agent prompt design methodology reverse-engineered from Claude Code
- Read [[overview/design-philosophy|Design Philosophy]] to understand the language's design principles in depth
- Check [[appendix/error-codes|Error Code Reference]] for troubleshooting
