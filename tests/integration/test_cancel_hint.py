#!/usr/bin/env python3
"""Integration test for cancel and hint functionality in helen-rust agent.

Simulates user interactions:
1. Send message that triggers tool calls
2. Click stop during inference
3. Send new message - should work correctly
4. Send hint during inference - should change the result

Usage:
    # Start helen-agent server first:
    cd /home/user/helen/helen-rust
    cargo run --release --bin helen-agent

    # Then run this test:
    python3 tests/integration/test_cancel_hint.py
"""

import asyncio
import json
import sys
import time
import websockets
from typing import List, Dict, Any, Optional
from dataclasses import dataclass, field


@dataclass
class WsMessage:
    """Represents a WebSocket message received from server."""
    type: str
    data: Dict[str, Any] = field(default_factory=dict)
    timestamp: float = field(default_factory=time.time)


class HelenAgentTestClient:
    """Test client for helen-rust agent WebSocket interface."""

    def __init__(self, url: str = "ws://localhost:8002/api/chat/ws"):
        self.url = url
        self.ws: Optional[websockets.WebSocketClientProtocol] = None
        self.received_messages: List[WsMessage] = []
        self._listener_task: Optional[asyncio.Task] = None
        self._connected = False

    async def connect(self):
        """Connect to WebSocket server."""
        self.ws = await websockets.connect(self.url)
        self._connected = True
        self._listener_task = asyncio.create_task(self._listen_loop())
        # Wait for initial status_update
        await self._wait_for_message_type("status_update", timeout=5.0)
        print(f"✓ Connected to {self.url}")

    async def disconnect(self):
        """Disconnect from WebSocket server."""
        if self._listener_task:
            self._listener_task.cancel()
            try:
                await self._listener_task
            except asyncio.CancelledError:
                pass
        if self.ws:
            await self.ws.close()
        self._connected = False
        print("✓ Disconnected")

    async def _listen_loop(self):
        """Background task to receive and store messages."""
        try:
            async for message in self.ws:
                if isinstance(message, str):
                    try:
                        data = json.loads(message)
                        msg_type = data.get("type", "unknown")
                        msg_data = data.get("data", {})
                        self.received_messages.append(WsMessage(
                            type=msg_type,
                            data=msg_data,
                            timestamp=time.time()
                        ))
                    except json.JSONDecodeError:
                        pass
        except websockets.exceptions.ConnectionClosed:
            pass
        except asyncio.CancelledError:
            pass

    def clear_messages(self):
        """Clear received messages buffer."""
        self.received_messages.clear()

    async def _wait_for_message_type(
        self,
        msg_type: str,
        timeout: float = 30.0,
        predicate: Optional[callable] = None
    ) -> Optional[WsMessage]:
        """Wait for a specific message type."""
        start = time.time()
        while time.time() - start < timeout:
            for msg in self.received_messages:
                if msg.type == msg_type:
                    if predicate is None or predicate(msg):
                        return msg
            await asyncio.sleep(0.1)
        return None

    async def _wait_for_messages(
        self,
        msg_types: List[str],
        timeout: float = 30.0
    ) -> List[WsMessage]:
        """Wait for multiple message types, return all received."""
        start = time.time()
        received = []
        while time.time() - start < timeout:
            for msg in self.received_messages:
                if msg.type in msg_types and msg not in received:
                    received.append(msg)
            if all(any(m.type == t for m in received) for t in msg_types):
                break
            await asyncio.sleep(0.1)
        return received

    async def send_message(self, content: str, client_id: str = "test"):
        """Send a user message."""
        await self.ws.send(json.dumps({
            "type": "message",
            "content": content,
            "client_id": client_id
        }))
        print(f"→ Sent message: {content[:50]}...")

    async def send_cancel(self):
        """Send cancel request."""
        await self.ws.send(json.dumps({"type": "cancel"}))
        print("→ Sent cancel")

    async def send_hint(self, content: str, client_id: str = "test"):
        """Send a hint for mid-processing injection."""
        await self.ws.send(json.dumps({
            "type": "hint",
            "content": content,
            "client_id": client_id
        }))
        print(f"→ Sent hint: {content[:50]}...")

    def get_chunks(self) -> str:
        """Get all LLM chunks received so far."""
        chunks = [m.data.get("content", "") for m in self.received_messages
                  if m.type == "llm_chunk"]
        return "".join(chunks)

    def has_message_type(self, msg_type: str) -> bool:
        """Check if a message type was received."""
        return any(m.type == msg_type for m in self.received_messages)

    def get_message_count(self, msg_type: str) -> int:
        """Count messages of a specific type."""
        return sum(1 for m in self.received_messages if m.type == msg_type)


