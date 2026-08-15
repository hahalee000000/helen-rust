//! PyAgent — a Helen agent callable from Python (Task 11.2).
//!
//! Port of `helen/python_bridge/agent_wrapper.py` (`HelenAgentWrapper`).
//! Parameter validation raises `TypeError` with the Python wrapper's exact
//! messages; the agent runs through a fresh interpreter per call (see
//! [`crate::loader`]).

use std::collections::HashMap;
use std::sync::Arc;

use helen_interpreter::value::Value;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyTuple};

use crate::convert::{helen_to_python, python_to_helen};
use crate::loader::{exception_to_pyerr, LoadedProgram};

/// Metadata for a single agent parameter (Send-safe mirror of `AgentParam`).
struct ParamInfo {
    name: String,
    has_default: bool,
}

/// A Helen agent callable from Python.
///
/// Holds the parsed AST (Send-safe) and builds a fresh interpreter per call —
/// the Rust `Interpreter` is Rc-based and not `Send`, so it cannot be stored
/// in a pyclass; this matches plan 13.1's "fresh Interpreter per call".
#[pyclass]
pub struct PyAgent {
    name: String,
    file: String,
    program: Arc<LoadedProgram>,
    agent_name: String,
    params: Vec<ParamInfo>,
}

impl PyAgent {
    /// Load an agent from a `.helen` file (full parse + semantic + execute
    /// pipeline; see `LoadedProgram::load`).
    pub fn load(file: &str, agent_name: &str) -> PyResult<Self> {
        let loaded = LoadedProgram::load(file)?;
        let interp = loaded
            .new_interpreter()
            .map_err(|e| exception_to_pyerr(&e))?;
        let decl = interp
            .agents
            .get(agent_name)
            .cloned()
            .ok_or_else(|| PyValueError::new_err(format!("Agent '{agent_name}' not found")))?;
        let params = decl
            .params
            .iter()
            .map(|p| ParamInfo {
                name: p.name.clone(),
                has_default: p.default_value.is_some(),
            })
            .collect();
        drop(interp);
        Ok(PyAgent {
            name: agent_name.to_string(),
            file: loaded.file.clone(),
            program: Arc::new(loaded),
            agent_name: agent_name.to_string(),
            params,
        })
    }

    /// Port of `HelenAgentWrapper._validate_args`.
    fn validate(&self, args: &HashMap<String, Value>) -> PyResult<()> {
        let param_names: std::collections::HashSet<&str> =
            self.params.iter().map(|p| p.name.as_str()).collect();
        // Unknown keyword argument.
        for name in args.keys() {
            if !param_names.contains(name.as_str()) {
                return Err(PyTypeError::new_err(format!(
                    "{}() got an unexpected keyword argument '{}'",
                    self.name, name
                )));
            }
        }
        // Missing required argument (no default value).
        for p in &self.params {
            if !args.contains_key(&p.name) && !p.has_default {
                return Err(PyTypeError::new_err(format!(
                    "{}() missing required argument: '{}'",
                    self.name, p.name
                )));
            }
        }
        Ok(())
    }
}

#[pymethods]
impl PyAgent {
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    fn helen_file(&self) -> String {
        self.file.clone()
    }

    fn __repr__(&self) -> String {
        format!("<HelenAgent '{}' from {}>", self.name, self.file)
    }

    /// Port of `HelenAgentWrapper.__call__`: positional args map to params by
    /// index, kwargs are merged (kwargs win on conflict, matching the Python
    /// `helen_args.update(kwargs)`), then args are validated and the agent is
    /// invoked in a fresh interpreter.
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        // 1. Positional args → named.
        let mut helen_args: HashMap<String, Value> = HashMap::new();
        for (i, arg) in args.iter().enumerate() {
            if i >= self.params.len() {
                return Err(PyTypeError::new_err(format!(
                    "{}() takes {} positional arguments but {} were given",
                    self.name,
                    self.params.len(),
                    args.len()
                )));
            }
            helen_args.insert(self.params[i].name.clone(), python_to_helen(py, &arg));
        }

        // 2. Keyword args (merge; kwargs win on conflict like the reference).
        if let Some(kw) = kwargs {
            for (k, v) in kw.iter() {
                let key: String = k
                    .extract()
                    .map_err(|_| PyTypeError::new_err("keyword arguments must be strings"))?;
                helen_args.insert(key.clone(), python_to_helen(py, &v));
            }
        }

        // 3. Validate (TypeError messages match the Python wrapper).
        self.validate(&helen_args)?;

        // 4. Fresh interpreter + call the agent.
        let mut interp = self
            .program
            .new_interpreter()
            .map_err(|e| exception_to_pyerr(&e))?;
        let decl = interp
            .agents
            .get(&self.agent_name)
            .cloned()
            .ok_or_else(|| {
                PyValueError::new_err(format!("Agent '{}' not found", self.agent_name))
            })?;
        let span = helen_core::source::SourceSpan::new(self.file.clone(), 0, 0, 0, 0);
        let result = interp
            .call_agent(&decl, helen_args, &span)
            .map_err(|e| exception_to_pyerr(&e))?;
        helen_to_python(py, &result)
    }
}
