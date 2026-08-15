# Python FFI Implementation Details

Complete implementation guide for Helen's Python Foreign Function Interface.

## Architecture Overview

```
Helen Code
    ↓
import "math" as math
    ↓
Semantic Analyzer (detects Python module)
    ↓
Interpreter._import_python_module()
    ↓
PythonRuntime.import_module("math")
    ↓
WrappedPythonModule("math", <module>)
    ↓
Environment.define("math", module)
    ↓
math.sqrt(16) → WrappedPythonObject call
    ↓
Type conversion: Helen → Python → Helen
    ↓
Result: 4.0
```

## File Structure

```
helen/ffi/
├── __init__.py              # Package exports
├── contracts.py             # Protocol definitions
├── type_converter.py        # Helen ↔ Python type conversion
├── python_object.py         # Wrap Python objects
├── python_module.py         # Wrap Python modules
└── python_runtime.py        # Module loading and execution context

tests/ffi/
├── test_type_converter.py   # Type conversion tests
├── test_python_object.py    # Object wrapper tests
├── test_python_runtime.py   # Module loading tests
└── test_helen_integration.py # End-to-end Helen code tests
```

## Contract Definitions (contracts.py)

```python
from typing import Protocol, runtime_checkable, Any

@runtime_checkable
class PythonObject(Protocol):
    """Contract for a Python object accessible from Helen."""
    
    def get_attribute(self, name: str) -> Any:
        """Get an attribute from the Python object."""
        ...
    
    def call(self, *args: Any, **kwargs: Any) -> Any:
        """Call the Python object as a function."""
        ...
    
    def __getitem__(self, key: Any) -> Any:
        """Get item by key (for dict/list access)."""
        ...
    
    def __str__(self) -> str:
        """String representation."""
        ...
    
    def unwrap(self) -> Any:
        """Get the underlying Python object."""
        ...

@runtime_checkable
class PythonModule(Protocol):
    """Contract for a Python module loaded from Helen."""
    
    name: str
    
    def __getattr__(self, name: str) -> Any:
        """Get a module attribute (function, class, constant)."""
        ...
    
    def get_module(self) -> Any:
        """Get the underlying Python module object."""
        ...

@runtime_checkable
class TypeConverter(Protocol):
    """Contract for converting between Helen and Python types."""
    
    def helen_to_python(self, value: Any) -> Any:
        """Convert a Helen value to Python."""
        ...
    
    def python_to_helen(self, value: Any) -> Any:
        """Convert a Python value to Helen."""
        ...

@runtime_checkable
class PythonRuntime(Protocol):
    """Contract for Python runtime management."""
    
    def import_module(self, module_name: str) -> PythonModule:
        """Import a Python module."""
        ...
    
    def eval_expression(self, expression: str) -> Any:
        """Evaluate a Python expression."""
        ...
    
    def exec_statement(self, statement: str) -> None:
        """Execute a Python statement."""
        ...
    
    def get_converter(self) -> TypeConverter:
        """Get the type converter for this runtime."""
        ...
```

## Type Conversion Implementation (type_converter.py)

```python
from typing import Any

class DefaultTypeConverter:
    """Default type converter implementation."""
    
    def helen_to_python(self, value: Any) -> Any:
        """Convert a Helen value to Python."""
        # None → None
        if value is None:
            return None
        
        # Primitives pass through
        if isinstance(value, (int, float, str, bool)):
            return value
        
        # Lists convert recursively
        if isinstance(value, list):
            return [self.helen_to_python(item) for item in value]
        
        # Dicts convert recursively
        if isinstance(value, dict):
            return {k: self.helen_to_python(v) for k, v in value.items()}
        
        # Wrapped objects unwrap
        if hasattr(value, 'unwrap'):
            return value.unwrap()
        
        # Everything else passes through
        return value
    
    def python_to_helen(self, value: Any) -> Any:
        """Convert a Python value to Helen."""
        # None → null
        if value is None:
            return None
        
        # Primitives pass through
        if isinstance(value, (int, float, str, bool)):
            return value
        
        # Tuples convert to lists
        if isinstance(value, tuple):
            return [self.python_to_helen(item) for item in value]
        
        # Lists convert recursively
        if isinstance(value, list):
            return [self.python_to_helen(item) for item in value]
        
        # Dicts convert recursively
        if isinstance(value, dict):
            return {k: self.python_to_helen(v) for k, v in value.items()}
        
        # Complex objects are wrapped
        from helen.ffi.python_object import WrappedPythonObject
        return WrappedPythonObject(value)
```

