#!/usr/bin/env python3
"""
Helen Web UI 后端

提供 ChatSessionActor 的导入接口，Web UI 通过这个模块调用 Helen。
v1.0：actor 成为唯一模式，移除非 actor 路径。

Session 恢复机制：
  启动时读取 .helen/current_session_id memento 文件（JSON: {main, child}），
  通过 set_session_id(主sid) 让主 Interpreter 复用历史主 session，
  子 session_id 由 chat_tui.helen 的 spawn_chat_actor() 通过 spawn resume() 复用。
"""

import sys
import os
import json
from pathlib import Path

# Helen agent 项目目录
HELEN_AGENT_DIR = os.path.dirname(os.path.abspath(__file__))

# 确保可以导入 helenagent 模块
sys.path.insert(0, HELEN_AGENT_DIR)

# 安装 Python Bridge import hook
try:
    from helen.python_bridge import install_import_hook
    install_import_hook()
except ImportError as e:
    print(f"✗ Python Bridge 不可用: {e}", file=sys.stderr)
    print("请确保已安装 Helen: pip install helen", file=sys.stderr)
    sys.exit(1)

# ── Session 恢复：读取 memento 文件，复用历史主 session_id ──
# memento 文件格式：{"main": "<主session_id>", "child": "<子session_id>"}
# 必须在 import chat_tui 之前调用 set_session_id，因为 import hook 在那一刻创建 Interpreter
_saved_main_sid = ""
_saved_child_sid = ""
_memento_path = Path.cwd() / ".helen" / "current_session_id"
if _memento_path.exists():
    try:
        _data = json.loads(_memento_path.read_text(encoding="utf-8"))
        _saved_main_sid = _data.get("main", "")
        _saved_child_sid = _data.get("child", "")
        if _saved_main_sid:
            from helen.python_bridge import set_session_id
            set_session_id(_saved_main_sid)
            print(f"[Session] 恢复主 session: {_saved_main_sid}", file=sys.stderr)
    except Exception as e:
        print(f"[Session] memento 读取失败（忽略）: {e}", file=sys.stderr)

# 导入 actor 接口（从 chat_actor.helen）
from chat_actor import (
    spawn_chat_actor,
    tui_chat_handler_actor,
    TUIChatAgent,
    exit_chat_actor,
    is_chat_actor_running,
    send_heartbeat,
)


def get_saved_child_sid() -> str:
    """返回 memento 中保存的子 session_id（供 chat_actor.helen 读取）"""
    return _saved_child_sid


def is_actor_mode_available() -> bool:
    """长驻 actor 模式是否可用"""
    return True


__all__ = [
    'spawn_chat_actor',
    'tui_chat_handler_actor',
    'TUIChatAgent',
    'exit_chat_actor',
    'is_chat_actor_running',
    'is_actor_mode_available',
    'get_saved_child_sid',
    'send_heartbeat',
]
