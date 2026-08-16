"""Helen Web UI - FastAPI 后端主入口"""
from pathlib import Path
from fastapi import FastAPI, Depends
from fastapi.middleware.cors import CORSMiddleware
from contextlib import asynccontextmanager

from app.config import settings
from app.auth import require_auth
from app.routers import chat, agents
from app.websocket.manager import WebSocketManager

@asynccontextmanager
async def lifespan(app: FastAPI):
    """应用生命周期管理"""
    # 启动时初始化
    import os
    from app.services.directory_manager import get_current_cwd
    print(f"🚀 Starting {settings.APP_NAME}...")
    # 防御：os.getcwd() 可能在 cwd 已被删除时抛 FileNotFoundError
    # （测试场景：TemporaryDirectory 退出后进程 cwd 失效）
    try:
        proc_cwd = os.getcwd()
    except FileNotFoundError:
        proc_cwd = "<cwd deleted>"
    print(f"📂 进程 cwd: {proc_cwd}")
    print(f"📂 会话目录: {get_current_cwd()}")
    app.state.websocket_manager = WebSocketManager()
    print(f"🌐 Helen path: {settings.HELEN_PATH}")

    # ── 鉴权初始化 ───────────────────────────────────────────
    token = settings.ensure_token()
    if token:
        import os
        project_cwd = os.environ.get("HELEN_WEBUI_CWD") or str(Path.cwd())
        token_path = Path(project_cwd) / ".helen" / "webui_token"
        print(f"🔐 Auth enabled. Token (copy to frontend):")
        print(f"   {token}")
        print(f"   (persisted to {token_path})")
    else:
        print("⚠️  Auth DISABLED (HELEN_WEBUI_TOKEN='' explicitly). "
              "Set HELEN_WEBUI_TOKEN or remove it from .env to enable.")

    yield
    # 关闭时清理
    print("🛑 Shutting down...")
    await app.state.websocket_manager.close_all()
    print("✅ Cleanup complete")

app = FastAPI(
    title=settings.APP_NAME,
    description="Web UI for Helen Programming Agent",
    version=settings.VERSION,
    lifespan=lifespan
)

# CORS 配置
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.CORS_ORIGINS,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# 注册路由（chat 分 http_router/ws_router，避免 router 级鉴权波及 WebSocket）
# agents 目前只有 HTTP 路由，用 http_router 做 router 级鉴权
app.include_router(chat.http_router, prefix="/api/chat", tags=["chat"])
app.include_router(chat.ws_router, prefix="/api/chat", tags=["chat"])
app.include_router(agents.http_router, prefix="/api/agents", tags=["agents"])

@app.get("/")
async def root(_token: str = Depends(require_auth)):
    """根路由"""
    return {
        "message": "Helen Web UI API",
        "version": settings.VERSION,
        "docs": "/docs"
    }

@app.get("/health")
async def health():
    """健康检查（无鉴权，供启动脚本/监控探测）"""
    return {
        "status": "ok",
        "app": settings.APP_NAME
    }

@app.get("/api/status")
async def api_status(_token: str = Depends(require_auth)):
    """API 状态"""
    manager = app.state.websocket_manager
    return {
        "status": "ok",
        "version": settings.VERSION,
        "active_connections": len(manager.active_connections),
        "config": {
            "helen_path": settings.HELEN_PATH,
        }
    }

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "app.main:app",
        host=settings.HOST,
        port=settings.PORT,
        reload=settings.DEBUG
    )
