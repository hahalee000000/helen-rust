"""Side-channel 索引：web_ui_session_id → [transcript message_uuid, ...]

多个 Web UI 会话共享一个 Helen transcript，此模块维护独立的索引文件，
记录每个 Web UI session 产生的 transcript 消息 UUID，用于按会话分组显示。

索引文件格式（.helen/session_index/<web_ui_session_id>.json）：
{
    "web_ui_session_id": "uuid-string",
    "message_uuids": ["msg_uuid_1", "msg_uuid_2", ...]
}
"""

import json
import os
from pathlib import Path
from typing import Optional

# 项目根目录（从 webui/backend/app/services/ 向上 5 层到 helenagent 根）
_AGENT_DIR = Path(__file__).resolve().parent.parent.parent.parent.parent


def get_transcript_path(helen_session_id: str) -> Optional[Path]:
    """获取 transcript.jsonl 路径(当前工作目录的 .helen/sessions/)

    v6.1:只查当前工作目录(directory_manager.get_current_cwd()),
    不回退 ~/.helen/sessions/(避免读到全局历史 session)。
    """
    from app.services import directory_manager
    cwd = directory_manager.get_current_cwd()
    path = Path(cwd) / ".helen" / "sessions" / helen_session_id / "transcript.jsonl"
    return path if path.exists() else None


def read_transcript_entries(helen_session_id: str) -> list[dict]:
    """读取 transcript.jsonl 全部条目"""
    path = get_transcript_path(helen_session_id)
    if not path:
        return []
    entries = []
    try:
        # v1.30.10: 显式指定 UTF-8 编码，避免 Windows GBK 默认编码导致读取失败
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    entries.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    except Exception:
        pass
    return entries


def get_current_helen_session_id() -> str:
    """获取当前 Helen session ID(读 memento)

    v6.1:只读 memento 文件(<cwd>/.helen/current_session_id,JSON {main, child}),
    这是 actor 实际用的 child session。memento 不存在 -> 返回空(首次启动,
    actor 会新建 session,无需回退找旧的——回退会读到非当前对话的 session)。
    """
    from app.services import directory_manager
    cwd = directory_manager.get_current_cwd()
    memento_path = Path(cwd) / ".helen" / "current_session_id"
    if not memento_path.exists():
        return ""
    try:
        # v1.30.10: 显式指定 UTF-8 编码
        memento_content = memento_path.read_text(encoding="utf-8").strip()
        if memento_content.startswith("{"):
            data = json.loads(memento_content)
            child_sid = data.get("child", "")
            if child_sid and get_transcript_path(child_sid) is not None:
                return child_sid
        elif memento_content and get_transcript_path(memento_content) is not None:
            # 纯文本 session_id(兼容旧格式)
            return memento_content
    except Exception:
        pass
    return ""

# ── 测试消息过滤 ──────────────────────────────────────────────

TEST_MESSAGE_PREFIX = "[TEST]"


def is_test_message(entry: dict) -> bool:
    """检测 transcript 条目是否为测试消息（以 [TEST] 开头）

    集成测试发送的消息会污染 transcript，通过此前缀标记过滤。
    """
    content = entry.get("content", "")
    if isinstance(content, str):
        return content.strip().startswith(TEST_MESSAGE_PREFIX)
    return False


def filter_test_messages(entries: list[dict]) -> list[dict]:
    """过滤掉测试消息，返回干净的消息列表"""
    return [e for e in entries if not is_test_message(e)]


# ── transcript -> 前端消息 ─────────────────────────────────────
# v6.1:transcript 是唯一数据源,替代 SQLite messages 表


