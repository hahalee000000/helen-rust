#!/usr/bin/env python3
"""
端到端集成测试：Web UI Session + Transcript + Side-channel 索引

前置条件：Web UI 后端必须在 http://localhost:8000 运行
    cd webui/backend && uvicorn app.main:app --reload --port 8000

运行：
    python tests/test_session_transcript_integration.py

测试场景：
1. 创建 Web UI session → 发送消息 → 检查索引创建
2. 创建多个 session → 删除一个 → 验证其他 session 不受影响
3. Transcript 过滤 API 验证
4. Session 重命名验证
"""

import requests
import asyncio
import websockets
import json
import time
import os
import sys
from pathlib import Path

BASE_URL = "http://localhost:8000"
PROJECT_ROOT = Path(__file__).resolve().parent.parent
INDEX_DIR = PROJECT_ROOT / ".helen" / "session_index"
SESSIONS_DIR = PROJECT_ROOT / ".helen" / "sessions"

# 测试结果计数
_passed = 0
_failed = 0


def check_server():
    """检查 Web UI 服务是否运行"""
    try:
        r = requests.get(f"{BASE_URL}/health", timeout=10)
        return r.status_code == 200
    except requests.ConnectionError:
        return False
    except requests.Timeout:
        return False


def assert_true(condition, message):
    global _passed, _failed
    if condition:
        print(f"  ✓ {message}")
        _passed += 1
    else:
        print(f"  ✗ FAIL: {message}")
        _failed += 1


def create_webui_session(title="测试会话"):
    r = requests.post(f"{BASE_URL}/api/chat/sessions", json={"title": title})
    assert r.status_code == 200
    return r.json()


def delete_webui_session(session_id):
    r = requests.delete(f"{BASE_URL}/api/chat/sessions/{session_id}")
    return r


def get_session(session_id):
    return requests.get(f"{BASE_URL}/api/chat/sessions/{session_id}")


def update_session(session_id, data):
    return requests.patch(f"{BASE_URL}/api/chat/sessions/{session_id}", json=data)


def get_transcript_by_session(session_id):
    return requests.get(f"{BASE_URL}/api/chat/sessions/{session_id}/transcript/messages")


def get_all_transcript():
    return requests.get(f"{BASE_URL}/api/chat/transcript/all")


def get_unmapped_transcript():
    return requests.get(f"{BASE_URL}/api/chat/transcript/unmapped")


async def send_message_via_ws(session_id, message, timeout=30):
    """通过 WebSocket 发送消息，返回 helen_session_id"""
    uri = f"ws://localhost:8000/api/chat/ws/{session_id}"
    async with websockets.connect(uri) as ws:
        await asyncio.sleep(0.3)
        await ws.send(json.dumps({"type": "message", "content": message}))

        helen_sid = None
        start = time.time()
        while time.time() - start < timeout:
            try:
                msg = await asyncio.wait_for(ws.recv(), timeout=1.0)
                data = json.loads(msg)
                if data.get("type") == "helen_session_id":
                    helen_sid = data.get("data", {}).get("session_id")
                if data.get("type") == "llm_complete":
                    break
            except asyncio.TimeoutError:
                continue
        return helen_sid


# ── 场景 1：索引写入 ──────────────────────────────────────────

