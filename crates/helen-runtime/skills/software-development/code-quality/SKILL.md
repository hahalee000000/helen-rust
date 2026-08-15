---
name: code-quality
description: "Code quality assessment, pre-commit verification, and parallel cleanup. Covers 7-dimension scoring framework, security/quality gates with independent reviewer, and 3-agent simplify workflow."
version: 1.3.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [code-quality, code-review, security, verification, scoring, cleanup, refactor]
---

# Code Quality — Assessment, Verification & Cleanup

Umbrella skill for code quality workflows: scoring, pre-commit gates, and parallel cleanup.

---

## 1. Quality Assessment (7-Dimension Framework)

Score each dimension 0-10, apply weights, compute weighted sum.

### Default Dimensions (Generic Software)

| Dimension | Weight | Focus |
|-----------|--------|-------|
| **Test Coverage** | 25% | Unit tests, boundary cases, exception handling |
| **Completeness** | 15% | Functions implemented, edge cases handled |
| **Accuracy** | 15% | Logic correctness, input validation, error handling |
| **Performance** | 10% | Algorithm efficiency, memory, complexity |
| **Security** | 15% | Input sanitization, injection prevention, secrets |
| **Maintainability** | 15% | Naming, docstrings, complexity, modularity |
| **Internationalization** | 5% | i18n readiness, Unicode, locale awareness |

### Alternative Dimensions (Language/Compiler/Framework Projects)

For DSLs, compilers, interpreters, or framework projects, use this dimension set instead:

| Dimension | Weight | Focus |
|-----------|--------|-------|
| **Architecture Design** | 20% | Layer separation, design patterns, modularity, dependency direction |
| **Code Quality** | 15% | Type annotations, docstrings, complexity, dead code, naming |
| **Security** | 20% | Sandboxing, input validation, SSRF/injection prevention, secrets |
| **Test Coverage** | 15% | Test count, coverage ratio, security tests, integration tests |
| **Documentation** | 10% | README, tutorials, API docs, docstring consistency, language uniformity |
| **Maintainability** | 10% | Code duplication, dead code, hardcoded values, circular dependencies |
| **Engineering Standards** | 10% | Lint compliance (flake8=0), CI/CD, type hints, pre-commit hooks |

**Choose the dimension set based on project type.** The alternative set weights Security and Architecture higher (critical for language runtimes) and drops Internationalization (rarely relevant for DSLs).

**Formula:** `total = Σ(score_i × weight_i)`. Max = 10.0.

### Grading Scale

| Grade | Range | Meaning |
|-------|-------|---------|
| S | 9.0-10.0 | Production-ready, exemplary |
| A | 7.5-8.9 | Good, minor improvements |
| B | 6.0-7.4 | Acceptable, needs work |
| C | 4.0-5.9 | Below standard |
| D | 0.0-3.9 | Unacceptable |

### Assessment Process

1. Parse code with `ast.parse()` for functions, classes, docstrings
2. Evaluate each dimension independently
3. Score 0-10 per dimension based on evidence
4. Apply weights, compute total
5. Generate one actionable recommendation per dimension <8

### Key Technical Notes

- **Docstring detection**: use `ast.get_docstring(node)`, NOT regex
- **Separate code vs test criteria**: different rubrics for each
- **Security checks are concrete**: look for actual implementations, not comments
- **No test files → Test Coverage is automatically 0**

### Complexity Refactoring Patterns

When CC > 15:
- **Table-driven dispatch** for large if/elif chains
- **Method extraction** for compound logic
- **Boolean accumulator** for multi-path detection

---

## 2. Pre-Commit Verification Gate

Automated pipeline before code lands. No agent should verify its own work.

### Steps

