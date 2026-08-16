"""Agent 管理路由测试

覆盖 /api/agents/* 端点:
- GET /status (所有 Agent 状态)
- GET /{name}/status (单个 Agent, 包括 not found 分支)
- GET /list (Agent 名称列表)

运行: cd webui/backend && pytest tests/test_agents.py -v
"""
import pytest
from unittest.mock import patch
from fastapi.testclient import TestClient
from app.main import app


@pytest.fixture
def client():
    """TestClient for the main app (auth disabled by conftest)."""
    return TestClient(app)


class TestGetAllAgentsStatus:
    def test_returns_all_agent_states(self, client):
        """GET /api/agents/status 返回所有 agent 状态"""
        resp = client.get("/api/agents/status")
        assert resp.status_code == 200
        data = resp.json()
        assert isinstance(data, dict)
        # Should have the known agents
        assert "Contractor" in data
        assert "TestBuilder" in data
        assert "Implementer" in data
        assert "QualityGate" in data
        assert "SkillEvaluator" in data

    def test_each_agent_has_status_field(self, client):
        """每个 agent 状态包含 status 字段"""
        resp = client.get("/api/agents/status")
        data = resp.json()
        for name, state in data.items():
            assert "status" in state


class TestGetAgentStatus:
    def test_existing_agent(self, client):
        """GET /api/agents/Contractor/status 返回该 agent 状态"""
        resp = client.get("/api/agents/Contractor/status")
        assert resp.status_code == 200
        data = resp.json()
        assert data["name"] == "Contractor"
        assert "status" in data

    def test_nonexistent_agent(self, client):
        """GET /api/agents/Unknown/status 返回 error"""
        resp = client.get("/api/agents/NonExistentAgent/status")
        assert resp.status_code == 200
        data = resp.json()
        assert "error" in data
        assert "not found" in data["error"].lower()


class TestListAgents:
    def test_returns_agent_names(self, client):
        """GET /api/agents/list 返回 agent 名称列表"""
        resp = client.get("/api/agents/list")
        assert resp.status_code == 200
        data = resp.json()
        assert isinstance(data, list)
        assert "Contractor" in data
        assert "TestBuilder" in data
        assert len(data) == 5
