"""目录管理器测试

验证 directory_manager 模块的基本功能。

运行: cd webui/backend && pytest tests/test_directory_manager.py -v
"""
import os
import pytest
import tempfile
from pathlib import Path
from unittest.mock import patch
from app.services.directory_manager import (
    get_current_cwd,
    set_current_cwd,
    get_display_name,
    get_project_db_path,
    get_project_helen_session_dir,
    get_project_memory_path,
    get_project_user_path,
    cwd_to_session_id,
    _init_cwd,
)
from app.services import directory_manager


class TestDirectoryManager:
    """目录管理器测试"""

    def test_get_current_cwd(self):
        """获取当前工作目录"""
        cwd = get_current_cwd()
        assert isinstance(cwd, str)
        assert len(cwd) > 0
        assert Path(cwd).is_absolute()

    def test_set_current_cwd_valid(self):
        """切换到有效目录"""
        with tempfile.TemporaryDirectory() as tmpdir:
            result = set_current_cwd(tmpdir)
            assert result["status"] == "ok"
            assert result["cwd"] == tmpdir
            assert result["display_name"] == Path(tmpdir).name

            # 验证全局变量已更新
            assert get_current_cwd() == tmpdir

    def test_set_current_cwd_invalid(self):
        """切换到无效目录"""
        result = set_current_cwd("/nonexistent/path/12345")
        assert result["status"] == "error"
        assert "目录不存在" in result["message"]

    def test_set_current_cwd_creates_helen_dir(self):
        """切换目录时自动创建 .helen 目录"""
        with tempfile.TemporaryDirectory() as tmpdir:
            result = set_current_cwd(tmpdir)
            assert result["status"] == "ok"

            # 验证 .helen 目录已创建
            helen_dir = Path(tmpdir) / ".helen"
            assert helen_dir.exists()
            assert helen_dir.is_dir()

    def test_get_display_name(self):
        """获取目录显示名称"""
        # 普通目录
        name = get_display_name("/home/user/project")
        assert name == "project"

        # 嵌套目录
        name = get_display_name("/tmp/a/b/c")
        assert name == "c"

    def test_get_project_db_path(self):
        """获取项目数据库路径"""
        with tempfile.TemporaryDirectory() as tmpdir:
            set_current_cwd(tmpdir)
            db_path = get_project_db_path()

            # 验证路径正确
            assert db_path.parent == Path(tmpdir) / ".helen"
            assert db_path.name == "webui.db"

            # 验证 .helen 目录已创建
            assert db_path.parent.exists()

    def test_get_project_helen_session_dir(self):
        """获取 Helen session 目录"""
        with tempfile.TemporaryDirectory() as tmpdir:
            set_current_cwd(tmpdir)
            session_dir = get_project_helen_session_dir()

            # 验证路径正确
            assert session_dir == Path(tmpdir) / ".helen" / "sessions"

    def test_get_project_memory_path(self):
        """获取 MEMORY.md 路径"""
        with tempfile.TemporaryDirectory() as tmpdir:
            set_current_cwd(tmpdir)
            memory_path = get_project_memory_path()

            # 验证路径正确
            assert memory_path == Path(tmpdir) / ".helen" / "MEMORY.md"

    def test_get_project_user_path(self):
        """获取 USER.md 路径"""
        with tempfile.TemporaryDirectory() as tmpdir:
            set_current_cwd(tmpdir)
            user_path = get_project_user_path()

            # 验证路径正确
            assert user_path == Path(tmpdir) / ".helen" / "USER.md"

    def test_directory_isolation(self):
        """验证不同目录的数据库路径独立"""
        with tempfile.TemporaryDirectory() as tmpdir1:
            with tempfile.TemporaryDirectory() as tmpdir2:
                # 切换到目录 1
                set_current_cwd(tmpdir1)
                db_path_1 = get_project_db_path()

                # 切换到目录 2
                set_current_cwd(tmpdir2)
                db_path_2 = get_project_db_path()

                # 验证路径不同
                assert db_path_1 != db_path_2
                assert str(tmpdir1) in str(db_path_1)
                assert str(tmpdir2) in str(db_path_2)

    def test_cwd_to_session_id_deterministic(self):
        """同一 cwd 总是得到同一 session_id"""
        cwd = "/home/user/project"
        assert cwd_to_session_id(cwd) == cwd_to_session_id(cwd)

    def test_cwd_to_session_id_url_safe(self):
        """session_id 是 URL 安全的（纯 hex，无 / 等特殊字符）"""
        sid = cwd_to_session_id("/home/rxx/helenagent")
        assert sid.isalnum()
        assert len(sid) == 16

    def test_cwd_to_session_id_distinct(self):
        """不同 cwd 得到不同 session_id"""
        sid1 = cwd_to_session_id("/home/user/project-a")
        sid2 = cwd_to_session_id("/home/user/project-b")
        assert sid1 != sid2


