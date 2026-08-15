# 异常层次 — helen-rust

> Rust `ExceptionValue` (`helen-interpreter/src/exceptions.rs`).
> Mirrors `helen/interpreter/exceptions.py`.

---

## 异常继承树

```
Exception
├── HelenRuntimeError           # 运行时错误基类
│   ├── LLMError                 # LLM 相关错误基类
│   │   ├── TimeoutError          # LLM 超时
│   │   ├── ModelError            # LLM 模型错误
│   │   └── AgentError            # Agent 调用失败 (携带 agent_name/agent_args/cause)
│   ├── ToolError                # 工具调用错误
│   ├── RuntimeError             # 通用运行时错误
│   └── AssertionError           # assert 语句失败
└── AnyError                     # 异常基类（catch-all）
```

## 控制流 Sentinel

`Flow::Break` / `Flow::Continue` / `Flow::Return(value)` — 控制流信号通过
枚举传播，不是错误（Python 用异常类实现同样语义）。

## `error_matches`（层级匹配，非精确名字）

Rust 的 `error_matches(e, name)` 按异常**层级**匹配，而非精确名字 —
与 Python 参考一致。这也是对计划 C1 的修正：catch 白名单为 **15 项**
（接受 `ValueError`/`TypeError`/`KeyError`/`IndexError`/
`FileNotFoundError`/`PermissionError`，拒绝 `PromptTooLongError`/
`LLMOutputContractError`），见 `compiler/semantic.md`。

## `__str__` 渲染

- 通用运行时错误：`RuntimeError:{loc} msg`（冒号 + 位置 + 消息）。
- 类特定覆盖（AgentError 携带 agent 名/参数，ToolError 携带工具名）。
- 未捕获异常在 CLI `--run` 中渲染为 `RuntimeError: {e}`，与参考一致。