1. **Get the diff**: `git diff --cached` (or `git diff HEAD~1 HEAD`)
2. **Static security scan**: grep for secrets, shell injection, eval/exec, pickle, SQL injection
3. **Baseline tests/linting**: detect project language, run tools, compare against baseline
4. **Self-review checklist**: secrets, validation, parameterized queries, debug prints
5. **Independent reviewer subagent**: delegate_task with ONLY the diff — fresh context
6. **Evaluate**: all passed → commit; any failures → auto-fix loop
7. **Auto-fix loop** (max 2 cycles): spawn THIRD agent to fix only reported issues
8. **Commit**: `git add -A && git commit -m "[verified] <description>"`

### Reviewer Prompt Rules

- security_concerns non-empty → passed must be false
- logic_errors non-empty → passed must be false
- Cannot parse diff → passed must be false
- Only passed=true when BOTH lists are empty

### When to Skip

- Documentation-only changes
- Pure config tweaks
- User says "skip verification"

---

## 3. Parallel Cleanup (Simplify)

Three focused reviewers running in parallel via `delegate_task` batch mode.

### The Three Reviewers

1. **Code Reuse**: duplicates existing functionality? Search for existing utils.
2. **Code Quality**: redundant state, parameter sprawl, copy-paste, leaky abstractions.
3. **Efficiency**: unnecessary work, missed concurrency, hot-path bloat, TOCTOU, memory leaks.

### Process

1. **Capture diff**: `git diff` (default) or scoped variant
2. **Launch 3 reviewers in parallel**: give EACH the WHOLE diff + repo path
3. **Aggregate**: merge findings, discard false positives, resolve conflicts
4. **Apply fixes**: `patch`/`write_file` (unless dry run)
5. **Verify**: run targeted tests + linter
6. **Summarize**: what changed, what was skipped and why

### Conflict Resolution Order

**correctness > user's stated focus > readability/reuse > micro-perf**

### Pitfalls

- Don't fan out wider than 3 reviewers
- Give WHOLE diff to each (cross-file issues hide in gaps)
- Require `file:line` evidence — drop findings without it
- Apply ≠ rewrite (scoped to what diff touched)
- Large diffs: scope down before delegating

---

## 4. Security Hardening for Python Runtimes

When auditing a Python runtime/DSL/agent framework for security:

### Pattern: Central Security Module + Entry-Point Integration

1. **Create `security.py`** with validation functions:
   - `validate_path(path, base_dir=)` — realpath + blocked dirs (/proc, /sys, /etc/shadow)
   - `validate_url(url)` — scheme whitelist + SSRF check (resolve hostname, block private IPs)
   - `validate_command(cmd)` — block dangerous patterns (rm -rf /, fork bombs)
   - `validate_pid(pid)` — block PID 0/1/self
   - `validate_kill_signal(sig)` — whitelist safe signals only
   - `safe_env_list()` — mask PASSWORD/SECRET/TOKEN/API_KEY values
   - `SecurityError` exception class

2. **Integrate at every entry point** (not just one):
   - File tools: `validate_path()` before read/write/patch
   - Network tools: `validate_url()` before fetch/request/download
   - Shell tools: `validate_command()` + `shell=False` default + `shlex.split()`
   - Process tools: `validate_pid()` + `validate_kill_signal()` before kill
   - Env tools: `safe_env_list()` instead of raw `os.environ`
   - Import resolver: `realpath()` + base_dir containment (no absolute path bypass)

3. **Key defaults**:
   - `shell=False` ALWAYS as default, explicit opt-in for shell=True
   - Path validation uses `os.path.realpath()` to resolve symlinks
   - URL validation resolves hostname via `socket.getaddrinfo()` and checks against private IP ranges
   - Download functions enforce max size with running counter

### Pitfalls
- `os.path.abspath()` does NOT resolve symlinks — use `os.path.realpath()`
- Allowing absolute paths as a "REPL convenience" bypasses all path safety
- `shell=True` with string commands = command injection; always `shlex.split()` when shell=False
- `random` module is NOT cryptographically secure — use `secrets` for security contexts
- XML parsing with `ET.fromstring()` default parser may process external entities (XXE)
- User-supplied regex patterns need timeout to prevent ReDoS

