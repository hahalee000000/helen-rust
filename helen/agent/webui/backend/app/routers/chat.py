"""聊天相关 API 路由"""
import asyncio
import json
import os
from pathlib import Path
from fastapi import APIRouter, Depends, WebSocket, WebSocketDisconnect, HTTPException, Body, UploadFile, File, Form, Query
from fastapi.responses import FileResponse
from typing import List, Optional
import uuid
from datetime import datetime

from app.auth import require_auth, verify_ws_token
from app.services.helen_bridge import helen_bridge
from app.services import hint_injector
from app.services import directory_manager
from app.services.stream_registry import stream_registry
from app.goal_handler import (
    build_goal_prompt, build_continuation_prompt,
    goal_appears_complete, parse_goal_status,
    DEFAULT_MAX_ITERATIONS,
)

# ⚠️ 架构说明：
# FastAPI 0.109 + Starlette 0.35 下，router 级 HTTP 依赖（如 Depends(require_auth)）
# 会波及同 router 下的 WebSocket 路由，导致 WS 握手失败（500）。
# 解决方案：HTTP 路由用 http_router（router 级鉴权），WebSocket 用 ws_router（无 router 级依赖）。
# WebSocket 鉴权在 handler 内通过 verify_ws_token(token) 完成。

# HTTP 路由（带 router 级鉴权）
http_router = APIRouter(dependencies=[Depends(require_auth)])

# WebSocket 路由（无 router 级依赖，handler 内鉴权）
ws_router = APIRouter()

# ── 文件上传常量 ──────────────────────────────────────────
MAX_FILE_SIZE = 50 * 1024 * 1024  # 50MB
ALLOWED_MIME_TYPES = {
    # 图片
    "image/jpeg", "image/png", "image/gif", "image/webp",
    # 音频
    "audio/mpeg", "audio/wav", "audio/ogg", "audio/mp4", "audio/m4a",
    # 视频
    "video/mp4", "video/webm", "video/quicktime",
}


# === 流式状态查询（前端 re-sync 用）===

@http_router.get("/status")
async def get_chat_status():
    """检查后端是否正在处理请求（前端 re-sync 用）+ 返回系统信息

    前端在初始挂载和 WS 重连时调用此端点，恢复 isLoading 状态，
    让 stop/hint 按钮在页面刷新后能正确显示。

    同时返回 version 和 helen_path 供 Settings 页面展示。
    """
    import importlib.metadata
    import sys as _sys

    # Robust version: works even when a namespace package named "helen"
    # (e.g. helen-rust/helen/) shadows the real Python package on sys.path.
    try:
        version = importlib.metadata.version("helen-lang")
    except importlib.metadata.PackageNotFoundError:
        try:
            version = importlib.metadata.version("helen")
        except importlib.metadata.PackageNotFoundError:
            import helen as _h
            version = getattr(_h, "__version__", "unknown")

    # Find the real helen package directory (the one with __init__.py),
    # skipping any namespace-package shadows that lack __init__.py.
    helen_path = ""
    for _p in _sys.path:
        _candidate = Path(_p) / "helen"
        if _candidate.is_dir() and (_candidate / "__init__.py").exists():
            helen_path = str(_candidate)
            break
    if not helen_path:
        # Fallback: use the webui's own location (helen/agent/webui/backend → helen/)
        helen_path = str(Path(__file__).resolve().parents[4])

    return {
        "is_processing": stream_registry.is_processing(),
        "version": version,
        "config": {
            "helen_path": helen_path,
        },
    }


# === 目录管理 API（单会话模式） ===

