"""Hint injector 服务测试

覆盖 enqueue_hint() 和 clear_session() 函数。

运行: cd webui/backend && pytest tests/test_hint_injector.py -v
"""
import pytest
from unittest.mock import patch, MagicMock


@pytest.fixture
def mock_hint_queue():
    """Mock the HintQueue used by hint_injector."""
    mock_queue = MagicMock()
    mock_hint = MagicMock()
    mock_queue.add_hint.return_value = mock_hint
    with patch("app.services.hint_injector.get_hint_queue", return_value=mock_queue):
        yield mock_queue


class TestEnqueueHint:
    def test_delegates_to_hint_queue(self, mock_hint_queue):
        """enqueue_hint 委托给 hint_queue.add_hint"""
        from app.services.hint_injector import enqueue_hint

        result = enqueue_hint("session-1", "try this", client_id="client-abc")
        mock_hint_queue.add_hint.assert_called_once_with("session-1", "try this", "client-abc")
        assert result is mock_hint_queue.add_hint.return_value

    def test_default_client_id(self, mock_hint_queue):
        """enqueue_hint 默认 client_id 为空字符串"""
        from app.services.hint_injector import enqueue_hint

        enqueue_hint("session-1", "hint text")
        mock_hint_queue.add_hint.assert_called_once_with("session-1", "hint text", "")


class TestClearSession:
    def test_delegates_to_hint_queue(self, mock_hint_queue):
        """clear_session 委托给 hint_queue.clear_session"""
        from app.services.hint_injector import clear_session

        clear_session("session-1")
        mock_hint_queue.clear_session.assert_called_once_with("session-1")
