//! M11 bridge integration tests (Rust side; pyo3 `auto-initialize`).
//!
//! These exercise the compiled `helen_rust` rlib directly with an embedded
//! Python interpreter. The full Python-level DoD suite lives in
//! `tests/test_bridge_python.py` (run after `maturin develop`).

use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyTuple};

/// Write a fixture to a per-call unique temp file (parallel tests share
/// the `helen_bridge_test` dir; `std::fs::write` is non-atomic, so shared
/// filenames could race with a concurrent reader's `read_to_string`).
fn write_temp_helen(name: &str, content: &str) -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join("helen_bridge_test");
    std::fs::create_dir_all(&dir).unwrap();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = dir.join(format!("{n}_{name}"));
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().to_string()
}

const AGENT_SRC: &str = r#"
// M11 test fixture: agents + functions.
agent SumAgent(a: int, b: int) {
    description "compute a + b"
    main {
        return a + b
    }
}

agent DefaultAgent(a: int, b: int = 100) {
    description "uses a default"
    main {
        return a + b
    }
}

fn add(a: int, b: int): int {
    return a + b
}
"#;

#[test]
fn load_agent_call_positional_and_kwargs() {
    let file = write_temp_helen("sum_agent.helen", AGENT_SRC);
    Python::with_gil(|py| {
        let a = Py::new(
            py,
            helen_python_bridge::load_agent(&file, "SumAgent").unwrap(),
        )
        .unwrap();
        // Positional.
        let out: i64 = a
            .bind(py)
            .as_any()
            .call1((10i32, 20i32))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(out, 30);
        // Keyword.
        let kwargs = PyDict::new(py);
        kwargs.set_item("a", 15).unwrap();
        kwargs.set_item("b", 25).unwrap();
        let out: i64 = a
            .bind(py)
            .as_any()
            .call((), Some(&kwargs))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(out, 40);
        // Mixed (positional then keyword).
        let kwargs = PyDict::new(py);
        kwargs.set_item("b", 40).unwrap();
        let out: i64 = a
            .bind(py)
            .as_any()
            .call((30i32,), Some(&kwargs))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(out, 70);
    });
}

#[test]
fn load_agent_type_error_messages() {
    let file = write_temp_helen("sum_agent.helen", AGENT_SRC);
    Python::with_gil(|py| {
        let a = Py::new(
            py,
            helen_python_bridge::load_agent(&file, "SumAgent").unwrap(),
        )
        .unwrap();
        // Too many positional.
        let err = a
            .bind(py)
            .as_any()
            .call1((10i32, 20i32, 30i32))
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("SumAgent() takes 2 positional arguments but 3 were given"),
            "got: {err}"
        );
        // Missing required.
        let err = a.bind(py).as_any().call1((10i32,)).unwrap_err();
        assert!(
            err.to_string()
                .contains("SumAgent() missing required argument: 'b'"),
            "got: {err}"
        );
        // Unknown keyword.
        let kwargs = PyDict::new(py);
        kwargs.set_item("c", 1).unwrap();
        let err = a.bind(py).as_any().call((), Some(&kwargs)).unwrap_err();
        assert!(
            err.to_string()
                .contains("SumAgent() got an unexpected keyword argument 'c'"),
            "got: {err}"
        );
    });
}

#[test]
fn agent_default_parameter_is_optional() {
    let file = write_temp_helen("sum_agent.helen", AGENT_SRC);
    Python::with_gil(|py| {
        let a = Py::new(
            py,
            helen_python_bridge::load_agent(&file, "DefaultAgent").unwrap(),
        )
        .unwrap();
        // `b` has a default — no TypeError.
        let out: i64 = a
            .bind(py)
            .as_any()
            .call1((5i32,))
            .unwrap()
            .extract()
            .unwrap();
        // M11: call_agent now evaluates defaults in the agent's isolated env
        // (Python `_call_agent` parity) -> 5 + 100 = 105.
        assert_eq!(out, 105);
    });
}

#[test]
fn load_function_positional_and_kwargs() {
    let file = write_temp_helen("sum_agent.helen", AGENT_SRC);
    Python::with_gil(|py| {
        let f = Py::new(
            py,
            helen_python_bridge::load_function(&file, "add").unwrap(),
        )
        .unwrap();
        let out: i64 = f
            .bind(py)
            .as_any()
            .call1((2i32, 3i32))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(out, 5);
        // Keyword fills remaining slot.
        let kwargs = PyDict::new(py);
        kwargs.set_item("b", 4).unwrap();
        let out: i64 = f
            .bind(py)
            .as_any()
            .call((2i32,), Some(&kwargs))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(out, 6);
        // Multiple values for argument.
        let kwargs = PyDict::new(py);
        kwargs.set_item("a", 9).unwrap();
        let err = f
            .bind(py)
            .as_any()
            .call((2i32,), Some(&kwargs))
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("add() got multiple values for argument 'a'"),
            "got: {err}"
        );
        // Unknown keyword.
        let kwargs = PyDict::new(py);
        kwargs.set_item("x", 1).unwrap();
        let err = f.bind(py).as_any().call((), Some(&kwargs)).unwrap_err();
        assert!(
            err.to_string()
                .contains("add() got an unexpected keyword argument 'x'"),
            "got: {err}"
        );
    });
}

