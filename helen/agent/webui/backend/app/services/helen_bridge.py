"""Helen 运行时桥接服务（支持热重载）

v1.0：actor 成为唯一模式。移除非 actor 路径。
"""
import asyncio
import sys
import os
import time
from typing import AsyncGenerator, Optional, Callable
from pathlib import Path
import json
from app.config import settings
from app.services.stream_manager import stream_manager

# 全局流式回调注册表
_stream_callbacks = {}

def register_stream_callback(session_id: str, callback: Callable):
    """注册流式回调"""
    _stream_callbacks[session_id] = callback

def unregister_stream_callback(session_id: str):
    """注销流式回调"""
    if session_id in _stream_callbacks:
        del _stream_callbacks[session_id]

def get_stream_callback(session_id: str) -> Optional[Callable]:
    """获取流式回调"""
    return _stream_callbacks.get(session_id)


class HelenFileWatcher:
    """监控 .helen 和 .py 文件变更（统一热重载）"""

    def __init__(self, watch_dirs: list[str], check_interval: float = 1.0):
        self.watch_dirs = watch_dirs
        self.check_interval = check_interval
        self._file_mtimes: dict[str, float] = {}
        self._onChange_callbacks: list[Callable] = []
        self._running = False
        self._task: Optional[asyncio.Task] = None
        self._file_mtimes = self._scan_files()  # 存储初始快照，避免首次检查误报

    def on_change(self, callback: Callable):
        self._onChange_callbacks.append(callback)

    def _scan_files(self) -> dict[str, float]:
        mtimes = {}
        for watch_dir in self.watch_dirs:
            watch_path = Path(watch_dir)
            if not watch_path.exists():
                continue
            for helen_file in watch_path.rglob("*.helen"):
                rel = helen_file.relative_to(watch_path)
                # 跳过 .helen/ 目录及其内部的所有内容（包括 .helen 目录本身）
                if ".helen" in rel.parts:
                    continue
                # 只跟踪文件，跳过目录
                if not helen_file.is_file():
                    continue
                try:
                    mtimes[str(helen_file)] = helen_file.stat().st_mtime
                except OSError:
                    pass
            for py_file in watch_path.glob("*.py"):
                try:
                    mtimes[str(py_file)] = py_file.stat().st_mtime
                except OSError:
                    pass
        return mtimes

    def _check_for_changes(self) -> list[str]:
        new_mtimes = self._scan_files()
        changed_files = []
        for filepath, mtime in new_mtimes.items():
            if filepath not in self._file_mtimes:
                changed_files.append(filepath)
            elif mtime > self._file_mtimes[filepath]:
                changed_files.append(filepath)
        for filepath in self._file_mtimes:
            if filepath not in new_mtimes:
                changed_files.append(filepath)
        if changed_files:
            pass  # 变更由 _watch_loop 处理，此处不重复打印
        self._file_mtimes = new_mtimes
        return changed_files

    async def start(self):
        if self._running:
            return
        self._running = True
        self._task = asyncio.create_task(self._watch_loop())
        print(f"[HelenFileWatcher] 开始监控 {len(self.watch_dirs)} 个目录")

    async def stop(self):
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
        print("[HelenFileWatcher] 已停止")

    async def _watch_loop(self):
        first_check = True
        while self._running:
            try:
                await asyncio.sleep(self.check_interval)
                changed = self._check_for_changes()
                # 首次检查跳过：_file_mtimes 初始为空，所有文件都是"新增"（误报）
                if first_check:
                    first_check = False
                    continue
                if changed:
                    print(f"[HelenFileWatcher] 检测到 {len(changed)} 个文件变更")
                    for callback in self._onChange_callbacks:
                        try:
                            callback(changed)
                        except Exception as e:
                            print(f"[HelenFileWatcher] 回调执行失败: {e}")
            except asyncio.CancelledError:
                break
            except Exception as e:
                print(f"[HelenFileWatcher] 监控循环错误: {e}")


