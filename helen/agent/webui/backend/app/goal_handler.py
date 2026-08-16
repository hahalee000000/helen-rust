"""Goal pursuit handler for WebUI /goal command.

A+B 方案：Python 端 auto-continue 循环，不改 Helen 语言。
每次迭代调用 actor 的 act()，检查目标是否完成，未完成则续传。
"""

from __future__ import annotations

import os
import re
import logging
from typing import Any

logger = logging.getLogger(__name__)


# ── 语言检测 ────────────────────────────────────────────────────────

def _get_ui_language() -> str:
    """获取 WebUI 界面语言（en 或 zh）"""
    return os.environ.get("HELEN_WEBUI_LANG", "en")


# ── Goal 模式 system prompt 注入（根据界面语言）────────────────────

def _get_goal_system_prompt_injection() -> str:
    """根据界面语言返回对应的系统提示注入"""
    lang = _get_ui_language()

    if lang == "zh":
        return """
你正在 Pursue 一个目标。每次回复结尾必须包含以下标记之一：

[GOAL_IN_PROGRESS] 还需要做什么：<简述剩余工作>
或
[GOAL_COMPLETE] 最终总结：<总结已完成的工作>

这会帮助系统判断是否需要继续。务必在每次回复末尾包含标记。
""".strip()
    else:  # en
        return """
You are pursuing a goal. Each response must end with one of these markers:

[GOAL_IN_PROGRESS] Remaining work: <brief description of what's left>
or
[GOAL_COMPLETE] Final summary: <summary of completed work>

This helps the system determine whether to continue. Always include the marker at the end of your response.
""".strip()


# ── 完成检测 ──────────────────────────────────────────────────────

# 正则匹配 [GOAL_COMPLETE] 和 [GOAL_IN_PROGRESS] 标记（支持中英文）
_GOAL_COMPLETE_RE = re.compile(
    r"\[GOAL_COMPLETE\]\s*(?:最终总结[：:]|Final summary[::])?\s*(.*)",
    re.IGNORECASE | re.DOTALL,
)
_GOAL_IN_PROGRESS_RE = re.compile(
    r"\[GOAL_IN_PROGRESS\]\s*(?:还需要做什么[：:]|Remaining work[::])?\s*(.*)",
    re.IGNORECASE | re.DOTALL,
)


def parse_goal_status(response: str) -> dict[str, Any]:
    """解析 LLM 回复中的 goal 状态标记。

    Returns:
        dict with keys:
        - status: "complete" | "in_progress" | "unknown"
        - summary: str (completion summary or remaining work description)
    """
    # 检查完成标记
    complete_match = _GOAL_COMPLETE_RE.search(response)
    if complete_match:
        summary = complete_match.group(1).strip()
        return {"status": "complete", "summary": summary}

    # 检查进行中标记
    progress_match = _GOAL_IN_PROGRESS_RE.search(response)
    if progress_match:
        remaining = progress_match.group(1).strip()
        return {"status": "in_progress", "summary": remaining}

    # 未检测到标记
    return {"status": "unknown", "summary": ""}


def goal_appears_complete(response: str) -> bool:
    """检查目标是否完成（LLM 自报告 + 启发式回退）。

    优先检查 [GOAL_COMPLETE] 标记。如果没有标记，回退到简单启发式。
    """
    status = parse_goal_status(response)
    if status["status"] == "complete":
        return True
    if status["status"] == "in_progress":
        return False

    # 回退：简单启发式（如果 LLM 没有按指示输出标记）
    response_lower = response.lower()
    strong_complete_markers = [
        "目标已完成", "任务已完成", "goal completed", "task completed",
        "all done", "successfully completed", "everything is done",
    ]
    return any(m in response_lower for m in strong_complete_markers)


# ── Prompt 构建 ──────────────────────────────────────────────────

def build_goal_prompt(goal_text: str) -> str:
    """构建初始 goal prompt（包含系统指令，根据界面语言）"""
    injection = _get_goal_system_prompt_injection()
    lang = _get_ui_language()

    if lang == "zh":
        return (
            f"{injection}\n\n"
            f"---\n\n"
            f"目标: {goal_text}\n\n"
            f"开始工作。完成后在回复末尾标注 [GOAL_COMPLETE]。"
        )
    else:  # en
        return (
            f"{injection}\n\n"
            f"---\n\n"
            f"Goal: {goal_text}\n\n"
            f"Start working. Mark [GOAL_COMPLETE] at the end of your response when done."
        )


def build_continuation_prompt(goal_text: str, last_response: str) -> str:
    """构建续传 prompt（根据界面语言）。

    包含原始目标 + 上次进度摘要，要求 LLM 继续。
    """
    # 截断上次响应，避免 prompt 过长
    last_summary = last_response[:800]
    if len(last_response) > 800:
        last_summary += "..."

    injection = _get_goal_system_prompt_injection()
    lang = _get_ui_language()

    if lang == "zh":
        return (
            f"{injection}\n\n"
            f"---\n\n"
            f"原始目标: {goal_text}\n\n"
            f"上次进度:\n{last_summary}\n\n"
            f"继续工作。不要重复已完成的部分。"
            f"如果目标已完成，在回复末尾标注 [GOAL_COMPLETE]。"
            f"如果还需要继续，标注 [GOAL_IN_PROGRESS]。"
        )
    else:  # en
        return (
            f"{injection}\n\n"
            f"---\n\n"
            f"Original goal: {goal_text}\n\n"
            f"Last progress:\n{last_summary}\n\n"
            f"Continue working. Don't repeat what's already done. "
            f"If the goal is complete, mark [GOAL_COMPLETE] at the end. "
            f"If more work is needed, mark [GOAL_IN_PROGRESS]."
        )


# ── 常量 ────────────────────────────────────────────────────────

DEFAULT_MAX_ITERATIONS = 10
