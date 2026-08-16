"""ChannelActorManager - 长驻 ChatSessionActor 的 Python 端管理器

负责：
1. Actor 生命周期（spawn / exit / restart）
2. 通过 mailbox 发送 user_input，阻塞等待 response_complete
3. 流式 chunk 不走本管理器（走 FFI stream_emitter，与 non-actor 路径相同）
4. 心跳保活（v1.29.12: 每 120s 发送心跳，防止 actor 300s 超时退出）

线程模型：
- spawn_chat_actor / tui_chat_handler_actor / exit_chat_actor 都是同步阻塞调用
- helen_bridge 在 executor 线程中调用它们
- 流式 chunk 通过 FFI 回调到达 asyncio.Queue（与 non-actor 路径一致）
- 心跳在独立守护线程中发送，不影响主流程

启用方式：环境变量 HELEN_USE_ACTOR=1
"""
import os
import threading
import logging
import time

logger = logging.getLogger(__name__)

# 心跳间隔（秒），必须小于 actor 的 receive 超时（300s）
HEARTBEAT_INTERVAL = 120


def actor_mode_enabled() -> bool:
    """Actor 模式始终启用（v1.0：actor 成为唯一模式）"""
    return True


class ChannelActorManager:
    """长驻 actor 的生命周期管理器（线程安全单例）"""

    def __init__(self):
        self._lock = threading.Lock()
        self._actor_spawned = False
        self._session_id: str | None = None
        self._heartbeat_thread: threading.Thread | None = None
        self._heartbeat_stop = threading.Event()

    def is_available(self) -> bool:
        """actor 模式是否可用（环境变量启用 + 接口存在）"""
        if not actor_mode_enabled():
            return False
        try:
            from chat_tui_web import is_actor_mode_available
            return is_actor_mode_available()
        except ImportError:
            return False

    def ensure_actor(self) -> dict:
        """确保 actor 已启动。

        返回 Helen spawn_chat_actor 的结果 map：
          {status: "started"|"already_running", session_id: "..."}
          {status: "error", error: "..."}
        """
        with self._lock:
            if self._actor_spawned:
                return {"status": "already_running", "session_id": self._session_id}
            from chat_tui_web import spawn_chat_actor
            result = spawn_chat_actor()
            status = result.get("status") if isinstance(result, dict) else None
            if status in ("started", "already_running"):
                self._actor_spawned = True
                self._session_id = result.get("session_id")
                logger.info("ChatSessionActor started (session=%s)", self._session_id)
                self._start_heartbeat()
            else:
                logger.warning("ChatSessionActor spawn failed: %s", result)
            return result if isinstance(result, dict) else {"status": "error", "error": str(result)}

    def _start_heartbeat(self):
        """启动心跳线程（v1.29.12: 每 120s 发送心跳，防止 actor 超时退出）"""
        self._heartbeat_stop.clear()
        self._heartbeat_thread = threading.Thread(
            target=self._heartbeat_loop, daemon=True, name="actor-heartbeat"
        )
        self._heartbeat_thread.start()
        logger.info("Heartbeat thread started (interval=%ds)", HEARTBEAT_INTERVAL)

    def _stop_heartbeat(self):
        """停止心跳线程"""
        self._heartbeat_stop.set()
        if self._heartbeat_thread and self._heartbeat_thread.is_alive():
            self._heartbeat_thread.join(timeout=5)
        self._heartbeat_thread = None

    def _heartbeat_loop(self):
        """心跳循环：每 HEARTBEAT_INTERVAL 秒发送一次心跳"""
        while not self._heartbeat_stop.is_set():
            # 使用 wait 代替 sleep，以便能快速响应停止信号
            if self._heartbeat_stop.wait(HEARTBEAT_INTERVAL):
                break  # 收到停止信号
            try:
                from chat_tui_web import send_heartbeat
                send_heartbeat()
                logger.debug("Heartbeat sent to actor")
            except Exception as e:
                logger.warning("Heartbeat failed: %s", e)
                break  # 心跳失败说明 actor 可能已死，退出心跳线程

    def send_message(self, user_input: str, file_paths: list | None = None) -> str:
        """发送消息到 actor，阻塞等待响应。

        流式 chunk 在阻塞期间通过 FFI stream_emitter 实时到达前端
        （不走本方法的返回值）。返回值是 actor 的完整响应文本。

        崩溃恢复：若调用异常（actor 已死），标记 actor 为未启动，
        下次调用会自动重新 spawn。
        """
        self.ensure_actor()
        from chat_tui_web import tui_chat_handler_actor
        try:
            return tui_chat_handler_actor(user_input, file_paths or [])
        except Exception as e:
            # actor 可能已崩溃（while 循环异常退出），标记为未启动以便下次重启
            logger.warning("send_message failed (actor may have crashed): %s", e)
            with self._lock:
                self._stop_heartbeat()
                self._actor_spawned = False
                self._session_id = None
            raise

    def exit_actor(self):
        """优雅退出 actor（用于热重载 / 进程退出）"""
        with self._lock:
            if not self._actor_spawned:
                return
            self._stop_heartbeat()
            try:
                from chat_tui_web import exit_chat_actor
                exit_chat_actor()
                logger.info("ChatSessionActor exited")
            except Exception as e:
                logger.warning("exit_chat_actor failed: %s", e)
            finally:
                self._actor_spawned = False
                self._session_id = None

    def restart_actor(self) -> dict:
        """重启 actor（用于 session 切换）"""
        self.exit_actor()
        return self.ensure_actor()

    def is_running(self) -> bool:
        """actor 是否正在运行（查询 Helen 端真实状态）"""
        try:
            from chat_tui_web import is_chat_actor_running
            return bool(is_chat_actor_running())
        except Exception:
            return False


# 全局单例
channel_actor_manager = ChannelActorManager()
