"""Token 鉴权依赖的单元测试"""
import pytest
from fastapi import Depends, FastAPI, WebSocket, WebSocketDisconnect
from fastapi.testclient import TestClient

from app.auth import require_auth, verify_ws_token, _token_matches
from app.config import settings


@pytest.fixture
def app_with_auth():
    """带鉴权的测试 app"""
    app = FastAPI()

    @app.get("/protected")
    async def protected(_token: str = Depends(require_auth)):
        return {"ok": True}

    @app.websocket("/ws")
    async def ws_endpoint(websocket: WebSocket, token: str = Depends(verify_ws_token)):
        await websocket.accept()
        await websocket.send_json({"authenticated": True})
        await websocket.close()

    return app


@pytest.fixture
def app_no_auth():
    """禁用鉴权的测试 app"""
    app = FastAPI()

    @app.get("/protected")
    async def protected(_token: str = Depends(require_auth)):
        return {"ok": True}

    return app


def test_auth_missing_token_returns_401(app_with_auth):
    settings.HELEN_WEBUI_TOKEN = "secret-token"
    try:
        client = TestClient(app_with_auth)
        resp = client.get("/protected")
        assert resp.status_code == 401
        assert "missing" in resp.json()["detail"].lower()
    finally:
        settings.HELEN_WEBUI_TOKEN = ""


def test_auth_wrong_token_returns_403(app_with_auth):
    settings.HELEN_WEBUI_TOKEN = "secret-token"
    try:
        client = TestClient(app_with_auth)
        resp = client.get("/protected", headers={"X-Helen-Token": "wrong"})
        assert resp.status_code == 403
    finally:
        settings.HELEN_WEBUI_TOKEN = ""


def test_auth_correct_token_returns_200(app_with_auth):
    settings.HELEN_WEBUI_TOKEN = "secret-token"
    try:
        client = TestClient(app_with_auth)
        resp = client.get("/protected", headers={"X-Helen-Token": "secret-token"})
        assert resp.status_code == 200
        assert resp.json() == {"ok": True}
    finally:
        settings.HELEN_WEBUI_TOKEN = ""


def test_auth_empty_token_disables_check(app_with_auth):
    settings.HELEN_WEBUI_TOKEN = ""
    client = TestClient(app_with_auth)
    # 无 header 也能通过（鉴权已禁用）
    resp = client.get("/protected")
    assert resp.status_code == 200


def test_ws_missing_token_rejected(app_with_auth):
    settings.HELEN_WEBUI_TOKEN = "secret-token"
    try:
        client = TestClient(app_with_auth)
        with pytest.raises(WebSocketDisconnect) as exc_info:
            with client.websocket_connect("/ws"):
                pass
        assert exc_info.value.code == 1008
    finally:
        settings.HELEN_WEBUI_TOKEN = ""


def test_ws_correct_token_accepted(app_with_auth):
    settings.HELEN_WEBUI_TOKEN = "secret-token"
    try:
        client = TestClient(app_with_auth)
        with client.websocket_connect("/ws?token=secret-token") as ws:
            data = ws.receive_json()
            assert data == {"authenticated": True}
    finally:
        settings.HELEN_WEBUI_TOKEN = ""


def test_ws_wrong_token_rejected(app_with_auth):
    """WS with wrong token gets WebSocketDisconnect code 1008."""
    settings.HELEN_WEBUI_TOKEN = "secret-token"
    try:
        client = TestClient(app_with_auth)
        with pytest.raises(WebSocketDisconnect) as exc_info:
            with client.websocket_connect("/ws?token=wrong-token"):
                pass
        assert exc_info.value.code == 1008
    finally:
        settings.HELEN_WEBUI_TOKEN = ""


class TestTokenMatches:
    """_token_matches() edge case tests (lines 25-32)."""

    def test_empty_expected_returns_true(self):
        """空 expected token (鉴权禁用) -> 任何 candidate 都匹配"""
        settings.HELEN_WEBUI_TOKEN = ""
        try:
            assert _token_matches("anything") is True
            assert _token_matches("") is True
        finally:
            pass  # already empty

    def test_empty_candidate_with_expected_returns_false(self):
        """有 expected 但 candidate 为空 -> False"""
        settings.HELEN_WEBUI_TOKEN = "secret-token"
        try:
            assert _token_matches("") is False
        finally:
            settings.HELEN_WEBUI_TOKEN = ""

    def test_matching_token_returns_true(self):
        """candidate 与 expected 相同 -> True"""
        settings.HELEN_WEBUI_TOKEN = "secret-token"
        try:
            assert _token_matches("secret-token") is True
        finally:
            settings.HELEN_WEBUI_TOKEN = ""

    def test_mismatching_token_returns_false(self):
        """candidate 与 expected 不同 -> False"""
        settings.HELEN_WEBUI_TOKEN = "secret-token"
        try:
            assert _token_matches("wrong-token") is False
        finally:
            settings.HELEN_WEBUI_TOKEN = ""


class TestVerifyWsTokenEdgeCases:
    """verify_ws_token() 边缘路径测试 (lines 68-80)."""

    def test_auth_disabled_returns_empty_string(self):
        """鉴权禁用时 verify_ws_token 返回空字符串"""
        settings.HELEN_WEBUI_TOKEN = ""
        result = verify_ws_token(token=None)
        assert result == ""

    def test_missing_token_raises(self):
        """token 为 None 且鉴权启用 -> WebSocketException"""
        settings.HELEN_WEBUI_TOKEN = "secret-token"
        try:
            from fastapi import WebSocketException
            with pytest.raises(WebSocketException):
                verify_ws_token(token=None)
        finally:
            settings.HELEN_WEBUI_TOKEN = ""

    def test_invalid_token_raises(self):
        """token 错误 -> WebSocketException"""
        settings.HELEN_WEBUI_TOKEN = "secret-token"
        try:
            from fastapi import WebSocketException
            with pytest.raises(WebSocketException):
                verify_ws_token(token="wrong")
        finally:
            settings.HELEN_WEBUI_TOKEN = ""

    def test_valid_token_returns_token(self):
        """token 正确 -> 返回 token"""
        settings.HELEN_WEBUI_TOKEN = "secret-token"
        try:
            result = verify_ws_token(token="secret-token")
            assert result == "secret-token"
        finally:
            settings.HELEN_WEBUI_TOKEN = ""
