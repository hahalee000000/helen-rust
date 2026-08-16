# 会话切换导致 Transcript 污染问题分析与修复

## 问题描述

用户报告：当 LLM 正在推理时切换会话，之前会话的 chunk 会显示在两个会话中，并且进入了 transcript。

## 根本原因分析

### 架构问题

Helen 的 session 管理是**全局单例**的：
- `.helen/.tui_session_id` 文件存储当前活跃的 Helen session ID
- 所有 Web UI 会话共享同一个 Helen session
- 通过 side-channel index 映射 Web UI session → transcript message UUIDs

### 问题流程

1. 用户在 Session A 发送消息，启动 `streaming_task`
2. `streaming_task` 开始执行，调用 `get_current_helen_session_id()` 读取 `.tui_session_id`
3. 用户切换到 Session B
4. Session A 的 WebSocket 断开，但 `streaming_task` 继续运行（之前的"持久化推理"设计）
5. 用户切换到 Session B 时，`.tui_session_id` 文件更新为 Session B 的 Helen session ID
6. Session A 的 `streaming_task` 继续运行，调用 `get_current_helen_session_id()` 读取到 Session B 的 Helen session ID
7. Session A 的 chunks 被写入 Session B 的 transcript 文件！
8. `update_session_index()` 将新消息的 UUIDs 添加到 Session A 的 index，但这些消息实际在 Session B 的 transcript 中
9. 结果：Session A 和 Session B 的 transcript 都被污染

### 代码证据

```python
# do_streaming() 开始时
helen_sid = get_current_helen_session_id()  # 读取 .tui_session_id 文件
pre_line_count = count_transcript_lines(helen_sid) if helen_sid else 0

# ... streaming 过程中 ...

# 完成时
async def update_session_index():
    nonlocal helen_sid, pre_line_count
    if not helen_sid:
        return
    new_uuids = get_new_message_uuids(helen_sid, pre_line_count)  # 从 helen_sid 的 transcript 读取
    if new_uuids:
        append_to_index(session_id, new_uuids)  # 添加到 Web UI session 的 index
```

问题在于：
- `helen_sid` 在 streaming 开始时读取一次
- 但 `.tui_session_id` 文件可能在 streaming 过程中被更新
- `get_new_message_uuids(helen_sid, pre_line_count)` 从 `helen_sid` 的 transcript 读取新消息
- 但如果用户切换了会话，`helen_sid` 可能已经不再是活跃的 session
- 更糟的情况：如果 `helen_sid` 是实时读取的（而不是缓存），会读取到新的 session ID

### 为什么"持久化推理"设计是错误的

之前的修复尝试是让 streaming task 独立于 WebSocket 连接，即使 WebSocket 断开也继续运行。但这个设计忽视了：

1. **Helen session 是全局的**：所有 Web UI 会话共享同一个 Helen session，通过 `.tui_session_id` 文件切换
2. **Transcript 是 Helen session 级别的**：不是 Web UI session 级别的
3. **切换会话 = 切换 Helen session**：用户切换 Web UI 会话时，`.tui_session_id` 文件会更新
4. **后台 streaming task 会写入错误的 transcript**：如果 streaming task 继续运行，它会写入新的 Helen session 的 transcript

## 解决方案

**WebSocket 断开时取消 streaming task**。

这是唯一正确的解决方案，因为：
1. 防止 transcript 污染
2. 防止 chunks 显示在错误的会话中
3. 符合用户预期（切换会话 = 结束当前推理）
4. 与 Helen 的 session 管理机制一致

## 修复内容

### 1. 恢复局部 `stream_task` 变量

```python
# 之前（错误的持久化设计）
if not hasattr(websocket.app.state, 'streaming_tasks'):
    websocket.app.state.streaming_tasks = {}
stream_task = websocket.app.state.streaming_tasks.get(session_id)

# 修复后（局部变量）
stream_task: Optional[asyncio.Task] = None
```

### 2. WebSocket 断开时取消 streaming task

```python
except WebSocketDisconnect:
    # 连接断开时清理正在进行的流式任务 + hint 队列
    # 必须取消 streaming task，否则 Helen session 切换后 chunks 会写入错误的 transcript
    if stream_task and not stream_task.done():
        stream_task.cancel()
        try:
            await stream_task
        except (asyncio.CancelledError, Exception):
            pass
    hint_injector.clear_session(session_id)
    manager.disconnect(session_id, websocket)
```

### 3. 移除 `streaming_tasks` 字典相关代码

- 移除 `websocket.app.state.streaming_tasks` 的初始化
- 移除 `finally` 块中的清理逻辑
- 简化 cancel 和 message 处理逻辑

## 用户影响

### 行为变化

| 场景 | 旧行为（持久化） | 新行为（取消） |
|------|-----------------|---------------|
| 切换会话 | ❌ Transcript 污染 | ✅ 推理中断 |
| 刷新页面 | ❌ Transcript 污染 | ✅ 推理中断 |
| 关闭标签页 | ❌ Transcript 污染 | ✅ 推理中断 |
| 网络断开 | ❌ Transcript 污染 | ✅ 推理中断 |

### 为什么这是正确的

1. **符合用户预期**：切换会话 = 结束当前对话，开始新对话
2. **数据一致性**：防止 transcript 污染和 side-channel index 不一致
3. **资源管理**：避免后台任务消耗资源
4. **架构一致**：与 Helen 的 session 管理机制一致

## 测试验证

运行所有后端测试，确保修复没有引入回归：

```bash
cd /home/rxx/helenagent/webui/backend
pytest tests/ -v
```

预期结果：所有测试通过（70/70）。

## 相关文档更新

- ✅ `CLAUDE.md`：更新 WebSocket 断开行为说明
- ✅ `wiki/webui-session-recovery.md`：删除"持久化推理"章节
- ✅ `docs/websocket-disconnect-persistence.md`：标记为废弃，添加问题说明

## 总结

"持久化推理"设计忽视了 Helen session 的全局性和 transcript 的共享性，导致切换会话时 transcript 污染。正确的解决方案是在 WebSocket 断开时取消 streaming task，确保数据一致性和架构清晰性。
