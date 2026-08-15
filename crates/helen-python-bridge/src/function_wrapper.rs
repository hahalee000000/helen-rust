//! PyFunction — a Helen function callable from Python (Task 11.3).
//!
//! Port of `helen/python_bridge/function_wrapper.py`
//! (`HelenFunctionWrapper.__call__`): positional args fill the parameter list
//! by index, kwargs fill remaining slots; error messages match the Python
//! wrapper exactly.

use std::sync::Arc;

use helen_interpreter::value::Value;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyTuple};

use crate::convert::{helen_to_python, python_to_helen};
use crate::loader::{exception_to_pyerr, LoadedProgram};

/// A Helen function callable from Python (fresh interpreter per call).
#[pyclass]
pub struct PyFunction {
    name: String,
    file: String,
    program: Arc<LoadedProgram>,
    func_name: String,
    param_names: Vec<String>,
}

impl PyFunction {
    pub fn load(file: &str, func_name: &str) -> PyResult<Self> {
        let loaded = LoadedProgram::load(file)?;
        let interp = loaded
            .new_interpreter()
            .map_err(|e| exception_to_pyerr(&e))?;
        let decl = interp.functions.get(func_name).cloned().ok_or_else(|| {
            PyValueError::new_err(format!("Function '{func_name}' not found in {file}"))
        })?;
        let param_names = decl.params.iter().map(|p| p.name.clone()).collect();
        drop(interp);
        Ok(PyFunction {
            name: func_name.to_string(),
            file: loaded.file.clone(),
            program: Arc::new(loaded),
            func_name: func_name.to_string(),
            param_names,
        })
    }
}

#[pymethods]
impl PyFunction {
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    fn helen_file(&self) -> String {
        self.file.clone()
    }

    fn __repr__(&self) -> String {
        format!("<HelenFunction '{}' from {}>", self.name, self.file)
    }

    /// Port of `HelenFunctionWrapper.__call__`.
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let n_params = self.param_names.len();
        let mut call_args: Vec<Value> = Vec::new();

        // Positional args.
        for (i, arg) in args.iter().enumerate() {
            if i >= n_params {
                return Err(PyTypeError::new_err(format!(
                    "{}() takes {} positional arguments but {} were given",
                    self.name,
                    n_params,
                    args.len()
                )));
            }
            call_args.push(python_to_helen(py, &arg));
        }

        // Keyword args.
        if let Some(kw) = kwargs {
            for (k, v) in kw.iter() {
                let key: String = k
                    .extract()
                    .map_err(|_| PyTypeError::new_err("keyword arguments must be strings"))?;
                let idx = match self.param_names.iter().position(|n| n == &key) {
                    Some(i) => i,
                    None => {
                        return Err(PyTypeError::new_err(format!(
                            "{}() got an unexpected keyword argument '{}'",
                            self.name, key
                        )))
                    }
                };
                if idx < call_args.len() {
                    return Err(PyTypeError::new_err(format!(
                        "{}() got multiple values for argument '{}'",
                        self.name, key
                    )));
                }
                // Pad with None (Python wrapper pads with None too).
                while call_args.len() <= idx {
                    call_args.push(Value::Null);
                }
                call_args[idx] = python_to_helen(py, &v);
            }
        }

        // Fresh interpreter + call.
        let mut interp = self
            .program
            .new_interpreter()
            .map_err(|e| exception_to_pyerr(&e))?;
        let decl = interp
            .functions
            .get(&self.func_name)
            .cloned()
            .ok_or_else(|| {
                PyValueError::new_err(format!(
                    "Function '{}' not found in {}",
                    self.func_name, self.file
                ))
            })?;
        // v1.39: pass the function's module env so imported functions can
        // access their own module's stdlib imports (Python
        // `_function_module_envs`).
        let parent_env = interp.function_module_envs.get(&self.func_name).cloned();
        let span = helen_core::source::SourceSpan::new(self.file.clone(), 0, 0, 0, 0);
        let result = interp
            .call_function(&decl, call_args, parent_env, &span)
            .map_err(|e| exception_to_pyerr(&e))?;
        helen_to_python(py, &result)
    }
}