class TestResults:
    """Track test results."""

    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.errors: List[str] = []

    def record_pass(self, test_name: str):
        self.passed += 1
        print(f"  ✓ {test_name}")

    def record_fail(self, test_name: str, reason: str):
        self.failed += 1
        self.errors.append(f"{test_name}: {reason}")
        print(f"  ✗ {test_name}: {reason}")

    def summary(self) -> bool:
        total = self.passed + self.failed
        print(f"\n{'='*60}")
        print(f"Test Summary: {self.passed}/{total} passed")
        if self.errors:
            print("\nFailures:")
            for err in self.errors:
                print(f"  - {err}")
        print(f"{'='*60}")
        return self.failed == 0


async def test_basic_message_flow(client: HelenAgentTestClient, results: TestResults):
    """Test 1: Basic message flow - send message, receive response."""
    print("\n[Test 1] Basic message flow")
    client.clear_messages()

    # Send a simple message
    await client.send_message("Hello, what is 2+2?")

    # Wait for processing to complete
    complete = await client._wait_for_message_type("processing_complete", timeout=30.0)

    if complete:
        results.record_pass("Received processing_complete")
    else:
        results.record_fail("Received processing_complete", "Timeout waiting for response")
        return

    # Check we got LLM chunks
    chunks = client.get_chunks()
    if len(chunks) > 0:
        results.record_pass(f"Received {len(chunks)} chars of LLM output")
    else:
        results.record_fail("Received LLM chunks", "No chunks received")


async def test_cancel_during_inference(client: HelenAgentTestClient, results: TestResults):
    """Test 2: Cancel during inference - should stop and allow new messages."""
    print("\n[Test 2] Cancel during inference")
    client.clear_messages()

    # Send a message that will take time (ask for a long response)
    await client.send_message("Write a detailed essay about the history of computing, at least 1000 words.")

    # Wait a bit for inference to start
    await asyncio.sleep(2.0)

    # Check we're receiving chunks
    initial_chunks = len(client.get_chunks())
    if initial_chunks == 0:
        results.record_fail("Inference started", "No chunks received before cancel")
        return

    print(f"  Received {initial_chunks} chars before cancel")

    # Send cancel
    await client.send_cancel()

    # Wait for cancelled confirmation
    cancelled = await client._wait_for_message_type("cancelled", timeout=5.0)
    if cancelled:
        results.record_pass("Received cancelled confirmation")
    else:
        results.record_fail("Received cancelled confirmation", "No cancelled message")

    # Wait a bit for cleanup
    await asyncio.sleep(1.0)

    # Now send a new message - should work
    client.clear_messages()
    await client.send_message("What is 1+1?")

    # Wait for response
    complete = await client._wait_for_message_type("processing_complete", timeout=30.0)
    if complete:
        results.record_pass("New message processed after cancel")
    else:
        results.record_fail("New message processed after cancel", "Timeout - cancel may have broken state")


async def test_cancel_and_resubmit_with_tool_calls(client: HelenAgentTestClient, results: TestResults):
    """Test 3: Cancel during tool call execution, then resubmit."""
    print("\n[Test 3] Cancel during tool calls, then resubmit")
    client.clear_messages()

    # Send a message that will trigger tool calls
    await client.send_message("Read the file /etc/hostname and tell me what it contains.")

    # Wait for tool call to start
    await asyncio.sleep(1.5)

    # Send cancel
    await client.send_cancel()

    # Wait for cancelled
    cancelled = await client._wait_for_message_type("cancelled", timeout=5.0)
    if cancelled:
        results.record_pass("Cancelled during tool call")
    else:
        results.record_fail("Cancelled during tool call", "No cancelled message")

    await asyncio.sleep(1.0)

    # Resubmit a different message
    client.clear_messages()
    await client.send_message("What is the current date?")

    complete = await client._wait_for_message_type("processing_complete", timeout=30.0)
    if complete:
        results.record_pass("Resubmit after tool call cancel succeeded")
    else:
        results.record_fail("Resubmit after tool call cancel", "Timeout")


