<!-- helen-rust edition: `helen quality` ported in M12; 7-dimension scoring parity verified. -->

# 质量评估

Helen 内置 7 维质量评估框架，可对 Helen 程序进行自动化质量分析。

## 概述

| 维度 | 权重 | 评估内容 |
|------|:----:|---------|
| 架构设计 | 20% | 函数长度、复杂度、嵌套深度、参数数量 |
| 代码质量 | 15% | 注释率、函数平均长度、平均复杂度 |
| 安全性 | 20% | 危险模式检测 |
| 测试覆盖 | 15% | 测试文件存在性 |
| 文档 | 10% | 函数 docstring 覆盖率 |
| 可维护性 | 10% | 长函数、高复杂度函数数量 |
| 工程规范 | 10% | 命名规范、文件大小 |

## CLI 命令

```bash
helen quality <file> [options]

Options:
  --json              JSON 输出
  --dimension <name>  单维度评估
  --threshold <n>     阈值检查
```

## 评分等级

| 等级 | 范围 | 含义 |
|:----:|:----:|------|
| S | 9.0-10.0 | 生产就绪 |
| A | 7.5-8.9 | 良好 |
| B | 6.0-7.4 | 可接受 |
| C | 4.0-5.9 | 低于标准 |
| D | 0.0-3.9 | 不可接受 |

## stdlib API

### `analyze_code(source, filename?)`

返回代码指标：
- `total_lines`, `code_lines`, `comment_lines`
- `function_count`, `agent_count`
- `avg_complexity`, `max_complexity`
- `functions[]` — 函数详情

### `check_security(source)`

返回安全问题列表：
- `line`, `severity`, `pattern`, `message`

### `quality_score(source, file_path?)`

返回 7 维评分：
- `architecture`, `code_quality`, `security`
- `test_coverage`, `documentation`, `maintainability`, `engineering`
- `total`, `grade`

### `quality_report(source, filename?)`

返回格式化的完整报告。

## 安全检查

检测的危险模式：

| 模式 | 严重度 |
|------|:------:|
| `eval()` | HIGH |
| `exec()` | HIGH |
| `shell=true` | HIGH |
| `import os` | MEDIUM |
| `import subprocess` | MEDIUM |
| `input()` | LOW |

## 示例

```helen
let source = read_file("my_program.helen")

// 获取指标
let metrics = analyze_code(source)
print("Functions: " + str(metrics["function_count"]))

// 安全检查
let issues = check_security(source)
print("Issues: " + str(len(issues)))

// 质量评分
let scores = quality_score(source, "my_program.helen")
print("Grade: " + scores["grade"])

// 完整报告
print(quality_report(source, "my_program.helen"))
```

## 相关文档

- [测试框架](testing.md)
- [CLI 工具](cli.md)