@http_router.get("/dir")
async def get_directory():
    """获取当前工作目录信息

    v6.1:transcript 是唯一数据源,不再 upsert SessionModel。
    返回 cwd 和 session_id(cwd hash,前端用于标识会话)。
    """
    cwd = directory_manager.get_current_cwd()
    display_name = directory_manager.get_display_name(cwd)
    session_id = directory_manager.cwd_to_session_id(cwd)

    # 获取 Helen session ID（如果可用）
    helen_session_id = None
    try:
        helen_session_id = await helen_bridge.get_session_id()
    except Exception:
        pass  # Helen 可能未初始化

    return {
        "cwd": cwd,
        "display_name": display_name,
        "session_id": session_id,
        "helen_session_id": helen_session_id,
    }


@http_router.post("/dir")
async def change_directory(body: dict = Body(...)):
    """切换工作目录

    切换后，所有后续请求将使用新目录的数据库和 session。

    Request body:
        {"path": "/path/to/project"}

    Returns:
        {
            "status": "ok",
            "cwd": "/absolute/path",
            "display_name": "project-name",
            "session_id": "<hash>",
            "helen_session_id": "xxx"
        }
    """
    path = body.get("path", "")
    if not path:
        raise HTTPException(status_code=400, detail="path is required")

    result = directory_manager.set_current_cwd(path)

    if result["status"] == "error":
        raise HTTPException(status_code=400, detail=result["message"])

    new_cwd = result["cwd"]
    display_name = result["display_name"]
    new_session_id = directory_manager.cwd_to_session_id(new_cwd)

    # 获取新目录的 Helen session ID
    helen_session_id = None
    try:
        helen_session_id = await helen_bridge.get_session_id()
    except Exception:
        pass

    result["session_id"] = new_session_id
    result["helen_session_id"] = helen_session_id
    return result


@http_router.get("/dir/messages")
def get_directory_messages(limit: int = 100, offset: int = 0):
    """获取当前工作目录的消息历史(从 Helen transcript 读取)

    v6.1:transcript 是唯一数据源,替代 SQLite messages 表。
    session_id 概念已移除,直接读当前 Helen session 的 transcript。
    """
    from app.services.session_index import transcript_to_messages
    messages = transcript_to_messages()
    if offset:
        messages = messages[offset:]
    if limit:
        messages = messages[:limit]
    return messages


# === 会话管理 API ===

@http_router.get("/sessions")
def list_sessions():
    """获取会话列表(从 Helen transcript 目录读取)

    v6.1:替代 SQLite sessions 表。返回历史 Helen session,
    供 TranscriptPage 下拉使用。按 modified_at desc 排序。
    """
    return helen_bridge.list_sessions_sync()

@http_router.delete("/sessions/{session_id}")
async def delete_session(session_id: str):
    """删除指定 Helen session 的 transcript(级联删除)

    v6.1:transcript 是唯一数据源,删除会话即删除 transcript。
    session_id 为 Helen session_id(由 GET /sessions 返回)。
    通过 /clear-session 斜杠命令触发 Helen 侧级联删除(避免孤儿 spawn transcripts)。
    """
    try:
        response = await helen_bridge.run_silent("/clear-session " + session_id)
        if not response or "__HELEN_CLEAR_SESSION_OK__" not in response:
            import logging
            logging.getLogger(__name__).warning(
                "Helen transcript 清理可能失败 (helen_sid=%s, response=%r)",
                session_id, response
            )
            return {"status": "warning", "message": "transcript cleanup may have failed", "response": response}
    except Exception as e:
        import logging
        logging.getLogger(__name__).warning(
            "Helen transcript 清理异常 (helen_sid=%s): %s", session_id, e
        )
        return {"status": "error", "message": str(e)}
    return {"status": "ok", "message": "Session deleted"}

@http_router.get("/sessions/{session_id}/messages")
def get_messages(session_id: str):
    """获取会话消息(从 Helen transcript 读取)

    v6.1:transcript 是唯一数据源。session_id 参数为兼容保留
    (前端 useChat 仍传 web session_id),实际读当前 Helen session。
    """
    from app.services.session_index import transcript_to_messages
    return transcript_to_messages()