async def test_hint_injection(client: HelenAgentTestClient, results: TestResults):
    """Test 4: Send hint during inference to change direction."""
    print("\n[Test 4] Hint injection during inference")
    client.clear_messages()

    # Start with a message
    await client.send_message("Tell me about Python programming language.")

    # Wait for inference to start
    await asyncio.sleep(2.0)

    # Send a hint to change direction
    await client.send_hint("Actually, focus on Rust programming language instead.")

    # Wait for hint_queued confirmation
    hint_queued = await client._wait_for_message_type("hint_queued", timeout=5.0)
    if hint_queued:
        results.record_pass("Hint queued confirmation received")
    else:
        results.record_fail("Hint queued confirmation", "No hint_queued message")

    # Wait for completion
    complete = await client._wait_for_message_type("processing_complete", timeout=60.0)
    if complete:
        results.record_pass("Inference completed with hint")
        # Check if response mentions Rust (the hint topic)
        chunks = client.get_chunks()
        if "rust" in chunks.lower() or "Rust" in chunks:
            results.record_pass("Response includes hint topic (Rust)")
        else:
            results.record_fail("Response includes hint topic", "Rust not mentioned in response")
    else:
        results.record_fail("Inference completed with hint", "Timeout")


async def test_multiple_cancels(client: HelenAgentTestClient, results: TestResults):
    """Test 5: Multiple cancel operations in sequence."""
    print("\n[Test 5] Multiple cancel operations")

    for i in range(3):
        client.clear_messages()
        await client.send_message(f"Write a long story about topic {i}.")
        await asyncio.sleep(1.5)
        await client.send_cancel()

        cancelled = await client._wait_for_message_type("cancelled", timeout=5.0)
        if cancelled:
            results.record_pass(f"Cancel {i+1} succeeded")
        else:
            results.record_fail(f"Cancel {i+1}", "No cancelled message")

        await asyncio.sleep(0.5)

    # Final message should work
    client.clear_messages()
    await client.send_message("Simple question: what is 3+3?")
    complete = await client._wait_for_message_type("processing_complete", timeout=30.0)
    if complete:
        results.record_pass("Final message after multiple cancels")
    else:
        results.record_fail("Final message after multiple cancels", "Timeout")


async def test_empty_hint(client: HelenAgentTestClient, results: TestResults):
    """Test 6: Empty hint should be ignored."""
    print("\n[Test 6] Empty hint handling")
    client.clear_messages()

    await client.send_message("What is 5+5?")
    await asyncio.sleep(1.0)

    # Send empty hint
    await client.send_hint("")

    # Should still complete normally
    complete = await client._wait_for_message_type("processing_complete", timeout=30.0)
    if complete:
        results.record_pass("Empty hint handled correctly")
    else:
        results.record_fail("Empty hint handled", "Timeout")


async def main():
    """Run all tests."""
    print("="*60)
    print("Helen-Rust Agent Cancel & Hint Integration Tests")
    print("="*60)

    results = TestResults()
    client = HelenAgentTestClient()

    try:
        await client.connect()

        # Run tests in sequence
        await test_basic_message_flow(client, results)
        await test_cancel_during_inference(client, results)
        await test_cancel_and_resubmit_with_tool_calls(client, results)
        await test_hint_injection(client, results)
        await test_multiple_cancels(client, results)
        await test_empty_hint(client, results)

    except Exception as e:
        print(f"\n✗ Test suite error: {e}")
        import traceback
        traceback.print_exc()
    finally:
        await client.disconnect()

    # Print summary and exit with appropriate code
    success = results.summary()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    asyncio.run(main())
