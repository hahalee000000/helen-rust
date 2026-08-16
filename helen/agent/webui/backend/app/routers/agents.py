"""Agent 管理 API 路由"""
from fastapi import APIRouter, Depends
from typing import List, Dict

from app.auth import require_auth

# ⚠️ 与 chat.py 同理：HTTP 路由用 http_router（router 级鉴权）。
# 如果未来新增 WebSocket 路由，需要拆出 ws_router（无 router 级依赖），
# 否则 FastAPI 0.109 + Starlette 0.35 会让 WS 握手失败。
http_router = APIRouter(dependencies=[Depends(require_auth)])

# 模拟的 Agent 状态数据
# 实际实现中应该从 Helen 运行时获取
agent_states = {
    "Contractor": {"status": "idle", "last_task": None},
    "TestBuilder": {"status": "idle", "last_task": None},
    "Implementer": {"status": "idle", "last_task": None},
    "QualityGate": {"status": "idle", "last_task": None},
    "SkillEvaluator": {"status": "idle", "last_task": None},
}

@http_router.get("/status")
async def get_all_agents_status():
    """获取所有 Agent 状态"""
    return agent_states

@http_router.get("/{agent_name}/status")
async def get_agent_status(agent_name: str):
    """获取单个 Agent 状态"""
    if agent_name not in agent_states:
        return {"error": f"Agent {agent_name} not found"}
    return {
        "name": agent_name,
        **agent_states[agent_name]
    }

@http_router.get("/list")
async def list_agents():
    """列出所有可用的 Agent"""
    return list(agent_states.keys())
