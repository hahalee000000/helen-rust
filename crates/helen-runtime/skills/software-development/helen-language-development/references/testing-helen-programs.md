# Testing Helen Programs from Python

## Pattern: Integration Test for Helen Source

When testing Helen language features end-to-end, write the Helen source as a Python string, parse it, execute it, and assert on results.

```python
from helen.core.lexer import Scanner
from helen.core.parser import Parser
from helen.core.errors import ErrorReporter
from helen.interpreter.interpreter import Interpreter
from helen.runtime.http_llm import HttpLLMRuntime

def test_helen_program():
    source = """
agent MyAgent(question: str) {
    prompt "You are helpful."
    main {
        let answer = llm act question
        return answer
    }
}

main {
    let result = MyAgent("test")
    return result
}
"""
    errors = ErrorReporter()
    scanner = Scanner(source=source, file="<test>")
    tokens = scanner.scan_all()
    parser = Parser(tokens, errors=errors)
    program = parser.parse()
    
    assert not errors.has_errors, f"Parse errors: {[e.message for e in errors.errors]}"
    
    llm_runtime = HttpLLMRuntime()
    interp = Interpreter(errors=errors, llm_runtime=llm_runtime)
    result = interp.interpret(program)
    
    assert result is not None
    assert len(result) > 0
```

## Pitfall: Relative Paths in Helen Programs

When Helen programs use `read_file("relative/path")`, they break when invoked from different working directories (e.g., REPL launched from user's home dir).

**Fix:** Python caller computes absolute paths and injects them as parameters:

```python
# Python side (e.g., REPL integration)
import helen.cli.repl as repl_module
module_dir = Path(repl_module.__file__).parent.parent
docs_path = module_dir.parent / "docs" / "tutorial.md"

# Inject absolute path into Helen source before execution
modified_source = source.replace(
    'let docs_path = "wiki/reference/01-getting-started.md"',
    f'let docs_path = "{docs_path}"'
)
```

```helen
// Helen side — accept path as parameter, don't hardcode
import std.core.*
agent HelenAssistant(question: str, docs_path: str) {
    functions {
        fn load_docs(): str {
            return read_file(docs_path)  // Uses parameter, not hardcoded path
        }
    }
    main {
        let docs = load_docs()
        let answer = llm act (docs + "\n\nQuestion: " + question)
        return answer
    }
}
```

## Architecture: Python Shell + Helen Core

For REPL integration features:
- **Python** handles: command parsing, path resolution, error formatting
- **Helen** handles: domain logic (knowledge loading, LLM interaction, response generation)

This keeps Helen programs portable and testable, while Python handles environment-specific concerns.
