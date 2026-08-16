"""Tests for ChannelActorManager."""
import sys
import types
import threading
import pytest
from unittest.mock import patch, MagicMock


@pytest.fixture
def mock_chat_tui_web(monkeypatch):
    """Mock chat_tui_web module to avoid requiring actual Helen runtime."""
    mock_module = types.ModuleType("chat_tui_web")
    mock_module.is_actor_mode_available = MagicMock(return_value=True)
    mock_module.spawn_chat_actor = MagicMock(return_value={"status": "started", "session_id": "test-session-123"})
    mock_module.send_heartbeat = MagicMock()
    mock_module.tui_chat_handler_actor = MagicMock(return_value="actor response")
    mock_module.exit_chat_actor = MagicMock()
    mock_module.is_chat_actor_running = MagicMock(return_value=True)

    monkeypatch.setitem(sys.modules, "chat_tui_web", mock_module)
    return mock_module


@pytest.fixture
def manager():
    """Fresh ChannelActorManager instance."""
    from app.services.channel_actor_manager import ChannelActorManager
    return ChannelActorManager()


class TestActorModeEnabled:
    def test_always_enabled(self):
        from app.services.channel_actor_manager import actor_mode_enabled
        assert actor_mode_enabled() is True


class TestChannelActorManager:
    def test_initial_state(self, manager):
        """New manager has no actor spawned."""
        assert manager._actor_spawned is False
        assert manager._session_id is None
        assert manager._heartbeat_thread is None

    def test_is_available_with_mock(self, manager, mock_chat_tui_web):
        """is_available returns True when chat_tui_web says so."""
        assert manager.is_available() is True

    def test_is_available_import_error(self, manager, monkeypatch):
        """is_available returns False on ImportError."""
        # Remove the mock
        monkeypatch.delitem(sys.modules, "chat_tui_web", raising=False)
        # Force ImportError by making chat_tui_web unavailable
        import builtins
        original_import = builtins.__import__
        def failing_import(name, *args, **kwargs):
            if name == "chat_tui_web":
                raise ImportError("mock")
            return original_import(name, *args, **kwargs)
        monkeypatch.setattr(builtins, "__import__", failing_import)
        assert manager.is_available() is False

    def test_ensure_actor_starts(self, manager, mock_chat_tui_web):
        """ensure_actor spawns a new actor."""
        result = manager.ensure_actor()
        assert result["status"] == "started"
        assert manager._actor_spawned is True
        assert manager._session_id == "test-session-123"
        mock_chat_tui_web.spawn_chat_actor.assert_called_once()

    def test_ensure_actor_already_running(self, manager, mock_chat_tui_web):
        """ensure_actor returns already_running if actor is up."""
        manager._actor_spawned = True
        manager._session_id = "existing-session"
        result = manager.ensure_actor()
        assert result["status"] == "already_running"
        assert result["session_id"] == "existing-session"
        mock_chat_tui_web.spawn_chat_actor.assert_not_called()

    def test_ensure_actor_failure(self, manager, mock_chat_tui_web):
        """ensure_actor returns error dict on spawn failure."""
        mock_chat_tui_web.spawn_chat_actor.return_value = {"status": "error", "error": "spawn failed"}
        result = manager.ensure_actor()
        assert result["status"] == "error"
        assert manager._actor_spawned is False

    def test_ensure_actor_non_dict_result(self, manager, mock_chat_tui_web):
        """ensure_actor handles non-dict spawn result."""
        mock_chat_tui_web.spawn_chat_actor.return_value = "unexpected string"
        result = manager.ensure_actor()
        assert result["status"] == "error"
        assert "unexpected string" in result["error"]

    def test_send_message(self, manager, mock_chat_tui_web):
        """send_message returns actor response."""
        # First, ensure actor is spawned
        manager._actor_spawned = True
        manager._session_id = "test-session"

        response = manager.send_message("hello")
        assert response == "actor response"
        mock_chat_tui_web.tui_chat_handler_actor.assert_called_once_with("hello", [])

    def test_send_message_with_files(self, manager, mock_chat_tui_web):
        """send_message passes file_paths."""
        manager._actor_spawned = True
        manager._session_id = "test-session"

        manager.send_message("hello", file_paths=["/tmp/file.txt"])
        mock_chat_tui_web.tui_chat_handler_actor.assert_called_once_with("hello", ["/tmp/file.txt"])

    def test_send_message_crash_recovery(self, manager, mock_chat_tui_web):
        """send_message marks actor as dead on exception."""
        manager._actor_spawned = True
        manager._session_id = "test-session"
        mock_chat_tui_web.tui_chat_handler_actor.side_effect = RuntimeError("actor crashed")

        with pytest.raises(RuntimeError, match="actor crashed"):
            manager.send_message("hello")

        assert manager._actor_spawned is False
        assert manager._session_id is None

    def test_exit_actor(self, manager, mock_chat_tui_web):
        """exit_actor calls Helen exit and resets state."""
        manager._actor_spawned = True
        manager._session_id = "test-session"

        manager.exit_actor()

        mock_chat_tui_web.exit_chat_actor.assert_called_once()
        assert manager._actor_spawned is False
        assert manager._session_id is None

    def test_exit_actor_not_spawned(self, manager, mock_chat_tui_web):
        """exit_actor is no-op when actor not spawned."""
        manager.exit_actor()
        mock_chat_tui_web.exit_chat_actor.assert_not_called()

    def test_exit_actor_handles_exception(self, manager, mock_chat_tui_web):
        """exit_actor handles exception gracefully."""
        manager._actor_spawned = True
        manager._session_id = "test-session"
        mock_chat_tui_web.exit_chat_actor.side_effect = RuntimeError("exit failed")

        # Should not raise
        manager.exit_actor()
        assert manager._actor_spawned is False

    def test_restart_actor(self, manager, mock_chat_tui_web):
        """restart_actor exits and re-ensures."""
        manager._actor_spawned = True
        manager._session_id = "old-session"

        result = manager.restart_actor()

        mock_chat_tui_web.exit_chat_actor.assert_called_once()
        # ensure_actor should be called again and spawn new actor
        assert mock_chat_tui_web.spawn_chat_actor.call_count == 1

    def test_is_running_true(self, manager, mock_chat_tui_web):
        """is_running returns True when actor is up."""
        assert manager.is_running() is True

    def test_is_running_false(self, manager, mock_chat_tui_web):
        """is_running returns False when actor is down."""
        mock_chat_tui_web.is_chat_actor_running.return_value = False
        assert manager.is_running() is False

    def test_is_running_exception(self, manager, mock_chat_tui_web):
        """is_running returns False on exception."""
        mock_chat_tui_web.is_chat_actor_running.side_effect = RuntimeError("ffil error")
        assert manager.is_running() is False

    def test_heartbeat_lifecycle(self, manager, mock_chat_tui_web):
        """_start_heartbeat / _stop_heartbeat manage thread."""
        manager._start_heartbeat()
        assert manager._heartbeat_thread is not None
        assert manager._heartbeat_thread.is_alive()

        manager._stop_heartbeat()
        # Thread should be stopped (may take a moment to join)
        import time
        time.sleep(0.1)
        assert manager._heartbeat_thread is None

    def test_heartbeat_loop_exception_breaks(self, manager, mock_chat_tui_web):
        """Heartbeat loop exits when send_heartbeat raises."""
        import time
        # Make send_heartbeat raise on first call
        mock_chat_tui_web.send_heartbeat.side_effect = RuntimeError("heartbeat failed")

        manager._start_heartbeat()
        # The heartbeat loop uses wait(HEARTBEAT_INTERVAL=120), but on exception
        # it breaks. We need to trigger the loop iteration. Since interval is 120s,
        # we patch HEARTBEAT_INTERVAL to be very short for this test.
        import app.services.channel_actor_manager as cam
        original_interval = cam.HEARTBEAT_INTERVAL
        cam.HEARTBEAT_INTERVAL = 0.05
        try:
            # Restart with short interval
            manager._stop_heartbeat()
            manager._start_heartbeat()
            # Wait for the heartbeat loop to run and hit the exception
            time.sleep(0.3)
            # Thread should have exited (broken out of loop)
            assert not manager._heartbeat_thread.is_alive()
        finally:
            cam.HEARTBEAT_INTERVAL = original_interval
            manager._stop_heartbeat()

    def test_heartbeat_loop_success_path(self, manager, mock_chat_tui_web):
        """Heartbeat loop sends heartbeats and stops cleanly."""
        import time
        import app.services.channel_actor_manager as cam
        original_interval = cam.HEARTBEAT_INTERVAL
        cam.HEARTBEAT_INTERVAL = 0.05
        try:
            manager._start_heartbeat()
            time.sleep(0.2)
            # Should have sent at least one heartbeat
            assert mock_chat_tui_web.send_heartbeat.called
            manager._stop_heartbeat()
        finally:
            cam.HEARTBEAT_INTERVAL = original_interval


class TestGlobalSingleton:
    def test_singleton_exists(self):
        """Global channel_actor_manager singleton exists."""
        from app.services.channel_actor_manager import channel_actor_manager
        assert channel_actor_manager is not None
