"""Token 鉴权依赖。

所有 `/api/*` HTTP 端点和 WebSocket 握手都走 `require_auth`。Token 来源:
  - HTTP:  `X-Helen-Token` 请求头
  - WS:    `?token=<token>` URL 查询参数

Token 在 `settings.ensure_token()` 中解析/生成。空串 = 禁用鉴权（仅用于本地开发，
启动日志会警告）。

设计要点:
  - 用 `hmac.compare_digest` 做常量时间比较，防时序侧信道。
  - 空 token 时依赖直接放行，避免每次请求都做字符串比较。
  - WebSocket 不能在握手后发 header，所以走 query param；同样用常量时间比较。
"""
from __future__ import annotations

import hmac
from typing import Optional

from fastapi import Header, HTTPException, Query, WebSocket, WebSocketException, status

from app.config import settings


def _token_matches(candidate: str) -> bool:
    """常量时间比较 candidate 与已配置 token。"""
    expected = settings.HELEN_WEBUI_TOKEN
    if not expected:
        return True  # 空 token = 鉴权禁用
    if not candidate:
        return False
    return hmac.compare_digest(candidate, expected)


async def require_auth(
    x_helen_token: Optional[str] = Header(default=None, alias="X-Helen-Token"),
) -> str:
    """FastAPI 依赖：校验 HTTP 请求的 token。

    Returns: 通过校验的 token 字符串（供路由按需使用）。

    Raises:
        HTTPException: 401 缺少 token；403 token 错误。
    """
    if not settings.HELEN_WEBUI_TOKEN:
        # 鉴权禁用：直接放行
        return ""
    if not x_helen_token:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="missing X-Helen-Token header",
            headers={"WWW-Authenticate": "Token"},
        )
    if not _token_matches(x_helen_token):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="invalid token",
        )
    return x_helen_token


def verify_ws_token(token: Optional[str] = Query(default=None)) -> str:
    """WebSocket 握手阶段的 token 校验。

    在 `websocket.accept()` 之前调用。token 错误时抛 `WebSocketException`
    关闭握手（code 1008 = Policy Violation）。
    """
    if not settings.HELEN_WEBUI_TOKEN:
        return ""
    if not token:
        raise WebSocketException(
            code=status.WS_1008_POLICY_VIOLATION,
            reason="missing ?token= query parameter",
        )
    if not _token_matches(token):
        raise WebSocketException(
            code=status.WS_1008_POLICY_VIOLATION,
            reason="invalid token",
        )
    return token
