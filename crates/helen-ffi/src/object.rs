//! PythonObject (Task 10.4) — port of `helen/ffi/python_object.py`.
//!
//! Wraps an arbitrary Python object as a Helen `NativeObject`: attribute
//! access, calls (both the direct-callable and method-by-name patterns),
//! item access, `str()`/`repr()`, and `unwrap()`.

use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::native::NativeObject;
use helen_interpreter::value::Value;
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyStringMethods, PyTuple, PyTypeMethods};

use crate::converter::{helen_to_python, python_to_helen};

/// Wrapper for Python objects accessible from Helen.
pub struct PythonObject {
    obj: pyo3::Py<pyo3::PyAny>,
}

impl PythonObject {
    pub fn new(obj: pyo3::Py<pyo3::PyAny>) -> Self {
        PythonObject { obj }
    }
}

impl NativeObject for PythonObject {
    fn type_name(&self) -> String {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            // Python `type(self._obj).__name__` via the type's `__name__`.
            let t = obj.get_type();
            t.name()
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "object".to_string())
        })
    }

    fn python_str(&self) -> String {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            // Python `str(obj)`.
            match obj.str() {
                Ok(s) => s.to_string_lossy().into_owned(),
                Err(_) => format!("<{} object>", self.type_name()),
            }
        })
    }

    fn python_repr(&self) -> String {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            // Python `WrappedPythonObject({obj!r})`.
            match obj.repr() {
                Ok(r) => format!("WrappedPythonObject({})", r.to_string_lossy()),
                Err(_) => format!("WrappedPythonObject(<{} object>)", self.type_name()),
            }
        })
    }

    #[allow(clippy::result_large_err)]
    fn get_attribute(&self, name: &str) -> Result<Value, ExceptionValue> {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            // Python `getattr(obj, name)` → wrapped.
            match obj.getattr(name) {
                Ok(attr) => Ok(python_to_helen(py, &attr)),
                Err(e) => Err(ffi_error(&e)),
            }
        })
    }

    #[allow(clippy::result_large_err)]
    fn call(&self, args: &[Value], kwargs: &[(String, Value)]) -> Result<Value, ExceptionValue> {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            let py_args: Vec<pyo3::PyObject> = args
                .iter()
                .map(|a| helen_to_python(py, a))
                .collect::<Result<_, _>>()
                .map_err(|e| ffi_error(&e))?;
            let py_kwargs: indexmap::IndexMap<&str, pyo3::PyObject> = kwargs
                .iter()
                .map(|(k, v)| helen_to_python(py, v).map(|pv| (k.as_str(), pv)))
                .collect::<Result<_, _>>()
                .map_err(|e| ffi_error(&e))?;

            if obj.is_callable() {
                // Pattern 1: direct call (wrapped function/class).
                let arg_tuple = PyTuple::new(py, py_args.iter().map(|a| a.bind(py)))
                    .map_err(|e| ffi_error(&e))?;
                let kw_dict = PyDict::new(py);
                for (k, v) in py_kwargs.iter() {
                    kw_dict
                        .set_item(*k, v.bind(py))
                        .map_err(|e| ffi_error(&e))?;
                }
                let result = obj.call(arg_tuple, Some(&kw_dict));
                match result {
                    Ok(r) => Ok(python_to_helen(py, &r)),
                    Err(e) => Err(ffi_error(&e)),
                }
            } else if let (Some((_, rest)), Value::Str(method_name)) =
                (args.split_first(), args.first().unwrap())
            {
                // Pattern 2: method-by-name dispatch for non-callable
                // instances: `instance.call("method", arg1, arg2)`.
                {
                    match obj.getattr(method_name.as_ref()) {
                        Ok(method) => {
                            let method = method.into_any();
                            if method.is_callable() {
                                let py_rest: Vec<pyo3::PyObject> = rest
                                    .iter()
                                    .map(|a| helen_to_python(py, a))
                                    .collect::<Result<_, _>>()
                                    .map_err(|e| ffi_error(&e))?;
                                let rest_tuple =
                                    PyTuple::new(py, py_rest.iter().map(|a| a.bind(py)))
                                        .map_err(|e| ffi_error(&e))?;
                                let kw_dict = PyDict::new(py);
                                for (k, v) in py_kwargs.iter() {
                                    kw_dict
                                        .set_item(*k, v.bind(py))
                                        .map_err(|e| ffi_error(&e))?;
                                }
                                let result = method.call(rest_tuple, Some(&kw_dict));
                                match result {
                                    Ok(r) => Ok(python_to_helen(py, &r)),
                                    Err(e) => Err(ffi_error(&e)),
                                }
                            } else {
                                Err(not_callable_error(&self.type_name()))
                            }
                        }
                        Err(e) => Err(ffi_error(&e)),
                    }
                }
            } else {
                Err(not_callable_error(&self.type_name()))
            }
        })
    }

    #[allow(clippy::result_large_err)]
    fn get_item(&self, key: &Value) -> Result<Value, ExceptionValue> {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            let pk = helen_to_python(py, key).map_err(|e| ffi_error(&e))?;
            match obj.get_item(pk.bind(py)) {
                Ok(v) => Ok(python_to_helen(py, &v)),
                Err(e) => Err(ffi_error(&e)),
            }
        })
    }

    #[allow(clippy::result_large_err)]
    fn set_item(&self, key: &Value, value: &Value) -> Result<(), ExceptionValue> {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            let pk = helen_to_python(py, key).map_err(|e| ffi_error(&e))?;
            let pv = helen_to_python(py, value).map_err(|e| ffi_error(&e))?;
            obj.set_item(pk.bind(py), pv.bind(py))
                .map_err(|e| ffi_error(&e))
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn unwrap_object(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        let obj = pyo3::Python::with_gil(|py| self.obj.clone_ref(py));
        Some(std::sync::Arc::new(obj))
    }
}

/// Convert a PyO3 error into a Helen `RuntimeError` exception value
/// (Python exceptions are wrapped as Helen `RuntimeError` — Helen has no
/// Python-named exception classes; matches `int("abc")` behavior).
pub fn ffi_error(e: &pyo3::PyErr) -> ExceptionValue {
    let msg = e.to_string();
    ExceptionValue::new("RuntimeError", msg, None)
}

pub(crate) fn not_callable_error(type_name: &str) -> ExceptionValue {
    ExceptionValue::new(
        "RuntimeError",
        format!("'{type_name}' object is not callable"),
        None,
    )
}
