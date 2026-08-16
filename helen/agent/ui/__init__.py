"""
HelenAgent UI - Web UI 支持模块

核心组件：
- stream_emitter: 流式事件传递（chat_session_actor 通过 FFI 调用）
- status_emitter: 状态栏发射（chat_session_actor 通过 FFI 调用）
- hint_queue: Hint 注入队列（chat_session_actor 通过 FFI 调用）

注：所有组件均无 rich/textual 依赖，供 WebUI 架构使用。
"""

# stream_emitter: 流式事件传递（FFI 入口）
from . import stream_emitter

# status_emitter: 状态栏发射（FFI 入口）
from . import status_emitter

# hint_queue: Hint 注入队列（FFI 入口）
from . import hint_queue

__all__ = ['stream_emitter', 'status_emitter', 'hint_queue']

__version__ = '0.2.0'
