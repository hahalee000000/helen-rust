# 提示词构建引擎

> 模块 M6 | `helen/runtime/prompt_builder.py` | 测试: `tests/runtime/test_prompt_builder.py`

---

## 概述

PromptBuilder 负责将 Agent 的 description、prompt、Skill Index 等组装为 LLM 可用的 System Prompt。

---

## 两阶段渐进式披露 (Two-Phase Progressive Disclosure)

HLD §3.7.1 定义的两阶段 skill 披露机制，平衡 token 消耗和信息完整性。

### Tier 1: Skill Index（轻量级索引）

```python
def build_skill_index(self) -> str:
    """Build the Tier 1 Skill Index for System Prompt injection.

    Scans skills via runtime.list_skills() and formats as
    <available_skills> XML block with name + description + category.
    """
    skills = self._runtime.list_skills()
    # 只读取 SKILL.md 的 YAML frontmatter
    # 格式化为 XML 块
```

**输出示例**：
```
<available_skills>
Before replying, scan skills below. If relevant,
use load_skill tool to load full content.

  devops:
    - helen-language: Helen programming language development... (tags: helen, language-design, interpreter)
  research:
    - research: Research discovery and monitoring... (tags: arxiv, papers, monitoring)
</available_skills>
```

**特点**：
- 包含 name + description + category + **tags**（轻量级）
- tags 字段帮助 LLM 通过关键词快速定位相关技能，提升命中率
- 注入到 System Prompt，LLM 始终可见
- 占用 ~16KB token（所有 skill 的索引）

### Tier 2: load_skill 工具（按需加载）

```python
# tools.py 中注册的 load_skill 工具
register_tool(
    name="load_skill",
    description="Load a skill's full content by name",
    parameters={
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "Skill name to load"}
        },
        "required": ["name"]
    },
    handler=_load_skill,  # 搜索 skill 目录，返回完整 SKILL.md
)
```

**工作流程**：
1. LLM 看到 System Prompt 中的 `<available_skills>` 索引
2. 如果某个 skill 与当前任务相关，LLM 调用 `load_skill(name)`
3. Helen 执行工具，返回完整 SKILL.md 内容（可能 67KB+）
4. LLM 根据完整内容执行任务

**优势**：
- 避免每次都发送所有 skill 的完整内容
- 只在需要时才加载，节省 token
- LLM 可以自主选择加载哪些 skill

---

## 模板渲染

```python
def render(self, template: str, variables: dict[str, str]) -> str:
    """单次正则替换 {{var}}，防止递归注入攻击。"""
    import re
    def replace(match):
        var_name = match.group(1)
        value = variables.get(var_name, match.group(0))
        # 如果值中包含 {{，不二次渲染
        if "{{" in str(value):
            return match.group(0)
        return value
    return re.sub(r'\{\{(\w+)\}\}', replace, template)
```

**安全特性**：
1. 仅替换 prompt 块中的模板变量
2. 变量值包含 `{{y}}` 时不二次渲染
3. 普通字符串不调用 render

---

## System Prompt 组装

Agent 的 `prompt` 字段渲染后作为 `system_prompt` 传递给 `runtime.act()`。

### 组装流程

```
┌──────────────────────────────────────────────────┐
│ System Prompt ({"role": "system"})               │
│                                                  │
│ ① 框架预设（可选）                                │
│ ② Agent description → "You are a translator"     │
│ ③ Skill Index (Tier 1) → <available_skills>...   │
│ ④ Tool Schemas → 含 load_skill 工具               │
│ ⑤ 渲染后的 Agent prompt → "Translate to French"  │
└──────────────────────────────────────────────────┘
```

### 最终消息结构

```json
[
  {"role": "system", "content": "②+③+⑤ 拼接结果"},
  {"role": "user", "content": "llm act 表达式值"},
  {"role": "assistant", "tool_calls": [...]},
  {"role": "tool", "content": "工具结果"},
  {"role": "assistant", "content": "最终响应"}
]
```

### 代码实现

```python
# interpreter.py
def visit_llm_act_expr(self, node):
    prompt_value = self.evaluate(node.prompt)  # llm act 的表达式值
    system_prompt = self._get_rendered_agent_prompt()  # 渲染后的 agent prompt
    tools = self._build_tools_list()  # agent tools + load_skill
    
    response = self._runtime.act(
        prompt=prompt_value,
        tools=tools,
        model=self._current_agent.model,
        temperature=self._current_agent.temperature,
        max_turns=self._current_agent.max_turns,
        system_prompt=system_prompt,  # ← 新增参数
    )
    return response
```

---

## 未定义变量处理

```helen
agent Greeting {
    prompt "Hello, {{name}}!"   // {{name}} 未定义
}
```

未定义的变量保持原样 `{{name}}`，不崩溃。
