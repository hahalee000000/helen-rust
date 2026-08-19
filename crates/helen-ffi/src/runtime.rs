//! PythonRuntime (Task 10.2) — port of `helen/ffi/python_runtime.py`.
//!
//! Manages Python module loading and an execution context. The runtime is
//! a process-global singleton; module imports are cached. GIL is acquired
//! per operation (never held across interpreter boundaries).

use std::collections::HashMap;
use std::sync::Mutex;

use helen_interpreter::native::NativeHandle;
use helen_interpreter::value::Value;

use crate::converter::python_to_helen;
use crate::module::PythonModule;
use pyo3::types::PyDictMethods;

/// Shared, process-global Python runtime (modules are process singletons).
pub struct PythonRuntime {
    modules: Mutex<HashMap<String, pyo3::Py<pyo3::PyAny>>>,
    /// Execution context for eval/exec (Python `self._context` dict).
    context: Mutex<pyo3::Py<pyo3::types::PyDict>>,
}

impl PythonRuntime {
    /// Initialize the Python interpreter (idempotent — `with_gil`
    /// auto-initializes). Module cache + eval context are fresh.
    pub fn new() -> Result<Self, String> {
        pyo3::prepare_freethreaded_python();
        let context = pyo3::Python::with_gil(|py| pyo3::types::PyDict::new(py).unbind());
        Ok(PythonRuntime {
            modules: Mutex::new(HashMap::new()),
            context: Mutex::new(context),
        })
    }

    /// Import a Python module, returning a Helen `Value::Native` wrapping
    /// the module (cached — same wrapper for repeated imports).
    ///
    /// Python parity: `ImportError` on failure with message
    /// `Cannot import module '{name}': {e}`.
    pub fn import_module(&self, module_name: &str) -> Result<Value, String> {
        if let Some(cached) = self.modules.lock().expect("mutex poisoned").get(module_name) {
            let obj = pyo3::Python::with_gil(|py| cached.clone_ref(py));
            let module = PythonModule::new(module_name.to_string(), obj);
            return Ok(Value::Native(NativeHandle(std::sync::Arc::new(module))));
        }
        let result = pyo3::Python::with_gil(|py| {
            match py.import(module_name) {
                Ok(m) => {
                    let obj = m.into_any().unbind();
                    // Bind into the eval context under the last dotted name.
                    let var_name = module_name.split('.').next_back().unwrap_or(module_name);
                    if let Ok(dict) = self.context.lock() {
                        let _ = dict.bind(py).set_item(var_name, obj.clone_ref(py));
                    }
                    Ok(obj)
                }
                Err(e) => Err(format!("Cannot import module '{module_name}': {}", e)),
            }
        });
        match result {
            Ok(obj) => {
                let obj_ref = pyo3::Python::with_gil(|py| obj.clone_ref(py));
                self.modules
                    .lock()
                    .unwrap()
                    .insert(module_name.to_string(), obj_ref);
                let module = PythonModule::new(module_name.to_string(), obj);
                Ok(Value::Native(NativeHandle(std::sync::Arc::new(module))))
            }
            Err(e) => Err(e),
        }
    }

    /// Evaluate a Python expression (Python `eval_expression`).
    pub fn eval_expression(&self, expression: &str) -> Result<Value, String> {
        pyo3::Python::with_gil(|py| {
            let guard = self.context.lock().expect("mutex poisoned");
            let dict = guard.bind(py);
            match py.eval(
                &std::ffi::CString::new(expression).unwrap(),
                Some(dict),
                None,
            ) {
                Ok(r) => Ok(python_to_helen(py, &r)),
                Err(e) => Err(e.to_string()),
            }
        })
    }

    /// Execute a Python statement (Python `exec_statement`).
    pub fn exec_statement(&self, statement: &str) -> Result<(), String> {
        pyo3::Python::with_gil(|py| {
            let guard = self.context.lock().expect("mutex poisoned");
            let dict = guard.bind(py);
            match py.run(
                &std::ffi::CString::new(statement).unwrap(),
                Some(dict),
                None,
            ) {
                Ok(()) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        })
    }

    /// Get the underlying module object for a previously-imported module.
    pub fn get_module_object(&self, module_name: &str) -> Option<pyo3::Py<pyo3::PyAny>> {
        let guard = self.modules.lock().expect("mutex poisoned");
        pyo3::Python::with_gil(|py| guard.get(module_name).map(|c| c.clone_ref(py)))
    }
}

/// Install the Python import hook into the interpreter's global registry.
/// After this, `import "math" as m` in Helen programs resolves via Python.
pub fn install_python_hook(runtime: &'static PythonRuntime) -> Result<(), String> {
    helen_interpreter::native::register_python_import_hook(Box::new(move |name: &str| {
        runtime.import_module(name)
    }))
    .map_err(|_| "Python import hook already registered".to_string())
}
