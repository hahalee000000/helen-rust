"""session_index 模块单元测试

v6.1:transcript 唯一数据源。覆盖 transcript_to_messages / read_session_preview /
_parse_user_content / _extract_from_boilerplate(prompt boilerplate 过滤)。

运行: cd webui/backend && pytest tests/test_session_index.py -v
"""

import json
import pytest


# ── Fixtures ─────────────────────────────────────────────────────

@pytest.fixture
def temp_agent_dir(tmp_path):
    """临时 agent 根目录"""
    agent_dir = tmp_path / "helenagent"
    agent_dir.mkdir()
    (agent_dir / ".helen").mkdir()
    return agent_dir


@pytest.fixture
def session_index(temp_agent_dir):
    """带临时目录的 session_index 模块"""
    from app.services import session_index, directory_manager
    from unittest.mock import patch
    original_agent_dir = session_index._AGENT_DIR
    session_index._AGENT_DIR = temp_agent_dir
    with patch.object(directory_manager, "get_current_cwd", return_value=str(temp_agent_dir)):
        yield session_index
    session_index._AGENT_DIR = original_agent_dir


def _write_transcript(agent_dir, sid, lines):
    """在 temp_agent_dir 下创建 transcript 文件"""
    sessions_dir = agent_dir / ".helen" / "sessions" / sid
    sessions_dir.mkdir(parents=True, exist_ok=True)
    (sessions_dir / "transcript.jsonl").write_text("".join(l + "\n" for l in lines))


# ── prompt boilerplate 过滤 ────────────────────────────────────

class TestExtractFromBoilerplate:
    def test_no_boilerplate(self, session_index):
        """无 prompt boilerplate -> 原样返回"""
        text, boilerplate = session_index._extract_from_boilerplate("你好")
        assert text == "你好"
        assert boilerplate == ""

    def test_with_boilerplate(self, session_index):
        """含 prompt boilerplate -> 分离用户输入"""
        content = "## Identity\nYou are HelenAgent\n## Reminders\nIMPORTANT: ...\n\n用户实际输入"
        text, boilerplate = session_index._extract_from_boilerplate(content)
        assert text == "用户实际输入"
        assert "## Identity" in boilerplate

    def test_pure_boilerplate_no_user_input(self, session_index):
        """纯 boilerplate 无用户输入 -> user_text 为空"""
        content = "## Identity\nYou are HelenAgent\n## Reminders\nIMPORTANT: ..."
        text, _ = session_index._extract_from_boilerplate(content)
        assert text == ""

    def test_no_identity_marker(self, session_index):
        """不以 ## Identity 开头 -> 不识别为 boilerplate"""
        content = "## 其他标题\n\n用户输入"
        text, boilerplate = session_index._extract_from_boilerplate(content)
        assert text == content
        assert boilerplate == ""


# ── _parse_user_content ─────────────────────────────────────────

class TestParseUserContent:
    def test_plain_string(self, session_index):
        """纯文本 user message"""
        text, hints, media, has_cmd = session_index._parse_user_content("你好")
        assert text == "你好"
        assert hints == []
        assert media == []
        assert has_cmd is False

    def test_internal_command(self, session_index):
        """内部协议命令 __helen_xxx__ -> has_internal_command"""
        text, _, _, has_cmd = session_index._parse_user_content("__helen_resume__abc")
        assert has_cmd is True
        assert text == ""

    def test_multimodal_with_image(self, session_index):
        """多模态:image_url part 提取为 Attachment"""
        content = [
            {"type": "text", "text": "这是什么?"},
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,abc"}},
        ]
        text, _, attachments, has_cmd = session_index._parse_user_content(content)
        assert text == "这是什么?"
        assert len(attachments) == 1
        assert attachments[0]["mime_type"] == "image/png"
        assert attachments[0]["url"].startswith("data:image/png;base64,")
        assert has_cmd is False

    def test_media_ref_attachment(self, session_index):
        """media_ref:Helen 的 session media 引用转 HTTP URL"""
        content = [
            {"type": "text", "text": "描述图片"},
            {"type": "media_ref", "path": "/home/x/p/.helen/sessions/sid-1/media/img.png",
             "mime": "image/png", "size": 1234},
        ]
        text, _, attachments, _ = session_index._parse_user_content(content)
        assert text == "描述图片"
        assert len(attachments) == 1
        assert attachments[0]["filename"] == "img.png"
        assert attachments[0]["mime_type"] == "image/png"
        assert attachments[0]["size"] == 1234
        assert attachments[0]["url"] == "/api/chat/sessions/sid-1/media/img.png"

    def test_system_hint_filtered(self, session_index):
        """[System Hint] 行被过滤到 hints"""
        text, hints, _, _ = session_index._parse_user_content("[System Hint] 提示内容\n用户输入")
        assert "用户输入" in text
        assert "[System Hint]" not in text
        assert hints == ["提示内容"]


# ── transcript_to_messages ─────────────────────────────────────