def _attachment_from_data_url(data_url: str, idx: int) -> dict:
    """从 data URL 解析出 Attachment 对象(供历史消息附件显示)。

    transcript 中多模态 user message 的 image_url/audio part 以
    `data:<mime>;base64,<data>` 形式内嵌。前端 AttachmentView 通过 url
    直接渲染,浏览器原生支持 data URL。

    新消息附件的 url 是 HTTP 路径(`/api/chat/uploads/<id>/file`),
    历史附件是 data URL,两者格式不同但前端统一通过 url 渲染。
    """
    import re
    mime = "application/octet-stream"
    ext = "bin"
    m = re.match(r"data:([^;]+);base64,", data_url)
    if m:
        mime = m.group(1)
        ext = mime.split("/")[-1] if "/" in mime else "bin"

    b64_data = data_url.split(",", 1)[-1] if "," in data_url else ""
    # base64 编码后体积约为原始的 4/3,故反向估算
    size = int(len(b64_data) * 0.75) if b64_data else 0

    if mime.startswith("image/"):
        kind = "image"
    elif mime.startswith("audio/"):
        kind = "audio"
    elif mime.startswith("video/"):
        kind = "video"
    else:
        kind = "file"
    return {
        "id": f"att-{idx}",
        "filename": f"{kind}-{idx}.{ext}",
        "mime_type": mime,
        "size": size,
        "url": data_url,
    }


def _extract_from_boilerplate(content: str) -> tuple[str, str]:
    """分离 prompt boilerplate 和用户实际输入。

    移植自前端 TranscriptPage.extractFromPromptBoilerplate。

    Helen actor 纯文本路径把 agent prompt(## Identity ... ## Reminders ...)
    和用户输入拼成一条 user message。boilerplate 以 ## Identity 开头,
    以最后一个 ## 段标题后的第一个空行与用户输入分隔。

    返回 (user_text, boilerplate)。无 boilerplate 特征时 user_text = content。
    """
    trimmed = content.strip()
    if not (trimmed.startswith("## ") and "## Identity" in trimmed):
        return content, ""

    lines = content.split("\n")
    # 找最后一个 ## 标题行
    last_heading_idx = -1
    for i in range(len(lines) - 1, -1, -1):
        if lines[i].startswith("## "):
            last_heading_idx = i
            break
    if last_heading_idx < 0:
        return content, ""

    # 标题后第一个空行 = boilerplate 与用户输入的分隔符
    blank_idx = -1
    for i in range(last_heading_idx + 1, len(lines)):
        if lines[i].strip() == "":
            blank_idx = i
            break
    if blank_idx < 0:
        # 无空行:整段是 boilerplate(纯 prompt,无用户输入)
        return "", content

    user_text = "\n".join(lines[blank_idx + 1:]).strip()
    boilerplate = "\n".join(lines[:blank_idx]) + "\n"
    return user_text, boilerplate


def _filter_system_hints(text: str) -> tuple[str, list[str]]:
    """过滤 [System Hint] 行,归入 hints 列表。"""
    hints = []
    text_lines = []
    for line in text.split("\n"):
        if line.startswith("[System Hint]"):
            hints.append(line.replace("[System Hint]", "").strip())
        else:
            text_lines.append(line)
    return "\n".join(text_lines), hints


def _parse_user_content(content) -> tuple[str, list[str], list[dict], bool]:
    """解析 user message content,分离用户输入 / system hint / 多模态附件 / 内部协议命令。

    返回 (main_text, system_hints, attachments, has_internal_command)。
    attachments 是完整 Attachment dict 列表(id/filename/mime_type/size/url),
    直接对应前端 Attachment 接口,无需二次转换。
    """
    if content is None:
        return "", [], [], False

    # 多模态 array
    if isinstance(content, list):
        text_lines = []
        attachments = []
        for part in content:
            if isinstance(part, str):
                text_lines.append(part)
                continue
            if isinstance(part, dict):
                ptype = part.get("type")
                if ptype == "text" and part.get("text"):
                    text_lines.append(part["text"])
                elif ptype == "image_url":
                    url = (part.get("image_url") or {}).get("url", "") or part.get("url", "")
                    if url:
                        attachments.append(_attachment_from_data_url(url, len(attachments) + 1))
                elif ptype == "input_audio":
                    data = (part.get("input_audio") or {}).get("data", "")
                    if data:
                        attachments.append(_attachment_from_data_url(data, len(attachments) + 1))
                elif ptype == "media_ref":
                    # media_ref:引用 session/media/ 目录的文件(path 是绝对路径)
                    # path 格式:<cwd>/.helen/sessions/<sid>/media/<filename>
                    path = part.get("path", "")
                    if path:
                        from pathlib import Path as _Path
                        p = _Path(path)
                        filename = p.name
                        sid = p.parent.parent.name if p.parent.parent else ""
                        mime = part.get("mime", "application/octet-stream")
                        if filename and sid:
                            attachments.append({
                                "id": f"media-{len(attachments) + 1}",
                                "filename": filename,
                                "mime_type": mime,
                                "size": part.get("size", 0),
                                "url": f"/api/chat/sessions/{sid}/media/{filename}",
                            })
        joined = "\n".join(text_lines)
        user_text, _ = _extract_from_boilerplate(joined)
        user_text, hints = _filter_system_hints(user_text)
        return user_text, hints, attachments, False

    # 纯字符串
    s = str(content)
    import re
    if re.match(r"^__helen_\w+__", s.strip()):
        return "", [], [], True

    user_text, _ = _extract_from_boilerplate(s)
    user_text, hints = _filter_system_hints(user_text)
    return user_text, hints, [], False


