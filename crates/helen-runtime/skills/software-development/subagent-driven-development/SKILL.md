---
name: subagent-driven-development
description: "Execute plans via delegate_task subagents (2-stage review)."
version: 1.1.0
author: Hermes Agent (adapted from obra/superpowers)
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [delegation, subagent, implementation, workflow, parallel]
    related_skills: [writing-plans, requesting-code-review, test-driven-development]
---

# Subagent-Driven Development

## Overview

Execute implementation plans by dispatching fresh subagents per task with systematic two-stage review.

**Core principle:** Fresh subagent per task + two-stage review (spec then quality) = high quality, fast iteration.

## When to Use

Use this skill when:
- You have an implementation plan (from writing-plans skill or user requirements)
- Tasks are mostly independent
- Quality and spec compliance are important
- You want automated review between tasks

**vs. manual execution:**
- Fresh context per task (no confusion from accumulated state)
- Automated review process catches issues early
- Consistent quality checks across all tasks
- Subagents can ask questions before starting work

## The Process

### 1. Read and Parse Plan

Read the plan file. Extract ALL tasks with their full text and context upfront. Create a todo list:

```python
# Read the plan
read_file("docs/plans/feature-plan.md")

# Create todo list with all tasks
todo([
    {"id": "task-1", "content": "Create User model with email field", "status": "pending"},
    {"id": "task-2", "content": "Add password hashing utility", "status": "pending"},
    {"id": "task-3", "content": "Create login endpoint", "status": "pending"},
])
```

**Key:** Read the plan ONCE. Extract everything. Don't make subagents read the plan file — provide the full task text directly in context.

### 2. Per-Task Workflow

For EACH task in the plan:

#### Step 1: Dispatch Implementer Subagent

Use `delegate_task` with complete context:

```python
delegate_task(
    goal="Implement Task 1: Create User model with email and password_hash fields",
    context="""
    TASK FROM PLAN:
    - Create: src/models/user.py
    - Add User class with email (str) and password_hash (str) fields
    - Use bcrypt for password hashing
    - Include __repr__ for debugging

    FOLLOW TDD:
    1. Write failing test in tests/models/test_user.py
    2. Run: pytest tests/models/test_user.py -v (verify FAIL)
    3. Write minimal implementation
    4. Run: pytest tests/models/test_user.py -v (verify PASS)
    5. Run: pytest tests/ -q (verify no regressions)
    6. Commit: git add -A && git commit -m "feat: add User model with password hashing"

    PROJECT CONTEXT:
    - Python 3.11, Flask app in src/app.py
    - Existing models in src/models/
    - Tests use pytest, run from project root
    - bcrypt already in requirements.txt
    """,
    toolsets=['terminal', 'file']
)
```

#### Step 2: Dispatch Spec Compliance Reviewer

After the implementer completes, verify against the original spec:

```python
delegate_task(
    goal="Review if implementation matches the spec from the plan",
    context="""
    ORIGINAL TASK SPEC:
    - Create src/models/user.py with User class
    - Fields: email (str), password_hash (str)
    - Use bcrypt for password hashing
    - Include __repr__

    CHECK:
    - [ ] All requirements from spec implemented?
    - [ ] File paths match spec?
    - [ ] Function signatures match spec?
    - [ ] Behavior matches expected?
    - [ ] Nothing extra added (no scope creep)?

    OUTPUT: PASS or list of specific spec gaps to fix.
    """,
    toolsets=['file']
)
```

**If spec issues found:** Fix gaps, then re-run spec review. Continue only when spec-compliant.

#### Step 3: Dispatch Code Quality Reviewer

After spec compliance passes:

