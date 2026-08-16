"""Thread-safe hint queue for mid-processing user hints.

Cross-thread communication: Python async event loop (WebSocket) writes,
Helen runtime thread (on_tool_end callback) reads.

v1.39.4: 简化为单例模式（非 per-session）。
每个 webui 进程只有一个 active actor，使用固定 key "_active"
避免 session_id 不匹配问题（chat.py 的 cwd hash vs actor 的 UUID）。
"""
import threading
import time
from dataclasses import dataclass
from typing import Dict, List


# v1.39.4: 固定 key，所有 hint 共用一个队列
_ACTIVE_KEY = "_active"


@dataclass
class Hint:
    text: str
    timestamp: float
    client_id: str  # for UI dedup / ack


class HintQueue:
    """FIFO queue of pending user hints (singleton, not per-session)."""

    def __init__(self):
        self._lock = threading.Lock()
        self._queue: List[Hint] = []

    def add_hint(self, session_id: str, text: str, client_id: str = "") -> Hint:
        """添加 hint。session_id 参数被忽略（向后兼容），使用固定 key。"""
        with self._lock:
            hint = Hint(text=text, timestamp=time.time(), client_id=client_id)
            self._queue.append(hint)
            return hint

    def pop_all_hints(self, session_id: str = "") -> List[Hint]:
        """Atomically pop all pending hints.

        session_id 参数被忽略（向后兼容），使用固定 key。
        Called from Helen's on_tool_end callback thread. Returns [] if empty.
        """
        with self._lock:
            hints = list(self._queue)
            self._queue.clear()
            return hints

    def has_pending(self, session_id: str = "") -> bool:
        """session_id 参数被忽略（向后兼容）。"""
        with self._lock:
            return bool(self._queue)

    def clear_session(self, session_id: str = "") -> None:
        """Drop all pending hints (called on WS disconnect / session cleanup).

        session_id 参数被忽略（向后兼容），清空整个队列。
        """
        with self._lock:
            self._queue.clear()

    def clear_all(self) -> None:
        with self._lock:
            self._queue.clear()


_instance = HintQueue()


def get_hint_queue() -> HintQueue:
    return _instance