**Key decisions**:
- Primitives (int, float, str, bool, None) pass through unchanged
- Collections (list, dict, tuple) convert recursively
- Complex objects (classes, numpy arrays, etc.) are wrapped to preserve functionality
- Lazy import of `WrappedPythonObject` to avoid circular dependency

## Object Wrapper Implementation (python_object.py)

```python
from typing import Any

class WrappedPythonObject:
    """Wrapper for Python objects accessible from Helen."""
    
    def __init__(self, obj: Any):
        self._obj = obj
        from helen.ffi.type_converter import DefaultTypeConverter
        self._converter = DefaultTypeConverter()
    
    def get_attribute(self, name: str) -> Any:
        """Get an attribute from the Python object."""
        value = getattr(self._obj, name)
        return self._converter.python_to_helen(value)
    
    def call(self, *args: Any, **kwargs: Any) -> Any:
        """Call the Python object as a function."""
        if not callable(self._obj):
            raise TypeError(f"'{type(self._obj).__name__}' object is not callable")
        
        # Convert Helen arguments to Python
        py_args = [self._converter.helen_to_python(arg) for arg in args]
        py_kwargs = {k: self._converter.helen_to_python(v) for k, v in kwargs.items()}
        
        # Call the object
        result = self._obj(*py_args, **py_kwargs)
        
        # Convert result back to Helen
        return self._converter.python_to_helen(result)
    
    def __getitem__(self, key: Any) -> Any:
        """Get item by key (for dict/list access)."""
        py_key = self._converter.helen_to_python(key)
        value = self._obj[py_key]
        return self._converter.python_to_helen(value)
    
    def __str__(self) -> str:
        return str(self._obj)
    
    def unwrap(self) -> Any:
        """Get the underlying Python object."""
        return self._obj
    
    def __getattr__(self, name: str) -> Any:
        """Allow direct attribute access (for convenience)."""
        if name.startswith('_'):
            raise AttributeError(f"'{type(self).__name__}' object has no attribute '{name}'")
        return self.get_attribute(name)
```

**Key decisions**:
- All attribute access goes through type converter
- Function calls convert arguments and return values
- `__getattr__` provides convenient `wrapper.attr` syntax
- Internal attributes (starting with `_`) raise AttributeError to avoid recursion

## Module Wrapper Implementation (python_module.py)

```python
from typing import Any

class WrappedPythonModule:
    """Wrapper for Python modules accessible from Helen."""
    
    def __init__(self, name: str, module: Any):
        self.name = name
        self._module = module
        from helen.ffi.type_converter import DefaultTypeConverter
        self._converter = DefaultTypeConverter()
    
    def __getattr__(self, name: str) -> Any:
        """Get a module attribute (function, class, constant)."""
        if name.startswith('_'):
            raise AttributeError(f"'{type(self).__name__}' object has no attribute '{name}'")
        
        value = getattr(self._module, name)
        return self._converter.python_to_helen(value)
    
    def get_module(self) -> Any:
        """Get the underlying Python module object."""
        return self._module
    
    def __repr__(self) -> str:
        return f"WrappedPythonModule({self.name!r})"
```

## Runtime Implementation (python_runtime.py)

