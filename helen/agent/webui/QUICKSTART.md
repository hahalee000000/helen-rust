# Helen Web UI - 快速启动指南

## 🚀 一键启动

```bash
cd ~/helenagent
./helen agent
```

访问：
- 前端：http://localhost:5173
- 后端 API：http://localhost:8000
- API 文档：http://localhost:8000/docs

## 🛑 停止服务

```bash
cd ~/helenagent/webui
./stop-all.sh
```

或在运行 `helen agent` 的终端按 `Ctrl+C`（会自动释放端口）。

## 📋 分别启动

**后端**：
```bash
cd ~/helenagent/webui
./start_webui.py (backend)
```

**前端**（新终端）：
```bash
cd ~/helenagent/webui
./start_webui.py (frontend)
```

## 🔧 环境要求

- Python 3.12+ 与 `~/.venv` 虚拟环境（已安装 helen + fastapi）
- Node.js 18+
- uv（Python 包管理器）

启动脚本会自动：
1. 使用 `~/.venv`（和 helenagent 共享的虚拟环境）
2. 检查并安装缺失的依赖
3. 自动推断 `HELEN_PATH`（webui 的父目录 = helenagent/）

## 🔧 配置

### 默认配置（无需修改）

大多数部署场景下无需修改任何配置：
- `HELEN_PATH` 自动推断为 `webui/` 的父目录
- CORS 允许 5173-5180 端口
- 数据库文件在 `backend/helen.db`

### 自定义配置

如需覆盖，编辑 `backend/.env`：

```bash
# 仅在非标准部署时才需要
HELEN_PATH=/opt/helenagent

# 修改端口
PORT=9000
```

## 📝 常见问题

### 端口被占用

修改 `backend/.env` 中的 `PORT`，或 `frontend/vite.config.ts` 中的 `server.port`。

### CORS 错误（Failed to fetch）

确保前端运行的端口在 `backend/.env` 的 `CORS_ORIGINS` 中。默认允许 5173-5180。

### Python 依赖缺失

启动脚本会自动安装依赖。如仍有问题，手动安装：
```bash
uv pip install --python ~/.venv/bin/python -r backend/requirements.txt
```

### 数据库重置

```bash
rm backend/helen.db
```

## 📖 完整文档

详细架构和技术栈说明见 [wiki/webui.md](../../wiki/webui.md)。
