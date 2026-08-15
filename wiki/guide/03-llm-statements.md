# Chapter 3: Talking to LLMs

## `llm act`: Making the LLM Act

`llm act` (Chinese: `大模型 执行`) is the most important keyword in Helen. It hands a task off to the LLM, which interprets it based on the prompt, calls tools when necessary, and ultimately returns a result.

```helen
agent WeatherAssistant {
    // "提示" in Chinese source — same as `prompt`
    prompt "You are a weather assistant, helping users learn about the weather."
    main {
        return llm act "What's the weather like in Beijing today?"
    }
}
```

### The Full Form of `llm act`

```helen
llm act "user message" [options...]
```

Options include:
- `tools = [...]` — tools available for this call (overrides the agent's `tools` declaration)
- `on_chunk handler` — callback for streaming output
- `on_complete handler` — callback when the response finishes
- `media(image)` — multimodal input (images, etc.)

### `llm act` with Tools

```helen
import std.core.*

agent Researcher {
    prompt "You are a research assistant. Use tools to search for information and give accurate answers."
    tools = ["web_search", "web_fetch"]

    main {
        // The LLM decides on its own whether to call tools
        return llm act "Who won the 2024 Nobel Prize in Physics?"
    }
}
```

When `llm act` runs, the LLM:
1. Reads the prompt and the user message
2. Decides whether it needs to use tools
3. If so, calls tools to gather information
4. Organizes an answer based on the tool results
5. Returns the final result

This "think → call tool → think again → answer" loop happens automatically — you don't orchestrate it by hand.

## `llm if`: Letting the LLM Judge

Sometimes you don't need the LLM to write a paragraph — you just need it to make a **classification decision**. That's what `llm if` (Chinese: `大模型 如果`) is for:

```helen
import std.core.*

agent SentimentAnalysis(text: str) {
    prompt "You are a sentiment analysis expert."

    main {
        let sentiment = llm if text {
            case "positive" { return "positive" }
            case "negative" { return "negative" }
            case "neutral"  { return "neutral" }
            default         { return "unknown" }
        }
        return sentiment
    }
}

main {
    print(SentimentAnalysis("The weather is lovely today, I'm in a great mood!"))  // positive
    print(SentimentAnalysis("Terrible, I had to queue for everything."))           // negative
    print(SentimentAnalysis("Today is Wednesday."))                                // neutral
}
```

How `llm if` works:
1. The LLM reads the text and all the `case` labels
2. It decides which category the text belongs to
3. It executes the code in the matching branch

### `llm if` vs `llm act`

| Feature | `llm act` | `llm if` |
|---------|-----------|----------|
| Purpose | Have the LLM perform a task | Have the LLM make a classification |
| Returns | The LLM's text answer | The matching branch's return value |
| Calls tools | Yes | No |
| Typical use | Translation, writing, analysis | Sentiment analysis, intent detection, routing |

## Streaming Output

When you need to display the LLM's reply in real time (for example, in a chat UI), enable streaming output:

```helen
import std.core.*

agent StreamingAssistant {
    prompt "You are a chat assistant."
    streaming true

    functions {
        fn handleChunk(chunk: str) {
            // Output immediately as each piece of text arrives
            print(chunk)  // no newline, display character by character
        }
    }

    main {
        return llm act "Tell a short story" on_chunk handleChunk
    }
}
```

### Streaming Callbacks in Detail

```helen
llm act "Write a poem" on_chunk handleChunk on_complete finishCallback
```

- `on_chunk` — called every time a piece of text arrives, with the text chunk as argument
- `on_complete` — called once the entire response is finished

### Streaming Without Callbacks

If you just want to see the text appear character by character, the simplest way is to turn on `streaming true` in the agent configuration:

```helen
agent SimpleStream {
    prompt "You are an assistant."
    streaming true
    main { return llm act "Tell me a joke" }
}
```

## Multi-Turn Conversation

Within an agent, you can call `llm act` multiple times and the LLM will remember the previous conversation:

```helen
import std.core.*

agent MultiTurn {
    prompt "You are a chat assistant, remember the previous conversation."

    main {
        // First turn
        let reply1 = llm act "My name is Xiao Ming"
        print(reply1)

        // Second turn: the LLM remembers your name is Xiao Ming
        let reply2 = llm act "What is my name?"
        print(reply2)  // "Your name is Xiao Ming"

        return reply2
    }
}
```

> **Note**: This conversational memory only lives within a single agent invocation. Each new agent invocation starts fresh (see Chapter 11: Scope Isolation).

## Controlling LLM Behavior

### Limiting Output Length

```helen
agent ShortAnswer {
    prompt "Answer questions briefly."
    max-tokens 100  // output at most 100 tokens

    main {
        return llm act "Explain quantum mechanics"
    }
}
```

### Limiting Tool-Calling Rounds

```helen
agent LimitedTools {
    prompt "You are an assistant."
    tools = ["web_search", "web_fetch"]
    max-turns 3  // at most 3 rounds of tool calls, prevents infinite loops

    main {
        return llm act "Search for and summarize today's news"
    }
}
```

### Thinking Mode

Some models support a "thinking mode" where they reason before answering:

```helen
agent DeepThinker {
    prompt "You are a math teacher."
    thinking-mode true
    reasoning-effort "high"  // low / medium / high / max

    main {
        return llm act "Prove that the square root of 2 is irrational."
    }
}
```

## Using `llm` Outside an Agent

`llm act` can also be used directly inside a `main` block — you don't have to wrap it in an agent:

```helen
import std.core.*

main {
    // Call the LLM directly in main
    let answer = llm act "What is 1 + 1?"
    print(answer)
}
```

That said, for anything non-trivial, encapsulating logic inside an agent keeps your code cleaner and more reusable.

## Chapter Summary

- `llm act` (`大模型 执行`) — have the LLM perform a task; can call tools automatically
- `llm if` (`大模型 如果`) — have the LLM make a classification and take the matching branch
- Streaming output via `streaming true` or an `on_chunk` callback
- Multiple `llm act` calls inside the same agent invocation share conversational memory
- Control the LLM's behavior with `max-tokens`, `max-turns`, `thinking-mode`, and `reasoning-effort`

## Further Reading

- [[reference/06-llm-statements|LLM Statements]] - Complete `llm act` / `llm if` reference: all options, callback signatures (`on_chunk`, `on_complete`, `on_tool_end`), streaming protocol, truncation detection
- [[reference/14-observability|AI-Native Observability]] - LLM call auditing, `llm_log`, and trace tools

## Next Chapter

[Chapter 4: Equipping Agents with Tools](04-tools.md) ->
