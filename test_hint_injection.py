#!/usr/bin/env python3
"""
Test script to verify hint injection works with LLM.
This simulates the tool call loop and injects a hint mid-conversation.
"""

import os
import sys
import json
import requests
from typing import List, Dict, Any

# Load API config from environment
API_KEY = os.environ.get("OPENAI_API_KEY", "")
API_BASE = os.environ.get("OPENAI_API_BASE", "https://api.openai.com/v1")
MODEL = os.environ.get("OPENAI_MODEL", "gpt-4o-mini")

if not API_KEY:
    print("ERROR: OPENAI_API_KEY not set")
    sys.exit(1)

def call_llm(messages: List[Dict[str, Any]], tools: List[Dict[str, Any]] = None) -> Dict[str, Any]:
    """Call LLM API with messages and optional tools."""
    headers = {
        "Authorization": f"Bearer {API_KEY}",
        "Content-Type": "application/json",
    }
    
    payload = {
        "model": MODEL,
        "messages": messages,
        "temperature": 0.7,
    }
    
    if tools:
        payload["tools"] = tools
        payload["tool_choice"] = "auto"
    
    response = requests.post(
        f"{API_BASE}/chat/completions",
        headers=headers,
        json=payload,
        timeout=60
    )
    
    if response.status_code != 200:
        print(f"ERROR: API call failed with status {response.status_code}")
        print(response.text)
        sys.exit(1)
    
    return response.json()["choices"][0]["message"]

def simulate_tool_call_with_hint():
    """Simulate a tool call scenario with hint injection."""
    
    # Define a simple web_search tool
    tools = [
        {
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web for information",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query"
                        }
                    },
                    "required": ["query"]
                }
            }
        }
    ]
    
    # Initial messages
    messages = [
        {
            "role": "system",
            "content": "You are a helpful assistant. Use the web_search tool when you need to find information."
        },
        {
            "role": "user",
            "content": "今天的AI新闻"
        }
    ]
    
    print("=" * 60)
    print("STEP 1: Initial LLM call (should request web_search)")
    print("=" * 60)
    print(f"Messages: {json.dumps(messages, ensure_ascii=False, indent=2)}")
    
    response1 = call_llm(messages, tools)
    print(f"\nLLM Response: {json.dumps(response1, ensure_ascii=False, indent=2)}")
    
    # Check if LLM wants to call web_search
    if "tool_calls" not in response1:
        print("\nERROR: LLM did not request tool call")
        return
    
    # Simulate tool execution
    tool_call = response1["tool_calls"][0]
    fn_name = tool_call["function"]["name"]
    fn_args = json.loads(tool_call["function"]["arguments"])
    
    print(f"\n{'=' * 60}")
    print(f"STEP 2: Simulating tool execution: {fn_name}({fn_args})")
    print("=" * 60)
    
    # Add assistant message with tool call
    messages.append(response1)
    
    # Simulate tool result
    tool_result = {
        "role": "tool",
        "tool_call_id": tool_call["id"],
        "content": "Found AI news: OpenAI released GPT-5, Anthropic launched Claude 4..."
    }
    messages.append(tool_result)
    
    print(f"Tool result: {tool_result['content']}")
    
    # NOW INJECT HINT (this is what should happen in Rust code)
    print(f"\n{'=' * 60}")
    print("STEP 3: Injecting hint message")
    print("=" * 60)
    
    hint_text = "改搜财经新闻"
    hint_message = {
        "role": "user",
        "content": f"[System Hint — after tool '{fn_name}']\n{hint_text}"
    }
    messages.append(hint_message)
    
    print(f"Injected hint: {hint_message['content']}")
    print(f"\nMessages after hint injection:")
    for i, msg in enumerate(messages):
        role = msg.get("role", "?")
        content = msg.get("content", "<tool_call>")
        if len(content) > 100:
            content = content[:100] + "..."
        print(f"  [{i}] role={role}, content={content}")
    
    # Call LLM again with hint injected
    print(f"\n{'=' * 60}")
    print("STEP 4: Second LLM call (should respond to hint)")
    print("=" * 60)
    
    response2 = call_llm(messages, tools)
    print(f"\nLLM Response: {json.dumps(response2, ensure_ascii=False, indent=2)}")
    
    # Check if LLM responded to hint
    if "tool_calls" in response2:
        tool_call2 = response2["tool_calls"][0]
        fn_args2 = json.loads(tool_call2["function"]["arguments"])
        print(f"\n{'=' * 60}")
        print("RESULT: LLM requested another tool call")
        print("=" * 60)
        print(f"Tool: {tool_call2['function']['name']}")
        print(f"Args: {fn_args2}")
        
        if "财经" in fn_args2.get("query", ""):
            print("\n✅ SUCCESS: LLM responded to hint and changed search query!")
        else:
            print("\n❌ FAILURE: LLM did not respond to hint")
    else:
        content = response2.get("content", "")
        print(f"\n{'=' * 60}")
        print("RESULT: LLM generated final response")
        print("=" * 60)
        print(f"Content: {content}")
        
        if "财经" in content:
            print("\n✅ SUCCESS: LLM responded to hint and changed topic!")
        else:
            print("\n❌ FAILURE: LLM did not respond to hint")

if __name__ == "__main__":
    print("Testing hint injection with LLM...")
    print(f"API Base: {API_BASE}")
    print(f"Model: {MODEL}")
    print()
    
    simulate_tool_call_with_hint()
