"""Tests for app.services.stream_manager — async event queue management."""
import asyncio

import pytest

from app.services.stream_manager import StreamEventManager, stream_manager


# ── StreamEventManager basics ───────────────────────────────────────────────

class TestStreamEventManagerGetQueue:
    def test_get_queue_returns_asyncio_queue(self):
        mgr = StreamEventManager()
        queue = mgr.get_queue("session-1")
        assert isinstance(queue, asyncio.Queue)

    def test_get_queue_returns_same_queue_for_same_session(self):
        mgr = StreamEventManager()
        q1 = mgr.get_queue("session-1")
        q2 = mgr.get_queue("session-1")
        assert q1 is q2

    def test_get_queue_returns_different_queues_for_different_sessions(self):
        mgr = StreamEventManager()
        q1 = mgr.get_queue("session-1")
        q2 = mgr.get_queue("session-2")
        assert q1 is not q2


class TestStreamEventManagerPushEvent:
    @pytest.mark.asyncio
    async def test_push_event_puts_event_in_queue(self):
        mgr = StreamEventManager()
        event = {"type": "message", "data": "hello"}
        await mgr.push_event("session-1", event)

        queue = mgr.get_queue("session-1")
        assert queue.qsize() == 1
        retrieved = await queue.get()
        assert retrieved == event

    @pytest.mark.asyncio
    async def test_push_multiple_events(self):
        mgr = StreamEventManager()
        events = [
            {"type": "message", "data": "first"},
            {"type": "message", "data": "second"},
            {"type": "complete"},
        ]
        for event in events:
            await mgr.push_event("session-1", event)

        queue = mgr.get_queue("session-1")
        assert queue.qsize() == 3

    @pytest.mark.asyncio
    async def test_push_event_creates_queue_if_not_exists(self):
        mgr = StreamEventManager()
        await mgr.push_event("new-session", {"type": "test"})
        queue = mgr.get_queue("new-session")
        assert queue.qsize() == 1


class TestStreamEventManagerStreamEvents:
    @pytest.mark.asyncio
    async def test_stream_events_yields_events(self):
        mgr = StreamEventManager()
        events_to_push = [
            {"type": "message", "data": "hello"},
            {"type": "message", "data": "world"},
            {"type": "complete"},
        ]
        for event in events_to_push:
            await mgr.push_event("session-1", event)

        received = []
        async for event in mgr.stream_events("session-1"):
            received.append(event)

        assert received == events_to_push

    @pytest.mark.asyncio
    async def test_stream_events_breaks_on_complete(self):
        mgr = StreamEventManager()
        await mgr.push_event("session-1", {"type": "message", "data": "msg"})
        await mgr.push_event("session-1", {"type": "complete"})
        await mgr.push_event("session-1", {"type": "message", "data": "after"})

        received = []
        async for event in mgr.stream_events("session-1"):
            received.append(event)

        # Should stop at "complete", not include "after"
        assert len(received) == 2
        assert received[-1]["type"] == "complete"

    @pytest.mark.asyncio
    async def test_stream_events_breaks_on_error(self):
        mgr = StreamEventManager()
        await mgr.push_event("session-1", {"type": "message", "data": "msg"})
        await mgr.push_event("session-1", {"type": "error", "message": "fail"})
        await mgr.push_event("session-1", {"type": "message", "data": "after"})

        received = []
        async for event in mgr.stream_events("session-1"):
            received.append(event)

        # Should stop at "error"
        assert len(received) == 2
        assert received[-1]["type"] == "error"

    @pytest.mark.asyncio
    async def test_stream_events_waits_for_events(self):
        """stream_events should wait for events when queue is empty."""
        mgr = StreamEventManager()
        results = []

        async def producer():
            await asyncio.sleep(0.05)
            await mgr.push_event("session-1", {"type": "message", "data": "delayed"})
            await asyncio.sleep(0.01)
            await mgr.push_event("session-1", {"type": "complete"})

        async def consumer():
            async for event in mgr.stream_events("session-1"):
                results.append(event)

        await asyncio.gather(producer(), consumer())
        assert len(results) == 2
        assert results[0]["data"] == "delayed"
        assert results[1]["type"] == "complete"

    @pytest.mark.asyncio
    async def test_stream_events_continues_for_non_terminal_types(self):
        """Non-complete/error events should not stop the stream."""
        mgr = StreamEventManager()
        events = [
            {"type": "message"},
            {"type": "chunk"},
            {"type": "progress"},
            {"type": "complete"},
        ]
        for event in events:
            await mgr.push_event("session-1", event)

        received = []
        async for event in mgr.stream_events("session-1"):
            received.append(event)

        assert len(received) == 4


class TestStreamEventManagerClearQueue:
    def test_clear_queue_removes_session(self):
        mgr = StreamEventManager()
        mgr.get_queue("session-1")
        assert "session-1" in mgr._queues

        mgr.clear_queue("session-1")
        assert "session-1" not in mgr._queues

    def test_clear_queue_nonexistent_session_no_error(self):
        """Clearing a non-existent session should not raise."""
        mgr = StreamEventManager()
        # Should not raise
        mgr.clear_queue("nonexistent-session")

    def test_clear_queue_only_affects_target_session(self):
        mgr = StreamEventManager()
        mgr.get_queue("session-1")
        mgr.get_queue("session-2")

        mgr.clear_queue("session-1")
        assert "session-1" not in mgr._queues
        assert "session-2" in mgr._queues


# ── Global instance ─────────────────────────────────────────────────────────

class TestGlobalStreamManager:
    def test_stream_manager_is_stream_event_manager_instance(self):
        assert isinstance(stream_manager, StreamEventManager)
