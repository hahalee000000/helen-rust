# 上下文五层压缩 & Transcript Store 一致性对比报告

**日期**: 2026-11-11
**对比版本**: Python helen v1.46.24 vs Rust helen-rust (main)

---

## 一、五层压缩管道 (Graduated Compression)

### 1.1 架构对比

| 维度 | Python | Rust | 状态 |
|------|--------|------|------|
| 文件 | `graduated_compression.py` (907 行) | `compression.rs` (1311 行) | ✅ |
| 5 层管道 | `graduated_compress()` | `graduated_compress()` | ✅ |
| 阈值常量 | `COMPRESSION_THRESHOLDS` dict | `COMPRESSION_THRESHOLDS` slice | ✅ |
| 层名称常量 | `LAYER_*` 字符串 | `LAYER_*` 常量 | ✅ |
| 配置常量 | 4 个常量 | 4 个常量 (值一致) | ✅ |

### 1.2 五层算法对比

| 层 | 阈值 | Python | Rust | 一致性 |
|----|------|--------|------|--------|
| L1: Budget Reduction | 60% | `_budget_reduction()` | `budget_reduction()` | ✅ 一致 |
| L2: Snip | 70% | `_snip(keep_recent=8)` | `snip(keep_recent=8)` | ✅ 一致 |
| L3: Microcompact | 80% | `_microcompact(keep_recent=5)` | `microcompact(keep_recent=5)` | ✅ 一致 |
| L4: Context Collapse | 90% | `_context_collapse()` | `context_collapse()` | ✅ 一致 |
| L5: Auto-Compact | 95% | `_auto_compact(llm_client=...)` | `auto_compact(keep_recent, target_tokens)` | ⚠️ 部分差异 |
| Hard Limit | 100% | `_hard_limit_trim()` | ❌ 未实现 | ❌ 缺失 |

### 1.3 关键差异

#### ❌ 差异 1: Hard Limit Trim (100% 安全网) 缺失

**Python** (v1.46.24 新增):
```python
# graduated_compression.py:155-167
current_ratio = _calculate_usage_ratio(current_history, max_tokens)
if current_ratio > 1.0:
    new_history = _hard_limit_trim(current_history, max_tokens)
    new_ratio = _calculate_usage_ratio(new_history, max_tokens)
    if new_ratio < current_ratio:
        current_history = new_history
        current_ratio = new_ratio
        last_layer = "hard_limit"
```

**Rust**: `graduated_compress()` 在 Layer 5 后直接返回，没有 100% 安全网。

**影响**: 当所有压缩层都不够时（如单条超大消息），Rust 不会做紧急截断，可能导致上下文溢出。

#### ⚠️ 差异 2: Layer 5 LLM 语义摘要缺失

**Python**:
```python
def _auto_compact(history, keep_recent=4, llm_client=None, target_tokens=2000):
    if llm_client is not None:
        from helen.runtime.llm_summarizer import LLMSummarizer
        summarizer = LLMSummarizer(llm_client)
        summary_text = summarizer.summarize(old_msgs, target_tokens=target_tokens)
        # ... LLM-generated semantic summary
    # Fallback: structural summarization
```

**Rust**:
```rust
fn auto_compact(history: &[Message], keep_recent: usize, target_tokens: usize) -> Vec<Message> {
    // No LLM client in the base port — structural fallback only
    structural_auto_compact(&system_msgs, &pinned_old, old_msgs, recent_msgs)
}
```

**影响**: Rust 的 Layer 5 只有结构化摘要（文件引用、工具统计），没有 LLM 语义摘要能力。

#### ❌ 差异 3: 压缩管道未集成到 LLM 调用链

**Python** (`llm_mixin.py:1783`):
```python
def _prepare_history_for_llm(self, system_prompt, current_prompt):
    history_for_compression = self._history
    agent_ctx = getattr(self, '_agent_context', None)
    if agent_ctx is not None:
        return agent_ctx.prepare_context(
            system_prompt=system_prompt,
            history=history_for_compression,
            max_tokens=max_tokens,
            current_prompt=current_prompt,
        )
    # Fallback: HistoryManager
    return self._history_manager.prepare_for_llm(...)
```

`AgentContextManager.prepare_context()` 内部调用 `graduated_compress()`。