```python
delegate_task(
    goal="Review code quality for Task 1 implementation",
    context="""
    FILES TO REVIEW:
    - src/models/user.py
    - tests/models/test_user.py

    CHECK:
    - [ ] Follows project conventions and style?
    - [ ] Proper error handling?
    - [ ] Clear variable/function names?
    - [ ] Adequate test coverage?
    - [ ] No obvious bugs or missed edge cases?
    - [ ] No security issues?

    OUTPUT FORMAT:
    - Critical Issues: [must fix before proceeding]
    - Important Issues: [should fix]
    - Minor Issues: [optional]
    - Verdict: APPROVED or REQUEST_CHANGES
    """,
    toolsets=['file']
)
```

**If quality issues found:** Fix issues, re-review. Continue only when approved.

#### Step 4: Mark Complete

```python
todo([{"id": "task-1", "content": "Create User model with email field", "status": "completed"}], merge=True)
```

### 3. Final Review

After ALL tasks are complete, dispatch a final integration reviewer:

```python
delegate_task(
    goal="Review the entire implementation for consistency and integration issues",
    context="""
    All tasks from the plan are complete. Review the full implementation:
    - Do all components work together?
    - Any inconsistencies between tasks?
    - All tests passing?
    - Ready for merge?
    """,
    toolsets=['terminal', 'file']
)
```

### 4. Verify and Commit

```bash
# Run full test suite
pytest tests/ -q

# Review all changes
git diff --stat

# Final commit if needed
git add -A && git commit -m "feat: complete [feature name] implementation"
```

## Task Granularity

**Each task = 2-5 minutes of focused work.**

**Too big:**
- "Implement user authentication system"

**Right size:**
- "Create User model with email and password fields"
- "Add password hashing function"
- "Create login endpoint"
- "Add JWT token generation"
- "Create registration endpoint"

## Red Flags — Never Do These

- Start implementation without a plan
- Skip reviews (spec compliance OR code quality)
- Proceed with unfixed critical/important issues
- Dispatch multiple implementation subagents for tasks that touch the same files
- Make subagent read the plan file (provide full text in context instead)
- Skip scene-setting context (subagent needs to understand where the task fits)
- Ignore subagent questions (answer before letting them proceed)
- Accept "close enough" on spec compliance
- Skip review loops (reviewer found issues → implementer fixes → review again)
- Let implementer self-review replace actual review (both are needed)
- **Start code quality review before spec compliance is PASS** (wrong order)
- Move to next task while either review has open issues
- **Delegate mechanical linting/formatting fixes to subagents** — these are fast, low-value tasks that can waste a 600s timeout if the subagent gets stuck. Fix flake8/black issues directly in the controller session.

## Phase-Gated Execution

When executing a multi-phase plan (e.g., compiler phases, layered architecture), enforce quality gates between phases:

1. **Complete all tasks in current phase**
2. **Run quality gate**: tests pass, coverage threshold, complexity limits, lint clean, security scan
3. **If gate fails**: fix issues before proceeding to next phase
4. **If gate passes**: mark phase complete, move to next

This prevents accumulating technical debt across phases. Each phase should be independently shippable.

Example gate metrics:
- Tests: 100% pass rate
- Coverage: ≥ 80% overall, ≥ 90% for core modules
- Complexity: no function CC > 20
- Lint: zero errors
- Security: zero critical findings

## Pitfalls — Subagent Timeouts on Complex Tasks

When dispatching subagents for tasks involving **semantic analysis, type systems, or coverage improvement**, timeouts (600s) are common if the task is too broad. Observed pattern:
- Subagent completes 25-29 API calls (reading files, some patches) then times out
- Usually happens when asked to "implement missing methods AND write tests" in one task
- More likely when the target file is >300 lines with complex interdependencies

**Mitigation: Two-step dispatch pattern**
1. **First subagent**: Implement missing methods/logic ONLY (no tests)
2. **Second subagent**: Write tests to improve coverage ONLY (no implementation)

Example:
```python
# Step 1: Implement
delegate_task(goal="Add visit_unary_op, visit_call, visit_index methods to type_checker.py", ...)

# Step 2: Test coverage
delegate_task(goal="Write tests for type_checker.py to achieve 90%+ coverage", ...)
```

