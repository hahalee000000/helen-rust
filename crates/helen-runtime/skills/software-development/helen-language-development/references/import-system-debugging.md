# Import System Debugging Guide

## Architecture Overview

The Helen import system spans three layers that must work together:

### 1. Semantic Analyzer (`helen/semantic/analyzer.py:visit_import_stmt`)

**Responsibilities:**
- Validates file exists at resolved path
- Registers data file imports (json/md/txt/yaml) as variables in symbol table
- Uses filename (sans extension) as variable name if no `as alias` specified
- Allows multiple aliases for same data file (no duplicate detection for data files)
- Only tracks duplicates for `.helen` files to prevent circular imports

**Key code:**
```python
# Register data files with or without alias
if path.endswith(('.json', '.md', '.txt', '.yaml', '.yml')):
    alias = node.alias if node.alias else os.path.splitext(os.path.basename(path))[0]
    from helen.semantic.symbols import Symbol
    sym = Symbol(alias, kind="import", is_const=False)
    self.symbols.define(alias, sym)

# Only track .helen files for duplicate detection
if path.endswith('.helen'):
    if path in self._imported_paths:
        return
    self._imported_paths.add(path)
```

### 2. Import Resolver (`helen/runtime/import_resolver.py:resolve`)

**Responsibilities:**
- Resolves import paths (relative to file or base_dir)
- **Path safety**: Allows absolute paths (for REPL/external imports) but prevents `../` escape
- Loads file content based on format (json/md/txt/yaml/helen)
- Caches loaded files in `_loaded` set to prevent duplicate loading
- Returns `ImportResult` with path, content, and format

**Key code:**
```python
def _is_safe_path(self, resolved: str) -> bool:
    """Check that resolved path is within base_dir (HLD 3.9.2 path safety).
    
    Prevents ../ escape outside the project directory.
    Absolute paths are allowed (for REPL and external imports).
    """
    abs_resolved = os.path.abspath(resolved)
    abs_base = os.path.abspath(self.base_dir)
    
    # Allow absolute paths (for REPL and external imports)
    if os.path.isabs(resolved):
        return True
    
    return abs_resolved.startswith(abs_base + os.sep) or abs_resolved == abs_base
```

### 3. Interpreter (`helen/interpreter/interpreter.py:visit_import_stmt`)

**Responsibilities:**
- Calls import resolver
- For `.helen` files: registers agents/functions to global namespace
- For data files: defines variable in environment with loaded content
- Uses user-specified alias or filename as variable name

**Key code:**
```python
if result.format == "helen":
    # Register agents/functions
    for name, agent in self.import_resolver.agents.items():
        if name not in self._agents:
            self._agents[name] = agent
    for name, func in self.import_resolver.functions.items():
        if name not in self._functions:
            self._functions[name] = func
else:
    # Register data by user-specified alias (or filename if no alias)
    alias = node.alias if node.alias else os.path.splitext(os.path.basename(result.path))[0]
    self.environment.define(alias, result.content)
```

## Common Pitfalls

### Pitfall 1: REPL import with absolute paths fails silently

**Symptom:**
```
>>> import "/tmp/setting.json" as test
>>> test
Error: [E0332] Undefined variable 'test'
```

**Root cause:**
Path safety check in `_is_safe_path()` rejected absolute paths outside `base_dir`. REPL's `base_dir` is current directory (`.`), so `/tmp/setting.json` was blocked.

**Fix:**
Modified `_is_safe_path()` to allow absolute paths while still preventing `../` escape:
```python
if os.path.isabs(resolved):
    return True
```

**Debug technique:**
Add debug prints at each layer to trace where data flow breaks:
```python
# In semantic analyzer
print(f"[DEBUG] Registering symbol: {alias}", file=sys.stderr)

# In import resolver
print(f"[DEBUG] Resolved path: {resolved}, safe: {self._is_safe_path(resolved)}", file=sys.stderr)

# In interpreter
print(f"[DEBUG] Defining variable: {alias}", file=sys.stderr)
```

### Pitfall 2: Multiple imports of same file with different aliases

**Symptom:**
```helen
import "config.json"
import "config.json" as cfg  // cfg not defined
```

**Root cause:**
Semantic analyzer was tracking all imports in `_imported_paths` set, causing second import to return early without registering the alias.

**Fix:**
Only track `.helen` files for duplicate detection:
```python
if path.endswith('.helen'):
    if path in self._imported_paths:
        return
    self._imported_paths.add(path)
```

Data files (json/md/txt/yaml) can have multiple aliases.

## Debugging Multi-Layer Issues

When a feature spans multiple layers (semantic → interpreter → runtime):

### Step 1: Add debug prints at each layer boundary

```python
print(f"[DEBUG] {layer_name}: {key_data}", file=sys.stderr)
```

### Step 2: Trace data flow top-down

1. **Semantic analyzer**: Does it register the symbol?
   - Check: `analyzer.symbols.global_scope.symbols.keys()`
   
2. **Interpreter**: Does it execute the statement?
   - Check: Is `visit_import_stmt` being called?
   - Check: What does `import_resolver.resolve()` return?
   
3. **Runtime**: Does it return the expected result?
   - Check: `interp.environment._store.keys()`

### Step 3: Verify environment state

```python
# Check symbol table
print(f"Global symbols: {list(analyzer.symbols.global_scope.symbols.keys())}")

# Check runtime environment
print(f"Environment vars: {list(interp.environment._store.keys())}")
```

### Step 4: Remove debug prints after fixing

Don't leave debug prints in production code. Remove all `print(..., file=sys.stderr)` statements after the issue is resolved.

## Testing Import Functionality

**File-based test:**
```helen
// test_import.helen
import std.core.*
import "config.json"
import "config.json" as cfg
import "readme.md" as docs

print(config.name)
print(cfg.version)
print(len(docs))
```

**REPL test:**
```
>>> import "/tmp/setting.json" as test
>>> test
{'name': 'Test', 'value': 42}
>>> test.name
'Test'
```

**Run tests:**
```bash
cd ~/helen && venv_python -m pytest tests/ -x
```

All 933 tests should pass.
