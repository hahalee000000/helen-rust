# M11 — Python Bridge (Python → Helen)

**Objective:** Reimplement `helen/python_bridge/*` so Python code can `from translator import TranslatorAgent` and call Helen agents like Python classes. Strategy: a **PyO3 cdylib** extension (`helen_bridge`) built with **maturin** + a thin pure-Python import hook (D9).

## Files

```
crates/helen-python-bridge/Cargo.toml     # [lib] crate-type = ["cdylib", "rlib"]; pyo3 0.23
crates/helen-python-bridge/pyproject.toml # [build-system] maturin; project name = helen-rust-bridge
crates/helen-python-bridge/src/lib.rs     // #[pymodule] helen_bridge
crates/helen-python-bridge/src/agent_wrapper.rs    // PyAgent wrapper
crates/helen-python-bridge/src/function_wrapper.rs // PyFunction wrapper
crates/helen-python-bridge/src/convert.rs          // Python <-> Value (bridge direction)
crates/helen-python-bridge/python/helen_bridge/import_hook.py  // meta-path finder (pure Python)
```

## Task 11.1: Bridge core (`#[pymodule]`)

Expose to Python:
- `helen_bridge.load_agent(file_path: str, agent_name: str) -> PyAgent`
- `helen_bridge.load_function(file_path: str, fn_name: str) -> PyFunction`
- `helen_bridge.parse_check(source: str) -> list[str]` (semantic error codes, for IDE)
- `helen_bridge.eval_helen(source: str, globals: dict) -> object`

Each `PyAgent`/`PyFunction` holds `Mutex<Box<HelenRuntime>>` where `HelenRuntime` owns a fresh `Interpreter` + resolved AST (cache the parsed AST; re-parse if file mtime changed — port the ImportResolver cache caveat: **fresh Interpreter per call or explicit cache-clear API**, mirroring Python's documented behavior).

## Task 11.2: Agent wrapper (port `agent_wrapper.py`)

- `PyAgent` implements `#[pyo3(signature = (*args, **kwargs))] fn __call__`.
- **Parameter validation**: positional/keyword mixing, missing-required → `TypeError`, unknown kwarg → `TypeError`, wrong count → `TypeError` (messages match Python wrapper).
- `async_call(*args, **kwargs)`: run the sync interpreter in `tokio::task::spawn_blocking` (bridge crate can use tokio); return via `PyFuture`.
- `__str__`/`__repr__` with agent description.

## Task 11.3: Function wrapper + decorators

`PyFunction`: wraps Helen `fn` for direct Python calls (port `function_wrapper.py`). `@helen_agent(file="x.helen", name="Agent")` decorator (port `decorators.py`) — implemented in the Python shim importing from `helen_bridge`.

## Task 11.4: Import hook (port `import_hook.py`)

Pure-Python meta-path finder: intercept imports whose module name resolves to a sibling `.helen` file (same dir as the importing module, plus `PYTHONPATH`), parse with `helen_bridge`, and inject classes: `__getattr__` on the module returns the agent class (construct → `PyAgent`). Port edge cases: missing file → `ModuleNotFoundError`, parse errors → `SyntaxError` with line info, `.helen` modules without requested agent → `AttributeError`.

## Task 11.5: Type conversion (port `type_converter.py` bridge direction)

Python → Helen: primitives, list, dict, None, and passthrough of `PyObject`-wrapped natives. Helen → Python: `Value` → Python types (int/float/str/bool/list/dict/None); `PythonObject` handles unwrap back to the original `PyObject` (identity-preserving).

## Task 11.6: Packaging + tests

```bash
cd crates/helen-python-bridge && maturin develop --release   # install into venv
# and for users: pip install helen-rust-bridge  (maturin build --release → wheels)
```

Port `tests/ffi/test_python_bridge*` and `wiki/reference/15-python-bridge.md` tutorial examples as pytest. CI job: install bridge into a Python 3.12 venv, run the tutorial's `translator.helen` end-to-end.

## Definition of Done — M11

- [ ] `from translator import TranslatorAgent; agent("Hello","French")` works (sync + keyword args).
- [ ] `await agent.async_call(...)` works.
- [ ] Missing args / unknown kwargs raise Python `TypeError` with the same messages as the Python bridge.
- [ ] Import-hook edge cases pass (`ModuleNotFoundError`, `SyntaxError`, `AttributeError`).
- [ ] Wheel builds via maturin; installed package passes the tutorial suite.