class HelenBridge:
    """Helen 运行时桥接器（支持热重载）"""

    def __init__(self, helen_path: str = None, agent_dir: str = None):
        self.helen_path = helen_path or settings.HELEN_PATH
        self.agent_dir = agent_dir or os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(__file__))))
        self._initialized = False
        self._file_watcher: Optional[HelenFileWatcher] = None
        self._reload_count = 0

        if self.helen_path not in sys.path:
            sys.path.insert(0, self.helen_path)

        os.environ['PYTHONPATH'] = self.helen_path + ':' + os.environ.get('PYTHONPATH', '')

    def _ensure_initialized(self):
        """确保 Helen 已初始化"""
        if self._initialized:
            return

        try:
            from helen.python_bridge import install_import_hook
            install_import_hook()
            self._install_stream_callback()
            self._start_file_watcher()
            self._initialized = True
        except ImportError as e:
            print(f"[HelenBridge] ✗ 初始化失败: {e}", file=sys.stderr)
            raise RuntimeError(f"Python Bridge 不可用: {e}")

    def _start_file_watcher(self):
        # 热重载已禁用 — 防止 LLM 修改工作目录文件时触发重启
        pass

    async def _ensure_watcher_started(self):
        if self._file_watcher and not self._file_watcher._running:
            await self._file_watcher.start()

    def _on_helen_files_changed(self, changed_files: list[str]):
        """统一热重载：处理 .helen 和 .py 文件变更"""
        self._reload_count += 1

        py_changed = [f for f in changed_files if f.endswith('.py')]
        helen_changed = [f for f in changed_files if f.endswith('.helen')]

        parts = []
        if helen_changed:
            parts.append(f"{len(helen_changed)} 个 .helen")
        if py_changed:
            parts.append(f"{len(py_changed)} 个 .py")
        print(f"[HelenBridge] 热重载 #{self._reload_count}: 检测到变更 ({', '.join(parts)})")

        # 热重载前保存 session_id
        try:
            old_sid = self.get_session_id_sync()
            if old_sid:
                os.environ["HELEN_SESSION_ID"] = old_sid
                print(f"  - 保存 session_id: {old_sid}")
        except Exception:
            pass

        # 热重载前退出 actor
        try:
            from app.services.channel_actor_manager import channel_actor_manager
            if channel_actor_manager._actor_spawned:
                print("  - 退出长驻 ChatSessionActor（热重载）")
                channel_actor_manager.exit_actor()
        except Exception as e:
            print(f"  - 退出 actor 失败（可忽略）: {e}")

        # 清除初始化标志
        self._initialized = False

        # 清除 Python 模块缓存
        modules_to_remove = []
        for mod_name in list(sys.modules.keys()):
            if mod_name in ('chat_tui', 'chat_tui_web') or mod_name.startswith('chat_tui.'):
                modules_to_remove.append(mod_name)
        for mod_name in modules_to_remove:
            del sys.modules[mod_name]
            print(f"  - 移除模块: {mod_name}")

        # 清除 __pycache__
        if py_changed:
            try:
                pycache = Path(self.agent_dir) / "__pycache__"
                if pycache.exists():
                    for pyc in pycache.glob("chat_tui*.pyc"):
                        pyc.unlink()
                        print(f"  - 删除缓存: {pyc.name}")
            except Exception as e:
                print(f"  - 清除 __pycache__ 失败: {e}")

        print(f"[HelenBridge] 热重载完成，下次请求将使用新代码")

    def _install_stream_callback(self):
        try:
            from ui import stream_emitter
            stream_emitter.register_stream_callback(self._handle_stream_event)
        except ImportError as e:
            print(f"[HelenBridge] ✗ stream callback 注册失败: {e}", file=sys.stderr)

    def _handle_stream_event(self, event_type: str, data: str):
        if not _stream_callbacks:
            # 正常场景：斜杠命令（run_silent）触发 FFI 事件但无 session callback
            # 或 uvicorn 热重载期间残留事件。不需要警告。
            return
        for session_id, callback in _stream_callbacks.items():
            try:
                callback(event_type, data)
            except Exception as e:
                print(f"Stream callback error: {e}", file=sys.stderr)

    def force_reload(self):
        self._on_helen_files_changed([])

    def get_session_id_sync(self) -> str:
        """同步获取当前 Helen session ID(读 memento)

        v6.1:委托给 session_index.get_current_helen_session_id(),读 memento
        找 actor 实际用的 child session。不再 walk 最新 mtime(会读到非当前
        对话的 session)。memento 不存在 -> 返回空(actor 会新建)。
        """
        from app.services.session_index import get_current_helen_session_id
        return get_current_helen_session_id()

    def list_sessions_sync(self) -> list[dict]:
        """列出当前工作目录的 Helen sessions,按 modified_at desc 排序。

        v6.1:只扫描 directory_manager.get_current_cwd() 下的 .helen/sessions/,
        不扫描 ~/.helen/sessions/(避免列出全局历史 session)。
        每项:{session_id, created_at, modified_at, size_bytes, message_count, preview}
        """
        from pathlib import Path
        from app.services import directory_manager
        from app.services.session_index import read_session_preview

        sessions_dir = Path(directory_manager.get_current_cwd()) / ".helen" / "sessions"
        result = []
        if not sessions_dir.exists():
            return result

        try:
            for d in sessions_dir.iterdir():
                if not d.is_dir():
                    continue
                transcript = d / "transcript.jsonl"
                if not transcript.exists():
                    continue
                try:
                    stat = transcript.stat()
                    msg_count = 0
                    # v1.30.10: 显式指定 UTF-8 编码
                    with open(transcript, encoding="utf-8") as f:
                        for line in f:
                            if '"type": "message"' in line:
                                msg_count += 1
                    result.append({
                        "session_id": d.name,
                        "created_at": stat.st_ctime,
                        "modified_at": stat.st_mtime,
                        "size_bytes": stat.st_size,
                        "message_count": msg_count,
                        "preview": read_session_preview(d.name),
                    })
                except Exception:
                    continue
        except Exception:
            pass

        result.sort(key=lambda x: x["modified_at"], reverse=True)
        return result

    async def get_session_id(self) -> str:
        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(None, self.get_session_id_sync)

    def cancel_session(self, session_id: str) -> bool:
        """中断当前 session 的 Helen LLM 流"""
        try:
            from ui import stream_emitter
            stream_emitter.request_cancel()
            return True
        except ImportError:
            return False

    async def run_silent(self, user_input: str) -> str:
        """同步执行 Helen（不流式，不推 chunk 到前端），返回 agent 原始结果。

        用于斜杠命令（/help /stats /compress 等）。
        v1.0：路由通过 ChatSessionActor，命令在 actor 上下文中执行。
        """
        self._ensure_initialized()
        await self._ensure_watcher_started()
        loop = asyncio.get_event_loop()

        def _run():
            from app.services.channel_actor_manager import channel_actor_manager
            return channel_actor_manager.send_message(user_input, [])

        try:
            return await loop.run_in_executor(None, _run)
        except Exception as e:
            return f"__HELEN_SESSION_ERR__silent run failed: {e}"

    async def run_chat_streaming(
        self, user_input: str, session_id: str, file_paths: list = None
    ) -> AsyncGenerator[dict, None]:
        """运行 Helen 聊天并以流式方式返回结果。

        v1.0：始终使用 actor 模式。
        """
        async for chunk in self.run_chat_streaming_actor(
            user_input, session_id, file_paths=file_paths
        ):
            yield chunk

    async def run_chat_streaming_actor(
        self, user_input: str, session_id: str, file_paths: list = None
    ) -> AsyncGenerator[dict, None]:
        """Actor 模式的流式聊天。

        Actor 长驻，上下文在 store 中自然累积（无每请求 resume_session 开销）。
        流式 chunk 通过 FFI stream_emitter -> asyncio.Queue。
        """
        from app.services.channel_actor_manager import channel_actor_manager

        self._ensure_initialized()
        await self._ensure_watcher_started()

        try:
            from ui import stream_emitter
            stream_emitter.clear_cancel()
        except ImportError:
            pass

        loop = asyncio.get_event_loop()
        event_queue: asyncio.Queue = asyncio.Queue()

        def stream_callback(event_type: str, data: str):
            loop.call_soon_threadsafe(
                event_queue.put_nowait,
                {"type": event_type, "content": data}
            )

        register_stream_callback(session_id, stream_callback)

        try:
            pending_file_paths = file_paths or []

            def run_sync():
                channel_actor_manager.ensure_actor()
                return channel_actor_manager.send_message(user_input, pending_file_paths)

            helen_task = loop.run_in_executor(None, run_sync)

            full_response = ""
            while True:
                try:
                    event = await asyncio.wait_for(event_queue.get(), timeout=0.1)
                    event_type = event.get("type", "")
                    content = event.get("content", "")

                    if event_type == "llm_chunk":
                        full_response += content
                        yield {"type": "llm_chunk", "content": content}
                    elif event_type == "error":
                        # v1.38.1: Surface LLM errors to the frontend instead of
                        # silently dropping them. Previously the error events from
                        # chat_session_actor were swallowed here, leaving the user
                        # with no feedback when the LLM call failed.
                        yield {"type": "error", "content": content}
                    elif event_type in ("agent_start", "agent_end", "phase_start",
                                        "processing_start", "processing_complete",
                                        "llm_complete", "hint_injected", "status_update",
                                        "helen_session_id"):
                        yield {"type": event_type, "content": content}
                except asyncio.TimeoutError:
                    if helen_task.done():
                        # 防御性 drain：reply.send() 在 _emit_actor_stream_event("processing_complete")
                        # 之前执行，helen_task.done() 变 True 时可能有事件正在 call_soon_threadsafe
                        # 排队途中。非阻塞地把残留事件消费掉，避免 processing_complete 丢失。
                        while True:
                            try:
                                evt = event_queue.get_nowait()
                                evt_type = evt.get("type", "")
                                if evt_type in ("processing_complete", "llm_complete",
                                                "agent_end", "status_update", "hint_injected",
                                                "processing_start", "agent_start", "error"):
                                    yield {"type": evt_type, "content": evt.get("content", "")}
                            except asyncio.QueueEmpty:
                                break
                        break
                    continue

            response = await helen_task

            if not full_response and response:
                yield {"type": "llm_chunk", "content": response}

            yield {"type": "complete"}

        except Exception as e:
            yield {
                "type": "error",
                "i18n_key": "error.actorExecution",
                "params": {"message": str(e)}
            }
        finally:
            unregister_stream_callback(session_id)


# 全局实例
helen_bridge = HelenBridge()