**Why this works**: Each subagent has a single responsibility, reducing context switching and API call count. Implementation tasks tend to get stuck on edge cases; test tasks tend to get stuck on fixture setup. Separating them prevents both failure modes.

**Mitigation: Generate large files (>500 lines) via execute_code + write_file**

When asked to generate a complete compiler/parser/interpreter file from a detailed spec, subagents consistently timeout (observed: 2 consecutive 600s timeouts on generating `parser.py` ~550 lines). The subagent reads many existing files, then attempts to write the large output, and the API call count exceeds the limit before completion.

**Fallback pattern**: Use `execute_code` with `write_file` to generate the file directly in the controller session. This bypasses subagent context limits entirely.

```python
# In Hermes Agent context:
from hermes_tools import write_file
result = write_file("helen/core/parser.py", parser_code)

# In Helen standalone context, use the equivalent file writing API
# (e.g., Python's pathlib or the runtime's file tools)
```

Only use subagents for files under ~300 lines or tasks with iterative read-patch cycles.

**Pitfall: Subagent corruption of existing files**

Observed: A subagent tasked with adding `accept` methods to `ast.py` produced corrupted output — line-number prefixes (e.g., `500|    """`) were injected into docstrings, causing `SyntaxError: unterminated triple-quoted string literal`. The subagent appears to have copied `read_file` output format (which prefixes lines with numbers) into the file content.

**Mitigation**: After any subagent write to a critical file, verify syntax immediately:
```bash
python -c "import ast; ast.parse(open('path/to/file.py').read())"
```
If syntax is broken, rewrite the file directly via `execute_code` + `write_file` rather than patching.

## Pitfalls — Concurrent File Modifications

When running parallel subagents that touch the same files, you'll see warnings like:
> "file was modified by sibling subagent but this agent never read it"

**Mitigation**:
- For sequential tasks (Phase 1 → Phase 2), run subagents **serially**, not in parallel
- If parallelism is needed, ensure subagents target **different files**
- After a subagent completes, **re-read** any files it modified before dispatching the next one

## Handling Issues

### If Subagent Asks Questions

- Answer clearly and completely
- Provide additional context if needed
- Don't rush them into implementation

### If Reviewer Finds Issues

- Implementer subagent (or a new one) fixes them
- Reviewer reviews again
- Repeat until approved
- Don't skip the re-review

### If Subagent Fails a Task

- Dispatch a new fix subagent with specific instructions about what went wrong
- Don't try to fix manually in the controller session (context pollution)

## Efficiency Notes

**Why fresh subagent per task:**
- Prevents context pollution from accumulated state
- Each subagent gets clean, focused context
- No confusion from prior tasks' code or reasoning

**Why two-stage review:**
- Spec review catches under/over-building early
- Quality review ensures the implementation is well-built
- Catches issues before they compound across tasks

**Cost trade-off:**
- More subagent invocations (implementer + 2 reviewers per task)
- But catches issues early (cheaper than debugging compounded problems later)

## Integration with Other Skills

### With writing-plans

This skill EXECUTES plans created by the writing-plans skill:
1. User requirements → writing-plans → implementation plan
2. Implementation plan → subagent-driven-development → working code

### With test-driven-development

Implementer subagents should follow TDD:
1. Write failing test first
2. Implement minimal code
3. Verify test passes
4. Commit

Include TDD instructions in every implementer context.

### With contract-driven-development

When building compilers, interpreters, or DSLs with a formal HLD and multiple phases, load the `contract-driven-development` skill. It replaces the standard plan → subagent flow with a Contracts → Tests → Impl(TDD) → Consistency Gate → Quality Gate pipeline for each phase. The Consistency Gate serves as the spec compliance check, but operates at the design-doc level rather than the task level.

### With requesting-code-review

The two-stage review process IS the code review. For final integration review, use the requesting-code-review skill's review dimensions.

### With systematic-debugging

If a subagent encounters bugs during implementation:
1. Follow systematic-debugging process
2. Find root cause before fixing
3. Write regression test
4. Resume implementation

## Example Workflow

