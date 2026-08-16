"""Transcript 端点测试

v6.1:transcript 唯一数据源。覆盖:
- GET /sessions/{id}/transcript:读指定 Helen session 的 transcript
- GET /dir/messages:从当前 transcript 读取消息(过滤 boilerplate)
- GET /sessions:列出历史 Helen session

运行: cd webui/backend && pytest tests/test_transcript_endpoints.py -v
"""

import json
import pytest
from unittest.mock import patch
from fastapi.testclient import TestClient
from app.main import app


@pytest.fixture
def client():
    """FastAPI 测试客户端(v6.1 无 DB)"""
    return TestClient(app)


@pytest.fixture
def temp_agent_dir(tmp_path):
    """临时 agent 根目录 + .helen/sessions/"""
    agent_dir = tmp_path / "helenagent"
    agent_dir.mkdir()
    helen_dir = agent_dir / ".helen"
    helen_dir.mkdir()
    sessions_dir = helen_dir / "sessions"
    sessions_dir.mkdir()
    return agent_dir, sessions_dir


@pytest.fixture
def mock_session_index(temp_agent_dir):
    """Mock session_index 使用临时目录"""
    agent_dir, sessions_dir = temp_agent_dir
    from app.services import session_index, directory_manager
    original_agent = session_index._AGENT_DIR
    session_index._AGENT_DIR = agent_dir
    # mock helen_bridge + directory_manager 避免读到真实 ~/.helen/sessions/
    with patch("app.services.helen_bridge.helen_bridge.get_session_id_sync", return_value=""), \
         patch.object(directory_manager, "get_current_cwd", return_value=str(agent_dir)):
        yield session_index, sessions_dir
    session_index._AGENT_DIR = original_agent


def _write_transcript(sessions_dir, helen_sid, messages):
    """创建模拟 transcript 文件"""
    sid_dir = sessions_dir / helen_sid
    sid_dir.mkdir(parents=True, exist_ok=True)
    lines = [json.dumps(m) for m in messages]
    (sid_dir / "transcript.jsonl").write_text("\n".join(lines) + "\n")


# ── GET /sessions/{id}/transcript ──────────────────────────────

class TestGetTranscript:
    def test_get_transcript_by_helen_sid(self, client, mock_session_index):
        """用 URL 传入的 helen session_id 读指定 transcript"""
        _, sessions_dir = mock_session_index
        _write_transcript(sessions_dir, "helen-sid-1", [
            {"type": "session_meta", "timestamp": 1.0},
            {"type": "message", "role": "user", "content": "你好", "uuid": "u1"},
            {"type": "message", "role": "assistant", "content": "你好!", "uuid": "a1"},
        ])
        resp = client.get("/api/chat/sessions/helen-sid-1/transcript")
        assert resp.status_code == 200
        data = resp.json()
        assert data["session_id"] == "helen-sid-1"
        assert data["total_entries"] == 2  # session_meta 被过滤
        assert data["roles"]["user"] == 1
        assert data["roles"]["assistant"] == 1

    def test_get_transcript_not_found(self, client, mock_session_index):
        """transcript 不存在 -> 404"""
        resp = client.get("/api/chat/sessions/nonexistent-sid/transcript")
        assert resp.status_code == 404


# ── GET /dir/messages ──────────────────────────────────────────

class TestDirectoryMessages:
    def test_dir_messages_filters_boilerplate(self, client, mock_session_index):
        """/dir/messages 过滤 prompt boilerplate"""
        _, sessions_dir = mock_session_index
        _write_transcript(sessions_dir, "helen-sid-1", [
            {"type": "session_meta", "timestamp": 1.0},
            {"type": "message", "role": "user", "content": "## Identity\nYou are X\n## Reminders\nIMPORTANT: ...\n\n用户输入", "uuid": "u1"},
            {"type": "message", "role": "assistant", "content": "回复", "uuid": "a1"},
        ])
        with patch("app.services.session_index.get_current_helen_session_id", return_value="helen-sid-1"):
            resp = client.get("/api/chat/dir/messages")
        assert resp.status_code == 200
        msgs = resp.json()
        assert len(msgs) == 2
        assert msgs[0]["content"] == "用户输入"
        assert msgs[1]["content"] == "回复"


# ── GET /sessions ──────────────────────────────────────────────

class TestListSessions:
    def test_list_sessions(self, client, mock_session_index):
        """列出历史 Helen session"""
        with patch("app.services.helen_bridge.helen_bridge.list_sessions_sync") as mock_list:
            mock_list.return_value = [
                {"session_id": "session-1", "created_at": 1.0, "modified_at": 1.0,
                 "size_bytes": 100, "message_count": 1, "preview": "你好"},
                {"session_id": "session-2", "created_at": 2.0, "modified_at": 2.0,
                 "size_bytes": 100, "message_count": 1, "preview": "世界"},
            ]
            resp = client.get("/api/chat/sessions")
        assert resp.status_code == 200
        sessions = resp.json()
        assert len(sessions) == 2
        ids = [s["session_id"] for s in sessions]
        assert "session-1" in ids
        assert "session-2" in ids
