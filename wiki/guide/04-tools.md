# Chapter 4: Equipping Agents with Tools

## What Are Tools?

Tools are the agent's "hands and eyes." An agent without tools can only answer questions using its "brain" (the LLM's built-in knowledge). With tools, an agent can:

- Read and write files
- Execute commands
- Search the web
- Call APIs
- Perform math calculations

Tools are the key mechanism by which Helen realizes its core principle: "prompts can freely invoke tools."

## Declaring Tools

Inside an agent, declare available tools with `tools` (`工具`):

```helen
import std.core.*

agent FileAssistant {                                  // agent = 智能体
    prompt "You are a file management assistant.       // prompt = 提示词
            You can read and write files to help users."
    tools = ["read_file", "write_file", "list_dir"]    // tools = 工具

    main {                                             // main = 主函
        return llm act "List all filenames in the current directory and tell me what files are there"
        // return = 返回, llm = 大模型, act = 执行
    }
}
```

Once you have declared tools, `llm act` lets the LLM automatically decide which tool to call and when. You don't need to orchestrate tool calls by hand.

## How Tool Invocation Works

Let's walk through a complete example to understand the full tool-calling loop:

```helen
agent WeatherQuery {
    prompt "You are a weather assistant. Use tools to fetch weather information."
    tools = ["web_search", "web_fetch"]

    main {
        return llm act "Will it rain in Beijing tomorrow?"
    }
}
```

Execution flow:

```
User message: "Will it rain in Beijing tomorrow?"
    ↓
LLM thinks: I need to search for weather info
    ↓
Tool call: web_search("Beijing tomorrow weather forecast")
    ↓
Tool returns: "Beijing tomorrow: sunny turning cloudy, 25-32°C..."
    ↓
LLM thinks: Got the search results, time to compose the answer
    ↓
Final answer: "It won't rain in Beijing tomorrow — sunny turning cloudy, 25 to 32 degrees."
```

This process is fully automatic. The LLM may invoke tools multiple times (e.g., search first, then fetch a page). The `max_turns` setting caps how many rounds are allowed.

## Common Built-in Tools

Helen ships with a variety of built-in tools you can reference directly in `tools`:

### File Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents |
| `write_file` | Write a file |
| `list_dir` | List directory contents |
| `path_exists` | Check whether a path exists |

### Web Tools

| Tool | Description |
|------|-------------|
| `web_search` | Search the web |
| `web_fetch` | Fetch page contents |
| `http_get` | Send an HTTP GET request |
| `http_post` | Send an HTTP POST request |

### System Tools

| Tool | Description |
|------|-------------|
| `shell_exec` | Execute a shell command |
| `calculate` | Perform math calculations |
| `patch_file` | Edit a file (precise replacement) |

### Knowledge Tools

| Tool | Description |
|------|-------------|
| `load_skill` | Load a skill document |
| `find_files` | Search for files |

## Custom Tools (Agent Functions)

Beyond built-in tools, you can define your own functions inside the agent's `functions` (`函数区`) block. **After defining a function, you still need to list its name in `tools` for the LLM to see and call it:**

```helen
import std.core.*
import std.crypto.*

agent PasswordAssistant {                                // agent = 智能体
    prompt "You are a password security assistant.       // prompt = 提示词
            You can help users generate and check passwords."
    tools = ["generate_password", "check_strength"]      // tools = 工具

    functions {                                          // functions = 函数区
        fn generate_password(length: int): str {         // fn = 函数, return = 返回
            let charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*"
            return random_string(length)                 // uses a stdlib function
        }

        fn check_strength(password: str): str {
            let score = 0
            if len(password) >= 8 { score = score + 1 }  // if = 如果, len = 长度
            if len(password) >= 12 { score = score + 1 }
            // ... more checks
            if score >= 3 { return "strong" }
            if score >= 2 { return "medium" }
            return "weak"
        }
    }

    main {                                               // main = 主函
        return llm act "Generate a 16-character password for me and check its strength"
        // return = 返回, llm act = 大模型 执行
    }
}
```