```python
import importlib
from typing import Any
from helen.ffi.contracts import PythonRuntime, PythonModule, TypeConverter
from helen.ffi.python_module import WrappedPythonModule
from helen.ffi.type_converter import DefaultTypeConverter

class DefaultPythonRuntime:
    """Default Python runtime implementation."""
    
    def __init__(self):
        self._modules: dict[str, WrappedPythonModule] = {}
        self._converter = DefaultTypeConverter()
        self._context: dict[str, Any] = {}  # For eval/exec
    
    def import_module(self, module_name: str) -> PythonModule:
        """Import a Python module."""
        # Check cache
        if module_name in self._modules:
            return self._modules[module_name]
        
        # Import the module
        try:
            module = importlib.import_module(module_name)
        except ImportError as e:
            raise ImportError(f"Cannot import module '{module_name}': {e}")
        
        # Wrap and cache
        wrapped = WrappedPythonModule(module_name, module)
        self._modules[module_name] = wrapped
        
        # Add to execution context
        var_name = module_name.split('.')[-1]
        self._context[var_name] = module
        
        return wrapped
    
    def eval_expression(self, expression: str) -> Any:
        """Evaluate a Python expression."""
        result = eval(expression, self._context)
        return self._converter.python_to_helen(result)
    
    def exec_statement(self, statement: str) -> None:
        """Execute a Python statement."""
        exec(statement, self._context)
    
    def get_converter(self) -> TypeConverter:
        """Get the type converter for this runtime."""
        return self._converter
```

## Semantic Analyzer Integration

In `helen/semantic/analyzer.py`, `visit_import_stmt()`:

```python
def visit_import_stmt(self, node: ImportStmtNode) -> None:
    path = node.module_path
    
    # Detect Python module vs Helen/data file
    # Python modules: no extension, or .py, or dotted names like "os.path"
    # Helen/data files: .helen, .json, .md, .txt, .yaml, .yml
    is_python_module = (
        path.endswith('.py') or
        not any(path.endswith(ext) for ext in ('.helen', '.json', '.md', '.txt', '.yaml', '.yml'))
    )
    
    if is_python_module:
        # Register alias as variable
        alias = node.alias if node.alias else path.split('.')[-1]
        from helen.semantic.symbols import Symbol
        sym = Symbol(alias, kind="import", is_const=False)
        self.symbols.define(alias, sym)
        return
    
    # ... handle Helen/data file imports
```

**Logic**:
- If path ends with `.py` → Python module
- If path doesn't end with any Helen/data extension → Python module
- Otherwise → Helen/data file

This handles:
- `"math"` → Python module (no extension)
- `"os.path"` → Python module (dotted, no extension)
- `"numpy"` → Python module (no extension)
- `"./utils.helen"` → Helen file
- `"./config.json"` → Data file

## Interpreter Integration

In `helen/interpreter/interpreter.py`:

### 1. Import execution

```python
def visit_import_stmt(self, node: ImportStmtNode) -> object:
    path = node.module_path
    is_python_module = (
        path.endswith('.py') or
        not any(path.endswith(ext) for ext in ('.helen', '.json', '.md', '.txt', '.yaml', '.yml'))
    )
    
    if is_python_module:
        return self._import_python_module(node)
    
    # ... handle Helen/data file imports

def _import_python_module(self, node: ImportStmtNode) -> object:
    from helen.ffi.python_runtime import DefaultPythonRuntime
    
    # Get or create Python runtime (lazy initialization)
    if not hasattr(self, '_python_runtime'):
        self._python_runtime = DefaultPythonRuntime()
    
    # Import the module
    module_name = node.module_path
    if module_name.endswith('.py'):
        module_name = module_name[:-3]  # Remove .py extension
    
    try:
        module = self._python_runtime.import_module(module_name)
        
        # Register the module under the alias (or module name if no alias)
        alias = node.alias if node.alias else module_name.split('.')[-1]
        self.environment.define(alias, module)
        
    except ImportError as e:
        self._runtime_error(node.span, f"Cannot import Python module '{module_name}': {e}")
        return None
    
    return None
```

### 2. Function call handling