**Rust** (`interpreter.rs`):
```rust
fn visit_llm_act(&mut self, la: &LlmAct) -> Result<Value, ExceptionValue> {
    // ...
    // 直接构建 messages，没有调用 graduated_compress
    // history 直接传给 llm_runtime.act()
}
```

`history.rs::prepare_for_llm()` 只做了 `trim_history()`（简单截断），没有调用 `graduated_compress()`。

**影响**: **这是最关键的差异** — Rust 的五层压缩管道虽然实现了，但从未在 LLM 调用时被使用。LLM 调用只做了简单的历史截断。

### 1.4 压缩管道集成路径

```
Python 调用链:
  visit_llm_act_expr()
    → _prepare_history_for_llm()
      → AgentContextManager.prepare_context()
        → graduated_compress()  ← 五层压缩在这里生效
        → HistoryManager.prepare_for_llm()

Rust 调用链:
  visit_llm_act()
    → (直接构建 messages)
      → history 直接传给 llm_runtime.act()
        ← 没有调用 graduated_compress()!
```

---

## 二、Transcript Store

### 2.1 核心数据结构对比

| 组件 | Python | Rust | 状态 |
|------|--------|------|------|
| `Message` | `history.py::Message` | `transcript.rs::Message` | ✅ |
| `BoundaryMarker` | `transcript_store.py::BoundaryMarker` | `transcript.rs::BoundaryMarker` | ✅ |
| `SessionMeta` | `transcript_store.py::SessionMeta` | `transcript.rs::SessionMeta` | ✅ |
| `Item` enum | `Message \| BoundaryMarker` | `Item::Message \| Item::Boundary` | ✅ |
| `TranscriptStore` | `transcript_store.py::TranscriptStore` | `transcript.rs::TranscriptStore` | ✅ |
| `JSONLBackend` | `transcript_store.py::JSONLBackend` | `transcript.rs::JsonlBackend` | ✅ |

### 2.2 功能对比

| 功能 | Python | Rust | 状态 |
|------|--------|------|------|
| UUID 生成 | `_generate_uuid()` (12 hex) | `generate_uuid()` (12 hex) | ✅ |
| append | ✅ | ✅ | ✅ |
| read_view | ✅ (缓存 + BoundaryMarker 重建) | ✅ (缓存 + BoundaryMarker 重建) | ✅ |
| record_compression | ✅ | ✅ | ✅ |
| UUID 索引 | `_uuid_index: dict` | `uuid_index: HashMap` | ✅ |
| LRU 缓存 | `_max_memory_items` + `_evict_old_items()` | `max_memory_items` + `evict_old_items()` | ✅ |
| BoundaryMarker 保护 | ✅ (eviction 时保护) | ✅ (eviction 时保护) | ✅ |
| query (过滤+分页) | ✅ | ✅ | ✅ |
| write_meta / read_meta | ✅ | ✅ | ✅ |
| update_pinned | ✅ | ✅ | ✅ |
| `parent_session_id` | ✅ (v1.23.7) | ✅ | ✅ |
| invocation tree 字段 | ✅ (v1.22) | ✅ | ✅ |
| visibility tracking | ✅ (v1.24) | ✅ | ✅ |
| 多模态内容 | ✅ (list[dict]) | ✅ (serde_json::Value) | ✅ |
| **MediaStorage** | ✅ (v1.17 Phase 3) | ❌ | ❌ 缺失 |
| **_resumed_message_uuids** | ✅ (v1.27) | ❌ | ❌ 缺失 |
| **persist 参数** | ✅ (v1.29) | ✅ | ✅ |

### 2.3 关键差异

#### ❌ 差异 1: MediaStorage 大媒体外部存储缺失

**Python** (`transcript_store.py:1210-1215`):
```python
# v1.17 Phase 3: Media storage for large content
self._media_storage: MediaStorage | None = None
if session_dir is not None:
    multimodal_config = get_multimodal_config()
    threshold_mb = multimodal_config.get("media_external_threshold_mb", 1.0)
    self._media_storage = MediaStorage(session_dir, threshold_mb)
```

在 `append()` 中：
```python
if self._media_storage is not None and isinstance(message.content, list):
    message.content = self._media_storage.process_content_parts(message.content)
```

**Rust**: 没有 `MediaStorage` 实现，所有媒体内容内联存储在 JSONL 中。