@http_router.get("/sessions/{session_id}/transcript")
async def get_transcript(session_id: str):
    """获取 Helen transcript（LLM 上下文的完整记录）

    从 .helen/sessions/<sid>/transcript.jsonl 读取，返回结构化的消息列表。
    用于调试和查看 Agent 执行过程。
    """
    import os
    from pathlib import Path
    from app.services import directory_manager

    # v6.1:优先用 URL 传入的 session_id(指定历史 session),
    # 找不到 transcript 时回退到当前 Helen session
    cwd = directory_manager.get_current_cwd()
    helen_sid = session_id

    def _find_transcript(sid: str):
        path = Path(cwd) / ".helen" / "sessions" / sid / "transcript.jsonl"
        return path if path.exists() else None

    transcript_path = _find_transcript(helen_sid)
    if not transcript_path:
        # 回退到当前 Helen session
        try:
            bridge_sid = await helen_bridge.get_session_id()
            if bridge_sid:
                helen_sid = bridge_sid
                transcript_path = _find_transcript(helen_sid)
        except Exception:
            pass

    if not transcript_path:
        raise HTTPException(status_code=404, detail=f"Transcript not found for session {helen_sid}")

    # 解析 transcript.jsonl
    entries = []
    try:
        # v1.30.10: 显式指定 UTF-8 编码
        with open(transcript_path, encoding="utf-8") as f:
            for line_num, line in enumerate(f, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = json.loads(line)
                    entry["_line"] = line_num  # 加行号便于调试
                    entries.append(entry)
                except json.JSONDecodeError as e:
                    entries.append({
                        "type": "parse_error",
                        "line": line_num,
                        "error": str(e),
                        "raw": line[:200]
                    })
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to read transcript: {e}")

    # 过滤测试消息（以 [TEST] 开头的集成测试消息）
    from app.services.session_index import filter_test_messages
    entries = filter_test_messages(entries)

    # 过滤元数据条目（session_meta 等，不是实际对话内容）
    entries = [e for e in entries if e.get("type") != "session_meta"]

    # 统计信息
    roles = {}
    tool_calls_count = 0
    for e in entries:
        if e.get("type") == "message":
            role = e.get("role", "unknown")
            roles[role] = roles.get(role, 0) + 1
            # Helen runtime 不填充结构化 tool_calls，从 content 文本中提取
            if e.get("tool_calls"):
                tool_calls_count += len(e["tool_calls"])
            else:
                content = str(e.get("content", ""))
                # 匹配 "Tool calls: [func1(...) → ..., func2(...) → ...]" 格式
                if content.startswith("Tool calls:"):
                    import re
                    tool_calls_count += len(re.findall(r'\w+\(', content))

    return {
        "session_id": helen_sid,
        "file": str(transcript_path),
        "total_entries": len(entries),
        "roles": roles,
        "tool_calls_count": tool_calls_count,
        "entries": entries,
    }


@http_router.get("/sessions/{session_id}/media/{filename}")
async def get_session_media(session_id: str, filename: str):
    """获取 session 的 media 文件(图片/音频等)

    Helen runtime 把用户上传的文件复制到 <sid>/media/<filename>,
    transcript 里的 media_ref 用绝对路径引用。本端点通过 HTTP URL
    暴露 media 文件给前端(AttachmentView 通过 url 渲染)。
    """
    # 防路径遍历
    if "/" in filename or "\\" in filename or ".." in filename or not filename:
        raise HTTPException(400, "Invalid filename")
    from app.services.session_index import get_transcript_path
    transcript_path = get_transcript_path(session_id)
    if not transcript_path:
        raise HTTPException(404, "Session not found")
    media_dir = transcript_path.parent / "media"
    media_path = media_dir / filename
    # v1.39.7: realpath 校验，确保文件在 media 目录内（防符号链接逃逸）
    import os
    real_media = os.path.realpath(media_path)
    real_media_dir = os.path.realpath(media_dir)
    if not real_media.startswith(real_media_dir + os.sep) and real_media != real_media_dir:
        raise HTTPException(403, "Access denied")
    if not media_path.exists() or not media_path.is_file():
        raise HTTPException(404, "Media file not found")
    import mimetypes
    mime = mimetypes.guess_type(filename)[0] or "application/octet-stream"
    return FileResponse(media_path, media_type=mime)


@ws_router.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket, token: Optional[str] = Query(default=None), lang: str = Query(default="en")):
    """WebSocket 聊天接口(广播模式,无 session_id)

    v6.1:单会话架构,所有连接共享当前工作目录。session_id 内部保留
    (供 helen_bridge 流式回调索引和 hint_injector 队列索引),不用于 WS 路由。

    v1.39.4:hint queue 已简化为单例模式（忽略 session_id 参数），
    修复了 hint 注入失败的 bug（chat.py 用 cwd hash，actor 用 UUID，不匹配）。

    v1.42:读取 lang 查询参数,通过环境变量传递给 Helen actor 用于 i18n。
    """
    # 鉴权：WebSocket 握手前校验 token（来自 ?token= 查询参数）
    verify_ws_token(token)
    # 存储 lang 供 Helen actor 读取（i18n 语言选择）
    os.environ["HELEN_WEBUI_LANG"] = lang
    manager = websocket.app.state.websocket_manager
    await manager.connect(websocket)

    # session_id 用于 helen_bridge/hint_injector 内部索引(不用于 WS 路由)
    cwd = directory_manager.get_current_cwd()
    session_id = directory_manager.cwd_to_session_id(cwd)

    # 跟踪当前正在进行的流式任务，以便 cancel 消息能中断它
    stream_task: Optional[asyncio.Task] = None

    async def do_streaming(user_message: str, file_paths: Optional[List[str]] = None):
        """后台执行 Helen 流式调用(独立于 WebSocket 连接)

        即使 WebSocket 断开,推理也继续,结果写入 transcript。
        v6.1:不再写 DB(transcript 是唯一数据源)。
        """
        stream_registry.register(session_id)
        try:
            async for chunk in helen_bridge.run_chat_streaming(
                user_message, session_id, file_paths=file_paths or []
            ):
                chunk_type = chunk.get("type")
                content = chunk.get("content", "")

                if chunk_type == "llm_chunk":
                    # 发送 chunk(WebSocket 可能已断开,捕获异常)
                    try:
                        await manager.broadcast({
                            "type": "llm_chunk",
                            "data": {"content": content}
                        })
                    except Exception:
                        pass
                elif chunk_type == "status_update":
                    # status_update 的 content 是 JSON 字符串（Helen FFI json.dumps），
                    # 解析后平铺发送，避免前端多包一层 {content: json_string}
                    try:
                        parsed = json.loads(content) if isinstance(content, str) else content
                    except (json.JSONDecodeError, TypeError):
                        parsed = {}
                    try:
                        await manager.broadcast({
                            "type": "status_update",
                            "data": parsed
                        })
                    except Exception:
                        pass
                elif chunk_type in ("agent_start", "agent_end", "phase_start",
                                   "processing_start", "processing_complete",
                                   "hint_injected"):
                    try:
                        await manager.broadcast({
                            "type": chunk_type,
                            "data": {"content": content}
                        })
                    except Exception:
                        pass
                elif chunk_type == "error":
                    # Error may carry i18n_key directly (from helen_bridge
                    # exception), as JSON string in content (from Helen
                    # actor), or as plain text fallback
                    if "i18n_key" in chunk:
                        error_data = {"i18n_key": chunk["i18n_key"], "params": chunk.get("params", {})}
                    else:
                        error_data = {"content": content}
                        try:
                            parsed = json.loads(content) if isinstance(content, str) else None
                            if isinstance(parsed, dict) and "i18n_key" in parsed:
                                error_data = {"i18n_key": parsed["i18n_key"], "params": parsed.get("params", {})}
                        except (json.JSONDecodeError, TypeError):
                            pass
                    try:
                        await manager.broadcast({
                            "type": "error",
                            "data": error_data
                        })
                    except Exception:
                        pass

            # 正常完成(未被取消):发送完成信号
            try:
                await manager.broadcast({"type": "llm_complete"})
            except Exception:
                pass

        except asyncio.CancelledError:
            # 被 cancel 打断:partial response 已由 Helen 写入 transcript
            pass
        finally:
            stream_registry.unregister(session_id)

    async def do_goal_streaming(goal_text: str, file_paths: Optional[List[str]] = None):
        """Goal pursuit loop: stream multiple iterations until goal complete.

        每次迭代调用 actor 的 act()，检查目标是否完成，未完成则续传。
        通过 [GOAL_COMPLETE] / [GOAL_IN_PROGRESS] 标记判断完成状态。
        """
        stream_registry.register(session_id)
        max_iterations = DEFAULT_MAX_ITERATIONS
        current_prompt = build_goal_prompt(goal_text)
        accumulated_text = ""

        try:
            for iteration in range(max_iterations):
                # 发送进度通知
                try:
                    await manager.broadcast({
                        "type": "goal_progress",
                        "data": {
                            "iteration": iteration + 1,
                            "max_iterations": max_iterations,
                            "goal": goal_text,
                        }
                    })
                except Exception:
                    pass

                # 单次流式调用，同时收集响应文本
                iteration_text = ""
                async for chunk in helen_bridge.run_chat_streaming(
                    current_prompt, session_id, file_paths=file_paths or []
                ):
                    chunk_type = chunk.get("type")
                    content = chunk.get("content", "")

                    if chunk_type == "llm_chunk":
                        iteration_text += content
                        # 转发给前端
                        try:
                            await manager.broadcast({
                                "type": "llm_chunk",
                                "data": {"content": content}
                            })
                        except Exception:
                            pass
                    elif chunk_type in ("status_update", "agent_start", "agent_end",
                                       "phase_start", "processing_start",
                                       "processing_complete", "hint_injected"):
                        try:
                            await manager.broadcast({
                                "type": chunk_type,
                                "data": {"content": content}
                            })
                        except Exception:
                            pass
                    elif chunk_type == "error":
                        try:
                            await manager.broadcast({
                                "type": "error",
                                "data": {"content": content}
                            })
                        except Exception:
                            pass

                accumulated_text += "\n\n" + iteration_text

                # 检查目标是否完成
                if goal_appears_complete(iteration_text):
                    goal_status = parse_goal_status(iteration_text)
                    summary = goal_status.get("summary", "")
                    try:
                        await manager.broadcast({
                            "type": "goal_complete",
                            "data": {
                                "status": "complete",
                                "message": "✅ 目标完成",
                                "summary": summary,
                                "iterations": iteration + 1,
                            }
                        })
                    except Exception:
                        pass
                    break

                # 未完成 — 准备续传
                if iteration < max_iterations - 1:
                    current_prompt = build_continuation_prompt(goal_text, iteration_text)
                else:
                    # 达到最大迭代
                    try:
                        await manager.broadcast({
                            "type": "goal_complete",
                            "data": {
                                "status": "max_iterations",
                                "message": f"⚠️ 达到最大迭代次数 ({max_iterations})",
                                "summary": "",
                                "iterations": max_iterations,
                            }
                        })
                    except Exception:
                        pass

            # 正常完成
            try:
                await manager.broadcast({"type": "llm_complete"})
            except Exception:
                pass

        except asyncio.CancelledError:
            pass
        finally:
            stream_registry.unregister(session_id)

    try:
        while True:
            data = await websocket.receive_json()

            if data.get("type") == "message":
                # 流正在跑 → 拒绝（提示用户改用 💡 提示功能）
                if stream_task and not stream_task.done():
                    await manager.broadcast({
                        "type": "error",
                        "data": {"i18n_key": "error.llmProcessing"}
                    })
                    continue

                user_message = data.get("content", "")

                # v6.0 单会话架构：移除了 __helen_resume__ / __helen_init__ 协议
                # 会话恢复现在由 ChatSession.main 内部直接使用 get_session_id() resume
                # 前端不再需要发送静默命令或维护 localStorage 中的 session ID

                # ── 斜杠命令：同步执行，响应作为用户气泡返回 ──
                # 斜杠命令（/help /compress /context 等）只修改 Helen 内部状态，
                # 不产生对 LLM 的可见对话。chat.py 直接 run_silent 执行。
                # /dir 命令特殊处理：切换工作目录 或 查询当前目录
                # - `/dir`          → 查询当前目录（无参数）
                # - `/dir <path>`   → 切换到指定路径
                if user_message == "/dir" or user_message.startswith("/dir "):
                    path = user_message[4:].strip() if len(user_message) > 4 else ""
                    current_cwd = directory_manager.get_current_cwd()

                    if not path:
                        # /dir 无参数 → 查询当前目录（不切换、不重启 actor）
                        display_name = directory_manager.get_display_name(current_cwd)
                        await manager.broadcast({
                            "type": "processing_complete",
                            "data": {
                                "i18n_key": "dir.currentDir",
                                "params": {"path": current_cwd, "name": display_name},
                                "is_slash_response": True
                            }
                        })
                        continue

                    result = directory_manager.set_current_cwd(path)

                    if result["status"] == "ok":
                        new_cwd = result["cwd"]

                        # 实际目录变化才重启 actor（避免 /dir <当前目录> 的无谓重启）
                        if new_cwd != current_cwd:
                            new_session_id = directory_manager.cwd_to_session_id(new_cwd)

                            try:
                                from app.services.channel_actor_manager import channel_actor_manager
                                if channel_actor_manager._actor_spawned:
                                    channel_actor_manager.exit_actor()
                            except Exception:
                                pass

                            # 获取新目录的 Helen session ID
                            helen_sid = None
                            try:
                                helen_sid = await helen_bridge.get_session_id()
                            except Exception as e:
                                import logging
                                logging.getLogger(__name__).warning(
                                    f"get_session_id() failed after /dir: {e}"
                                )

                            # 通知前端目录已切换（含新 session_id，前端据此重建 WebSocket）
                            await manager.broadcast({
                                "type": "directory_changed",
                                "data": {
                                    "cwd": new_cwd,
                                    "display_name": result["display_name"],
                                    "session_id": new_session_id,
                                    "helen_session_id": helen_sid,
                                }
                            })

                        # 发送成功响应（i18n 化，前端按语言偏好翻译）
                        await manager.broadcast({
                            "type": "processing_complete",
                            "data": {
                                "i18n_key": "dir.switchedTo",
                                "params": {"name": result["display_name"], "path": new_cwd},
                                "is_slash_response": True
                            }
                        })
                    else:
                        # 切换失败（i18n 化）
                        await manager.broadcast({
                            "type": "processing_complete",
                            "data": {
                                "i18n_key": "dir.switchFailed",
                                "params": {"reason": result["message"]},
                                "is_slash_response": True
                            }
                        })
                    continue

                # ── /goal 命令：目标驱动自动续传 ──
                if user_message.startswith("/goal "):
                    goal_text = user_message[6:].strip()
                    if not goal_text:
                        await manager.broadcast({
                            "type": "processing_complete",
                            "data": {"content": "用法: /goal <目标描述>\n例如: /goal 写一个 Python 计算器", "is_slash_response": True}
                        })
                        continue

                    # 启动 goal 循环（后台任务，不阻塞 WS 接收）
                    stream_task = asyncio.create_task(do_goal_streaming(goal_text, []))
                    continue

                # ── 斜杠命令:同步执行,不持久化 ──
                if user_message.startswith("/"):
                    response = await helen_bridge.run_silent(user_message)

                    # /clear 的响应中嵌入静默标记 __HELEN_CLEAR_OK__
                    is_clear = response and "__HELEN_CLEAR_OK__" in response
                    if is_clear:
                        response = response.replace("__HELEN_CLEAR_OK__", "").strip()

                    # /clear-session 的响应中嵌入静默标记 __HELEN_CLEAR_SESSION_OK__
                    is_clear_session = response and "__HELEN_CLEAR_SESSION_OK__" in response
                    if is_clear_session:
                        response = response.replace("__HELEN_CLEAR_SESSION_OK__", "").strip()

                    # /clear-session 后 actor 退出，需要重启
                    is_restart = response and "__HELEN_RESTART_ACTOR__" in response
                    if is_restart:
                        response = response.replace("__HELEN_RESTART_ACTOR__", "").strip()

                    if is_clear:
                        # /clear:transcript 已插入 BoundaryMarker,前端清空显示
                        await manager.broadcast({"type": "clear_messages", "data": {}})
                    elif response:
                        await manager.broadcast({
                            "type": "processing_complete",
                            "data": {"content": response, "is_slash_response": True}
                        })
                    else:
                        # 空响应（无输出类命令）：发完成信号即可
                        await manager.broadcast({
                            "type": "processing_complete",
                            "data": {}
                        })

                    # /clear-session 后重启 actor
                    if is_restart:
                        try:
                            from app.services.channel_actor_manager import channel_actor_manager
                            channel_actor_manager.restart_actor()
                        except Exception:
                            pass

                    continue

                # 提取附件（多模态支持）
                attachment_ids = data.get("attachments") or []
                file_paths = []
                if attachment_ids:
                    cwd = directory_manager.get_current_cwd()
                    for upload_id in attachment_ids:
                        # 防御：upload_id 必须是合法 UUID 格式，防止路径遍历
                        if not upload_id or "/" in upload_id or "\\" in upload_id or ".." in upload_id:
                            continue
                        file_path = Path(cwd) / ".helen" / "uploads" / upload_id / "file"
                        if file_path.exists():
                            file_paths.append(str(file_path))

                # 启动后台流式任务(不阻塞 WS 接收循环)
                # v6.1:不写 DB,transcript 由 Helen runtime 自动写入
                stream_task = asyncio.create_task(do_streaming(user_message, file_paths))

            elif data.get("type") == "hint":
                # 推理中追加提示：入队，不取消当前流。
                # Helen 的 on_tool_end 回调会在下一个工具结束后读取并注入。
                hint_text = data.get("content", "")
                client_id = data.get("client_id", "")
                if not hint_text or not hint_text.strip():
                    continue
                hint_injector.enqueue_hint(session_id, hint_text.strip(), client_id)
                await manager.broadcast({
                    "type": "hint_queued",
                    "data": {"content": hint_text, "client_id": client_id}
                })

            elif data.get("type") == "cancel":
                # 用户请求中断当前 LLM 流
                if stream_task and not stream_task.done():
                    helen_bridge.cancel_session(session_id)
                    stream_task.cancel()
                    try:
                        await stream_task
                    except (asyncio.CancelledError, Exception):
                        pass
                    stream_task = None
                    await manager.broadcast({
                        "type": "cancelled",
                        "data": {"content": ""}
                    })

    except WebSocketDisconnect:
        # v6.1:不取消 stream_task,推理继续写入 transcript。
        # 用户返回页面时从 transcript 加载完整响应。
        # await stream_task 让它跑完后再返回。
        hint_injector.clear_session(session_id)
        manager.disconnect(websocket)
        if stream_task and not stream_task.done():
            try:
                await stream_task
            except (asyncio.CancelledError, Exception):
                pass
    except Exception as e:
        print(f"WebSocket error: {e}")
        hint_injector.clear_session(session_id)
        manager.disconnect(websocket)
        if stream_task and not stream_task.done():
            try:
                await stream_task
            except (asyncio.CancelledError, Exception):
                pass


