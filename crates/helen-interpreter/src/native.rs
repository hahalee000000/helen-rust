//! Native object handles (Task 10.1) — FFI bridge abstraction.
//!
//! `Value::Native(NativeHandle)` lets Helen programs hold foreign objects
//! (Python objects via PyO3, etc.) without the interpreter depending on any
//! FFI crate. `helen-ffi` implements [`NativeObject`] for its PyO3 wrappers
//! and registers a Python import hook here so `import "math" as m` works.
//!
//! Port of the `helen/ffi/*` contract surface: `PythonObject`,
//! `PythonModule`, `PythonRuntime`, `TypeConverter`.

use std::sync::OnceLock;

use crate::exceptions::ExceptionValue;
use crate::value::Value;

/// A native (foreign-language) object surfaced as a Helen `Value`.
///
/// The interpreter dispatches attribute access / calls / item access / str()
/// through this trait. Implementations must be `Send + Sync` (the FFI crate
/// wraps PyO3 objects in a `Mutex` for `Send`).
pub trait NativeObject: Send + Sync + 'static {
    /// Python `type(obj).__name__` (used by `type()` builtin + errors).
    fn type_name(&self) -> String;
    /// Python `str(obj)`.
    fn python_str(&self) -> String;
    /// Python `repr(obj)`.
    fn python_repr(&self) -> String;
    /// `getattr(obj, name)` → converted Helen value.
    fn get_attribute(&self, name: &str) -> Result<Value, ExceptionValue>;
    /// `obj(*args, **kwargs)` (or method-by-name dispatch for non-callable
    /// instances — Python `WrappedPythonObject.call` pattern 2).
    fn call(&self, args: &[Value], kwargs: &[(String, Value)]) -> Result<Value, ExceptionValue>;
    /// `obj[key]`.
    fn get_item(&self, key: &Value) -> Result<Value, ExceptionValue>;
    /// `obj[key] = value`.
    fn set_item(&self, key: &Value, value: &Value) -> Result<(), ExceptionValue>;
    /// The underlying foreign object, if extractable (unwrap).
    fn unwrap_object(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        None
    }
    /// Type-erased self (for downcasting to concrete FFI wrappers).
    /// Implementations must return `self` (concrete `&T` coerced to `&dyn Any`).
    fn as_any(&self) -> &dyn std::any::Any;
}

impl NativeHandle {
    /// Downcast the wrapped object to a concrete `NativeObject` impl.
    pub fn downcast_ref<T: NativeObject + 'static>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref::<T>()
    }
}

/// A Helen value wrapping a native object.
#[derive(Clone)]
pub struct NativeHandle(pub std::sync::Arc<dyn NativeObject>);

impl std::fmt::Debug for NativeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeHandle")
    }
}

/// Python import hook — registered by `helen-ffi` when the `python-ffi`
/// feature is compiled in (Task 10.5). The interpreter stays PyO3-free; the
/// FFI crate installs this hook at init.
type PythonImportHook = Box<dyn Fn(&str) -> Result<Value, String> + Send + Sync>;

static PYTHON_IMPORT_HOOK: OnceLock<PythonImportHook> = OnceLock::new();

/// Register the Python import hook (called by `helen-ffi`).
#[allow(clippy::result_unit_err)]
pub fn register_python_import_hook(hook: PythonImportHook) -> Result<(), ()> {
    PYTHON_IMPORT_HOOK.set(hook).map_err(|_| ())
}

/// The registered Python import hook, if any.
pub fn python_import_hook() -> Option<&'static PythonImportHook> {
    PYTHON_IMPORT_HOOK.get()
}

/// Create a Helen `RuntimeError` exception value (Python exceptions map to
/// `RuntimeError` — Helen has no Python-named exception classes).
pub fn runtime_error_exception(message: String) -> ExceptionValue {
    ExceptionValue::new("RuntimeError", message, None)
}
