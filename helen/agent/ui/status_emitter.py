"""
Statusline 数据收集 + FFI 入口

Helen 端在关键节点（ChatSession 入口 / llm_complete / on_tool_end 注入 hint 后）
通过 Python FFI 调用 emit_status()，把当前会话状态（hostname / cwd / model / 上下文占用率）
经由 stream_emitter 推送到所有连接的 WebSocket 客户端。

前端 ChatWindow 底部的 <StatusLine /> 组件实时显示这些信息。

设计要点：
- 静态信息（hostname / cwd / user）首次调用时采集并缓存，避免重复 shell_exec
- 动态信息（usage_ratio）每次实时查询（由 Helen 侧 context_stats() 提供）
- 复用 stream_emitter 通道，与 hint_injection 机制同构
- 异常时静默返回 False，不影响 Helen 主流程（CLI 模式降级）
"""

import os
import socket
import threading
from typing import Optional


_static_lock = threading.Lock()
_static_cache: Optional[dict] = None


def _collect_static() -> dict:
    """采集静态环境信息（首次调用后缓存）"""
    return {
        "hostname": socket.gethostname().split(".")[0] or "unknown",
        "cwd": os.getcwd(),
        "user": os.environ.get("USER") or os.environ.get("USERNAME") or "",
    }


def get_status_snapshot(usage_ratio: float = -1.0, model: str = "") -> dict:
    """组装 statusline 数据（线程安全）

    Args:
        usage_ratio: 上下文占用率（0.0 - 1.0+，由 Helen context_stats()["usage_ratio"] 提供）。
                     负数表示未知，前端显示为 0。
        model: LLM 模型名（由 Helen 侧传入，与 chat_actor.helen agent 定义一致）
    """
    global _static_cache
    with _static_lock:
        if _static_cache is None:
            _static_cache = _collect_static()

    snap = dict(_static_cache)
    snap["usage_ratio"] = float(usage_ratio) if usage_ratio >= 0 else 0.0
    snap["model"] = model or "unknown"
    return snap


def emit_status(usage_ratio: float = -1.0, model: str = "") -> bool:
    """Helen FFI 入口：组装数据并通过 stream_emitter 推送到所有 WS 客户端

    由 Helen 侧 _emit_statusline() 通过 `import "ui.status_emitter"` FFI 调用。
    异常时返回 False，不抛出。
    """
    try:
        import json
        from ui.stream_emitter import emit_stream_event

        snap = get_status_snapshot(usage_ratio, model)
        emit_stream_event("status_update", json.dumps(snap))
        return True
    except Exception:
        # CLI 模式（无 stream_emitter 回调）或 FFI 不可用时静默降级
        return False


def reset_static_cache() -> None:
    """清除静态缓存（测试用，或 cwd 改变后强制重采）"""
    global _static_cache
    with _static_lock:
        _static_cache = None
