//! PythonModule (Task 10.4) — port of `helen/ffi/python_module.py`.
//!
//! Wraps an imported Python module as a Helen `NativeObject`: attribute
//! access converts the attribute via the type converter.

use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::native::NativeObject;
use helen_interpreter::value::Value;
use pyo3::types::PyAnyMethods;

use crate::converter::python_to_helen;

/// Wrapper for Python modules accessible from Helen.
pub struct PythonModule {
    name: String,
    module: pyo3::Py<pyo3::PyAny>,
}

impl PythonModule {
    pub fn new(name: String, module: pyo3::Py<pyo3::PyAny>) -> Self {
        PythonModule { name, module }
    }
}

impl NativeObject for PythonModule {
    fn type_name(&self) -> String {
        "module".to_string()
    }

    fn python_str(&self) -> String {
        // Python: WrappedPythonModule has no __str__ → repr is used.
        format!("WrappedPythonModule({:?})", self.name)
    }

    fn python_repr(&self) -> String {
        format!("WrappedPythonModule({:?})", self.name)
    }

    #[allow(clippy::result_large_err)]
    fn get_attribute(&self, name: &str) -> Result<Value, ExceptionValue> {
        pyo3::Python::with_gil(|py| {
            let module = self.module.bind(py);
            // Python `getattr(self._module, name)` → wrapped.
            match module.getattr(name) {
                Ok(attr) => Ok(python_to_helen(py, &attr)),
                Err(e) => Err(crate::object::ffi_error(&e)),
            }
        })
    }

    fn call(&self, _args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, ExceptionValue> {
        // Python: modules are not callable → TypeError.
        Err(crate::object::not_callable_error("module"))
    }

    fn get_item(&self, _key: &Value) -> Result<Value, ExceptionValue> {
        Err(crate::object::not_callable_error("module"))
    }

    fn set_item(&self, _key: &Value, _value: &Value) -> Result<(), ExceptionValue> {
        Err(crate::object::not_callable_error("module"))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn unwrap_object(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        let module = pyo3::Python::with_gil(|py| self.module.clone_ref(py));
        Some(std::sync::Arc::new(module))
    }
}