#[test]
fn describe_file_lists_declarations_in_order() {
    let file = write_temp_helen("sum_agent.helen", AGENT_SRC);
    Python::with_gil(|_py| {
        let decls = helen_python_bridge::describe_file(&file).unwrap();
        let kinds: Vec<(&str, &str)> = decls
            .iter()
            .map(|(k, n, _d)| (k.as_str(), n.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("agent", "SumAgent"),
                ("agent", "DefaultAgent"),
                ("function", "add"),
            ]
        );
        assert_eq!(decls[0].2, "compute a + b");
    });
}

#[test]
fn parse_check_returns_semantic_codes() {
    Python::with_gil(|_py| {
        // Valid main-block program → no codes.
        assert_eq!(
            helen_python_bridge::parse_check(
                "import std.core.*\nmain {\n    let x = 1\n    print(x)\n}\n"
            )
            .unwrap(),
            Vec::<String>::new()
        );
        // Undefined variable → a semantic error code.
        let codes = helen_python_bridge::parse_check("main {\n    print(y)\n}\n").unwrap();
        assert!(
            !codes.is_empty(),
            "expected at least one E-code for undefined variable"
        );
        // Top-level statement in a real file → E0355 (TOP_LEVEL_STATEMENT).
        let codes = helen_python_bridge::parse_check("let x = 1").unwrap();
        assert!(codes.contains(&"E0355".to_string()), "codes: {codes:?}");
        // Parse failure raises RuntimeError.
        let err = helen_python_bridge::parse_check("let = =").unwrap_err();
        assert!(err.to_string().contains("Failed to parse"), "got: {err}");
    });
}

#[test]
fn eval_helen_with_globals() {
    Python::with_gil(|py| {
        let globals = PyDict::new(py);
        globals.set_item("x", 21).unwrap();
        let out: i64 = helen_python_bridge::eval_helen(py, "x * 2", &globals)
            .unwrap()
            .extract(py)
            .unwrap();
        assert_eq!(out, 42);
        // String result.
        let globals = PyDict::new(py);
        let out: String = helen_python_bridge::eval_helen(py, "\"hello\" + \" world\"", &globals)
            .unwrap()
            .extract(py)
            .unwrap();
        assert_eq!(out, "hello world");
    });
}

#[test]
fn list_and_dict_args_convert() {
    let file = write_temp_helen(
        "list_agent.helen",
        r#"
import std.core.*

agent LenAgent(items: list) {
    main {
        return len(items)
    }
}
"#,
    );
    Python::with_gil(|py| {
        let a = Py::new(
            py,
            helen_python_bridge::load_agent(&file, "LenAgent").unwrap(),
        )
        .unwrap();
        let items = PyTuple::new(py, [1i32, 2i32, 3i32]).unwrap();
        let out: i64 = a
            .bind(py)
            .as_any()
            .call1((items,))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(out, 3);
    });
}

#[test]
fn agent_runtime_error_maps_to_python() {
    let file = write_temp_helen(
        "throw_agent.helen",
        r#"
agent ThrowAgent() {
    main {
        throw RuntimeError("boom")
    }
}
"#,
    );
    Python::with_gil(|py| {
        let a = Py::new(
            py,
            helen_python_bridge::load_agent(&file, "ThrowAgent").unwrap(),
        )
        .unwrap();
        let err = a.bind(py).as_any().call0().unwrap_err();
        // RuntimeError maps to Python RuntimeError with the message.
        assert!(
            err.value(py).get_type().name().unwrap() == "RuntimeError",
            "got: {err}"
        );
        assert!(err.to_string().contains("boom"), "got: {err}");
    });
}

// ── Additional bridge tests for coverage ────────────────────────────

#[test]
fn load_nonexistent_agent_raises() {
    let file = write_temp_helen("sum_agent.helen", AGENT_SRC);
    Python::with_gil(|_py| {
        let result = helen_python_bridge::load_agent(&file, "NonExistentAgent");
        match result {
            Err(e) => {
                let err = e.to_string();
                assert!(
                    err.contains("NonExistentAgent") || err.contains("not found"),
                    "got: {err}"
                );
            }
            Ok(_) => panic!("Expected error for nonexistent agent"),
        }
    });
}