@http_router.post("/reload")
async def reload_helen():
    """手动触发 Helen 代码热重载

    清除缓存的 agent 实例和 Python 模块，下次请求将使用最新代码。
    """
    helen_bridge.force_reload()
    return {
        "status": "ok",
        "message": "Helen 代码已重新加载",
        "reload_count": helen_bridge._reload_count
    }


# === 文件上传 API（多模态支持） ===

@http_router.post("/upload")
async def upload_file(
    file: UploadFile = File(...),
    session_id: str = Form(None),
):
    """上传文件用于多模态交互

    接收 multipart/form-data 文件，保存到 .helen/uploads/<upload_id>/。
    返回 upload_id 和 metadata，前端在发送消息时携带 upload_id 列表。

    支持的 MIME 类型：
    - 图片：image/jpeg, image/png, image/gif, image/webp
    - 音频：audio/mpeg, audio/wav, audio/ogg, audio/mp4, audio/m4a
    - 视频：video/mp4, video/webm, video/quicktime

    文件大小限制：50MB
    """
    # 验证 MIME 类型
    if file.content_type not in ALLOWED_MIME_TYPES:
        raise HTTPException(
            400,
            f"Unsupported file type: {file.content_type}. "
            f"Allowed: {', '.join(sorted(ALLOWED_MIME_TYPES))}"
        )

    # 读取文件内容并验证大小
    contents = await file.read()
    if len(contents) > MAX_FILE_SIZE:
        raise HTTPException(
            413,
            f"File too large ({len(contents)} bytes). Max: {MAX_FILE_SIZE} bytes (50MB)"
        )

    # 生成 upload_id 并保存文件
    cwd = directory_manager.get_current_cwd()
    upload_id = str(uuid.uuid4())
    upload_dir = Path(cwd) / ".helen" / "uploads" / upload_id
    upload_dir.mkdir(parents=True, exist_ok=True)

    # 保存元数据
    metadata = {
        "upload_id": upload_id,
        "filename": file.filename,
        "mime_type": file.content_type,
        "size": len(contents),
        "created_at": datetime.now().isoformat(),
    }
    # v1.30.10: 显式指定 UTF-8 编码
    (upload_dir / "metadata.json").write_text(json.dumps(metadata), encoding="utf-8")

    # 保存文件内容
    (upload_dir / "file").write_bytes(contents)

    return {
        "upload_id": upload_id,
        "filename": file.filename,
        "mime_type": file.content_type,
        "size": len(contents),
        "url": f"/api/chat/uploads/{upload_id}/file",
    }