class TestInitCwd:
    """_init_cwd() 环境变量和回退路径测试"""

    def test_env_var_valid_dir(self, tmp_path):
        """HELEN_WEBUI_CWD 指向有效目录 -> 返回该目录"""
        target = str(tmp_path / "myproject")
        os.makedirs(target, exist_ok=True)
        with patch.dict(os.environ, {"HELEN_WEBUI_CWD": target}):
            result = _init_cwd()
        assert result == str(Path(target).resolve())

    def test_env_var_nonexistent_dir_fallback(self, tmp_path, capsys):
        """HELEN_WEBUI_CWD 指向不存在目录 -> 回退到 os.getcwd()"""
        with patch.dict(os.environ, {"HELEN_WEBUI_CWD": "/nonexistent/path/xyz123"}):
            result = _init_cwd()
        # Should fall back to os.getcwd()
        assert Path(result).is_dir()
        captured = capsys.readouterr()
        assert "不是有效目录" in captured.out

    def test_env_var_empty(self):
        """HELEN_WEBUI_CWD 为空 -> 使用 os.getcwd()"""
        with patch.dict(os.environ, {"HELEN_WEBUI_CWD": ""}):
            result = _init_cwd()
        assert result == str(Path(os.getcwd()).resolve())

    def test_env_var_not_set(self):
        """HELEN_WEBUI_CWD 未设置 -> 使用 os.getcwd()"""
        env = os.environ.copy()
        env.pop("HELEN_WEBUI_CWD", None)
        with patch.dict(os.environ, env, clear=True):
            result = _init_cwd()
        assert Path(result).is_dir()


class TestGetCurrentCwdFallback:
    """get_current_cwd() 回退路径测试"""

    def test_fallback_to_home_when_cwd_deleted(self, tmp_path):
        """逻辑 cwd 和进程 cwd 都失效 -> 回退到 HOME"""
        # Create a temp dir, set it as logical cwd, then delete it
        deleted_dir = str(tmp_path / "will_be_deleted")
        os.makedirs(deleted_dir)
        original_cwd = directory_manager._current_cwd
        original_proc_cwd = os.getcwd()

        try:
            directory_manager._current_cwd = deleted_dir
            os.chdir(deleted_dir)
            # Now delete the directory
            os.chdir(str(tmp_path))
            os.rmdir(deleted_dir)

            # Also make os.getcwd() fail by patching
            with patch("os.getcwd", side_effect=FileNotFoundError("cwd deleted")):
                result = get_current_cwd()

            # Should fall back to HOME
            assert result == str(Path.home())
        finally:
            directory_manager._current_cwd = original_cwd
            try:
                os.chdir(original_proc_cwd)
            except (FileNotFoundError, OSError):
                os.chdir(str(Path.home()))

    def test_fallback_to_proc_cwd_when_logical_deleted(self, tmp_path):
        """逻辑 cwd 失效但进程 cwd 有效 -> 回退到进程 cwd"""
        deleted_dir = str(tmp_path / "will_be_deleted")
        os.makedirs(deleted_dir)
        original_cwd = directory_manager._current_cwd
        proc_cwd = os.getcwd()

        try:
            directory_manager._current_cwd = deleted_dir
            # Actually delete the directory so is_dir() returns False
            os.rmdir(deleted_dir)
            result = get_current_cwd()
            # Should fall back to process cwd
            assert result == str(Path(proc_cwd).resolve())
        finally:
            directory_manager._current_cwd = original_cwd

    def test_get_display_name_default_cwd(self):
        """get_display_name() 无参数 -> 使用 get_current_cwd()"""
        name = get_display_name()
        assert isinstance(name, str)
        assert len(name) > 0
