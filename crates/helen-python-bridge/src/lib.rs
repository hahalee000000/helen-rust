//! M11 — Python Bridge (Python → Helen).
//!
//! PyO3 extension exposing Helen agents/functions to Python. Built by maturin
//! as `helen_rust._core` (see `pyproject.toml`); the pure-Python shim package
//! (`python/helen_rust/`) adds the import hook, decorators, and async support
//! on top of this module.
//!
//! Public API (Task 11.1):
//! - `load_agent(file_path, agent_name) -> PyAgent`
//! - `load_function(file_path, fn_name) -> PyFunction`
//! - `parse_check(source) -> list[str]` (semantic error codes)
//! - `eval_helen(source, globals) -> object`
//! - `describe_file(file_path) -> list[(kind, name, description)]` (import hook)

mod agent_wrapper;
mod convert;
mod function_wrapper;
mod loader;

use helen_core::ast::Stmt;
use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::value::Value;
use helen_parser::Parser;
use helen_semantic::analyze_codes;
use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods};

/// Load a Helen agent as a Python-callable object.
#[pyfunction]
pub fn load_agent(file_path: &str, agent_name: &str) -> PyResult<agent_wrapper::PyAgent> {
    agent_wrapper::PyAgent::load(file_path, agent_name)
}

/// Load a Helen function as a Python-callable object.
#[pyfunction]
pub fn load_function(file_path: &str, fn_name: &str) -> PyResult<function_wrapper::PyFunction> {
    function_wrapper::PyFunction::load(file_path, fn_name)
}

/// Semantic error codes for a Helen source string (IDE support).
///
/// Returns `["E0100", ...]` in emission order. Parse failures raise
/// `RuntimeError` with the parser messages (no codes are produced).
#[pyfunction]
pub fn parse_check(source: &str) -> PyResult<Vec<String>> {
    let mut scanner = Scanner::new(source, "<check>");
    let tokens = scanner.scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    if !parser.errors().is_empty() {
        let msgs = parser
            .errors()
            .iter()
            .map(|e| e.message().to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(PyRuntimeError::new_err(format!("Failed to parse: {msgs}")));
    }
    Ok(analyze_codes(&program))
}

/// Evaluate a Helen source snippet with a `globals` dict; returns the last
/// expression/statement value converted to Python.
#[pyfunction]
pub fn eval_helen(py: Python<'_>, source: &str, globals: &Bound<'_, PyDict>) -> PyResult<PyObject> {
    let mut scanner = Scanner::new(source, "<eval>");
    let tokens = scanner.scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    if !parser.errors().is_empty() {
        let msgs = parser
            .errors()
            .iter()
            .map(|e| e.message().to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(PyRuntimeError::new_err(format!("Failed to parse: {msgs}")));
    }
    let mut interp = Interpreter::new();
    for (k, v) in globals.iter() {
        let key: String = k
            .extract()
            .map_err(|_| PyTypeError::new_err("globals keys must be strings"))?;
        let hv = convert::python_to_helen(py, &v);
        interp.environment.borrow_mut().define(&key, hv, false);
    }
    // Single expression statement: evaluate and return its value (the Rust
    // interpreter's `Stmt::Expr` discards the value — documented M3
    // divergence from Python `interpret`, which returns it).
    if program.statements.len() == 1 {
        if let Stmt::Expr(e) = &program.statements[0] {
            let v = interp
                .eval_expr(&e.expression)
                .map_err(|e| loader::exception_to_pyerr(&e))?;
            return convert::helen_to_python(py, &v);
        }
    }
    let result = interp
        .interpret(&program)
        .map_err(|e| loader::exception_to_pyerr(&e))?;
    convert::helen_to_python(py, &result.unwrap_or(Value::Null))
}

/// List declarations in a Helen file: `[(kind, name, description), ...]`
/// (kind is `"agent"` or `"function"`) in source order. Used by the Python
/// import hook to inject module attributes.
#[pyfunction]
pub fn describe_file(file_path: &str) -> PyResult<Vec<(String, String, String)>> {
    let loaded = loader::LoadedProgram::load(file_path)?;
    // Fresh interpreter registers all declarations; iterate the *program
    // statements* for deterministic source order (Python iterates
    // `program.statements`).
    let interp = loaded
        .new_interpreter()
        .map_err(|e| loader::exception_to_pyerr(&e))?;
    let mut out = Vec::new();
    for stmt in &loaded.program.statements {
        match stmt {
            Stmt::AgentDecl(a) => {
                out.push((
                    "agent".to_string(),
                    a.name.clone(),
                    loader::agent_description(a),
                ));
            }
            Stmt::FunctionDecl(f) => {
                out.push(("function".to_string(), f.name.clone(), String::new()));
            }
            _ => {}
        }
    }
    drop(interp);
    Ok(out)
}

/// The `helen_rust._core` extension module.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(load_agent, m)?)?;
    m.add_function(wrap_pyfunction!(load_function, m)?)?;
    m.add_function(wrap_pyfunction!(parse_check, m)?)?;
    m.add_function(wrap_pyfunction!(eval_helen, m)?)?;
    m.add_function(wrap_pyfunction!(describe_file, m)?)?;
    m.add_class::<agent_wrapper::PyAgent>()?;
    m.add_class::<function_wrapper::PyFunction>()?;
    Ok(())
}
