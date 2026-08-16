# Helen Web UI

基于 FastAPI + React 的 Helen Programming Agent Web 界面。

## 📋 特性

- ✨ **现代化界面**：React + TypeScript + Tailwind CSS
- 💬 **实时聊天**：WebSocket 双向通信
- 🤖 **Agent 可视化**：实时显示 Agent 状态
- 📱 **响应式设计**：支持桌面和移动端
- 📝 **会话管理**：创建、切换、删除会话
- 🎨 **美观界面**：专业的 UI 设计和交互体验

## 🏗️ 架构

```
helenagent-web/
├── backend/          # FastAPI 后端
│   ├── app/
│   │   ├── main.py          # 主入口
│   │   ├── config.py        # 配置管理
│   │   ├── database.py      # 数据库
│   │   ├── models/          # 数据模型
│   │   ├── routers/         # API 路由
│   │   ├── services/        # 业务逻辑
│   │   └── websocket/       # WebSocket 管理
│   └── requirements.txt
│
└── frontend/         # React 前端
    ├── src/
    │   ├── components/      # UI 组件
    │   ├── pages/           # 页面
    │   ├── hooks/           # 自定义 Hooks
    │   ├── stores/          # 状态管理
    │   ├── services/        # API 服务
    │   └── types/           # TypeScript 类型
    └── package.json
```

## 🚀 快速开始

### 前置要求

- Python 3.12+
- Node.js 18+
- npm 或 yarn

### 一键启动

```bash
# 从项目根目录运行
cd ..
./helen agent
```

这会同时启动后端和前端服务。

### 分别启动

**启动后端**：
```bash
./start_webui.py (backend)
```

**启动前端**（新终端）：
```bash
./start_webui.py (frontend)
```

### 访问应用

- **前端界面**：http://localhost:5173
- **后端 API**：http://localhost:8000
- **API 文档**：http://localhost:8000/docs

## 📦 技术栈

### 后端
- **FastAPI** - 现代 Web 框架
- **SQLAlchemy** - ORM
- **WebSocket** - 实时通信
- **SQLite** - 数据库
- **Pydantic** - 数据验证

### 前端
- **React 18** - UI 框架
- **TypeScript** - 类型安全
- **Vite** - 构建工具
- **Tailwind CSS** - 样式
- **Zustand** - 状态管理
- **React Query** - 数据获取
- **Lucide React** - 图标库

## 🔧 配置

### 后端配置

编辑 `backend/.env` 文件：

```bash
# 应用配置
DEBUG=true
HOST=0.0.0.0
PORT=8000

# 数据库
DATABASE_URL=sqlite:///./helen.db

# Helen 配置（默认自动推断，通常无需手动设置）
# HELEN_PATH=/path/to/helenagent
```

### 前端配置

编辑 `frontend/vite.config.ts`：

```typescript
export default defineConfig({
  server: {
    port: 5173,  // 修改端口
  },
})
```

## 📖 使用指南

### 1. 创建会话

点击左侧"新会话"按钮创建新的聊天会话。

### 2. 发送消息

在输入框中输入消息，按 Enter 发送（Shift+Enter 换行）。

### 3. 查看 Agent 状态

导航到 "Agents" 页面查看所有 Agent 的实时状态。

### 4. 管理会话

- 点击左侧会话列表切换会话
- 点击删除按钮删除会话

## 🔌 API 接口

### 会话管理

- `POST /api/chat/sessions` - 创建会话
- `GET /api/chat/sessions` - 获取会话列表
- `GET /api/chat/sessions/{id}` - 获取单个会话
- `DELETE /api/chat/sessions/{id}` - 删除会话
- `GET /api/chat/sessions/{id}/messages` - 获取消息
- `WS /api/chat/ws/{id}` - WebSocket 聊天

### Agent 管理

- `GET /api/agents/status` - 获取所有 Agent 状态
- `GET /api/agents/{name}/status` - 获取单个 Agent 状态

完整 API 文档：http://localhost:8000/docs

## 🛠️ 开发

### 后端开发

```bash
cd backend
python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
uvicorn app.main:app --reload
```

### 前端开发

```bash
cd frontend
npm install
npm run dev
```

### 构建生产版本

```bash
cd frontend
npm run build
```

## 📝 待办事项

- [ ] 集成真实的 Helen 运行时
- [ ] Agent 并发可视化
- [ ] 代码高亮和 diff 视图
- [ ] 会话导出功能
- [ ] 深色模式
- [ ] 移动端优化
- [ ] 性能监控面板

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

MIT License

## 🔗 相关链接

- [Helen Programming Language](https://github.com/hahalee000000/helen)
- [FastAPI 文档](https://fastapi.tiangolo.com/)
- [React 文档](https://react.dev/)
- [Tailwind CSS](https://tailwindcss.com/)
