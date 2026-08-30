#!/usr/bin/env python3
"""
Comprehensive WebUI test for tool call display functionality.
Tests that tool calls and results are properly streamed to the frontend.
"""

import asyncio
import websockets
import json
import sys

URI = "ws://localhost:8024/api/chat/ws"


async def recv_until_complete(ws, timeout=30.0):
    """Receive messages until llm_complete or processing_complete."""
    chunks = []
    tool_calls = []
    tool_results = []

    while True:
        msg = await asyncio.wait_for(ws.recv(), timeout=timeout)
        data = json.loads(msg)
        msg_type = data.get("type")

        if msg_type == "llm_chunk":
            content = data.get("data", {}).get("content", "")
            chunks.append(content)
            if "🔧 Calling" in content:
                tool_calls.append(content)
            if "✅" in content and "returned:" in content:
                tool_results.append(content)
        elif msg_type in ("llm_complete", "processing_complete"):
            break
        elif msg_type == "error":
            raise RuntimeError(f"Server error: {data}")

    return chunks, tool_calls, tool_results


async def test_tool_display():
    """Test 1: Single tool call is displayed."""
    print("=" * 60)
    print("🧪 Test 1: Single Tool Call Display")
    print("=" * 60)

    async with websockets.connect(URI) as ws:
        # status_update
        msg = await ws.recv()
        data = json.loads(msg)
        assert data["type"] == "status_update"

        await ws.send(json.dumps({
            "type": "message",
            "content": "Use the calculate tool to compute 2+2"
        }))
        print("📤 Sent: Use the calculate tool to compute 2+2")

        chunks, tool_calls, tool_results = await recv_until_complete(ws)

        print(f"  Chunks: {len(chunks)}, Tool calls: {len(tool_calls)}, Tool results: {len(tool_results)}")
        for tc in tool_calls:
            print(f"  🔧 {tc.strip()[:80]}")
        for tr in tool_results:
            print(f"  ✅ {tr.strip()[:80]}")

        ok = len(tool_calls) >= 1 and len(tool_results) >= 1
        print(f"{'✅ PASS' if ok else '❌ FAIL'}")
        return ok


async def test_multiple_tools():
    """Test 2: Multiple tool calls in sequence."""
    print("\n" + "=" * 60)
    print("🧪 Test 2: Multiple Tool Calls")
    print("=" * 60)

    async with websockets.connect(URI) as ws:
        msg = await ws.recv()
        data = json.loads(msg)
        assert data["type"] == "status_update"

        await ws.send(json.dumps({
            "type": "message",
            "content": "Calculate 10+5, then 20*3, then 100/4"
        }))
        print("📤 Sent: Calculate 10+5, then 20*3, then 100/4")

        chunks, tool_calls, tool_results = await recv_until_complete(ws)

        print(f"  Chunks: {len(chunks)}, Tool calls: {len(tool_calls)}, Tool results: {len(tool_results)}")
        for tc in tool_calls:
            print(f"  🔧 {tc.strip()[:80]}")
        for tr in tool_results:
            print(f"  ✅ {tr.strip()[:80]}")

        ok = len(tool_calls) >= 3 and len(tool_results) >= 3
        print(f"{'✅ PASS' if ok else '❌ FAIL'}")
        return ok


async def test_no_tool_call():
    """Test 3: Normal message without tool calls."""
    print("\n" + "=" * 60)
    print("🧪 Test 3: Normal Message (No Tools)")
    print("=" * 60)

    async with websockets.connect(URI) as ws:
        msg = await ws.recv()
        data = json.loads(msg)
        assert data["type"] == "status_update"

        await ws.send(json.dumps({
            "type": "message",
            "content": "Hello, how are you?"
        }))
        print("📤 Sent: Hello, how are you?")

        chunks, tool_calls, tool_results = await recv_until_complete(ws)
        full_text = "".join(chunks)

        print(f"  Chunks: {len(chunks)}, Tool calls: {len(tool_calls)}")
        print(f"  Response: {full_text[:100]}...")

        ok = len(tool_calls) == 0 and len(chunks) > 0
        print(f"{'✅ PASS' if ok else '❌ FAIL'}")
        return ok


async def main():
    print("=" * 60)
    print("🚀 WebUI Tool Display Test Suite")
    print("=" * 60)

    results = []
    results.append(await test_tool_display())
    results.append(await test_multiple_tools())
    results.append(await test_no_tool_call())

    print("\n" + "=" * 60)
    print("📊 Final Summary")
    print("=" * 60)
    passed = sum(results)
    total = len(results)
    print(f"Tests passed: {passed}/{total}")

    if passed == total:
        print("\n🎉 All tests passed!")
        return 0
    else:
        print(f"\n⚠️  {total - passed} test(s) failed")
        return 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
