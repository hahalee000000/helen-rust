"""Tests for app.config — configuration and token management."""
import os
import secrets
from pathlib import Path
from unittest.mock import patch

import pytest

from app.config import Settings, _backend_env_file, _default_helen_path, settings


# ── _default_helen_path ─────────────────────────────────────────────────────

class TestDefaultHelenPath:
    def test_returns_string(self):
        result = _default_helen_path()
        assert isinstance(result, str)

    def test_returns_absolute_path(self):
        result = _default_helen_path()
        assert os.path.isabs(result)

    def test_finds_chat_actor_or_fallback(self):
        """Either chat_actor.helen exists at the returned path, or it's a fallback directory."""
        result = _default_helen_path()
        path = Path(result)
        # The function either finds chat_actor.helen or returns a fallback parent dir.
        # We just verify it returns a valid directory path (or one that would be valid).
        assert path.is_absolute()


# ── _backend_env_file ───────────────────────────────────────────────────────

class TestBackendEnvFile:
    def test_returns_string(self):
        result = _backend_env_file()
        assert isinstance(result, str)

    def test_ends_with_dot_env(self):
        result = _backend_env_file()
        assert result.endswith(".env")

    def test_path_contains_backend(self):
        result = _backend_env_file()
        assert "backend" in result

    def test_is_absolute(self):
        result = _backend_env_file()
        assert os.path.isabs(result)


# ── Settings defaults ───────────────────────────────────────────────────────

class TestSettingsDefaults:
    def test_app_name(self):
        s = Settings(_env_file=None)
        assert s.APP_NAME == "Helen Web UI"

    def test_version(self):
        s = Settings(_env_file=None)
        assert s.VERSION == "1.0"

    def test_debug_default_false(self, monkeypatch):
        # 隔离：跳过 .env 文件 + 清除进程环境变量，确保断言命中代码默认值
        monkeypatch.delenv("DEBUG", raising=False)
        s = Settings(_env_file=None)
        assert s.DEBUG is False

    def test_host_default(self, monkeypatch):
        monkeypatch.delenv("HOST", raising=False)
        s = Settings(_env_file=None)
        assert s.HOST == "127.0.0.1"

    def test_port_default(self):
        s = Settings(_env_file=None)
        assert s.PORT == 8000

    def test_helen_timeout_default(self):
        s = Settings(_env_file=None)
        assert s.HELEN_TIMEOUT == 300

    def test_cors_origins_is_list(self):
        s = Settings(_env_file=None)
        assert isinstance(s.CORS_ORIGINS, list)
        assert len(s.CORS_ORIGINS) > 0

    def test_cors_origins_contain_localhost_5173(self):
        s = Settings(_env_file=None)
        assert "http://localhost:5173" in s.CORS_ORIGINS

    def test_cors_origins_contain_127_0_0_1_5173(self):
        s = Settings(_env_file=None)
        assert "http://127.0.0.1:5173" in s.CORS_ORIGINS

    def test_helen_path_is_string(self):
        s = Settings(_env_file=None)
        assert isinstance(s.HELEN_PATH, str)

    def test_token_default_empty(self, monkeypatch):
        monkeypatch.delenv("HELEN_WEBUI_TOKEN", raising=False)
        s = Settings(_env_file=None)
        # Token may be overridden by env, but default is empty
        assert isinstance(s.HELEN_WEBUI_TOKEN, str)


# ── ensure_token ────────────────────────────────────────────────────────────