@http_router.get("/uploads/{upload_id}/file")
async def get_upload_file(upload_id: str):
    """获取已上传的文件

    通过 upload_id 访问已上传的文件，返回文件内容和正确的 MIME 类型。
    """
    # 验证 upload_id 格式（防止路径遍历）
    if not upload_id or "/" in upload_id or "\\" in upload_id or ".." in upload_id:
        raise HTTPException(400, "Invalid upload_id")

    cwd = directory_manager.get_current_cwd()
    upload_dir = Path(cwd) / ".helen" / "uploads" / upload_id
    file_path = upload_dir / "file"

    # v1.39.7: realpath 校验，确保文件在 uploads 目录内（防符号链接逃逸）
    import os
    real_file = os.path.realpath(file_path)
    real_upload_dir = os.path.realpath(upload_dir)
    if not real_file.startswith(real_upload_dir + os.sep) and real_file != real_upload_dir:
        raise HTTPException(403, "Access denied")

    if not file_path.exists():
        raise HTTPException(404, "File not found")

    # 读取 metadata 获取 MIME 类型
    metadata_path = upload_dir / "metadata.json"
    if not metadata_path.exists():
        raise HTTPException(404, "File metadata not found")

    # v1.30.10: 显式指定 UTF-8 编码
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    return FileResponse(file_path, media_type=metadata["mime_type"])
