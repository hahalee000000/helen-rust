# /clear 命令修复总结

## 问题描述

用户在 Helen WebUI 中执行 `/clear` 命令后：
- ✅ 前端显示立即清空（M26.10 已修复）
- ✅ SessionStats 重置（M26.11 已修复）
- ✅ BoundaryMarker 插入 transcript（M26.14 已修复）
- ❌ **但是**：刷新页面后所有消息重新出现

## 根本原因

`TranscriptReader::entries_to_messages()` 函数忽略了 `boundary_marker` 条目。当页面刷新时：
1. 前端调用 `GET /api/chat/dir/messages`
2. 后端通过 `TranscriptReader` 读取 transcript.jsonl
3. `entries_to_messages()` 跳过 boundary_marker 条目但返回所有 message 条目
4. 应该被 boundary 隐藏的消息重新出现

Python 参考实现的 `read_view()` 方法正确地应用 boundary markers 来隐藏 `[head_uuid, tail_uuid]` 范围内的消息，但 Rust 移植没有实现这个逻辑。

## 解决方案

### 代码修改

**文件**: `crates/helen-agent/src/transcript.rs`

1. **为 `TranscriptEntry` 添加 boundary_marker 字段**：
   ```rust
   pub head_uuid: Option<String>,
   pub tail_uuid: Option<String>,
   pub anchor_uuid: Option<String>,
   pub summary: Option<String>,
   pub layer: Option<String>,
   ```

2. **修改 `entries_to_messages()` 以尊重 boundary markers**：
   - 构建 UUID → 索引映射
   - 从所有 boundary_marker 条目收集被压缩（隐藏）的 UUID
   - 对于每个 boundary，标记 `[head_idx, tail_idx]` 范围内的消息为压缩
   - 跳过 UUID 在压缩集合中的消息

3. **更新 `extract_preview()` 以尊重 boundary markers**：
   - 使用与 `entries_to_messages()` 相同的逻辑，确保会话预览不显示已清除的消息

### 算法

```rust
// 1. 构建 UUID 索引
let uuid_index: HashMap<&str, usize> = entries
    .iter()
    .enumerate()
    .filter_map(|(i, e)| e.uuid.as_deref().map(|uuid| (uuid, i)))
    .collect();

// 2. 从所有 boundary_marker 收集压缩的 UUID
let mut compressed_uuids: HashSet<String> = HashSet::new();
for entry in entries {
    if entry.entry_type != "boundary_marker" { continue; }
    
    let head_idx = uuid_index.get(entry.head_uuid.as_deref()?)?;
    let tail_idx = uuid_index.get(entry.tail_uuid.as_deref()?)?;
    
    // 标记范围内的所有消息为压缩
    let (lo, hi) = if head_idx <= tail_idx {
        (head_idx, tail_idx)
    } else {
        (tail_idx, head_idx)
    };
    
    for entry in entries.iter().take(hi + 1).skip(lo) {
        if let Some(ref uuid) = entry.uuid {
            compressed_uuids.insert(uuid.clone());
        }
    }
}

// 3. 构建结果时跳过压缩的消息
for entry in entries {
    if entry.entry_type != "message" { continue; }
    if compressed_uuids.contains(entry.uuid.as_deref()?) { continue; }
    // ... 处理消息
}
```

## 回归测试

### 单元测试（在 `transcript.rs` 中）

添加了 6 个单元测试覆盖：
- `test_entries_to_messages_no_boundary`: 基线 - 没有 boundary 时返回所有消息
- `test_entries_to_messages_boundary_clears_all`: 覆盖所有消息的 boundary 隐藏它们
- `test_entries_to_messages_boundary_partial`: 覆盖子集的 boundary 只隐藏那些
- `test_entries_to_messages_multiple_boundaries`: 多个 boundary 正确工作
- `test_entries_to_messages_boundary_with_missing_uuids`: 优雅处理无效 boundary
- `test_entries_to_messages_boundary_empty_head_tail`: 空 head/tail 被忽略

### 集成测试（在 `tests/test_transcript.rs` 中）

添加了 3 个集成测试模拟真实场景：

1. **`test_clear_command_hides_messages_after_refresh`**：
   - 模拟 `/clear` 命令后的 transcript
   - 验证所有消息被 boundary_marker 隐藏
   - **这是报告问题的主要回归测试**

2. **`test_clear_then_new_messages_only_shows_new`**：
   - 模拟：对话 → /clear → 新对话
   - 验证只有新消息（boundary 之后）可见

3. **`test_multiple_clear_commands`**：
   - 模拟多次 `/clear` 操作
   - 验证所有 boundary 正确应用

## 测试结果

```
running 11 tests
test test_read_transcript_with_entries ... ok
test test_read_transcript_skips_invalid_json ... ok
test test_multiple_clear_commands ... ok
test test_session_info_structure ... ok
test test_transcript_to_messages_extracts_timestamp ... ok
test test_clear_command_hides_messages_after_refresh ... ok
test test_read_empty_transcript ... ok
test test_list_sessions ... ok
test test_transcript_to_messages_filters_non_messages ... ok
test test_transcript_to_messages_filters_test_messages ... ok
test test_clear_then_new_messages_only_shows_new ... ok

test result: ok. 11 passed; 0 failed
```

## 为什么之前没有发现

之前的修复（M26.10, M26.11, M26.14）专注于**运行时行为**：
- M26.10: 发送 `clear_messages` WS 事件清空前端
- M26.11: 重置 SessionStats 快照
- M26.14: 实际插入 BoundaryMarker（之前是 stub）

但没有测试**持久化层**：当页面刷新并从 transcript 重新加载消息时会发生什么。

现有测试 `test_transcript_to_messages_filters_non_messages` 只验证 boundary_marker 条目本身不作为消息返回，但没有验证它们应该**隐藏**它们覆盖的消息。

## 经验教训

1. **测试完整的用户旅程**：不要只单独测试组件。测试完整流程：用户操作 → 运行时行为 → 持久化 → 重新加载。

2. **回归测试应该覆盖确切的失败场景**：测试应该模拟"执行 /clear → 刷新页面 → 验证消息不重新出现"。

3. **移植需要一致性测试**：Python 参考实现正确有这个逻辑。与参考的差异测试会更早发现这个问题。

## 相关提交

- M26.10 (ae57d9ca): 修复 /clear 不发送 clear_messages WS 事件
- M26.11 (f07e2a52): 修复 /clear 不重置 SessionStats
- M26.14 (439b184e): 修复 clear_context() 实际插入 BoundaryMarker
- **本次修复**: 让 TranscriptReader 尊重 boundary_marker 条目

## 修改的文件

- `crates/helen-agent/src/transcript.rs`: 核心修复 + 单元测试
- `crates/helen-agent/tests/test_transcript.rs`: 集成回归测试
- `wiki/rust/m26-clear-command-fix.md`: 详细文档

## 验证步骤

1. 运行单元测试：`cargo test -p helen-agent --lib transcript::`
2. 运行集成测试：`cargo test -p helen-agent --test test_transcript`
3. 运行 clippy：`cargo clippy -p helen-agent -- -D warnings`
4. 手动测试：启动 WebUI → 发送消息 → 执行 /clear → 刷新页面 → 验证消息不重新出现
