# 版本历史 — helen-rust

> helen-rust v0.1.0 | 从 Python Helen v1.45.0 移植的 Rust 重实现。
> 完整演进见 `wiki/log.md`；里程碑规划见 `wiki/plan/README.md`。

---

## M0: 工作区与差分测试框架

- 10-crate workspace、CI、差分对撞 harness（`reference.py`）。

## M1: 词法 + 语法前端

- **1.1–1.2** tokens + lexer：88 个 `TokenType`、`LiteralValue`(BigInt)、
  99 项双语关键字映射（48 英 + 51 中）；36/36 token-stream 差分。
- **1.3–1.5** AST + AstPrinter（S-expression 字节一致）+ Pratt 解析器；
  47/47 corpus 文件 parse 差分通过。

## M2: 语义分析

- `helen-semantic`：types/symbols/type_utils/diagnostics/analyzer。
- **修正**：catch 白名单 15 项（非计划中的 11 项）；`catch X err` 语法；
  语义差分（exit 2 + E-code 顺序）。
- **Phase B**：`TypeRefKind`（Simple/Optional/Union）修复注释折叠 bug。

## M3: 解释器核心

- value model（BigInt 整数、字节字符串）、environment、exceptions、
  closures、builtins、CLI `--run`；3.4–3.7 分提交。

## M4: 标准库

- stdlib registry：22 模块、378 builtins + 别名；tuple 值；
  JSON/CSV Python 格式一致；7/7 run-diff。

## M5: LLM 运行时

- providers、HTTP+SSE、config、prompt builder、token 计数、
  tool dispatch + agent 设置透传。

## M6: Agent 运行时

- 11 工具注册表（schema 字节一致）、fuzzy_match 9 策略、
  skills 系统（3 层查找）、llm act tool loop、agent 作用域隔离。

## M7: 并发

- Channel（双向、close/cancel）、spawn OS 线程 + deep-owned snapshot、
  SharedStore runtime、ReadOnlyView 守卫、mailbox_select。

## M8: 上下文管理

- transcript/session/history/compression/memory/observability +
  SQLite 后端（与 Python 字节兼容）。

## M9: MCP

- JSON-RPC over stdio（initialize/tools-list/tools-call/shutdown）、
  server 管理 + 注册表、MCPError 层级、fixture mock server。

## M10: Python FFI

- `Value::Native`、helen-ffi crate（pyo3）、import-hook 回退
  （`import "math"` → module calls）、custom-provider 加载器。

## M11: Python Bridge

- maturin cdylib `helen_rust`（load_agent/load_function/parse_check/
  eval_helen/describe_file）、PyAgent/PyFunction 包装、纯 Python shim、
  13 pytest DoD 测试 + 9 Rust 集成测试。

## M12: CLI / REPL / LSP

- 完整 cli/* + lsp/server.py 移植。

## M13: 一致性测试与基准

- Tier A/B/C 差分：language 100/100、agent 170/172、cli+ffi 128+1skip、
  semantic 21/21、error-diff 70/70；display corpus 10/10；
  基准 Rust 15–50× 更快；覆盖率 68.82%。

## M14: 打包、文档、验收

- install.sh、check-parity.sh（10/10 门禁）、sync-corpus.sh、release.yml、
  LICENSE(MIT)、docs/ docgen 一致（md 字节一致）、transcript JSONL
  互操作测试（D8 双向）、STATUS.md 交接、MAINTENANCE.md 维护指南。
- 验收 D1–D8 全部通过。