```
[Read plan: docs/plans/auth-feature.md]
[Create todo list with 5 tasks]

--- Task 1: Create User model ---
[Dispatch implementer subagent]
  Implementer: "Should email be unique?"
  You: "Yes, email must be unique"
  Implementer: Implemented, 3/3 tests passing, committed.

[Dispatch spec reviewer]
  Spec reviewer: ✅ PASS — all requirements met

[Dispatch quality reviewer]
  Quality reviewer: ✅ APPROVED — clean code, good tests

[Mark Task 1 complete]

--- Task 2: Password hashing ---
[Dispatch implementer subagent]
  Implementer: No questions, implemented, 5/5 tests passing.

[Dispatch spec reviewer]
  Spec reviewer: ❌ Missing: password strength validation (spec says "min 8 chars")

[Implementer fixes]
  Implementer: Added validation, 7/7 tests passing.

[Dispatch spec reviewer again]
  Spec reviewer: ✅ PASS

[Dispatch quality reviewer]
  Quality reviewer: Important: Magic number 8, extract to constant
  Implementer: Extracted MIN_PASSWORD_LENGTH constant
  Quality reviewer: ✅ APPROVED

[Mark Task 2 complete]

... (continue for all tasks)

[After all tasks: dispatch final integration reviewer]
[Run full test suite: all passing]
[Done!]
```

## Helen Standalone Mode

When running Helen independently (without Hermes Agent), use a **temporary Helen agent script** to execute subagent tasks. This replaces Hermes's `delegate_task` with Helen-native subprocess execution.

### Architecture

```
Controller (main Helen session)
    │
    ├── Task 1 → Write temp script → subprocess → Collect result
    ├── Task 2 → Write temp script → subprocess → Collect result
    └── Task 3 → Write temp script → subprocess → Collect result
```

### Pattern: Temporary Agent Script

For each task, create a temporary Helen script that:
1. Reads task context from a JSON file
2. Executes the task using Helen's stdlib tools
3. Writes results to a JSON output file

```helen
// temp_agent.helen — Generated per task
import std.core.*
import std.io.*
import "json" as json
import "os" as os

agent TaskAgent() {
    description: "Execute a single development task"
    
    main {
        // Read task context
        let context_file = os.getenv("TASK_CONTEXT_FILE")
        let context = json.parse(os.read_file(context_file))
        
        let task_desc = context["task"]
        let file_path = context["file"]
        let expected_tests = context["tests"]
        
        // Execute task using Helen tools
        // ... implementation using stdlib file/system tools ...
        
        // Write result
        let result = {"status": "completed", "files_changed": [file_path]}
        let output_file = os.getenv("TASK_OUTPUT_FILE")
        os.write_file(output_file, json.stringify(result))
    }
}

main {
    let agent = TaskAgent()
    agent.main()
}
```

### Controller Pattern (Python orchestration)

```python
import subprocess
import json
import tempfile
import os

def dispatch_helen_task(task_desc: str, context: dict) -> dict:
    """Dispatch a task to a Helen subprocess agent."""
    
    # Write task context to temp file
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
        json.dump({"task": task_desc, **context}, f)
        context_file = f.name
    
    output_file = context_file.replace('.json', '_output.json')
    
    # Run Helen agent script
    env = os.environ.copy()
    env["TASK_CONTEXT_FILE"] = context_file
    env["TASK_OUTPUT_FILE"] = output_file
    
    result = subprocess.run(
        ["helen", "temp_agent.helen"],
        env=env,
        capture_output=True,
        text=True,
        timeout=600
    )
    
    # Read result
    if os.path.exists(output_file):
        with open(output_file) as f:
            return json.load(f)
    else:
        return {"status": "failed", "error": result.stderr}
    
    # Cleanup
    os.unlink(context_file)
    if os.path.exists(output_file):
        os.unlink(output_file)

# Usage
for task in plan_tasks:
    result = dispatch_helen_task(
        task_desc=task["description"],
        context={"file": task["file"], "tests": task["tests"]}
    )
    print(f"Task {task['id']}: {result['status']}")
```