**影响**: 大图片/视频会导致 JSONL 文件膨胀。

#### ❌ 差异 2: 会话恢复支持缺失

**Python** (`transcript_store.py:1228-1234`):
```python
# v1.27: UUIDs of messages loaded from a previous session via
# load_from_backend (session resume). These are the "resumed history" -
# at _enter_invocation time the new invocation_id is added to each
# such message's visible_to_invocation_ids so the LLM in a new agent
# invocation can actually see the prior session's conversation.
self._resumed_message_uuids: set[str] = set()
```

**Rust**: 没有 `_resumed_message_uuids` 字段。

**影响**: 会话恢复后，旧消息可能因为 invocation_id 隔离而对新的 agent 调用不可见。

---

## 三、总结

### 3.1 一致性评分

| 模块 | 评分 | 说明 |
|------|------|------|
| 五层压缩算法 | **B** | 5 层核心算法一致，缺 Hard Limit Trim 和 LLM 语义摘要 |
| 压缩管道集成 | **D** | 函数存在但未集成到 LLM 调用链 |
| Transcript Store | **A-** | 核心功能完整，缺 MediaStorage 和会话恢复 |

### 3.2 需要修复的问题（按优先级）

| 优先级 | 问题 | 影响 |
|--------|------|------|
| **P0** | 压缩管道未集成到 LLM 调用链 | 五层压缩形同虚设，LLM 调用只做简单截断 |
| **P1** | 缺少 Hard Limit Trim (100% 安全网) | 超大消息可能导致上下文溢出 |
| **P2** | 缺少 LLM 语义摘要 (Layer 5) | 压缩质量低于 Python |
| **P3** | 缺少 MediaStorage | 大媒体导致 JSONL 膨胀 |
| **P3** | 缺少会话恢复支持 | 恢复后旧消息不可见 |

### 3.3 修复建议

#### P0: 集成压缩管道

在 `interpreter.rs::visit_llm_act()` 中，在构建 messages 之前调用 `graduated_compress()`：

```rust
// 在 visit_llm_act() 中
let usage_ratio = calculate_usage_ratio(&history, max_tokens);
let (compressed, _layer) = graduated_compress(&history, usage_ratio, Some(max_tokens));
// 使用 compressed 而不是 history
```

或者更完整地实现 `AgentContextManager`（对应 Python 的 `agent_context.py`）。

#### P1: 添加 Hard Limit Trim

在 `compression.rs::graduated_compress()` 末尾添加：

```rust
// Hard limit enforcement at 100%
let current_ratio = calculate_usage_ratio(&current_history, max_tokens);
if current_ratio > 1.0 {
    let new_history = hard_limit_trim(&current_history, max_tokens);
    let new_ratio = calculate_usage_ratio(&new_history, max_tokens);
    if new_ratio < current_ratio {
        current_history = new_history;
        last_layer = "hard_limit".to_string();
    }
}
```

#### P2: LLM 语义摘要

需要实现 `LlmSummarizer`（类似 Python 的 `llm_summarizer.py`），并在 `auto_compact()` 中集成。

---

## 四、测试覆盖

### 压缩测试

| 测试 | Python | Rust |
|------|--------|------|
| 阈值以下不压缩 | ✅ | ✅ `graduated_compress_no_op_below_threshold` |
| L1 Budget Reduction | ✅ | ✅ `graduated_compress_l1_at_high_usage` |
| L2 Snip | ✅ | ✅ `snip_drops_stale_turns` |
| L3 Microcompact | ✅ | ✅ `microcompact_clears_old_tool_results` |
| L4 Context Collapse | ✅ | ✅ `context_collapse_produces_timeline` |
| L5 Auto-Compact | ✅ | ✅ (structural only) |
| Hard Limit Trim | ✅ | ❌ |
| 集成测试 (LLM 调用链) | ✅ | ❌ |

### Transcript Store 测试

| 测试 | Python | Rust |
|------|--------|------|
| append/read_view | ✅ | ✅ |
| record_compression | ✅ | ✅ |
| UUID 索引 | ✅ | ✅ |
| LRU 缓存 | ✅ | ✅ |
| query 过滤 | ✅ | ✅ |
| meta 持久化 | ✅ | ✅ |
| update_pinned | ✅ | ✅ |
| JSONL 格式兼容 | ✅ | ✅ `transcript_interop.rs` |
