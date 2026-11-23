#!/usr/bin/env python3
"""
Simple WebSocket test for Helen agent hint injection.
Uses synchronous websocket-client for easier debugging.
"""

import websocket
import json
import time
import sys

WS_URL = "ws://localhost:8001/api/chat/ws"

def main():
    print(f"Connecting to {WS_URL}...")
    
    # Create WebSocket connection
    ws = websocket.create_connection(WS_URL, timeout=30)
    print("Connected!")
    
    # Receive initial status update
    try:
        msg = ws.recv()
        data = json.loads(msg)
        print(f"[RECV] {data.get('type')}: {json.dumps(data, ensure_ascii=False)[:100]}")
    except Exception as e:
        print(f"[ERROR] Failed to receive initial message: {e}")
        return
    
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
    print(f"[SENT] {user_msg['content']}")
    
    # Receive messages for 5 seconds
    print("\nReceiving messages for 5 seconds...")
    start_time = time.time()
    tool_call_seen = False
    
    while time.time() - start_time < 5:
        try:
            ws.settimeout(1.0)
            msg = ws.recv()
            data = json.loads(msg)
            msg_type = data.get("type", "unknown")
            
            if msg_type == "processing_start":
                print(f"[RECV] processing_start")
            elif msg_type == "llm_chunk":
                content = data.get("data", {}).get("content", "")
                print(content, end="", flush=True)
            elif msg_type == "llm_complete":
                print()
                print(f"[RECV] llm_complete")
            elif msg_type == "processing_complete":
                print(f"[RECV] processing_complete")
            elif msg_type == "tool_call":
                tool_name = data.get("data", {}).get("name", "?")
                print(f"[RECV] tool_call: {tool_name}")
                tool_call_seen = True
            elif msg_type == "tool_result":
                tool_name = data.get("data", {}).get("name", "?")
                print(f"[RECV] tool_result: {tool_name}")
            elif msg_type == "error":
                error_msg = data.get("data", {}).get("content", "Unknown")
                print(f"[RECV] ERROR: {error_msg}")
            else:
                print(f"[RECV] {msg_type}: {json.dumps(data, ensure_ascii=False)[:100]}")
        except websocket.WebSocketTimeoutException:
            continue
        except Exception as e:
            print(f"[ERROR] {e}")
            break
    
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
    print(f"[SENT] hint: {hint_msg['content']}")
    
    # Receive messages for 10 seconds
    print("\nReceiving messages for 10 seconds...")
    start_time = time.time()
    
    while time.time() - start_time < 10:
        try:
            ws.settimeout(1.0)
            msg = ws.recv()
            data = json.loads(msg)
            msg_type = data.get("type", "unknown")
            
            if msg_type == "llm_chunk":
                content = data.get("data", {}).get("content", "")
                print(content, end="", flush=True)
            elif msg_type == "hint_queued":
                content = data.get("data", {}).get("content", "")
                print(f"\n[RECV] hint_queued: {content}")
            elif msg_type == "processing_complete":
                print(f"\n[RECV] processing_complete")
            elif msg_type == "error":
                error_msg = data.get("data", {}).get("content", "Unknown")
                print(f"\n[RECV] ERROR: {error_msg}")
            else:
                print(f"\n[RECV] {msg_type}: {json.dumps(data, ensure_ascii=False)[:100]}")
        except websocket.WebSocketTimeoutException:
            continue
        except Exception as e:
            print(f"\n[ERROR] {e}")
            break
    
    print("\n" + "=" * 60)
    print("Test complete")
    print("=" * 60)
    
    ws.close()

if __name__ == "__main__":
    main()