def test_scenario_1_index_creation():
    """场景 1：创建 session + 发送消息 → 检查索引创建"""
    print("\n═══ 场景 1：索引写入 ═══")

    # 1. 创建 Web UI session
    session = create_webui_session("索引测试会话")
    sid = session["session_id"]
    print(f"  创建 Web UI session: {sid}")

    # 2. 发送消息（触发 Helen 处理 → transcript 写入 → 索引更新）
    print("  发送消息...")
    helen_sid = asyncio.run(send_message_via_ws(sid, "[TEST] 这是一个索引测试消息"))
    print(f"  Helen session: {helen_sid}")
    time.sleep(1)

    # 3. 检查索引文件是否创建
    index_file = INDEX_DIR / f"{sid}.json"
    assert_true(index_file.exists(), f"索引文件已创建: {index_file.name}")

    if index_file.exists():
        data = json.loads(index_file.read_text())
        assert_true("web_ui_session_id" in data, "索引文件包含 web_ui_session_id")
        assert_true("message_uuids" in data, "索引文件包含 message_uuids")
        assert_true(len(data["message_uuids"]) > 0, f"索引包含 {len(data['message_uuids'])} 个 UUID")

    # 4. 验证 transcript 过滤 API
    resp = get_transcript_by_session(sid)
    assert_true(resp.status_code == 200, f"transcript 过滤 API 返回 200 (got {resp.status_code})")
    if resp.status_code == 200:
        data = resp.json()
        assert_true("messages" in data, "响应包含 messages")
        assert_true(data["total"] > 0, f"过滤后包含 {data['total']} 条消息")

    # 清理
    delete_webui_session(sid)
    print()


# ── 场景 2：删除行为 ──────────────────────────────────────────

def test_scenario_2_delete_behavior():
    """场景 2：删除一个 session → 验证其他 session 不受影响"""
    print("═══ 场景 2：删除行为 ═══")

    # 1. 创建两个 session
    session_a = create_webui_session("会话 A")
    session_b = create_webui_session("会话 B")
    sid_a, sid_b = session_a["session_id"], session_b["session_id"]
    print(f"  创建 A: {sid_a}")
    print(f"  创建 B: {sid_b}")

    # 2. 各发送消息
    print("  发送消息到 A...")
    asyncio.run(send_message_via_ws(sid_a, "[TEST] 来自 A 的消息"))
    time.sleep(1)
    print("  发送消息到 B...")
    asyncio.run(send_message_via_ws(sid_b, "[TEST] 来自 B 的消息"))
    time.sleep(1)

    # 3. 检查两个索引都存在
    index_a = INDEX_DIR / f"{sid_a}.json"
    index_b = INDEX_DIR / f"{sid_b}.json"
    assert_true(index_a.exists(), "A 的索引存在")
    assert_true(index_b.exists(), "B 的索引存在")

    # 4. 删除 A
    print("  删除 A...")
    resp = delete_webui_session(sid_a)
    assert_true(resp.status_code == 200, f"删除 A 返回 200 (got {resp.status_code})")

    # 5. 验证 A 的索引被删除
    assert_true(not index_a.exists(), "A 的索引被删除")

    # 6. 验证 B 的索引完好
    assert_true(index_b.exists(), "B 的索引完好")

    # 7. 验证 transcript 文件仍可访问（通过 fallback 找到最新 transcript）
    #    .tui_session_id 可能是过时的，但 transcript/all 端点会回退到最新的 transcript
    resp_transcript = get_all_transcript()
    assert_true(resp_transcript.status_code == 200, "Transcript 仍可通过 fallback 访问")

    # 8. 验证 A 已从 session 列表中消失
    assert_true(get_session(sid_a).status_code == 404, "A 在 session 列表中已不存在")
    assert_true(get_session(sid_b).status_code == 200, "B 仍在 session 列表中")

    # 清理
    delete_webui_session(sid_b)
    print()


# ── 场景 3：Transcript 过滤 ──────────────────────────────────

