# M10 — Python FFI (Helen → Python)

**Objective:** Reimplement `helen/ffi/*` so Helen programs can `import "numpy" as np` / `import "requests" as req` and call Python from the Rust interpreter. Strategy: **embed CPython via PyO3** (D9).

## Files

```
crates/helen-ffi/Cargo.toml            # pyo3 = { version = "0.23", features = ["auto-initialize"] }
crates/helen-ffi/src/lib.rs            // gate behind feature "python-ffi" (default off)
crates/helen-ffi/src/runtime.rs        // PythonRuntime: GIL, import, dispatch
crates/helen-ffi/src/module.rs         // PythonModule value
crates/helen-ffi/src/object.rs         // PythonObject value
crates/helen-ffi/src/converter.rs      // TypeConverter both directions
```

This crate is **optional** (not in the default workspace build): pure-Rust users don't need Python headers. `cargo build -p helen-ffi --features python-ffi` builds it.

## Task 10.1: Value integration

Add a `Value::PyObject(PyObjectHandle)` variant — but keep `helen-interpreter` **free of pyo3**. Solution: define a **native handle** abstraction in `helen-interpreter`:

```rust
// helen-interpreter/src/native.rs
pub trait NativeObject: std::any::Any + Send + Sync { fn type_name(&self) -> &'static str; }
pub struct NativeHandle(pub Arc<dyn NativeObject>);   // Value::Native(NativeHandle)
```

`helen-ffi` implements `NativeObject` for PyO3 `PyObject` (wrapped in `Mutex` for Send). The interpreter calls `NativeObject`-typed methods (attribute/item/call/str) through the handle. This keeps the core portable while FFI provides Python semantics.

## Task 10.2: FFI runtime (port `ffi/python_runtime.py`)

```rust
pub struct PyRuntime;  // Python::with_gil everywhere; port init/lifecycle
impl PyRuntime {
  pub fn import_module(&self, name: &str) -> Result<PythonModule, ExceptionValue>;
  pub fn eval(&self, code: &str) -> Result<Value, ExceptionValue>;   // if Python eval exists in v1.44
}
```

GIL safety: never hold `Python` across interpreter boundaries; convert to/from Helen `Value` inside the GIL scope.

## Task 10.3: TypeConverter (port `ffi/type_converter.py`)

Helen → Python: `Int/Float/Bool/Str → PyInt/PyFloat/PyBool/PyStr`; `List → PyList` (recursively, with `clone_deep`), `Map → PyDict`; `Null → None`; `NativeHandle(PyObject) → same PyObject` (no copy). Python → Helen: int → Int (arbitrary precision, D3 — no overflow path), float → Float, str → Str, bool → Bool, None → Null, list/tuple → List (deep convert), dict → Map, **anything else → PythonObject wrapper** (recursively referenced).

## Task 10.4: PythonModule / PythonObject (port `python_module.py`, `python_object.py`, `contracts.py`)

- `PythonModule(name)`: attribute access `np.array` → converts result via converter; method calls with positional/keyword args.
- `PythonObject`: `get_attribute`, `call`, `get_item`, `set_item`, `str()` repr, `unwrap()`; **callable objects** (`np.zeros(...)`) and **indexable objects** (`obj[0]`, `dict["key"]`) surface as `Value::Callable`/`Value::Indexable` native handles so Helen syntax `np.zeros(3)` and `arr[0]` work.
- `_contracts.py` is documentation — no runtime code, but keep a `CONTRACTS.md` parity note.

## Task 10.5: Import hook wiring

In `helen-interpreter/src/import.rs`: when `import "numpy" as np` fails to resolve as a Helen module, fall back to the FFI runtime (if the `python-ffi` feature is compiled in). Mirror Python's import priority (Helen first, then Python).

## Task 10.6: Tests (port `tests/ffi/`, 4 files)

- `import "math" as m; m.sqrt(16)` → 4.0
- `import "numpy" as np; np.array([1,2,3])` round-trip
- Dict/list deep conversion round-trips; object repr/str
- Callable + indexable access; exception mapping (Python exceptions are **wrapped as Helen `RuntimeError`** — Helen has no Python-named exception classes; matches stdlib behavior like `int("abc")`)

## Definition of Done — M10

- [ ] `examples/python_bridge/*.helen` FFI examples run on the Rust interpreter.
- [ ] `tests/ffi` parity (4 files) green.
- [ ] Optional feature compiles cleanly; workspace default build unaffected.
- [ ] Custom LLM provider loader (M5.3) works through this crate.