---

## 5. Batch Improvement Workflow (P0/P1/P2)

After a quality assessment, execute improvements in priority tiers:

### Priority Tiers
- **P0 (Immediate)**: Security vulnerabilities, undefined names (F821), data loss risks
- **P1 (This iteration)**: Dead code, code duplication, lint warnings, long functions
- **P2 (Continuous)**: CI/CD, security tests, doc consistency, config extraction

### Execution Pattern
1. **Read all target files** first (parallel delegate_task for large codebases)
2. **Create central modules** before modifying consumers (e.g., security.py before tools.py)
3. **Apply changes file-by-file** with `patch` — do NOT use `execute_code` for multi-file edits (timeout risk)
4. **Verify incrementally**: `flake8 --count` after each batch, `pytest --tb=short -q` after major changes
5. **flake8清零 technique**: trailing whitespace (sed) → F401 unused imports → F811/F841 → E-code formatting

### Code Deduplication Techniques

When eliminating duplication across modules:

1. **Extract shared function to utility module**: Create `<module>/type_utils.py` or similar, move the function there as a standalone function (not a method), have both callers delegate to it. Example: `_type_from_typenode` existed in both `analyzer.py` and `interpreter.py` → extracted to `semantic/type_utils.py` as `type_from_typenode()`.

2. **Unify duplicate classes by canonical import**: When the same dataclass exists in two files, keep it in the more fundamental module and import it in the other. Example: `Message` class in both `runtime/__init__.py` and `runtime/history.py` → keep in `history.py`, import in `__init__.py`.

3. **Extract repeated literals to module constants**: When the same tuple/dict/set appears 3+ times, make it a module-level constant. Example: `bare_form_tokens` appeared 3 times in `parser.py` → `BARE_FORM_TOKENS` at module top.

### God Class Refactoring via Mixin Extraction

When a class exceeds ~1000 lines with clearly separable concerns (e.g., an interpreter with LLM methods, file I/O, and core evaluation mixed together):

1. **Identify cohesive method groups**: Look for methods that share a subset of instance attributes but not others. Example: LLM methods all use `self.llm_runtime`, `self._current_agent`, `self._history` but not `self._functions` or `self.import_resolver`.

2. **Create a Mixin class** in a separate file:
   ```python
   class LlmMixin:
       # Declare host-provided attributes with Any type
       llm_runtime: Any
       environment: Any
       _current_agent: Any
       
       # Do NOT declare stub methods for host-provided methods
       # (causes incompatible override errors)
   ```

3. **Key pitfalls in mixin design**:
   - Use `self: Any` parameter annotation on all mixin methods to avoid type checker complaints about missing attributes
   - Do NOT declare stub methods like `def _execute_stmts(self): raise NotImplementedError` — these create override conflicts when the host class implements them as `@staticmethod` or with different signatures
   - Instead, add a comment: `# _execute_stmts and _stringify are provided by the host class`
   - Import only the AST node types the mixin actually uses

4. **Wire up via multiple inheritance**:
   ```python
   class Interpreter(LlmMixin, Visitor[object]):
       """...LLM-related methods are inherited from LlmMixin."""
   ```

5. **Remove original methods from host class** AFTER creating the mixin
6. **Remove now-unused imports** from host class (the LLM node types, `Any` if no longer needed)
7. **Verify**: run full test suite + flake8 — mixin methods should be transparent to callers

**Result**: interpreter.py went from 1588 → 1144 lines (-28%), with 502 lines of LLM logic in a focused, independently maintainable module.

