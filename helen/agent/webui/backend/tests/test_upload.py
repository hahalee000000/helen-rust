"""
TDD 测试：文件上传 API（阶段 1）

运行：cd webui/backend && pytest tests/test_upload.py -v

测试策略：
- 验证文件上传成功（200 + metadata）
- 验证 MIME 类型检查（400）
- 验证文件大小限制（413）
- 验证文件获取端点
- 验证所有支持的 MIME 类型
- 验证 404 情况
"""
import io
import json
import pytest
from pathlib import Path


# ── Fixtures ──────────────────────────────────────────────

@pytest.fixture
def tmp_upload_dir(tmp_path, monkeypatch):
    """隔离的上传目录

    monkeypatch directory_manager.get_current_cwd() 返回 tmp_path，
    这样上传的文件保存到 tmp_path/.helen/uploads/ 而不是真实项目目录。
    """
    upload_dir = tmp_path / ".helen" / "uploads"
    upload_dir.mkdir(parents=True)

    from app.services import directory_manager
    monkeypatch.setattr(directory_manager, "get_current_cwd", lambda: str(tmp_path))

    return upload_dir


@pytest.fixture
def test_client():
    """FastAPI 测试客户端(v6.1 无 DB)"""
    from fastapi.testclient import TestClient
    from app.main import app
    return TestClient(app)


# ── 测试用例 ──────────────────────────────────────────────

class TestUploadAPI:
    """文件上传 API 测试"""

    def test_upload_valid_image(self, test_client, tmp_upload_dir):
        """上传有效图片 → 返回 upload_id + metadata"""
        file_content = b"fake jpeg content"
        response = test_client.post(
            "/api/chat/upload",
            files={"file": ("test.jpg", io.BytesIO(file_content), "image/jpeg")}
        )
        assert response.status_code == 200
        data = response.json()
        assert "upload_id" in data
        assert data["filename"] == "test.jpg"
        assert data["mime_type"] == "image/jpeg"
        assert data["size"] == len(file_content)
        assert "url" in data
        # 验证文件已保存到隔离目录
        upload_dir = tmp_upload_dir / data["upload_id"]
        assert (upload_dir / "file").exists()
        assert (upload_dir / "file").read_bytes() == file_content
        assert (upload_dir / "metadata.json").exists()
        metadata = json.loads((upload_dir / "metadata.json").read_text())
        assert metadata["upload_id"] == data["upload_id"]
        assert metadata["filename"] == "test.jpg"
        assert metadata["mime_type"] == "image/jpeg"

    def test_upload_reject_invalid_mime(self, test_client, tmp_upload_dir):
        """上传不支持的 MIME 类型 → 400"""
        response = test_client.post(
            "/api/chat/upload",
            files={"file": ("test.exe", io.BytesIO(b"binary"), "application/x-executable")}
        )
        assert response.status_code == 400
        assert "unsupported" in response.json().get("detail", "").lower() or \
               "unsupported" in str(response.text).lower()

    def test_upload_reject_too_large(self, test_client, tmp_upload_dir, monkeypatch):
        """上传超大文件 → 413"""
        import app.routers.chat as chat_module
        monkeypatch.setattr(chat_module, "MAX_FILE_SIZE", 100)
        response = test_client.post(
            "/api/chat/upload",
            files={"file": ("big.png", io.BytesIO(b"x" * 200), "image/png")}
        )
        assert response.status_code == 413

    def test_get_uploaded_file(self, test_client, tmp_upload_dir):
        """上传后通过 URL 获取文件"""
        file_content = b"png data here"
        upload_resp = test_client.post(
            "/api/chat/upload",
            files={"file": ("test.png", io.BytesIO(file_content), "image/png")}
        )
        upload_id = upload_resp.json()["upload_id"]

        get_resp = test_client.get(f"/api/chat/uploads/{upload_id}/file")
        assert get_resp.status_code == 200
        assert get_resp.content == file_content
        assert get_resp.headers["content-type"] == "image/png"

    def test_get_nonexistent_upload(self, test_client, tmp_upload_dir):
        """获取不存在的 upload → 404"""
        response = test_client.get("/api/chat/uploads/nonexistent-id/file")
        assert response.status_code == 404

    @pytest.mark.parametrize("mime", [
        "image/jpeg", "image/png", "image/gif", "image/webp",
        "audio/wav", "audio/ogg",
        "video/mp4", "video/webm",
    ])
    def test_upload_all_supported_mimes(self, test_client, tmp_upload_dir, mime):
        """所有声明支持的 MIME 类型都能上传"""
        response = test_client.post(
            "/api/chat/upload",
            files={"file": (f"test.{mime.split('/')[-1]}", io.BytesIO(b"data"), mime)}
        )
        assert response.status_code == 200, f"Failed for MIME type: {mime}"

    def test_upload_creates_metadata_with_timestamp(self, test_client, tmp_upload_dir):
        """上传文件的 metadata 包含 created_at 时间戳"""
        response = test_client.post(
            "/api/chat/upload",
            files={"file": ("test.jpg", io.BytesIO(b"content"), "image/jpeg")}
        )
        upload_id = response.json()["upload_id"]
        metadata_path = tmp_upload_dir / upload_id / "metadata.json"
        metadata = json.loads(metadata_path.read_text())
        assert "created_at" in metadata
        # created_at 应该是 ISO 格式字符串
        assert isinstance(metadata["created_at"], str)
        assert "T" in metadata["created_at"]  # ISO 格式包含 T

    def test_upload_multiple_files_gets_different_ids(self, test_client, tmp_upload_dir):
        """上传多个文件获得不同的 upload_id"""
        resp1 = test_client.post(
            "/api/chat/upload",
            files={"file": ("a.png", io.BytesIO(b"aaa"), "image/png")}
        )
        resp2 = test_client.post(
            "/api/chat/upload",
            files={"file": ("b.png", io.BytesIO(b"bbb"), "image/png")}
        )
        id1 = resp1.json()["upload_id"]
        id2 = resp2.json()["upload_id"]
        assert id1 != id2