#[test]
fn load_nonexistent_function_raises() {
    let file = write_temp_helen("sum_agent.helen", AGENT_SRC);
    Python::with_gil(|_py| {
        let result = helen_python_bridge::load_function(&file, "nonexistent_fn");
        match result {
            Err(e) => {
                let err = e.to_string();
                assert!(
                    err.contains("nonexistent_fn") || err.contains("not found"),
                    "got: {err}"
                );
            }
            Ok(_) => panic!("Expected error for nonexistent function"),
        }
    });
}

#[test]
fn load_from_nonexistent_file_raises() {
    Python::with_gil(|_py| {
        let result = helen_python_bridge::load_agent("/nonexistent/path.helen", "Test");
        match result {
            Err(e) => {
                let err = e.to_string();
                assert!(
                    err.contains("No such file")
                        || err.contains("not found")
                        || err.contains("cannot"),
                    "got: {err}"
                );
            }
            Ok(_) => panic!("Expected error for nonexistent file"),
        }
    });
}

#[test]
fn eval_helen_with_empty_globals() {
    Python::with_gil(|py| {
        let globals = PyDict::new(py);
        let out: i64 = helen_python_bridge::eval_helen(py, "1 + 2", &globals)
            .unwrap()
            .extract(py)
            .unwrap();
        assert_eq!(out, 3);
    });
}

#[test]
fn eval_helen_with_string_operations() {
    Python::with_gil(|py| {
        let globals = PyDict::new(py);
        let out: String = helen_python_bridge::eval_helen(py, "\"hello\" + \" world\"", &globals)
            .unwrap()
            .extract(py)
            .unwrap();
        assert_eq!(out, "hello world");
    });
}

#[test]
fn parse_check_valid_program_returns_empty() {
    let codes = helen_python_bridge::parse_check("fn test(): int { return 42 }").unwrap();
    assert!(codes.is_empty(), "expected no errors, got: {codes:?}");
}

#[test]
fn parse_check_syntax_error_raises() {
    let err = helen_python_bridge::parse_check("fn { }").unwrap_err();
    assert!(err.to_string().contains("Failed to parse"), "got: {err}");
}

#[test]
fn describe_file_empty_file() {
    let file = write_temp_helen("empty.helen", "");
    Python::with_gil(|_py| {
        let decls = helen_python_bridge::describe_file(&file).unwrap();
        assert!(decls.is_empty(), "expected no declarations, got: {decls:?}");
    });
}

#[test]
fn describe_file_multiple_agents() {
    let file = write_temp_helen(
        "multi.helen",
        r#"
agent Agent1() { main { return 1 } }
agent Agent2() { main { return 2 } }
agent Agent3() { main { return 3 } }
"#,
    );
    Python::with_gil(|_py| {
        let decls = helen_python_bridge::describe_file(&file).unwrap();
        assert_eq!(decls.len(), 3);
        assert_eq!(decls[0].1, "Agent1");
        assert_eq!(decls[1].1, "Agent2");
        assert_eq!(decls[2].1, "Agent3");
    });
}

#[test]
fn agent_with_string_return() {
    let file = write_temp_helen(
        "string_agent.helen",
        r#"
agent StringAgent(msg: str) {
    main {
        return msg
    }
}
"#,
    );
    Python::with_gil(|py| {
        let a = Py::new(
            py,
            helen_python_bridge::load_agent(&file, "StringAgent").unwrap(),
        )
        .unwrap();
        let out: String = a
            .bind(py)
            .as_any()
            .call1(("hello world",))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(out, "hello world");
    });
}

#[test]
fn agent_with_boolean_return() {
    let file = write_temp_helen(
        "bool_agent.helen",
        r#"
agent BoolAgent(flag: bool) {
    main {
        return flag
    }
}
"#,
    );
    Python::with_gil(|py| {
        let a = Py::new(
            py,
            helen_python_bridge::load_agent(&file, "BoolAgent").unwrap(),
        )
        .unwrap();
        let out: bool = a
            .bind(py)
            .as_any()
            .call1((true,))
            .unwrap()
            .extract()
            .unwrap();
        assert!(out);
    });
}

#[test]
fn function_with_multiple_parameters() {
    let file = write_temp_helen(
        "multi_param.helen",
        r#"
fn add_three(a: int, b: int, c: int): int {
    return a + b + c
}
"#,
    );
    Python::with_gil(|py| {
        let f = Py::new(
            py,
            helen_python_bridge::load_function(&file, "add_three").unwrap(),
        )
        .unwrap();
        let out: i64 = f
            .bind(py)
            .as_any()
            .call1((10i32, 20i32, 30i32))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(out, 60);
    });
}

#[test]
fn agent_with_description() {
    let file = write_temp_helen(
        "desc_agent.helen",
        r#"
agent DescribedAgent() {
    description "This is a test agent"
    main {
        return 42
    }
}
"#,
    );
    Python::with_gil(|_py| {
        let decls = helen_python_bridge::describe_file(&file).unwrap();
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].2, "This is a test agent");
    });
}
