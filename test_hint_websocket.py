#!/usr/bin/env python3
"""
Test script to simulate WebSocket communication with Helen agent.
Simulates: user message → tool call → hint injection → observe LLM response
"""

import websocket
import json
import time
import sys
from typing import Optional

WS_URL = "ws://localhost:8001/api/chat/ws"

def on_message(ws, message):
    """Handle incoming WebSocket messages."""
    try:
        data = json.loads(message)
        msg_type = data.get("type", "unknown")
        
        if msg_type == "status_update":
            print(f"[STATUS] CWD: {data.get('data', {}).get('cwd', '?')}")
        
        elif msg_type == "processing_start":
            print(f"[PROCESSING] Started")
        
        elif msg_type == "llm_chunk":
            content = data.get("data", {}).get("content", "")
            print(content, end="", flush=True)
        
        elif msg_type == "llm_complete":
            print()  # New line after streaming
            print(f"[LLM] Complete")
        
        elif msg_type == "processing_complete":
            is_slash = data.get("data", {}).get("is_slash_response", False)
            content = data.get("data", {}).get("content", "")
            print(f"[COMPLETE] is_slash={is_slash}")
            if content:
                print(f"  Content: {content[:100]}")
        
        elif msg_type == "tool_call":
            tool_name = data.get("data", {}).get("name", "?")
            tool_args = data.get("data", {}).get("arguments", {})
            print(f"[TOOL] {tool_name}({json.dumps(tool_args, ensure_ascii=False)})")
        
        elif msg_type == "tool_result":
            tool_name = data.get("data", {}).get("name", "?")
            result = data.get("data", {}).get("result", "")
            print(f"[TOOL RESULT] {tool_name}: {result[:100]}...")
        
        elif msg_type == "hint_queued":
            content = data.get("data", {}).get("content", "")
            print(f"[HINT] Queued: {content}")
        
        elif msg_type == "error":
            error_msg = data.get("data", {}).get("content", "Unknown error")
            print(f"[ERROR] {error_msg}")
        
        else:
            print(f"[{msg_type.upper()}] {json.dumps(data, ensure_ascii=False)[:100]}")
    
    except Exception as e:
        print(f"[PARSE ERROR] {e}: {message[:100]}")

def on_error(ws, error):
    """Handle WebSocket errors."""
    print(f"[WS ERROR] {error}")

def on_close(ws, close_status_code, close_msg):
    """Handle WebSocket close."""
    print(f"[WS CLOSED] code={close_status_code}, msg={close_msg}")

def on_open(ws):
    """Handle WebSocket open."""
    print(f"[WS CONNECTED] to {WS_URL}")
    
    # Step 1: Send user message
    print("\n" + "=" * 60)
    print("STEP 1: Sending user message")
    print("=" * 60)
    user_msg = {
        "type": "message",
        "content": "今天的AI新闻",
        "client_id": "test-client"
    }
    ws.send(json.dumps(user_msg))
    print(f"Sent: {user_msg['content']}")
    
    # Wait for tool calls (simulate waiting for LLM to request tools)
    print("\nWaiting for tool calls... (will send hint after 5 seconds)")
    time.sleep(5)
    
    # Step 2: Send hint message
    print("\n" + "=" * 60)
    print("STEP 2: Sending hint message")
    print("=" * 60)
    hint_msg = {
        "type": "hint",
        "content": "改搜财经新闻",
        "client_id": "test-client"
    }
    ws.send(json.dumps(hint_msg))
    print(f"Sent hint: {hint_msg['content']}")
    
    # Wait for response
    print("\nWaiting for LLM response...")
    time.sleep(10)
    
    # Close connection
    print("\n" + "=" * 60)
    print("Closing connection")
    print("=" * 60)
    ws.close()

def main():
    """Main test function."""
    print("Testing hint injection via WebSocket...")
    print(f"Connecting to: {WS_URL}")
    print()
    
    # Enable WebSocket trace for debugging
    # websocket.enableTrace(True)
    
    ws = websocket.WebSocketApp(
        WS_URL,
        on_open=on_open,
        on_message=on_message,
        on_error=on_error,
        on_close=on_close
    )
    
    # Run WebSocket connection
    ws.run_forever()

if __name__ == "__main__":
    main()
