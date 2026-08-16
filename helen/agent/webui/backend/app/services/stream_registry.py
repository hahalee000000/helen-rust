"""StreamRegistry - 跟踪当前正在进行的流式推理任务

解决：页面刷新/WS 重连后前端无法知道后端是否仍在处理请求。
前端通过 GET /api/chat/status 查询 is_processing 状态，
在初始挂载和 WS 重连时调用，恢复 isLoading 状态。

线程安全：do_streaming 由 asyncio task 驱动，/status 由 HTTP handler 驱动，
可能在不同线程执行，用 threading.Lock 保护。
"""
import threading


class StreamRegistry:
    """跟踪当前有活跃流式推理的 session 集合"""

    def __init__(self):
        self._lock = threading.Lock()
        self._active_sessions: set = set()

    def register(self, session_id: str):
        """标记 session 开始流式推理"""
        with self._lock:
            self._active_sessions.add(session_id)

    def unregister(self, session_id: str):
        """标记 session 流式推理结束（完成/取消/崩溃均调用）"""
        with self._lock:
            self._active_sessions.discard(session_id)

    def is_processing(self) -> bool:
        """是否有任何 session 正在流式推理"""
        with self._lock:
            return len(self._active_sessions) > 0


# 模块级单例，被 chat.py 和 status 端点共享
stream_registry = StreamRegistry()
