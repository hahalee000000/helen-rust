---
name: helen-repl-extension
description: "Extend Helen REPL with Helen programs: architecture, path resolution, parameter injection."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [helen, repl, extension, integration, python]
    related_skills: [test-driven-development, writing-plans]
---

# Helen REPL Extension Patterns

## Overview

Build REPL extensions by writing Helen programs that get loaded and executed by the Python REPL infrastructure. The Helen program defines the logic; Python provides paths, I/O, and infrastructure.

## When to Use

- Adding AI-powered features to REPL (assistants, analyzers)
- Building REPL commands that need Helen's LLM integration
- Creating reusable Helen programs invoked from REPL
- Extending REPL with domain-specific functionality

## Architecture Pattern

```
┌─────────────────────────────────────────┐
│  Python REPL (helen/cli/repl.py)        │
│  :command <args>                        │
└─────────────┬───────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│  Helen Program (helen/<feature>.helen)  │
│  - Defines agent/functions/main         │
│  - Uses Helen stdlib (read_file, etc.)  │
│  - Uses LLM integration (llm act)       │
└─────────────┬───────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│  Python Infrastructure                  │
│  - Computes absolute paths              │
│  - Injects parameters via string replace│
│  - Executes modified Helen program      │
└─────────────────────────────────────────┘
```

## Core Techniques

### 1. Module-Relative Path Resolution

**Problem**: Relative paths fail when REPL runs from different directories.

**Solution**: Compute absolute paths based on module location.

```python
# In helen/cli/repl.py
import helen.cli.repl as repl_module
module_dir = Path(repl_module.__file__).parent.parent  # helen/cli -> helen/
assistant_path = module_dir / "agent" / "helen_assistant.helen"
docs_path = module_dir.parent / "docs" / "tutorial.md"
source_dir = module_dir  # helen/ directory
```

**Pattern**: `Path(module.__file__).parent.N` where N is the number of levels to traverse up.

### 2. Parameter Injection via String Replacement

**Problem**: Helen program needs runtime parameters (user input, absolute paths).

**Solution**: REPL modifies Helen source before execution via string replacement.

```python
# Helen program has placeholder values:
# let question = "default question"
# let docs_path = "reports/tutorial.md"

# REPL replaces with actual values:
modified_source = source.replace(
    'let question = "default question"',
    f'let question = "{user_question}"'
).replace(
    'let docs_path = "reports/tutorial.md"',
    f'let docs_path = "{docs_path}"'
)
```

**Requirements**:
- Helen program must have unique placeholder strings
- Placeholders should have comments for clarity: `// Relative path for development`
- REPL must replace ALL placeholders before execution

### 3. Multi-Parameter Injection

When Helen program accepts multiple parameters:

```helen
agent HelenAssistant(question: str, docs_path: str, source_dir: str) {
    // ...
}

main {
    let question = "default"  // Placeholder
    let docs_path = "reports/tutorial.md"  // Placeholder
    let source_dir = "helen/"  // Placeholder
    let result = HelenAssistant(question, docs_path, source_dir)
}
```

Python REPL injects all three:

```python
modified_source = source.replace(
    'let question = "default"',
    f'let question = "{question}"'
).replace(
    'let docs_path = "reports/tutorial.md"  // Placeholder',
    f'let docs_path = "{docs_path}"'
).replace(
    'let source_dir = "helen/"  // Placeholder',
    f'let source_dir = "{source_dir}/"'
)
```

## Implementation Workflow

### Step 1: Design Helen Program

Write the Helen program with:
- Agent declaration with parameters
- Functions that use parameters
- Main block with placeholder values
- Standalone execution example

```helen
import std.core.*
agent MyFeature(param1: str, param2: str) {
    prompt "..."
    
    functions {
        fn do_something(): str {
            // Use param1, param2
        }
    }
    
    main {
        let result = do_something()
        return result
    }
}

main {
    let param1 = "default1"  // Placeholder for REPL injection
    let param2 = "default2"  // Placeholder for REPL injection
    let result = MyFeature(param1, param2)
    print(result)
}
```

### Step 2: Write Tests (TDD)

```python
def test_feature_program_exists(self):
    """Feature program file exists."""
    path = Path("helen/agent/my_feature.helen")
    assert path.exists()

def test_feature_loads_resources(self):
    """Feature can load required resources."""
    source = """
agent MyFeature(param1: str, param2: str) {
    functions {
        fn load(): str {
            return read_file(param1)
        }
    }
    main {
        return load()
    }
}
main {
    return MyFeature("reports/tutorial.md", "helen/")
}
"""
    # Parse and execute
    # Assert resource loaded

def test_feature_injects_parameters(self):
    """REPL injects parameters correctly."""
    # Mock _run_feature
    # Call :command with args
    # Assert parameters injected
```

### Step 3: Implement REPL Command