def transcript_to_messages(helen_session_id: str = "") -> list[dict]:
    """读取 transcript,转换成前端 Message 格式(用户输入已过滤 boilerplate)。

    transcript 是唯一数据源(v6.1:替代 SQLite messages 表)。

    - helen_session_id 为空时,使用当前 Helen session
    - 过滤 type != "message"(跳过 session_meta / boundary_marker)
    - 跳过 [TEST] 测试消息
    - user message:过滤 prompt boilerplate / [System Hint] / 内部协议命令,
      只保留用户实际输入 + 多模态附件(见 _parse_user_content)
    - 纯 boilerplate 无用户输入且无附件的 user message:跳过
    - assistant message:content 原样保留
    - timestamp:消息级无 timestamp 字段,用 session_meta.timestamp

    返回:[{id, role, content, attachments, timestamp}]
    """
    from datetime import datetime

    if not helen_session_id:
        helen_session_id = get_current_helen_session_id()
    if not helen_session_id:
        return []

    entries = read_transcript_entries(helen_session_id)

    # session_meta 的 timestamp 作为消息时间戳(消息本身无 timestamp 字段)
    # v1.30.12: 将 Unix 时间戳(秒)转换为 ISO 格式字符串,避免前端误解析为毫秒
    session_timestamp_iso = None
    for e in entries:
        if e.get("type") == "session_meta":
            ts = e.get("timestamp")
            if ts is not None and isinstance(ts, (int, float)):
                try:
                    session_timestamp_iso = datetime.fromtimestamp(ts).isoformat()
                except (ValueError, OSError):
                    session_timestamp_iso = None
            break

    messages = []
    for e in entries:
        if e.get("type") != "message":
            continue
        if is_test_message(e):
            continue

        role = e.get("role", "user")
        uuid = e.get("uuid", "")
        content = e.get("content", "")

        if role == "user":
            main_text, _hints, attachments, has_internal_cmd = _parse_user_content(content)
            if has_internal_cmd:
                continue
            # 斜杠命令:不显示历史(防御性,通常 transcript 无此条目)
            if main_text.strip().startswith("/"):
                continue
            # 纯 boilerplate(无用户输入且无附件):跳过
            if not main_text and not attachments:
                continue
            text_content = main_text
        else:
            # assistant / 其他角色:content 原样
            if isinstance(content, str):
                text_content = content
            elif isinstance(content, list):
                texts = [
                    p.get("text", "")
                    for p in content
                    if isinstance(p, dict) and p.get("type") == "text"
                ]
                text_content = "\n".join(texts)
            else:
                text_content = str(content)
            attachments = []

        messages.append({
            "id": uuid,
            "role": role,
            "content": text_content,
            "attachments": attachments,
            "timestamp": session_timestamp_iso,
        })

    return messages


def read_session_preview(helen_session_id: str, max_chars: int = 50) -> str:
    """读取 transcript 首条 user 消息的实际输入(过滤 boilerplate),截断作预览。"""
    entries = read_transcript_entries(helen_session_id)
    for e in entries:
        if e.get("type") != "message":
            continue
        if e.get("role") != "user":
            continue
        main_text, _hints, media_parts, has_internal_cmd = _parse_user_content(e.get("content", ""))
        if has_internal_cmd:
            continue
        if main_text.strip().startswith("/"):
            continue
        if not main_text and not media_parts:
            continue
        preview = main_text.strip() if main_text else "[附件]"
        return preview[:max_chars]
    return ""
