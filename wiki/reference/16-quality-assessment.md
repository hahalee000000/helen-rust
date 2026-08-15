# Tutorial 16: Quality Assessment (7-Dimension Framework)

> Helen has a built-in 7-dimension quality assessment framework for automated quality analysis

---

## Quick Start

### CLI Commands

```bash
# Basic assessment
helen quality my_program.helen

# JSON output
helen quality my_program.helen --json

# Set threshold (CI integration)
helen quality my_program.helen --threshold 7.5

# Single dimension assessment
helen quality my_program.helen --dimension security
```

### Using in Helen Code

```helen
import std.core.*
import std.quality.*
main {
    let source = read_file("my_program.helen")

    // Get code metrics
    let metrics = analyze_code(source, "my_program.helen")
    print("Functions: " + str(metrics["function_count"]))
    print("Complexity: " + str(metrics["avg_complexity"]))

    // Security check
    let issues = check_security(source)
    print("Security issues: " + str(len(issues)))

    // Quality scoring
    let scores = quality_score(source, "my_program.helen")
    print("Total score: " + str(scores["total"]))
    print("Grade: " + scores["grade"])

    // Full report
    let report = quality_report(source, "my_program.helen")
    print(report)
}
```

## 7 Dimensions

| Dimension | Weight | What It Evaluates |
|-----------|:------:|-------------------|
| **Architecture** | 20% | Function length, complexity, nesting depth, parameter count |
| **Code Quality** | 15% | Comment ratio, average function length, average complexity |
| **Security** | 20% | Dangerous pattern detection (eval, shell=True, unvalidated input, etc.) |
| **Test Coverage** | 15% | Test file existence, test-to-code ratio |
| **Documentation** | 10% | Function docstring coverage |
| **Maintainability** | 10% | Count of long functions, count of high-complexity functions |
| **Engineering** | 10% | Naming conventions, file size |

## Grading Scale

| Grade | Score Range | Meaning |
|:-----:|:-----------:|---------|
| S | 9.0-10.0 | Production-ready, exemplary |
| A | 7.5-8.9 | Good, minor improvements needed |
| B | 6.0-7.4 | Acceptable, improvements needed |
| C | 4.0-5.9 | Below standard |
| D | 0.0-3.9 | Unacceptable |

## Example Output

```
============================================================
  HELEN QUALITY REPORT
============================================================
  File: calculator.helen

  Code Metrics:
    Total lines: 150
    Code lines: 120
    Comment lines: 25 (17%)
    Functions: 8
    Agents: 1
    Avg function length: 12.5 lines
    Max function length: 35 lines
    Avg complexity: 2.3
    Max complexity: 6

  Quality Scores (0-10):
    Architecture:      9.50 (20%)
    Code Quality:      8.00 (15%)
    Security:          10.00 (20%)
    Test Coverage:     6.00 (15%)
    Documentation:     7.50 (10%)
    Maintainability:   9.00 (10%)
    Engineering:       8.50 (10%)
    ─────────────────────────────
    TOTAL:             8.48
    GRADE:             A

  Recommendations:
    • Add test file for better coverage
    • Add docstrings to 2 undocumented functions

============================================================
```

## Security Checks

The following dangerous patterns are automatically detected:

| Pattern | Severity | Description |
|---------|:--------:|-------------|
| `eval()` | HIGH | Can execute arbitrary code |
| `exec()` | HIGH | Can execute arbitrary code |
| `shell=true` | HIGH | Command injection risk |
| `import os` | MEDIUM | System resource access |
| `import subprocess` | MEDIUM | Command execution |
| `open(..., "w")` | MEDIUM | File writing without validation |
| `input()` | LOW | User input needs validation |

## CI Integration

```bash
# Use threshold in CI
helen quality src/*.helen --threshold 7.0 --json > quality.json

# Check if threshold is met
if [ $? -ne 0 ]; then
    echo "Quality threshold not met!"
    exit 1
fi
```

## API Reference

### `analyze_code(source, filename?)`

Analyzes code metrics, returns:
- `total_lines`, `code_lines`, `comment_lines`, `blank_lines`
- `comment_ratio`, `function_count`, `agent_count`
- `avg_function_length`, `max_function_length`
- `avg_complexity`, `max_complexity`
- `functions[]` — detailed information for each function

### `check_security(source)`

Checks for security issues, returns a list of issues:
- `line` — line number
- `severity` — "high" / "medium" / "low"
- `pattern` — matched pattern
- `message` — issue description

### `quality_score(source, file_path?)`

Computes 7-dimension scores, returns:
- `architecture`, `code_quality`, `security`
- `test_coverage`, `documentation`, `maintainability`, `engineering`
- `total` — weighted total score
- `grade` — letter grade

### `quality_report(source, filename?)`

Generates a formatted full report string.

## Exercises

1. Run quality assessment on your Helen program
2. Improve code based on recommendations to raise the score
3. Set a quality threshold in CI
4. Use `--dimension security` to focus on security improvements
5. Compare scores before and after improvements

---

> **Related Documentation**: [[toolchain/quality|Quality Assessment Tool Reference]] | [[reference/12-testing|Testing Framework and TDD]]
