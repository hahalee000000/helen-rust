# Helen-Rust Agent 延迟问题分析与修复建议

## 1. 🔴 `import "ui.stream_emitter"` 每次回调都失败（最大影响）

Helen agent 代码在**每个流式回调**中都执行 `import "ui.stream_emitter"`：

- `_streaming_chunks_cb_with_channel` — 每个 LLM chunk 触发一次
- `_actor_tool_end_cb` — 每个工具调用结束触发
- `_emit_actor_stream_event` — 每次事件发射
- `_actor_chunks_cb` — 每个 chunk

但 `ui/stream_emitter.py` 是 **Python 模块**，Rust runtime 没有编译 `python-ffi` feature（`helen-agent/Cargo.toml` 中未启用），所以：

```
import "ui.stream_emitter" → ResolvedImport::Python → 无 python_import_hook → RuntimeError → try/catch {} 静默吞掉
```

**每个 LLM chunk 都要走一遍**：import resolver → 检查 python hook → None → 创建 RuntimeError 异常 → throw → catch。一次响应几百个 chunk，累积的异常创建/抛出/捕获开销很可观。

同样，`_actor_emit_statusline()` 中的 `import "ui.status_emitter"` 也面临相同问题。

**位置**: `crates/helen-agent/agent/chat_session_actor.helen:260,303,322,445,616,662,668,685,723`

## 2. 🟡 WebSocket handler 的 50ms 轮询

```rust
// actor_ws.rs:310
tokio::select! {
    chunk_result = stream_rx.recv() => { /* 转发 chunk */ }
    _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {
        if let Some(output) = bridge_ref.receive_output().await {
            /* 检测完成 */
        }
    }
}
```

Complete 响应**只在 sleep 分支中检测**（`actor_ws.rs:311`），最坏情况下增加 50ms 延迟。用户发命令后，即使 agent 已秒级返回，前端还要等这个轮询周期。

## 3. 🟡 Hint 仅在工具边界注入

`hint_queue_pop_all()` 只在 `_actor_tool_end_cb` 中调用（`chat_session_actor.helen:332`）。当 LLM 在做纯文本生成（不调用工具）时，hint 队列无人消费，直到下一个 tool call 结束才被处理。

如果 LLM 做一段长文本输出（比如写一个长文件），期间用户追加的 hint 会一直排队等待。

## 4. 🟢 Cancel 机制有双层，但 Helen 层失效

Rust interpreter 层面有原生的 `agent_cancel_event` 检查（`interpreter.rs:4163-4168`），每个 SSE event 都会检查 `AtomicBool`。`bridge.cancel()` 设置的是同一个 `Arc<AtomicBool>`，所以**底层 cancel 是工作的**。

但 Helen 层的 cancel 检查（`import "ui.stream_emitter" as _CancelCheck`）因 Python 模块不可用而完全失效。这意味着：
- Rust 层 cancel → interpreter 停止 SSE streaming ✅
- 但 Helen 层的 `cancel_all_llm_calls()` 和 `clear_cancel()` 不会被 Helen 回调调用
- `clear_cancel()` 依赖 `_handle_actor_user_input_stream` 的 catch 块（同样 import 失败）来清理

## 修复建议

| 问题 | 修复 |
|------|------|
| `import "ui.stream_emitter"` 失败开销 | 将 cancel/hint/stream 功能实现为 Rust native module（在 `helen-interpreter/src/stdlib.rs` 注册），或创建 `.helen` 文件提供等价的 native function wrapper |
| 50ms 轮询检测 complete | 用 `tokio::sync::Notify` 或 `tokio::sync::mpsc` 替代 `sleep` 轮询，complete 到达时立即唤醒 |
| Hint 仅工具边界注入 | 在 `on_chunk` 回调中也检查并注入 hint，或将 hint 注入改为独立线程/定时器 |
| 每次回调重复 import | 将 import 移到 agent 的 `main {}` 顶层（只执行一次），后续直接引用 |
