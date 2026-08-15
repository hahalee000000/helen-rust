//! File loading pipeline shared by the agent/function wrappers.
//!
//! Port of the Python bridge's `_load_agent` / `import_hook.exec_module`
//! pipeline (v1.44): Scanner (with file path) → Parser → SemanticAnalyzer
//! (base_dir = file's parent, so relative imports resolve against the .helen
//! file's directory, not the CWD) → Interpreter.
//!
//! M11 deviation (documented in plan 13.1): the Rust bridge keeps the parsed
//! AST and builds a **fresh Interpreter per call** ("fresh Interpreter per
//! call or explicit cache-clear API"). The Rust `Interpreter` is Rc-based and
//! not `Send`, so it cannot live in a pyclass; re-registering declarations
//! per call is cheap and keeps the wrapper fully `Send` (async-safe).

use std::path::Path;
use std::sync::Arc;

use helen_core::ast::{AgentDecl, Program, Stmt};
use helen_core::lexer::Scanner;
use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;
use helen_semantic::analyzer::SemanticAnalyzer;
use helen_semantic::diagnostics::ErrorReporter;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

/// A parsed + semantically-checked Helen program with the main block filtered
/// out (mirrors the Python bridge's "register agent/function/const, do NOT
/// execute main" load behavior).
pub struct LoadedProgram {
    pub file: String,
    pub program: Arc<Program>,
}

impl LoadedProgram {
    /// Full load pipeline. Raises Python `RuntimeError` with the reference
    /// bridge's exact messages:
    ///   `Failed to parse '{file}': {msgs}`
    ///   `Failed to load '{file}': {msgs}`
    ///   `Failed to initialize '{file}': {msgs}`
    pub fn load(file: &str) -> PyResult<LoadedProgram> {
        let path = Path::new(file);
        let source = std::fs::read_to_string(path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to read '{file}': {e}")))?;

        // 1. Lex (file path passed so span.file enables relative-import
        //    resolution and error location).
        let mut scanner = Scanner::new(&source, file);
        let tokens = scanner.scan_all();

        // 2. Parse.
        let mut parser = Parser::new(tokens);
        let program = parser.parse();
        if !parser.errors().is_empty() {
            let msgs = parser
                .errors()
                .iter()
                .map(|e| e.message().to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(PyRuntimeError::new_err(format!(
                "Failed to parse '{file}': {msgs}"
            )));
        }

        // 3. Semantic analysis (base_dir = file's parent).
        let base_dir = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let reporter = ErrorReporter::new();
        let mut analyzer = SemanticAnalyzer::new(reporter, &base_dir);
        analyzer.analyze(&program);
        if analyzer.errors.has_errors() {
            let msgs = analyzer
                .errors
                .errors()
                .iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(PyRuntimeError::new_err(format!(
                "Failed to load '{file}': {msgs}"
            )));
        }

        // 4. Filter out top-level main blocks (bridge loads declarations only).
        let statements: Vec<Stmt> = program
            .statements
            .iter()
            .filter(|s| !matches!(s, Stmt::MainBlock(_)))
            .cloned()
            .collect();
        let filtered = Program {
            statements,
            span: program.span.clone(),
        };

        // 5. Execute top-level declarations in a scratch interpreter so load
        //    failures (import errors, top-level runtime errors) surface here
        //    with the reference message ("Failed to initialize").
        let mut interp = Interpreter::new();
        interp.set_source_file(file);
        if let Err(e) = interp.interpret(&filtered) {
            return Err(PyRuntimeError::new_err(format!(
                "Failed to initialize '{file}': {}",
                exception_message(&e)
            )));
        }

        Ok(LoadedProgram {
            file: file.to_string(),
            program: Arc::new(filtered),
        })
    }

    /// Build a fresh interpreter with the program registered (per-call).
    #[allow(clippy::result_large_err)] // Python bridge parity: propagate the Helen exception
    pub fn new_interpreter(&self) -> Result<Interpreter, ExceptionValue> {
        let mut interp = Interpreter::new();
        interp.set_source_file(&self.file);
        interp.interpret(&self.program)?;
        Ok(interp)
    }
}

/// Human-readable exception message (Python `RuntimeError: {e}` parity).
pub fn exception_message(e: &ExceptionValue) -> String {
    if e.message.is_empty() {
        ExceptionValue::default_message(&e.class_name)
    } else {
        e.message.clone()
    }
}

/// Map a Helen exception to the matching Python builtin exception.
///
/// Python's `_call_agent` propagates Helen exceptions as their native Python
/// class (RuntimeError/ValueError/TypeError/...); this mirrors that for the
/// common predefined classes and falls back to `RuntimeError` otherwise.
pub fn exception_to_pyerr(e: &ExceptionValue) -> PyErr {
    let msg = exception_message(e);
    let cls = e.class_name.as_str();
    use pyo3::exceptions::*;
    match cls {
        "ValueError" => PyValueError::new_err(msg),
        "TypeError" => PyTypeError::new_err(msg),
        "KeyError" => PyKeyError::new_err(msg),
        "IndexError" => PyIndexError::new_err(msg),
        "ZeroDivisionError" => PyZeroDivisionError::new_err(msg),
        "AssertionError" => PyAssertionError::new_err(msg),
        "ImportError" => PyImportError::new_err(msg),
        "RuntimeError" => PyRuntimeError::new_err(msg),
        "OverflowError" => PyOverflowError::new_err(msg),
        "AttributeError" => PyAttributeError::new_err(msg),
        _ => PyRuntimeError::new_err(format!("{}: {}", e.class_name, msg)),
    }
}

/// Extract the agent's `description "..."` from its declarations block.
pub fn agent_description(a: &AgentDecl) -> String {
    use helen_core::ast::Expr;
    use helen_core::tokens::LiteralValue;
    for d in &a.declarations {
        if let Some(Expr::Literal(helen_core::ast::Lit {
            value: LiteralValue::Str(s),
            ..
        })) = &d.description
        {
            return s.clone();
        }
    }
    String::new()
}
