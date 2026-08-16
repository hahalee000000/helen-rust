# Helen Programming Agent

> 用纯 Helen 语言实现的自进化编程系统（v1.0 架构）。
> 父项目 Helen 语言 v1.25 — Prompt-first Agent Programming Language。

## 架构（v1.0）

v1.0 合并所有 specialist agents 到 ChatSessionActor，LLM 直接调用所有工具：

```
Web UI (webui/) — 浏览器界面 + WebSocket
  ↓
chat_tui.helen — Actor 生命周期管理
  ↓ Channel mailbox
ChatSessionActor — 唯一 agent，长驻 while 循环
  ├── 文件操作：read_file / write_file / patch_file
  ├── 质量检查：run_helen_check / get_scores / get_metrics
  ├── 测试工具：run_helen_tests / verify_after_change
  ├── 技能管理：save_new_skill / list_existing_skills / load_skill
  └── Hooks：save_code_file / patch_code_file / pre_exit_check
```

**关键设计**：
- **ChatSessionActor 是唯一 agent** — 长驻 while 循环，上下文在 store 自然累积
- **LLM 直接调用所有工具** — 复杂任务从 5 次串行 LLM 推理降到 1 次
- **按需 load_skill** — 领域知识保留在 skills 中，LLM 按需加载
- **三层完整性防御** — LLM 自检 → 测试覆盖率 → 代码完整性 skill
- **Hooks 机制** — 代码文件写入后自动 `helen check`

## 文件结构

```
.
├── chat_session_actor.helen  # 唯一 agent：长驻 ChatSessionActor
├── chat_tui.helen            # Actor 生命周期管理
├── chat_tui_web.py           # Web UI Python 入口
├── commands.helen            # 斜杠命令系统
├── context.helen             # 环境事实收集 + 上下文注入
├── context_manager.helen     # 上下文管理
├── memory_utils.helen        # 记忆工具
├── output.helen              # 输出核心
├── task_manager.helen        # 任务追踪
├── session_stats.helen       # 会话统计
├── contracts/
│   └── contracts.helen       # 工具集 const (CHAT_TOOLS)
├── ui/                       # 流式输出 Python 模块
├── webui/                    # Web UI（独立服务）
├── tests/                    # 测试文件
├── .helen/
│   └── skills/               # 技能目录（6 个项目级技能）
│       ├── architecture/helen-contractor-design/SKILL.md
│       ├── testing/helen-test-patterns/SKILL.md
│       ├── testing/helen-tdd-methodology/SKILL.md
│       ├── code-quality/helen-quality-rubrics/SKILL.md
│       └── code-quality/helen-code-integrity/SKILL.md
└── CLAUDE.md                 # Claude Code harness 项目指令
```

## 使用方式

```bash
# 启动 Web UI
cd webui && ./start-all.sh

# 语法检查
helen check chat_session_actor.helen
helen check chat_tui.helen
helen check commands.helen

# 运行架构完整性验证
bash tests/verify_v1.0_integrity.sh
```

## 斜杠命令

| 命令 | 功能 |
|------|------|
| `/help` | 显示帮助 |
| `/clear` | 清空对话上下文 |
| `/compress` | LLM 语义压缩上下文 |
| `/stats` | 显示会话统计 |
| `/mode` | 切换输出模式 |

## 设计原则

1. **纯 Helen 语言**：Agent 和所有业务逻辑用 Helen 编写
2. **单 Agent 全工具**：ChatSessionActor 直接使用所有工具，无中间编排层
3. **知识按需加载**：领域方法论以 skill 形式存在，LLM 通过 load_skill 获取
4. **三层完整性防御**：LLM 自检 → 测试覆盖率 → 代码完整性
5. **上下文自然累积**：Actor 长驻，无需每请求 resume_session

## 文档

- `CLAUDE.md` — Claude Code harness 项目指令（架构/约定/语言参考）
- `wiki/` — 项目 wiki（架构、特性、设计决策）
- GitHub Issues — Helen 语言层面的不足与建议（https://github.com/hahalee000000/helen/issues）
