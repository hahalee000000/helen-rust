---
name: helen-quality
description: "Helen 7-Dimension Quality Assessment Guide — Code Analysis, Security Scoring, CI Integration"
version: 1.0.0
author: Helen Team
license: MIT
metadata:
  hermes:
    tags: [helen, quality, assessment, security, ci]
---
<!-- helen-rust edition: `helen quality` ported in M12; 7-dimension scoring parity verified. -->


# Helen Quality Assessment

## Overview

Helen includes a built-in 7-dimension quality assessment framework that automates quality analysis of Helen programs.

## Quick Start

### CLI

```bash
helen quality my_program.helen
helen quality my_program.helen --json
helen quality my_program.helen --threshold 7.5
helen quality my_program.helen --dimension security
```

### In Code

```helen
import std.core.*
import std.quality.*
let source = read_file("my_program.helen")
let scores = quality_score(source, "my_program.helen")
print("Grade: " + scores["grade"])
```

## 7 Dimensions

| Dimension | Weight | What It Evaluates |
|------|:----:|---------|
| Architecture | 20% | Function length, complexity, nesting, parameters |
| Code Quality | 15% | Comment ratio, function length, complexity |
| Security | 20% | Dangerous pattern detection |
| Test Coverage | 15% | Test file existence |
| Documentation | 10% | Docstring coverage |
| Maintainability | 10% | Long functions, high-complexity functions |
| Engineering | 10% | Naming, file size |

## Grade Levels

| Grade | Range |
|:----:|:----:|
| S | 9.0-10.0 |
| A | 7.5-8.9 |
| B | 6.0-7.4 |
| C | 4.0-5.9 |
| D | 0.0-3.9 |

## API

### `analyze_code(source, filename?)`

Code metrics:
- `total_lines`, `code_lines`, `comment_lines`
- `function_count`, `agent_count`
- `avg_complexity`, `max_complexity`
- `functions[]`

### `check_security(source)`

Security issues:
- `line`, `severity`, `pattern`, `message`

### `quality_score(source, file_path?)`

7-dimension scores:
- `architecture`, `code_quality`, `security`
- `test_coverage`, `documentation`, `maintainability`, `engineering`
- `total`, `grade`

### `quality_report(source, filename?)`

Formatted report string.

## Security Checks

| Pattern | Severity |
|------|:------:|
| `eval()` | HIGH |
| `exec()` | HIGH |
| `shell=true` | HIGH |
| `import os` | MEDIUM |
| `import subprocess` | MEDIUM |
| `input()` | LOW |

## Test Coverage Scoring Details

The test coverage dimension (weight 15%) uses file-location heuristics, scored by the highest matching tier:

| Strategy | Score | Condition |
|------|:----:|------|
| `// @test-location:` annotation | **8.0** | Source file contains annotation pointing to existing test file |
| Sibling test file | **8.0** | `<name>_test.helen` or `test_<name>.helen` exists |
| Parent `tests/` match | **7.0** | `*.py` in parent `tests/` directory with filename containing source file stem |
| Sibling `tests/` directory | **6.0** | `tests/` directory next to source file contains any test files |
| No tests | **2.0** | No tests found |

### `// @test-location:` Annotation

Integration tests for agent-heavy programs are typically placed in a separate `tests/` directory, where filenames don't necessarily match the source file stem — these tend to fall into the 6.0 tier. Using an annotation explicitly declares the test location, scoring 8.0 directly:

```helen
// @test-location: tests/integration/test_programmer.py

agent Programmer {
    description "Code writing assistant"
    main {
        llm act "Write code"
    }
}
```

The annotation path supports both absolute and relative paths (relative to the source file). The file must actually exist.

## Engineering Dimension — Naming Convention

The engineering dimension (weight 10%) penalizes **shadowing builtins**. Helen has 364 stdlib functions and 99 keywords — never use them as variable names. Use **suffix-qualified names**:

| ❌ Penalized | ✅ Preferred |
|-------------|-------------|
| `let map = {}` | `let config_map = {}` |
| `let list = []` | `let item_list = []` |
| `let count = 0` | `let total_count = 0` |
| `let str = "x"` | `let raw_str = "x"` |
| `let result = ...` | `let find_result = ...` |

Common suffixes: `_map`, `_list`, `_count`, `_text`, `_result`, `_config`, `_data`, `_file`, `_path`, `_name`, `_id`, `_value`

Chinese identifiers (`结果字典`, `配置列表`, `扫描数量`) naturally avoid English builtin collisions.

See [helen-syntax: Variable Naming Convention](../helen-syntax/SKILL.md#variable-naming-convention--avoid-shadowing-builtins) for full details.

## CI Integration

```bash
helen quality src/*.helen --threshold 7.0 --json > quality.json
```

## Related Documentation

- [Wiki](../../../wiki/helen/toolchain/quality.md)
- [Example](../../examples/quality_example.helen)