### Pitfalls
- `execute_code` with many `patch()` calls can timeout — use individual `patch` calls or small `terminal` python scripts instead
- When removing dead code, search for callers first (`search_files` for the function name)
- flake8 E127/E128 fixes require reading exact line context — alignment is column-sensitive
- After `sed -i 's/[[:space:]]*$//'` for trailing whitespace, re-run flake8 to see remaining issues
- Always run full test suite after security changes — they can break legitimate use cases
- GitHub PAT needs `workflow` scope to push `.github/workflows/` files — if push fails with "refusing to allow a Personal Access Token to create or update workflow", either update PAT scope or commit the file locally and push other changes first
- When extracting shared utilities, the return type annotation may need to be broadened (e.g., from union of specific subtypes to the base `Type`) to satisfy type checkers at all call sites

---

## 6. Systematic Coverage Improvement

When CI fails on `--cov-fail-under=N`, use this structured approach to raise coverage.

### Diagnosis
1. **Run per-module coverage**: `pytest tests/<module>/ --cov=helen.<module> --cov-report=term-missing -q`
2. **Identify uncovered lines**: the `Missing` column shows exact line numbers
3. **Categorize gaps**:
   - **Happy path not tested** → add basic functionality tests
   - **Error/edge cases not tested** → add exception and boundary tests
   - **Network/external calls** → mock with `unittest.mock.patch`
   - **Extracted module not covered** → create dedicated test file

### Mocking Patterns for Security/Network Code

```python
# Test private IP detection without real DNS
from unittest.mock import patch
import socket

def test_blocks_private_ip(self):
    with patch('socket.getaddrinfo') as mock_gai:
        mock_gai.return_value = [
            (socket.AF_INET, socket.SOCK_STREAM, 0, '', ('10.0.0.1', 0))
        ]
        with pytest.raises(SecurityError, match="private|reserved"):
            validate_url("http://private.example.com")

# Test unresolvable hostname
def test_blocks_unresolvable(self):
    with patch('socket.getaddrinfo') as mock_gai:
        mock_gai.side_effect = socket.gaierror("resolution failed")
        with pytest.raises(SecurityError, match="Cannot resolve"):
            validate_url("http://nonexistent.example.com")

# Test invalid IP graceful handling (ValueError branch)
def test_handles_invalid_ip(self):
    with patch('socket.getaddrinfo') as mock_gai:
        mock_gai.return_value = [
            (socket.AF_INET, socket.SOCK_STREAM, 0, '', ('not-an-ip', 0))
        ]
        result = validate_url("http://example.com")  # Should NOT raise
```

### Coverage for Extracted/Refactored Modules

When you extract code from a large class into a separate module (mixin, utility), existing tests exercise it **through the host class**, not directly. This often yields low coverage (30-40%) even though the code IS being tested.

**Options:**
1. **Accept indirect coverage** — if the code is exercised through integration tests, the coverage number is misleading but the code IS tested. Consider `# pragma: no cover` on thin delegation methods.
2. **Add direct unit tests** — create a dedicated test file for the extracted module. This is the preferred approach for shared utilities (e.g., `type_utils.py`).
3. **Lower the threshold** — if the overall project coverage is healthy but one extracted module drags it down, adjust `--cov-fail-under` or exclude the module from coverage measurement.

### SourceSpan Construction in Tests

Helen's `SourceSpan` requires 5 args: `SourceSpan(file, start_line, start_col, end_line, end_col)`. A common test helper:

```python
def make_span():
    return SourceSpan(file="test.helen", start_line=1, start_col=1, end_line=1, end_col=10)
```

### Pitfalls
- `pytest --cov` with full test suite can OOM on memory-constrained machines (1.8GB RAM). Run per-module instead.
- `coverage run -m pytest` may not produce `.coverage` file if the process is killed (OOM). Use `pytest --cov` instead.
- When testing `UnionType`, the attribute is `members` (not `types`).
- `LiteralTypeNode.values` expects `list[ExpressionNode]`, not `list[int]` — wrap in `LiteralNode`.
- After achieving 100% on a module, verify with `--cov-report=term-missing` to confirm no lines are skipped.
- `pytest --cov` output format varies — don't rely on grep patterns like `grep -E "^(helen/runtime|TOTAL)"`. Use `tail -N` or redirect to file instead.
- See `references/coverage-improvement-patterns.md` for concrete mocking patterns and per-module diagnosis commands.
- See `references/parallel-test-creation.md` for using `delegate_task` to create multiple test files in parallel (3x faster than sequential).