```python
def _run_my_feature(param1: str, param2: str) -> str:
    """Run the feature program."""
    # 1. Compute absolute paths
    module_dir = Path(repl_module.__file__).parent.parent
    feature_path = module_dir / "agent" / "my_feature.helen"
    
    # 2. Validate paths exist
    if not feature_path.exists():
        return f"Error: Feature not found at {feature_path}"
    
    # 3. Load Helen program
    source = feature_path.read_text(encoding="utf-8")
    
    # 4. Inject parameters
    modified_source = source.replace(
        'let param1 = "default1"  // Placeholder',
        f'let param1 = "{param1}"'
    ).replace(
        'let param2 = "default2"  // Placeholder',
        f'let param2 = "{param2}"'
    )
    
    # 5. Parse and execute
    errors = ErrorReporter()
    scanner = Scanner(source=modified_source, file=str(feature_path))
    tokens = scanner.scan_all()
    parser = Parser(tokens, errors=errors)
    program = parser.parse()
    
    if errors.has_errors:
        return f"Parse error: {errors.format_report()}"
    
    llm_runtime = HttpLLMRuntime()
    interp = Interpreter(errors=errors, llm_runtime=llm_runtime)
    
    try:
        result = interp.interpret(program)
        return result if result else "No result"
    except Exception as e:
        return f"Runtime error: {e}"

def _handle_repl_command(line: str, interp, analyzer) -> bool:
    # ...
    if cmd == ":mycommand":
        if not arg:
            print("Usage: :mycommand <arg>")
            return True
        response = _run_my_feature(arg, "other_param")
        print(f"\n{response}\n")
        return True
```

### Step 4: Update Documentation

- Add command to `:help` output
- Update `reports/tutorial.md` with usage examples
- Update `~/wiki/helen/tutorial/` with examples

## Pitfalls

### 1. Relative Path Failures

**Symptom**: `FileNotFoundError: reports/tutorial.md` when running from different directory.

**Cause**: Helen program uses relative paths; REPL runs from user's CWD.

**Fix**: Always compute absolute paths in Python REPL using module-relative resolution.

### 2. Incomplete Parameter Injection

**Symptom**: Helen program uses default values instead of user input.

**Cause**: REPL didn't replace all placeholders, or placeholder strings don't match exactly.

**Fix**: 
- Use unique placeholder strings with comments
- Verify all placeholders are replaced
- Test with different parameter values

### 3. Missing Resource Validation

**Symptom**: Cryptic runtime error when resource doesn't exist.

**Cause**: REPL doesn't validate paths before execution.

**Fix**: Check all paths exist before executing Helen program:

```python
if not docs_path.exists():
    return f"Error: Documentation not found at {docs_path}"
if not source_dir.exists():
    return f"Error: Source directory not found at {source_dir}"
```

### 4. Incomplete Knowledge Base

**Symptom**: Assistant answers syntax questions but can't explain internals.

**Cause**: Helen program only loads documentation, not source code.

**Fix**: Load both documentation AND source code for comprehensive coverage:

```helen
import std.core.*
fn load_documentation(): str {
    return read_file(docs_path)
}

fn load_source_code(): str {
    let parser = read_file(source_dir + "core/parser.py")
    let interpreter = read_file(source_dir + "interpreter/interpreter.py")
    // ... load key files
    return combined_sources
}
```

## Testing Strategy

1. **Existence test**: Verify Helen program file exists
2. **Resource loading test**: Verify program can load required files
3. **Parameter injection test**: Verify REPL injects parameters correctly
4. **Integration test**: End-to-end test of REPL command
5. **Error handling test**: Verify graceful errors for missing resources

## Example: Helen Language Assistant

**Helen program** (`helen/agent/helen_assistant.helen`):
- Accepts: `question`, `docs_path`, `source_dir`
- Loads: documentation + source code
- Uses: `llm act` to generate answers

**REPL integration** (`helen/cli/repl.py`):
- Command: `:ask <question>`
- Computes absolute paths
- Injects parameters
- Executes Helen program
- Prints response

**Tests** (`tests/integration/test_helen_assistant.py`):
- 5 tests covering existence, loading, context building, LLM answers, source loading

## Verification Checklist

Before declaring extension complete:

- [ ] Helen program exists at `helen/<category>/<feature>.helen`
- [ ] Helen program uses parameters (not hardcoded paths)
- [ ] REPL computes absolute paths using module-relative resolution
- [ ] REPL validates all paths exist before execution
- [ ] REPL injects ALL parameters via string replacement
- [ ] Placeholder strings in Helen program are unique and commented
- [ ] Tests cover: existence, resource loading, parameter injection, integration
- [ ] Documentation updated: `reports/tutorial.md`, `~/wiki/helen/tutorial/`
- [ ] `:help` command updated with new command
- [ ] All tests pass (run `pytest tests/ -v`)