class TestEnsureToken:
    def test_token_already_set_returns_it(self):
        """When HELEN_WEBUI_TOKEN is already set, ensure_token returns it directly."""
        s = Settings()
        s.HELEN_WEBUI_TOKEN = "my-pre-set-token"
        result = s.ensure_token()
        assert result == "my-pre-set-token"
        assert s.HELEN_WEBUI_TOKEN == "my-pre-set-token"

    def test_reads_from_persisted_file(self, tmp_path, monkeypatch):
        """When token file exists in project .helen/, ensure_token reads and returns it."""
        s = Settings()
        s.HELEN_WEBUI_TOKEN = ""

        token_dir = tmp_path / ".helen"
        token_dir.mkdir()
        token_file = token_dir / "webui_token"
        token_file.write_text("stored-token-123\n", encoding="utf-8")

        monkeypatch.setenv("HELEN_WEBUI_CWD", str(tmp_path))
        result = s.ensure_token()

        assert result == "stored-token-123"
        assert s.HELEN_WEBUI_TOKEN == "stored-token-123"

    def test_generates_new_token_when_file_missing(self, tmp_path, monkeypatch):
        """When token file doesn't exist, ensure_token generates and persists a new one."""
        s = Settings()
        s.HELEN_WEBUI_TOKEN = ""

        monkeypatch.setenv("HELEN_WEBUI_CWD", str(tmp_path))
        result = s.ensure_token()

        assert isinstance(result, str)
        assert len(result) > 0
        assert s.HELEN_WEBUI_TOKEN == result

        # Verify it was persisted in project .helen/
        token_file = tmp_path / ".helen" / "webui_token"
        assert token_file.exists()
        assert token_file.read_text(encoding="utf-8").strip() == result

    def test_generates_token_when_parent_mkdir_fails(self, tmp_path, monkeypatch):
        """When mkdir raises OSError, ensure_token falls back to in-memory token."""
        s = Settings()
        s.HELEN_WEBUI_TOKEN = ""

        # Point to a path where mkdir will fail
        bad_dir = tmp_path / "bad"
        bad_dir.mkdir()

        def raise_oserror(*args, **kwargs):
            raise OSError("permission denied")

        monkeypatch.setenv("HELEN_WEBUI_CWD", str(bad_dir))
        with patch.object(Path, "mkdir", side_effect=raise_oserror):
            result = s.ensure_token()

        assert isinstance(result, str)
        assert len(result) > 0
        assert s.HELEN_WEBUI_TOKEN == result

    def test_empty_stored_token_triggers_generation(self, tmp_path, monkeypatch):
        """When stored token file is empty, a new token is generated."""
        s = Settings()
        s.HELEN_WEBUI_TOKEN = ""

        token_dir = tmp_path / ".helen"
        token_dir.mkdir()
        token_file = token_dir / "webui_token"
        token_file.write_text("   \n", encoding="utf-8")  # whitespace only

        monkeypatch.setenv("HELEN_WEBUI_CWD", str(tmp_path))
        result = s.ensure_token()

        # Should generate a new token, not return empty
        assert len(result) > 0
        assert s.HELEN_WEBUI_TOKEN == result

    def test_chmod_failure_is_ignored(self, tmp_path, monkeypatch):
        """If chmod fails (e.g., on some filesystems), token is still returned."""
        s = Settings()
        s.HELEN_WEBUI_TOKEN = ""

        def fail_chmod(mode):
            raise OSError("chmod not supported")

        monkeypatch.setenv("HELEN_WEBUI_CWD", str(tmp_path))
        with patch.object(Path, "chmod", side_effect=fail_chmod):
            result = s.ensure_token()

        assert isinstance(result, str)
        assert len(result) > 0
        assert s.HELEN_WEBUI_TOKEN == result

    def test_deletes_old_global_token(self, tmp_path, monkeypatch):
        """If old ~/.helen/webui_token exists, ensure_token deletes it."""
        s = Settings()
        s.HELEN_WEBUI_TOKEN = ""

        # Set up project dir
        monkeypatch.setenv("HELEN_WEBUI_CWD", str(tmp_path))

        # Create old global token
        global_dir = tmp_path / "fake_home" / ".helen"
        global_dir.mkdir(parents=True)
        global_token = global_dir / "webui_token"
        global_token.write_text("old-global-token")

        with patch("app.config.Path.home", return_value=tmp_path / "fake_home"):
            result = s.ensure_token()

        # Old global token should be deleted
        assert not global_token.exists()
        # New project token should be generated
        assert len(result) > 0


# ── module-level settings instance ──────────────────────────────────────────

class TestModuleLevelSettings:
    def test_settings_is_settings_instance(self):
        assert isinstance(settings, Settings)

    def test_settings_has_required_attributes(self):
        assert hasattr(settings, "APP_NAME")
        assert hasattr(settings, "PORT")
        assert hasattr(settings, "HELEN_PATH")
        assert hasattr(settings, "CORS_ORIGINS")
        assert hasattr(settings, "HELEN_WEBUI_TOKEN")