> **Key point**: Functions defined in `functions` can only be called by the LLM if their names are listed in `tools`. `tools` acts as a whitelist for the LLM — functions not on the list are invisible to it. However, Helen code in `main` can call any function in `functions` freely, unrestricted by `tools`.

## Tool Callbacks (Advanced)

Sometimes you want to inject your own logic before or after a tool runs. Helen provides the `on_tool_end` callback for this:

```helen
import std.core.*

agent AssistantWithCallback {
    prompt "You are an assistant."
    tools = ["read_file"]

    functions {
        fn on_tool_end_cb(tool_name: str, result: str): str? {   // fn = 函数, null = 空
            if tool_name == "read_file" {                        // if = 如果
                print("File read, length: " + str(len(result)))  // print = 打印, str = 字符串化, len = 长度
                return "File contents acquired, please analyze the following:"  // inject hint for the LLM
            }
            return null                                          // returning null means no extra hint
        }
    }

    main {
        return llm act "Read config.json and analyze it" on_tool_end on_tool_end_cb
    }
}
```

`on_tool_end` fires after every tool invocation. You can use it to:
- Log tool calls
- Inspect tool results
- Inject extra hints for the LLM
- Return `null` to skip any extra processing

## Principle of Least Privilege

**Give an agent only the tools it needs** — don't grant it more permissions than necessary. This is both a security concern and an accuracy improvement: the fewer tools available, the less likely the LLM is to pick the wrong one.

```helen
// ✅ Good: only the tools actually needed
agent ReadOnlyAnalysis {
    tools = ["read_file"]  // only needs to read files
    main { return llm act "Analyze this file" }
}

// ❌ Bad: over-privileged
agent OverAuthorized {
    tools = ["read_file", "write_file", "shell_exec", "web_search"]
    // Only needs read_file, but was granted write and shell execution too
    main { return llm act "Analyze this file" }
}
```

## Managing Tool Lists with Constants

When multiple agents share the same tool set, factor the common configuration into a `const`:

```helen
// Defined at module scope
const FILE_TOOLS = ["read_file", "write_file", "path_exists"]     // const = 常量
const WEB_TOOLS = ["web_search", "web_fetch"]
const SYSTEM_TOOLS = ["shell_exec", "calculate"]

agent FileProcessor {
    tools = FILE_TOOLS
    main { return llm act "Process the file" }
}

agent WebCrawler {
    tools = FILE_TOOLS + WEB_TOOLS    // compose tool sets
    main { return llm act "Crawl web content and save it" }
}
```

## MCP Tools (Advanced)

Helen v1.33+ supports external tools via MCP (Model Context Protocol). Create a `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "database": {
      "command": "npx",
      "args": ["-y", "@example/db-mcp"]
    }
  }
}
```

After configuration, MCP tools are auto-discovered and can be used just like built-in tools:

```helen
agent DatabaseAssistant {
    tools = ["query_database", "read_file"]
    main { return llm act "Query data from the users table" }
}
```

Tool precedence: built-in tools > agent functions > MCP tools.

## Chapter Summary

- Tools let agents read/write files, search the web, and execute commands
- Declare available tools in an agent with `tools = [...]`
- Functions defined in `functions` must be listed in `tools` to be callable by the LLM
- The LLM automatically decides which tool to call and when
- Follow the principle of least privilege — grant only necessary tools
- The `on_tool_end` callback lets you inject custom logic after tool execution
- MCP tools extend the tool ecosystem

## Further Reading

- [[reference/05-agents|Agent Programming]] - The `tools` declaration field and the `functions {}` block (LLM-callable tools defined as Helen functions)
- [[reference/11-building-agents|Building Multi-Agent Systems]] - Complete case study of a tool-equipped multi-agent system
- [[reference/18-helen-agent|Helen Programming Agent]] - The built-in coding assistant's tool architecture

## Next Chapter

-> [Chapter 5: Variables and Data Types](05-basics.md)