class TestTranscriptToMessages:
    def test_filters_session_meta_and_boundary(self, session_index, temp_agent_dir):
        """跳过 session_meta / boundary_marker"""
        sid = "test-session"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 123.0}),
            json.dumps({"type": "message", "role": "user", "content": "你好", "uuid": "u1"}),
            json.dumps({"type": "boundary_marker"}),
            json.dumps({"type": "message", "role": "assistant", "content": "你好!", "uuid": "a1"}),
        ])
        msgs = session_index.transcript_to_messages(sid)
        assert len(msgs) == 2
        assert msgs[0]["role"] == "user"
        assert msgs[0]["content"] == "你好"
        assert msgs[1]["role"] == "assistant"
        assert msgs[1]["content"] == "你好!"

    def test_filters_prompt_boilerplate(self, session_index, temp_agent_dir):
        """user message 含 prompt boilerplate -> 只保留用户输入"""
        sid = "test-session"
        boilerplate = "## Identity\nYou are HelenAgent\n## Reminders\nIMPORTANT: ...\n\n用户实际输入"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 123.0}),
            json.dumps({"type": "message", "role": "user", "content": boilerplate, "uuid": "u1"}),
            json.dumps({"type": "message", "role": "assistant", "content": "收到", "uuid": "a1"}),
        ])
        msgs = session_index.transcript_to_messages(sid)
        assert len(msgs) == 2
        assert msgs[0]["content"] == "用户实际输入"
        assert msgs[1]["content"] == "收到"

    def test_pure_boilerplate_skipped(self, session_index, temp_agent_dir):
        """纯 boilerplate 无用户输入的 user message -> 跳过"""
        sid = "test-session"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 123.0}),
            json.dumps({"type": "message", "role": "user", "content": "## Identity\nYou are HelenAgent\n## Reminders\nIMPORTANT: ...", "uuid": "u1"}),
            json.dumps({"type": "message", "role": "user", "content": "真实输入", "uuid": "u2"}),
        ])
        msgs = session_index.transcript_to_messages(sid)
        assert len(msgs) == 1
        assert msgs[0]["content"] == "真实输入"

    def test_multimodal_attachments(self, session_index, temp_agent_dir):
        """多模态 user message -> 附件提取为 Attachment"""
        sid = "test-session"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 123.0}),
            json.dumps({"type": "message", "role": "user", "content": [
                {"type": "text", "text": "看图"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,abc"}},
            ], "uuid": "u1"}),
        ])
        msgs = session_index.transcript_to_messages(sid)
        assert len(msgs) == 1
        assert msgs[0]["content"] == "看图"
        assert len(msgs[0]["attachments"]) == 1
        assert msgs[0]["attachments"][0]["mime_type"] == "image/png"

    def test_test_messages_filtered(self, session_index, temp_agent_dir):
        """[TEST] 消息被过滤"""
        sid = "test-session"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 123.0}),
            json.dumps({"type": "message", "role": "user", "content": "[TEST] 测试消息", "uuid": "u1"}),
            json.dumps({"type": "message", "role": "user", "content": "真实输入", "uuid": "u2"}),
        ])
        msgs = session_index.transcript_to_messages(sid)
        assert len(msgs) == 1
        assert msgs[0]["content"] == "真实输入"

    def test_timestamp_from_session_meta(self, session_index, temp_agent_dir):
        """timestamp 取自 session_meta，转换为 ISO 格式"""
        from datetime import datetime
        sid = "test-session"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 999.0}),
            json.dumps({"type": "message", "role": "user", "content": "你好", "uuid": "u1"}),
        ])
        msgs = session_index.transcript_to_messages(sid)
        # v1.30.12: timestamp 现在是 ISO 格式字符串，不是原始浮点数
        expected_iso = datetime.fromtimestamp(999.0).isoformat()
        assert msgs[0]["timestamp"] == expected_iso


# ── read_session_preview ───────────────────────────────────────

class TestReadSessionPreview:
    def test_preview_first_user_message(self, session_index, temp_agent_dir):
        """预览取首条 user 消息"""
        sid = "test-session"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 123.0}),
            json.dumps({"type": "message", "role": "user", "content": "这是首条消息", "uuid": "u1"}),
            json.dumps({"type": "message", "role": "assistant", "content": "回复", "uuid": "a1"}),
        ])
        preview = session_index.read_session_preview(sid)
        assert preview == "这是首条消息"

    def test_preview_filters_boilerplate(self, session_index, temp_agent_dir):
        """预览过滤 prompt boilerplate"""
        sid = "test-session"
        boilerplate = "## Identity\nYou are HelenAgent\n## Reminders\nIMPORTANT: ...\n\n用户实际输入"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 123.0}),
            json.dumps({"type": "message", "role": "user", "content": boilerplate, "uuid": "u1"}),
        ])
        preview = session_index.read_session_preview(sid)
        assert preview == "用户实际输入"

    def test_preview_empty_session(self, session_index, temp_agent_dir):
        """空 session -> 空预览"""
        sid = "test-session"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 123.0}),
        ])
        preview = session_index.read_session_preview(sid)
        assert preview == ""