def test_scenario_3_transcript_filter():
    """场景 3：验证 transcript 过滤 API"""
    print("═══ 场景 3：Transcript 过滤 ═══")

    # 创建 session + 发送消息
    session = create_webui_session("过滤测试会话")
    sid = session["session_id"]
    asyncio.run(send_message_via_ws(sid, "[TEST] 过滤测试消息"))
    time.sleep(1)

    # 1. 获取全部 transcript
    resp_all = get_all_transcript()
    assert_true(resp_all.status_code == 200, f"获取全部 transcript 返回 200 (got {resp_all.status_code})")
    all_count = 0
    if resp_all.status_code == 200:
        all_data = resp_all.json()
        all_count = all_data.get("total", 0)
        assert_true(all_count > 0, f"全部 transcript 包含 {all_count} 条消息")

    # 2. 获取按 session 过滤的 transcript
    resp_filtered = get_transcript_by_session(sid)
    assert_true(resp_filtered.status_code == 200, f"按 session 过滤返回 200 (got {resp_filtered.status_code})")
    if resp_filtered.status_code == 200:
        filtered_data = resp_filtered.json()
        filtered_count = filtered_data.get("total", 0)
        assert_true(filtered_count > 0, f"过滤后包含 {filtered_count} 条消息")
        assert_true(filtered_count <= all_count, f"过滤后消息数 ({filtered_count}) ≤ 全部消息数 ({all_count})")

    # 3. 获取未映射消息
    resp_unmapped = get_unmapped_transcript()
    assert_true(resp_unmapped.status_code == 200, f"未映射消息返回 200 (got {resp_unmapped.status_code})")
    if resp_unmapped.status_code == 200:
        unmapped_data = resp_unmapped.json()
        unmapped_count = unmapped_data.get("total", 0)
        assert_true(isinstance(unmapped_count, int), f"未映射消息数: {unmapped_count}")

    # 清理
    delete_webui_session(sid)
    print()


# ── 场景 4：重命名 ────────────────────────────────────────────

def test_scenario_4_rename():
    """场景 4：验证 session 重命名"""
    print("═══ 场景 4：重命名 ═══")

    session = create_webui_session("原始标题")
    sid = session["session_id"]

    # 1. 验证初始状态
    resp = get_session(sid)
    assert_true(resp.status_code == 200, "获取 session 返回 200")
    initial_data = resp.json()
    assert_true(initial_data.get("name", "") == "", "初始 name 为空")

    # 2. 重命名
    resp = update_session(sid, {"name": "新名字", "description": "测试描述"})
    assert_true(resp.status_code == 200, f"重命名返回 200 (got {resp.status_code})")
    if resp.status_code == 200:
        data = resp.json()
        assert_true(data.get("name") == "新名字", f"name 更新为 '新名字' (got '{data.get('name')}')")
        assert_true(data.get("description") == "测试描述", f"description 更新为 '测试描述'")

    # 3. 验证持久化
    resp = get_session(sid)
    data = resp.json()
    assert_true(data.get("name") == "新名字", "GET 验证 name 已持久化")
    assert_true(data.get("description") == "测试描述", "GET 验证 description 已持久化")

    # 4. 部分更新
    resp = update_session(sid, {"name": "另一个名字"})
    if resp.status_code == 200:
        data = resp.json()
        assert_true(data.get("name") == "另一个名字", "部分更新 name 成功")
        assert_true(data.get("description") == "测试描述", "部分更新不影响其他字段")

    # 清理
    delete_webui_session(sid)
    print()


# ── 主流程 ────────────────────────────────────────────────────

def main():
    global _passed, _failed
    print("=" * 70)
    print("端到端集成测试：Session + Transcript + Side-channel 索引")
    print("=" * 70)

    # 检查服务
    if not check_server():
        print(f"\n✗ Web UI 服务未运行在 {BASE_URL}")
        print("  请先启动: cd webui/backend && uvicorn app.main:app --reload --port 8000")
        sys.exit(1)
    print(f"✓ Web UI 服务运行在 {BASE_URL}")

    # 检查目录
    INDEX_DIR.mkdir(parents=True, exist_ok=True)

    try:
        test_scenario_1_index_creation()
        test_scenario_2_delete_behavior()
        test_scenario_3_transcript_filter()
        test_scenario_4_rename()
    except Exception as e:
        print(f"\n✗ 测试中断: {e}")
        import traceback
        traceback.print_exc()

    print("=" * 70)
    print(f"结果: {_passed} 通过, {_failed} 失败")
    print("=" * 70)

    sys.exit(0 if _failed == 0 else 1)


if __name__ == "__main__":
    main()