---

## 7. Stub/Mock/Dead-Code Audit

When a user asks to "check for stubs", "find mock code", or "audit code quality", run this systematic scan to separate **real problems** from **legitimate patterns**.

### Search Patterns (run all in parallel)

```
# Dead code / unfinished implementations
stub|Stub|STUB|TODO|FIXME|HACK|XXX|NotImplemented|placeholder
raise NotImplementedError
\.\.\.                          # Ellipsis — Protocol bodies vs dead code
except.*:\s*$                   # Bare except with no binding
if TYPE_CHECKING:               # Check if block body is empty (pass)
```

### Classification Matrix

| Pattern | Legitimate ✅ | Problem 🔴 |
|---------|---------------|------------|
| `...` (ellipsis) | Protocol/ABC method body in `*_contracts.py` | Real function that should have implementation |
| `pass` | Exception class body (`class FooError(Exception): pass`) | Empty `if TYPE_CHECKING: pass` block (refactor residue) |
| `NotImplemented` | `return NotImplemented` in `__eq__`/`__lt__` | `raise NotImplementedError` in non-abstract method |
| `Mock*` class | Test helper with `Mock` prefix in runtime/ | Mock leaking into production code path |
| `except Exception: pass` | — | 🔴 ALWAYS a problem — silent error swallowing |
| Phase comments | — | "stubs for Phase N" when code is already implemented |

### Key Diagnostic: Empty TYPE_CHECKING Blocks

After refactoring, `if TYPE_CHECKING:` blocks may become empty (`pass`). These are dead code:
- **8 files** had this pattern in Helen: `ast.py`, `tokens.py`, `errors.py`, `analyzer.py`, `interpreter.py`, `prompt_builder.py`, `formatter.py`, `symbols.py`
- **Fix**: either delete the empty block, or add the conditional imports that were originally there

### Key Diagnostic: Silent Exception Swallowing

`except Exception: pass` (no `as e`, no logging) is **always** a code quality problem:
- Makes debugging impossible — errors vanish silently
- Common in: config loaders, network tools, LSP servers
- **Fix**: at minimum `except Exception as e: logging.debug("...", exc_info=e)`

### Acceptable Zero-Coverage Categories

When reporting coverage, these file types legitimately show 0% and should NOT be flagged:
- `*_contracts.py` — Protocol definitions with `...` bodies (interface-only)
- `constants.py` — Pure constant definitions, covered indirectly through consumers
- `__init__.py` with only imports — re-exports, covered by consumer tests

### Pitfalls
- Do NOT flag `MockLLMRuntime` or similar test doubles as "mock code that needs replacing" — they are a deliberate design pattern for deterministic testing
- `return NotImplemented` in `__eq__` is Python standard practice, NOT a stub
- Protocol `...` bodies are semantically different from function stubs — they declare interface contracts
- When counting "stub" occurrences, separate the search results into categories before reporting — a raw count is misleading

---

## Integration

| Situation | Use |
|-----------|-----|
| Assessing code quality / rating code | Section 1 (7-dimension framework) |
| Before commit/push | Section 2 (verification gate) |
| "simplify my changes" / cleanup | Section 3 (parallel cleanup) |
| After debugging to verify no regression | Section 1 (quick assessment) |
| After each subagent task | Section 2 (verification gate) |
| Security audit of Python runtime | Section 4 (security hardening) |
| Post-assessment improvement execution | Section 5 (batch improvement) |
| CI coverage threshold failure | Section 6 (coverage improvement) |
| "find stubs" / "check for mock code" / dead code audit | Section 7 (stub/mock audit) |
