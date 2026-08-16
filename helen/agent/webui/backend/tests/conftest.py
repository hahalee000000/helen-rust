"""Pytest 配置"""
import pytest
import sys
import os
from pathlib import Path

# 添加项目根目录到 Python 路径
sys.path.insert(0, str(Path(__file__).parent.parent))


@pytest.fixture(autouse=True)
def _disable_webui_auth_for_tests():
    """测试期间禁用 Web UI token 鉴权。

    测试用 TestClient 直接调 FastAPI，不带 X-Helen-Token header。
    将 settings.HELEN_WEBUI_TOKEN 强制置空后，auth.require_auth 会直接放行。
    测试结束后恢复原值。
    """
    from app.config import settings
    original = settings.HELEN_WEBUI_TOKEN
    settings.HELEN_WEBUI_TOKEN = ""
    yield
    settings.HELEN_WEBUI_TOKEN = original


@pytest.fixture(autouse=True)
def _isolate_cwd_and_directory_manager():
    """隔离每个测试的 cwd 和 directory_manager 状态

    set_current_cwd() 现在会 os.chdir() 并修改 _current_cwd 全局变量。
    测试中用 TemporaryDirectory 切换目录后，with 退出会删除目录，
    导致后续测试的 os.getcwd() 失败（FileNotFoundError）。

    这个 autouse fixture：
    1. 保存测试前的 cwd 和 _current_cwd
    2. 测试结束后恢复两者
    3. 如果 _current_cwd 已被删除，回退到 backend 目录（确保不污染其他测试）
    """
    original_cwd = os.getcwd()
    from app.services import directory_manager
    original_logical_cwd = directory_manager._current_cwd

    yield

    # 恢复进程 cwd（如果原 cwd 还存在）
    try:
        if Path(original_cwd).is_dir():
            os.chdir(original_cwd)
    except (FileNotFoundError, OSError):
        pass

    # 恢复逻辑 cwd（如果原路径还存在）
    if original_logical_cwd and Path(original_logical_cwd).is_dir():
        directory_manager._current_cwd = original_logical_cwd
    else:
        # 原逻辑 cwd 已失效，回退到 backend 目录
        directory_manager._current_cwd = str(Path(__file__).resolve().parents[1])


@pytest.fixture
def anyio_backend():
    return 'asyncio'