In `visit_call()`, add check for `WrappedPythonObject`:

```python
def visit_call(self, node: CallNode) -> object:
    # ... existing code to get callee and args ...
    
    # Check if callee is a Python FFI object
    from helen.ffi.python_object import WrappedPythonObject
    if isinstance(callee, WrappedPythonObject):
        return callee.call(*args)
    
    # ... rest of call handling
```

## Test Patterns

### Type conversion tests

```python
def test_helen_list_to_python(self):
    converter = DefaultTypeConverter()
    result = converter.helen_to_python([1, 2, 3])
    assert result == [1, 2, 3]
    assert isinstance(result, list)

def test_python_complex_object_wrapped(self):
    converter = DefaultTypeConverter()
    class CustomClass:
        def __init__(self):
            self.value = 42
    obj = CustomClass()
    result = converter.python_to_helen(obj)
    assert hasattr(result, 'unwrap')
    assert result.unwrap() is obj
```

### Object wrapper tests

```python
def test_call_function(self):
    def add(a, b):
        return a + b
    wrapper = WrappedPythonObject(add)
    result = wrapper.call(2, 3)
    assert result == 5

def test_get_attribute(self):
    class TestClass:
        def __init__(self):
            self.value = 42
    obj = TestClass()
    wrapper = WrappedPythonObject(obj)
    assert wrapper.get_attribute("value") == 42
```

### Integration tests

```python
def test_import_python_module(self):
    source = '''
    import "math" as math
    main {
        let result = math.sqrt(16)
        print(result)
    }
    '''
    errors = ErrorReporter()
    scanner = Scanner(source=source, file="<test>")
    tokens = scanner.scan_all()
    parser = Parser(tokens, errors=errors)
    program = parser.parse()
    
    analyzer = SemanticAnalyzer(errors)
    analyzer.analyze(program)
    
    interp = Interpreter(errors=errors)
    interp.interpret(program)
    
    assert not errors.has_errors
```

## Usage Examples

### Basic math operations

```helen
import std.core.*
import std.math.*
import "math" as math

main {
    let sqrt_result = math.sqrt(16)
    print(sqrt_result)  // 4.0
    
    let pi = math.pi
    print(pi)  // 3.141592653589793
    
    let power = math.pow(2, 10)
    print(power)  // 1024.0
}
```

### JSON processing

```helen
import std.core.*
import "json" as json

main {
    let data = {"name": "Alice", "age": 30}
    let json_str = json.dumps(data)
    print(json_str)  // {"name": "Alice", "age": 30}
    
    let parsed = json.loads('{"x": 1, "y": 2}')
    print(parsed["x"])  // 1
}
```

### Nested modules

```helen
import std.core.*
import std.str.*
import "os.path" as path

main {
    let joined = path.join("a", "b", "c")
    print(joined)  // a/b/c
    
    let ext = path.splitext("file.txt")
    print(ext)  // ["file", ".txt"]
}
```

## Common Pitfalls

1. **Circular imports**: `type_converter.py` must lazily import `WrappedPythonObject`
2. **Module detection**: Use negative check for Helen/data extensions, not positive check for Python
3. **Nested module aliases**: Use last part of dotted name as default alias
4. **Type conversion recursion**: Lists and dicts must convert recursively
5. **Module caching**: Cache imported modules to avoid re-importing
6. **Function call integration**: Must check for `WrappedPythonObject` in `visit_call()`

## Performance Considerations

- Module imports are cached (first import is slow, subsequent are fast)
- Type conversion adds overhead for complex objects
- Wrapped objects add indirection for attribute access
- For hot paths, consider unwrapping to native Python objects

## Future Enhancements

- Support for Python class instantiation: `let obj = MyClass(args)`
- Support for Python iterators: `for item in python_list { ... }`
- Support for Python context managers: `with open_file("path") as f { ... }`
- Type hints integration for better IDE support
- Automatic conversion of numpy arrays to Helen lists (optional)
