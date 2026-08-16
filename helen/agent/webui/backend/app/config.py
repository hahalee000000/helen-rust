from pydantic_settings import BaseSettings
from typing import Optional
from pathlib import Path
import os
import secrets


def _default_helen_path() -> str:
    """自动推断 helenagent 项目根目录

    查找策略：从 webui 目录向上遍历，找到包含 chat_actor.helen 的目录。
    如果找不到，默认返回 webui 的父目录（通常是 helenagent/）。
    """
    # 从本文件向上查找（webui/backend/app/config.py → helenagent/）
    here = Path(__file__).resolve()
    # parents[3] = helenagent/（webui 的父目录）
    candidate = here.parents[3]
    if (candidate / "chat_actor.helen").exists():
        return str(candidate)

    # 兜底：从当前工作目录向上查找
    cwd = Path.cwd()
    for parent in [cwd] + list(cwd.parents):
        if (parent / "chat_actor.helen").exists():
            return str(parent)

    # 最后兜底：webui 的父目录
    return str(here.parents[3])


def _backend_env_file() -> str:
    """返回 backend/.env 的绝对路径

    修复前启动脚本总是 cd 到 backend/ 目录，pydantic-settings 用相对路径 ".env" 能正确加载。
    修复后进程 cwd 是用户的真实工作目录，必须用绝对路径指向 backend/.env。
    """
    # 本文件位于 webui/backend/app/config.py，backend/ 是 parents[1]
    return str(Path(__file__).resolve().parents[1] / ".env")


class Settings(BaseSettings):
    # 应用配置
    APP_NAME: str = "Helen Web UI"
    VERSION: str = "1.0"
    DEBUG: bool = False
    HOST: str = "127.0.0.1"
    PORT: int = 8000

    # Helen 配置（默认自动推断，可通过环境变量或 .env 覆盖）
    HELEN_PATH: str = _default_helen_path()
    HELEN_TIMEOUT: int = 300

    # ── 鉴权 ──────────────────────────────────────────────────────
    # Web UI 访问 token。为空串 "" 时禁用鉴权（仅用于开发/测试，日志会警告）。
    # 默认：首次启动时自动生成并持久化到 <cwd>/.helen/webui_token（项目级），
    # 后续启动复用。用户可通过 .env 中 HELEN_WEBUI_TOKEN=xxx 覆盖。
    HELEN_WEBUI_TOKEN: str = ""

    # CORS 配置（允许 vite 常用端口 5173-5180）
    CORS_ORIGINS: list[str] = [
        "http://localhost:5173", "http://127.0.0.1:5173",
        "http://localhost:5174", "http://127.0.0.1:5174",
        "http://localhost:5175", "http://127.0.0.1:5175",
        "http://localhost:5176", "http://127.0.0.1:5176",
        "http://localhost:5177", "http://127.0.0.1:5177",
        "http://localhost:5178", "http://127.0.0.1:5178",
        "http://localhost:5179", "http://127.0.0.1:5179",
        "http://localhost:5180", "http://127.0.0.1:5180",
    ]

    class Config:
        env_file = _backend_env_file()
        case_sensitive = True
        # v6.1 移除了 SQLite（transcript 作为 SSOT），保留 extra="ignore" 防止未来
        # .env 中的陈旧字段（如 DATABASE_URL）打断启动。
        extra = "ignore"

    def ensure_token(self) -> str:
        """解析 token：优先 .env / 环境变量；为空则加载持久化文件；都不存在则生成。

        Token 存储在项目目录 <cwd>/.helen/webui_token 中，每个项目独立。
        如果存在旧的全局 ~/.helen/webui_token，自动迁移并删除。

        Returns: 解析后的 token（空串表示禁用鉴权）。
        """
        if self.HELEN_WEBUI_TOKEN:
            return self.HELEN_WEBUI_TOKEN

        # 项目目录：<cwd>/.helen/webui_token
        project_cwd = os.environ.get("HELEN_WEBUI_CWD") or str(Path.cwd())
        token_path = Path(project_cwd) / ".helen" / "webui_token"

        # 迁移：删除旧的全局 token（如果存在）
        global_token_path = Path.home() / ".helen" / "webui_token"
        if global_token_path.exists():
            try:
                global_token_path.unlink()
            except OSError:
                pass

        try:
            token_path.parent.mkdir(parents=True, exist_ok=True)
            if token_path.exists():
                stored = token_path.read_text(encoding="utf-8").strip()
                if stored:
                    self.HELEN_WEBUI_TOKEN = stored
                    return stored
            # 生成新 token 并落盘
            new_token = secrets.token_urlsafe(32)
            token_path.write_text(new_token, encoding="utf-8")
            try:
                token_path.chmod(0o600)
            except OSError:
                pass
            self.HELEN_WEBUI_TOKEN = new_token
            return new_token
        except OSError:
            # 落盘失败就退化为每次启动生成新 token（内存中有效）
            new_token = secrets.token_urlsafe(32)
            self.HELEN_WEBUI_TOKEN = new_token
            return new_token


settings = Settings()
