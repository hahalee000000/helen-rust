"""基于工作目录的会话管理

单会话架构的核心模块：目录 = 会话边界

每个项目目录自动拥有一个独立的会话，包含：
- .helen/webui.db（SQLite 消息历史）
- .helen/sessions/<sid>/transcript.jsonl（Helen transcript）
- .helen/MEMORY.md（项目记忆）
- .helen/USER.md（用户偏好）
"""
import hashlib
import os
from pathlib import Path

# 全局当前工作目录（单进程 = 单工作目录）
#
# 初始化策略：
#   优先 HELEN_WEBUI_CWD 环境变量（显式覆盖）；
#   否则用 os.getcwd()（start_webui.py 已通过 HELEN_WEBUI_CWD 环境变量传递工作目录）。
#
# 历史背景：
#   旧版启动脚本总是 cd 到 backend/ 目录，导致 os.getcwd() 永远是
#   ~/helenagent/webui/backend/，所有目录共享同一个 DB 和 Helen session。
#   修复后 start_webui.py 用 `python -c "os.chdir(USER_CWD); uvicorn.run(...)"`
#   保持进程 cwd 为用户启动 helen agent 时的目录。
#
# 防御性设计：
#   即使工作目录切换失败，HELEN_WEBUI_CWD 环境变量仍可强制指定。
#   两者都失败时退回 os.getcwd()。
def _init_cwd() -> str:
    env_cwd = os.environ.get("HELEN_WEBUI_CWD", "")
    if env_cwd:
        resolved = str(Path(env_cwd).resolve())
        if Path(resolved).is_dir():
            return resolved
        print(f"⚠️  HELEN_WEBUI_CWD={env_cwd} 不是有效目录，退回 os.getcwd()")
    return str(Path(os.getcwd()).resolve())

_current_cwd: str = _init_cwd()


def cwd_to_session_id(cwd: str) -> str:
    """把工作目录映射到稳定、URL 安全的 session_id。

    使用 SHA256(cwd)[:16] —— 短、URL 安全（hex）、确定性（同一 cwd 永远得到同一 ID）。
    避免把含 `/` 的 cwd 直接作为 URL path 段，否则 uvicorn 解码后路由会解析错。
    """
    return hashlib.sha256(cwd.encode("utf-8")).hexdigest()[:16]


def get_current_cwd() -> str:
    """获取当前工作目录

    Returns:
        当前工作目录的绝对路径

    防御：如果 _current_cwd 已被删除（测试场景：TemporaryDirectory 退出），
    回退到进程实际 cwd；如果进程 cwd 也失效，用临时目录兜底。
    """
    global _current_cwd
    # 检查逻辑 cwd 是否还有效
    if _current_cwd and Path(_current_cwd).is_dir():
        return _current_cwd
    # 逻辑 cwd 失效，尝试用进程 cwd
    try:
        proc_cwd = str(Path(os.getcwd()).resolve())
        if Path(proc_cwd).is_dir():
            _current_cwd = proc_cwd
            return _current_cwd
    except (FileNotFoundError, OSError):
        pass
    # 进程 cwd 也失效（比如被 chdir 到了已删除的目录），用 HOME 兜底
    fallback = str(Path.home())
    _current_cwd = fallback
    try:
        os.chdir(fallback)
    except OSError:
        pass
    print(f"⚠️  cwd 失效，回退到 {fallback}")
    return _current_cwd


def set_current_cwd(path: str) -> dict:
    """切换工作目录

    切换后，所有后续请求将使用新目录的：
    - SQLite 数据库（.helen/webui.db）
    - Helen session（通过 get_session_id()）
    - 记忆文件（.helen/MEMORY.md, USER.md）

    重要：同时调用 os.chdir()，让进程 cwd 与逻辑 cwd 一致。
    Helen TranscriptStore 用 os.getcwd() 决定 .helen/sessions/ 位置，
    如果只改 _current_cwd 不 chdir，TranscriptStore 不会跟着切换。

    注意：进程 cwd 是全局状态，理论上并发请求会有竞争。
    但 Web UI 是单用户本地服务，实际使用中没有并发切换目录的场景。

    Args:
        path: 目标目录路径（绝对或相对）

    Returns:
        {
            "status": "ok" | "error",
            "cwd": "/absolute/path",  # 成功时
            "display_name": "project-name",  # 成功时
            "message": "错误信息"  # 失败时
        }
    """
    global _current_cwd

    try:
        # 解析为绝对路径（此时 os.getcwd() 是上一个有效 cwd，相对路径能正确解析）
        # expanduser() 展开 ~ 和 ~user，resolve() 转为绝对路径并解析符号链接
        abs_path = str(Path(path).expanduser().resolve())

        # 验证目录存在
        if not Path(abs_path).is_dir():
            return {
                "status": "error",
                "message": f"目录不存在: {path}"
            }

        # 切换逻辑 cwd
        _current_cwd = abs_path

        # 同步切换进程 cwd —— Helen TranscriptStore 依赖这个
        try:
            os.chdir(abs_path)
        except OSError as e:
            print(f"⚠️  os.chdir({abs_path}) 失败: {e}（逻辑 cwd 已切换，Helen session 可能未跟随）")

        # 确保 .helen 目录存在
        helen_dir = Path(abs_path) / ".helen"
        helen_dir.mkdir(exist_ok=True)

        return {
            "status": "ok",
            "cwd": abs_path,
            "display_name": get_display_name(abs_path),
        }

    except Exception as e:
        return {
            "status": "error",
            "message": f"切换目录失败: {str(e)}"
        }


def get_display_name(cwd: str = None) -> str:
    """获取目录的显示名称

    Args:
        cwd: 工作目录路径，默认为当前目录

    Returns:
        目录名（如 "project"），根目录返回完整路径
    """
    if cwd is None:
        cwd = get_current_cwd()

    path = Path(cwd)
    return path.name or str(path)


def get_project_db_path() -> Path:
    """获取当前项目的 SQLite 数据库路径

    路径：<cwd>/.helen/webui.db

    Returns:
        数据库文件的 Path 对象
    """
    cwd = Path(get_current_cwd())
    helen_dir = cwd / ".helen"
    helen_dir.mkdir(exist_ok=True)
    return helen_dir / "webui.db"


def get_project_helen_session_dir() -> Path:
    """获取当前项目的 Helen session 目录

    路径：<cwd>/.helen/sessions/

    Returns:
        sessions 目录的 Path 对象
    """
    cwd = Path(get_current_cwd())
    return cwd / ".helen" / "sessions"


def get_project_memory_path() -> Path:
    """获取当前项目的 MEMORY.md 路径

    路径：<cwd>/.helen/MEMORY.md

    Returns:
        MEMORY.md 文件的 Path 对象
    """
    cwd = Path(get_current_cwd())
    return cwd / ".helen" / "MEMORY.md"


def get_project_user_path() -> Path:
    """获取当前项目的 USER.md 路径

    路径：<cwd>/.helen/USER.md

    Returns:
        USER.md 文件的 Path 对象
    """
    cwd = Path(get_current_cwd())
    return cwd / ".helen" / "USER.md"
