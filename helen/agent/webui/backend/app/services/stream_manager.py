"""流式事件管理"""
import asyncio
from typing import Dict, AsyncGenerator
from collections import defaultdict

class StreamEventManager:
    """管理流式事件队列"""

    def __init__(self):
        self._queues: Dict[str, asyncio.Queue] = defaultdict(asyncio.Queue)

    def get_queue(self, session_id: str) -> asyncio.Queue:
        """获取指定会话的事件队列"""
        return self._queues[session_id]

    async def push_event(self, session_id: str, event: dict):
        """推送事件到队列"""
        queue = self._queues[session_id]
        await queue.put(event)

    async def stream_events(self, session_id: str) -> AsyncGenerator[dict, None]:
        """流式读取事件"""
        queue = self._queues[session_id]
        while True:
            event = await queue.get()
            yield event
            queue.task_done()

            # 如果是完成或错误事件，结束流
            if event.get("type") in ["complete", "error"]:
                break

    def clear_queue(self, session_id: str):
        """清理队列"""
        if session_id in self._queues:
            del self._queues[session_id]

# 全局实例
stream_manager = StreamEventManager()