### Concurrent Execution (spawn + Channel)

Helen's `spawn` primitive enables concurrent task execution via Channel message queues:

```helen
import std.core.*
import std.system.*
import "os" as os
import "json" as json

agent TaskRunner(task_id: str, context: str, reply: Channel) {
    main {
        // Each agent runs in its own thread via spawn
        let result = os.exec(["helen", "agent_script.helen"], 
                             env={"TASK_ID": task_id, "CONTEXT": context})
        reply.send(result)
    }
}

main {
    // Spawn agents - each returns a Channel (mailbox) immediately
    let m1 = spawn TaskRunner("task-1", "Implement User model")
    let m2 = spawn TaskRunner("task-2", "Add password hashing")
    let m3 = spawn TaskRunner("task-3", "Create login endpoint")
    
    // Collect results from each mailbox (agents run concurrently)
    let results = [m1.receive(), m2.receive(), m3.receive()]
    
    // Process results
    for i, result in results {
        print("Task {i}: {result}")
    }
}
```

### Review Workflow in Helen

After each task completes, dispatch reviewer agents:

```python
def review_task(task_desc: str, implementation: str) -> dict:
    """Dispatch a reviewer agent to check implementation."""
    
    review_context = {
        "task_spec": task_desc,
        "implementation": implementation,
        "review_criteria": [
            "All requirements from spec implemented?",
            "File paths match spec?",
            "Function signatures match spec?",
            "No scope creep?"
        ]
    }
    
    return dispatch_helen_task(
        task_desc="Review implementation against spec",
        context=review_context
    )
```

### Advantages of Helen Standalone Mode

1. **No external dependencies** — Pure Helen + Python stdlib
2. **Memory isolation** — Each subprocess has its own memory space
3. **Crash isolation** — One task failure doesn't affect others
4. **True concurrency** — `spawn` runs each agent in its own daemon thread
5. **Portable** — Works on any system with Python 3.8+

### Limitations

1. **No LLM integration** — Helen standalone doesn't have built-in LLM calls
   - Workaround: Use Helen's Python FFI to call OpenAI/Anthropic APIs
   - Or: Use Hermes Agent for LLM-powered review
2. **Manual context passing** — Must serialize/deserialize task context
3. **No automatic retry** — Must implement retry logic in controller

### When to Use Each Mode

| Scenario | Recommended Mode |
|----------|-----------------|
| Helen development with LLM review | Hermes Agent (`delegate_task`) |
| Pure Helen project, no LLM | Helen standalone (subprocess agents) |
| Mixed workflow | Helen for implementation, Hermes for review |
| CI/CD pipeline | Helen standalone (deterministic, no API costs) |

## Remember

```
Fresh subagent per task
Two-stage review every time
Spec compliance FIRST
Code quality SECOND
Never skip reviews
Catch issues early
```

**Quality is not an accident. It's the result of systematic process.**

## Further reading (load when relevant)

When the orchestration involves significant context usage, long review loops, or complex validation checkpoints, load these references for the specific discipline:

- **`references/context-budget-discipline.md`** — Four-tier context degradation model (PEAK / GOOD / DEGRADING / POOR), read-depth rules that scale with context window size, and early warning signs of silent degradation. Load when a run will clearly consume significant context (multi-phase plans, many subagents, large artifacts).
- **`references/gates-taxonomy.md`** — The four canonical gate types (Pre-flight, Revision, Escalation, Abort) with behavior, recovery, and examples. Load when designing or reviewing any workflow that has validation checkpoints — use the vocabulary explicitly so each gate has defined entry, failure behavior, and resumption rules.
- **`references/compliance-checking-pattern.md`** — Automated Python script patterns for verifying code matches design documents (TokenType, AST nodes, error codes, Visitor methods). Use after each phase to ensure implementation fidelity.

Both references adapted from gsd-build/get-shit-done (MIT © 2025 Lex Christopherson).