# ── get_current_helen_session_id ───────────────────────────────

class TestGetCurrentHelenSessionId:
    def test_no_memento_returns_empty(self, session_index, temp_agent_dir):
        """memento 不存在 -> 返回空字符串"""
        result = session_index.get_current_helen_session_id()
        assert result == ""

    def test_json_memento_with_child(self, session_index, temp_agent_dir):
        """JSON memento 返回 child session ID（当 transcript 存在时）"""
        sid = "child-session-123"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 1.0}),
        ])
        memento_path = temp_agent_dir / ".helen" / "current_session_id"
        memento_path.write_text(json.dumps({"main": "main-sid", "child": sid}))

        result = session_index.get_current_helen_session_id()
        assert result == sid

    def test_json_memento_child_no_transcript(self, session_index, temp_agent_dir):
        """JSON memento 的 child session 无 transcript -> 返回空"""
        memento_path = temp_agent_dir / ".helen" / "current_session_id"
        memento_path.write_text(json.dumps({"main": "main-sid", "child": "nonexistent"}))

        result = session_index.get_current_helen_session_id()
        assert result == ""

    def test_json_memento_empty_child(self, session_index, temp_agent_dir):
        """JSON memento child 为空 -> 返回空"""
        memento_path = temp_agent_dir / ".helen" / "current_session_id"
        memento_path.write_text(json.dumps({"main": "main-sid", "child": ""}))

        result = session_index.get_current_helen_session_id()
        assert result == ""

    def test_plain_text_memento(self, session_index, temp_agent_dir):
        """纯文本 memento（旧格式兼容）"""
        sid = "plain-text-session"
        _write_transcript(temp_agent_dir, sid, [
            json.dumps({"type": "session_meta", "timestamp": 1.0}),
        ])
        memento_path = temp_agent_dir / ".helen" / "current_session_id"
        memento_path.write_text(sid)

        result = session_index.get_current_helen_session_id()
        assert result == sid

    def test_plain_text_memento_no_transcript(self, session_index, temp_agent_dir):
        """纯文本 memento 但 transcript 不存在 -> 返回空"""
        memento_path = temp_agent_dir / ".helen" / "current_session_id"
        memento_path.write_text("nonexistent-session")

        result = session_index.get_current_helen_session_id()
        assert result == ""

    def test_memento_invalid_json(self, session_index, temp_agent_dir):
        """memento 以 { 开头但不是有效 JSON -> 返回空"""
        memento_path = temp_agent_dir / ".helen" / "current_session_id"
        memento_path.write_text("{invalid json")

        result = session_index.get_current_helen_session_id()
        assert result == ""

    def test_memento_empty_file(self, session_index, temp_agent_dir):
        """memento 文件为空 -> 返回空"""
        memento_path = temp_agent_dir / ".helen" / "current_session_id"
        memento_path.write_text("")

        result = session_index.get_current_helen_session_id()
        assert result == ""


# ── _attachment_from_data_url ──────────────────────────────────

class TestAttachmentFromDataUrl:
    def test_image_data_url(self, session_index):
        """image data URL -> kind=image, 正确的 mime_type"""
        data_url = "data:image/png;base64,iVBORw0KGgo="
        att = session_index._attachment_from_data_url(data_url, 1)
        assert att["id"] == "att-1"
        assert att["filename"] == "image-1.png"
        assert att["mime_type"] == "image/png"
        assert att["url"] == data_url
        assert att["size"] > 0

    def test_audio_data_url(self, session_index):
        """audio data URL -> kind=audio"""
        data_url = "data:audio/wav;base64,UklGR"
        att = session_index._attachment_from_data_url(data_url, 2)
        assert att["filename"] == "audio-2.wav"
        assert att["mime_type"] == "audio/wav"

    def test_video_data_url(self, session_index):
        """video data URL -> kind=video"""
        data_url = "data:video/mp4;base64,AAAA"
        att = session_index._attachment_from_data_url(data_url, 3)
        assert att["filename"] == "video-3.mp4"
        assert att["mime_type"] == "video/mp4"

    def test_other_mime_data_url(self, session_index):
        """非 image/audio/video -> kind=file"""
        data_url = "data:application/pdf;base64,JSVD"
        att = session_index._attachment_from_data_url(data_url, 4)
        assert att["filename"] == "file-4.pdf"
        assert att["mime_type"] == "application/pdf"

    def test_no_comma_in_data_url(self, session_index):
        """无逗号的 data URL -> b64_data 为空"""
        data_url = "data:image/png;base64"
        att = session_index._attachment_from_data_url(data_url, 5)
        assert att["size"] == 0

    def test_no_base64_prefix(self, session_index):
        """不匹配的 data URL -> 默认 mime"""
        data_url = "something-without-proper-prefix"
        att = session_index._attachment_from_data_url(data_url, 6)
        assert att["mime_type"] == "application/octet-stream"
        assert att["filename"] == "file-6.bin"
