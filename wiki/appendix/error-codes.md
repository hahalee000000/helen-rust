# 错误码参考 — helen-rust

> `ErrorCode` enum in `crates/helen-core/src/errors.rs`.
> Mirrors the Python `helen.core.errors.ErrorCode` enum 1:1 (values 300–357).
> Differential gate: exit-code parity (2) + E-code emission order.

---

## 词法错误 (E0300-E0309)

| 代码 | 名称 | 触发条件 |
|---|---|---|
| E0300 | ScannerError | 非法字符 |
| E0301 | ParserError | 通用语法错误 |
| E0302 | UnexpectedToken | 意外的 Token |
| E0303 | MissingToken | 缺少期望的 Token |
| E0304 | InvalidLiteral | 无效的数字字面量 |
| E0305 | InvalidEscape | 无效的转义序列 |
| E0306 | UnterminatedString | 字符串未闭合 |
| E0307 | InvalidIdentifier | 无效标识符 |
| E0308 | DeprecatedSyntax | 已废弃语法 |
| E0309 | ReservedKeyword | 使用保留关键字作标识符 |

## 语法错误 (E0310-E0320)

| 代码 | 名称 | 触发条件 |
|---|---|---|
| E0310 | TypeMismatch | 类型不匹配 |
| E0311 | UndefinedVariable | 未定义变量 |
| E0312 | UndefinedFunction | 未定义函数 |
| E0313 | DuplicateDeclaration | 重复声明 |
| E0314 | MissingReturn | 缺少返回语句 |
| E0315 | InvalidBreak | 无效的 break |
| E0316 | InvalidContinue | 无效的 continue |
| E0317 | MissingDefaultCase | match 缺少 default |
| E0318 | AsyncOnNonCall | (保留) async/await 已删除 |
| E0319 | InvalidAgentParam | 无效 Agent 参数 |
| E0320 | UnterminatedBlock | 块未闭合 |

## 语义错误 (E0330-E0357)

| 代码 | 名称 | 触发条件 |
|---|---|---|
| E0330 | SemanticError | 通用语义错误 |
| E0331 | SemanticTypeError | 语义类型错误 |
| E0332 | UndeclaredVariable | 未声明变量 |
| E0333 | DuplicateSymbol | 重复符号 |
| E0334 | AgentRuntimeError | Agent 运行时错误 |
| E0335 | DuplicateAgentName | 重复 Agent 名 |
| E0336 | DuplicateParam | 重复参数名 |
| E0337 | MissingPrompt | Agent 缺少 prompt |
| E0338 | BreakOutsideLoop | break 在循环外 |
| E0339 | ContinueOutsideLoop | continue 在循环外 |
| E0340 | ReturnOutsideFunction | return 在函数外 |
| E0341 | ImportNotFound | 导入文件不存在 |
| E0342 | InvalidCatchType | 无效 catch 类型 |
| E0343 | CatchAllNotLast | catch-all 不在最后 |
| E0344 | LlmIfNoDefault | llm if 缺少 default |
| E0345 | MatchNoDefault | match 缺少 default |
| E0346 | ConstAssignment | 常量赋值 |
| E0347 | AgentParamMismatch | Agent 参数不匹配 |
| E0348 | InvalidAgentName | 无效 Agent 名 |
| E0349 | MissingDefaultBranch | 缺少默认分支 |
| E0350 | ScopeViolation | 跨 Agent 变量引用 |
| E0351 | ImportError | 导入错误 (shared-store 相关, 参考兼容) |
| E0352 | RuntimeError | 运行时错误 (保留位) |
| E0353 | InvalidToolsDeclaration | 无效 tools 声明 |
| E0354 | BuiltinShadowed | 内置函数被遮蔽 |
| E0355 | TopLevelStatement | 顶层语句 (agent/import 上下文外) |
| E0356 | UndeclaredAgentFunction | 未声明的 agent 函数 |
| E0357 | AgentFunctionArgMismatch | agent 函数参数不匹配 |

## 校验

```bash
# Tier C semantic differential (21 tests) vs reference
bash scripts/diff-semantic.sh
# Error-code parity CSV (70/70)
python3 scripts/gen-error-diff.py --all
```
