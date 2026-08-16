"""Tests for helen/agent/webui/backend/app/websocket/manager.py

WebSocketManager manages active WebSocket connections: connect, disconnect,
broadcast, and close_all operations.
"""
import pytest
from unittest.mock import MagicMock, AsyncMock


@pytest.fixture
def ws_manager():
    from app.websocket.manager import WebSocketManager
    return WebSocketManager()


def _make_ws(send_error=None, close_error=None):
    """Create a mock WebSocket with async accept/send/close methods."""
    ws = MagicMock()
    ws.accept = AsyncMock()
    ws.send_json = AsyncMock(side_effect=send_error) if send_error else AsyncMock()
    ws.close = AsyncMock(side_effect=close_error) if close_error else AsyncMock()
    return ws


class TestConnect:
    """WebSocketManager.connect tests."""

    @pytest.mark.asyncio
    async def test_connect_adds_to_active_and_accepts(self, ws_manager):
        ws = _make_ws()
        await ws_manager.connect(ws)
        ws.accept.assert_awaited_once()
        assert ws in ws_manager.active_connections

    @pytest.mark.asyncio
    async def test_connect_multiple(self, ws_manager):
        ws1, ws2 = _make_ws(), _make_ws()
        await ws_manager.connect(ws1)
        await ws_manager.connect(ws2)
        assert len(ws_manager.active_connections) == 2


class TestDisconnect:
    """WebSocketManager.disconnect tests."""

    @pytest.mark.asyncio
    async def test_disconnect_removes_connection(self, ws_manager):
        ws = _make_ws()
        await ws_manager.connect(ws)
        ws_manager.disconnect(ws)
        assert ws not in ws_manager.active_connections

    def test_disconnect_nonexistent_is_noop(self, ws_manager):
        ws = _make_ws()
        ws_manager.disconnect(ws)  # should not raise

    @pytest.mark.asyncio
    async def test_disconnect_only_removes_target(self, ws_manager):
        ws1, ws2 = _make_ws(), _make_ws()
        await ws_manager.connect(ws1)
        await ws_manager.connect(ws2)
        ws_manager.disconnect(ws1)
        assert ws2 in ws_manager.active_connections
        assert ws1 not in ws_manager.active_connections


class TestBroadcast:
    """WebSocketManager.broadcast tests."""

    @pytest.mark.asyncio
    async def test_broadcast_sends_to_all(self, ws_manager):
        ws1, ws2 = _make_ws(), _make_ws()
        await ws_manager.connect(ws1)
        await ws_manager.connect(ws2)
        await ws_manager.broadcast({"type": "hello"})
        ws1.send_json.assert_awaited_once_with({"type": "hello"})
        ws2.send_json.assert_awaited_once_with({"type": "hello"})

    @pytest.mark.asyncio
    async def test_broadcast_removes_failed_connection(self, ws_manager):
        good_ws = _make_ws()
        bad_ws = _make_ws(send_error=RuntimeError("broken"))
        await ws_manager.connect(good_ws)
        await ws_manager.connect(bad_ws)
        await ws_manager.broadcast({"type": "hello"})
        # bad_ws should have been disconnected
        assert bad_ws not in ws_manager.active_connections
        assert good_ws in ws_manager.active_connections
        good_ws.send_json.assert_awaited_once_with({"type": "hello"})

    @pytest.mark.asyncio
    async def test_broadcast_empty_connections(self, ws_manager):
        # No connections: should not raise
        await ws_manager.broadcast({"type": "hello"})


class TestCloseAll:
    """WebSocketManager.close_all tests."""

    @pytest.mark.asyncio
    async def test_close_all_closes_and_clears(self, ws_manager):
        ws1, ws2 = _make_ws(), _make_ws()
        await ws_manager.connect(ws1)
        await ws_manager.connect(ws2)
        await ws_manager.close_all()
        ws1.close.assert_awaited_once()
        ws2.close.assert_awaited_once()
        assert len(ws_manager.active_connections) == 0

    @pytest.mark.asyncio
    async def test_close_all_handles_exception(self, ws_manager):
        ws1 = _make_ws(close_error=RuntimeError("close failed"))
        ws2 = _make_ws()
        await ws_manager.connect(ws1)
        await ws_manager.connect(ws2)
        await ws_manager.close_all()  # should not raise
        assert len(ws_manager.active_connections) == 0
        ws2.close.assert_awaited_once()

    @pytest.mark.asyncio
    async def test_close_all_when_empty(self, ws_manager):
        await ws_manager.close_all()  # should not raise
