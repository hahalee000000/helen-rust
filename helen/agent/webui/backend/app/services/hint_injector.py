"""Hint injection service — backend-side glue for the hint queue.

Called by chat.py WebSocket handler to enqueue hints; the Helen-side
on_tool_end callback reads the same queue via ui.hint_queue FFI.
"""
from ui.hint_queue import get_hint_queue, Hint


def enqueue_hint(session_id: str, text: str, client_id: str = "") -> Hint:
    """Add a hint to the per-session queue. Thread-safe."""
    return get_hint_queue().add_hint(session_id, text, client_id)


def clear_session(session_id: str) -> None:
    """Drop any pending hints (on WS disconnect)."""
    get_hint_queue().clear_session(session_id)
