//! M10 FFI tests — port of `helen/tests/ffi/test_*` (feature-gated).
//!
//! Run with: `cargo test -p helen-ffi --features python-ffi`

#![cfg(feature = "python-ffi")]
#![allow(clippy::approx_constant)]

use helen_ffi::converter::{helen_to_python, python_to_helen};
use helen_ffi::module::PythonModule;
use helen_ffi::object::PythonObject;
use helen_ffi::runtime::PythonRuntime;
use helen_interpreter::native::NativeObject;
use helen_interpreter::value::Value;
use pyo3::conversion::IntoPyObject;
use pyo3::types::{PyAnyMethods, PyDictMethods, PyListMethods};

fn rt() -> PythonRuntime {
    PythonRuntime::new().expect("Python runtime init")
}

// ---------------------------------------------------------------------------
// Type converter (port of test_type_converter.py)
// ---------------------------------------------------------------------------

#[test]
fn helen_to_python_primitive_roundtrip() {
    pyo3::Python::with_gil(|py| {
        // int
        let v = helen_to_python(py, &Value::Int(42.into())).unwrap();
        assert_eq!(v.bind(py).extract::<i64>().unwrap(), 42);
        // float
        let v = helen_to_python(py, &Value::Float(3.14)).unwrap();
        assert_eq!(v.bind(py).extract::<f64>().unwrap(), 3.14);
        // str
        let v = helen_to_python(py, &Value::Str("hello".into())).unwrap();
        assert_eq!(v.bind(py).extract::<String>().unwrap(), "hello");
        // bool
        let v = helen_to_python(py, &Value::Bool(true)).unwrap();
        assert!(v.bind(py).extract::<bool>().unwrap());
        // null
        let v = helen_to_python(py, &Value::Null).unwrap();
        assert!(v.bind(py).is_none());
    });
}

#[test]
fn helen_list_map_to_python() {
    pyo3::Python::with_gil(|py| {
        // list
        let items = vec![Value::Int(1.into()), Value::Int(2.into())];
        let v = helen_to_python(
            py,
            &Value::List(std::rc::Rc::new(std::cell::RefCell::new(items))),
        )
        .unwrap();
        let py_list = v.bind(py).downcast::<pyo3::types::PyList>().unwrap();
        assert_eq!(py_list.len(), 2);
        assert_eq!(py_list.get_item(0).unwrap().extract::<i64>().unwrap(), 1);
        assert_eq!(py_list.get_item(1).unwrap().extract::<i64>().unwrap(), 2);
    });
}

#[test]
fn python_to_helen_roundtrip() {
    pyo3::Python::with_gil(|py| {
        // int
        let v = python_to_helen(py, &42i64.into_pyobject(py).unwrap().into_any());
        assert_eq!(v, Value::Int(42.into()));
        // float
        let v = python_to_helen(py, &3.14f64.into_pyobject(py).unwrap().into_any());
        assert_eq!(v, Value::Float(3.14));
        // str
        let v = python_to_helen(py, &"hello".into_pyobject(py).unwrap().into_any());
        assert_eq!(v, Value::Str("hello".into()));
        // bool
        let b = pyo3::types::PyBool::new(py, true).to_owned().into_any();
        let v = python_to_helen(py, &b);
        assert_eq!(v, Value::Bool(true));
        // None
        let none = py.None().into_bound(py);
        let v = python_to_helen(py, &none);
        assert_eq!(v, Value::Null);
    });
}

#[test]
fn python_tuple_to_helen_list() {
    pyo3::Python::with_gil(|py| {
        let tuple = pyo3::types::PyTuple::new(py, [1i64, 2, 3]).unwrap();
        let v = python_to_helen(py, &tuple.into_any());
        assert!(matches!(v, Value::List(_)));
    });
}

// ---------------------------------------------------------------------------
// PythonObject (port of test_python_object.py)
// ---------------------------------------------------------------------------

#[test]
fn object_wrap_and_str() {
    pyo3::Python::with_gil(|py| {
        let obj = 42i64.into_pyobject(py).unwrap().into_any().unbind();
        let wrapped = PythonObject::new(obj);
        assert_eq!(wrapped.type_name(), "int");
        assert_eq!(wrapped.python_str(), "42");
    });
}

#[test]
fn object_unwrap_returns_original() {
    pyo3::Python::with_gil(|py| {
        let list = pyo3::types::PyList::new(py, [1i64, 2, 3]).unwrap();
        let obj = list.into_any().unbind();
        let wrapped = PythonObject::new(obj.clone_ref(py));
        let unwrapped = wrapped.unwrap_object().unwrap();
        assert!(unwrapped.downcast::<pyo3::Py<pyo3::PyAny>>().is_ok());
    });
}

#[test]
fn object_get_attribute() {
    pyo3::Python::with_gil(|py| {
        // Build a small class instance with a value attribute.
        let globals = pyo3::types::PyDict::new(py);
        let code = "class C:\n    def __init__(self):\n        self.value = 42\n        self.name = 'test'\nobj = C()\n";
        py.run(&std::ffi::CString::new(code).unwrap(), Some(&globals), None)
            .unwrap();
        let obj = globals.get_item("obj").unwrap().unwrap().unbind();
        let wrapped = PythonObject::new(obj);
        let v = wrapped.get_attribute("value").unwrap();
        assert_eq!(v, Value::Int(42.into()));
        let name = wrapped.get_attribute("name").unwrap();
        assert_eq!(name, Value::Str("test".into()));
        // Missing attribute → error.
        assert!(wrapped.get_attribute("nonexistent").is_err());
    });
}

#[test]
fn object_call_function_with_kwargs() {
    pyo3::Python::with_gil(|py| {
        let globals = pyo3::types::PyDict::new(py);
        let code = "def greet(name, greeting='Hello'):\n    return f'{greeting}, {name}!'\n";
        py.run(&std::ffi::CString::new(code).unwrap(), Some(&globals), None)
            .unwrap();
        let obj = globals.get_item("greet").unwrap().unwrap().unbind();
        let wrapped = PythonObject::new(obj);
        let kwargs = vec![("greeting".to_string(), Value::Str("Hi".into()))];
        let v = wrapped
            .call(&[Value::Str("Alice".into())], &kwargs)
            .unwrap();
        assert_eq!(v, Value::Str("Hi, Alice!".into()));
    });
}

#[test]
fn object_getitem_list_and_dict() {
    pyo3::Python::with_gil(|py| {
        let list = pyo3::types::PyList::new(py, [10i64, 20, 30])
            .unwrap()
            .into_any()
            .unbind();
        let wrapped = PythonObject::new(list);
        let v = wrapped.get_item(&Value::Int(0.into())).unwrap();
        assert_eq!(v, Value::Int(10.into()));
        let v = wrapped.get_item(&Value::Int(2.into())).unwrap();
        assert_eq!(v, Value::Int(30.into()));

        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("a", 1i64).unwrap();
        let wrapped = PythonObject::new(dict.into_any().unbind());
        let v = wrapped.get_item(&Value::Str("a".into())).unwrap();
        assert_eq!(v, Value::Int(1.into()));
        assert!(wrapped.get_item(&Value::Str("nope".into())).is_err());
    });
}

// ---------------------------------------------------------------------------
// PythonModule + PythonRuntime (port of test_python_runtime.py)
// ---------------------------------------------------------------------------

#[test]
fn module_getattr_function_and_constant() {
    let rt = rt();
    let module = rt.import_module("math").unwrap();
    let Value::Native(handle) = module else {
        panic!("expected native");
    };
    let wrapped = handle
        .downcast_ref::<PythonModule>()
        .expect("module wrapper");
    assert_eq!(wrapped.type_name(), "module");
    // sqrt(16) → 4.0
    let sqrt = wrapped.get_attribute("sqrt").unwrap();
    let result = call_value(&sqrt, &[Value::Int(16.into())], &[]);
    assert_eq!(result, Value::Float(4.0));
    // pi ≈ 3.14159
    let pi = wrapped.get_attribute("pi").unwrap();
    assert!(matches!(pi, Value::Float(f) if (f - 3.14159).abs() < 1e-4));
    // missing attribute → error
    assert!(wrapped.get_attribute("nonexistent_function").is_err());
}

#[test]
fn runtime_import_and_call() {
    let rt = rt();
    let module = rt.import_module("math").unwrap();
    let Value::Native(handle) = module else {
        panic!("expected native");
    };
    let wrapped = handle
        .downcast_ref::<PythonModule>()
        .expect("module wrapper");
    let sqrt = wrapped.get_attribute("sqrt").unwrap();
    let result = call_value(&sqrt, &[Value::Int(25.into())], &[]);
    assert_eq!(result, Value::Float(5.0));
    // pi
    let pi = wrapped.get_attribute("pi").unwrap();
    assert!(matches!(pi, Value::Float(f) if (f - 3.14159).abs() < 1e-4));
}

#[test]
fn runtime_import_nonexistent_raises() {
    let rt = rt();
    let err = rt.import_module("nonexistent_module_xyz123").unwrap_err();
    assert!(err.contains("Cannot import module"), "got: {err}");
}

#[test]
fn runtime_import_nested_module() {
    let rt = rt();
    let module = rt.import_module("os.path").unwrap();
    let Value::Native(handle) = module else {
        panic!("expected native");
    };
    let wrapped = handle
        .downcast_ref::<PythonModule>()
        .expect("module wrapper");
    // os.path.join accessible
    assert!(wrapped.get_attribute("join").is_ok());
}

#[test]
fn runtime_import_json_module() {
    let rt = rt();
    let module = rt.import_module("json").unwrap();
    let Value::Native(handle) = module else {
        panic!("expected native");
    };
    let wrapped = handle
        .downcast_ref::<PythonModule>()
        .expect("module wrapper");
    // dumps({"a": 1, "b": 2}) → contains '"a"'
    let dumps = wrapped.get_attribute("dumps").unwrap();
    let map = Value::Map(std::rc::Rc::new(std::cell::RefCell::new(
        indexmap::IndexMap::from([
            (Value::Str("a".into()), Value::Int(1.into())),
            (Value::Str("b".into()), Value::Int(2.into())),
        ]),
    )));
    let result = call_value(&dumps, &[map], &[]);
    let Value::Str(s) = result else {
        panic!("expected string, got {result:?}");
    };
    assert!(s.contains("\"a\""), "got: {s}");
    assert!(s.contains("\"b\""), "got: {s}");
}

#[test]
fn runtime_eval_expression() {
    let rt = rt();
    assert_eq!(rt.eval_expression("2 + 3").unwrap(), Value::Int(5.into()));
    assert_eq!(
        rt.eval_expression("'hello' + ' ' + 'world'").unwrap(),
        Value::Str("hello world".into())
    );
}

#[test]
fn runtime_exec_statement() {
    let rt = rt();
    rt.exec_statement("x = 40 + 2").unwrap();
    assert_eq!(rt.eval_expression("x").unwrap(), Value::Int(42.into()));
}

#[test]
fn runtime_import_is_cached_same_wrapper() {
    let rt = rt();
    let a = rt.import_module("math").unwrap();
    let b = rt.import_module("math").unwrap();
    assert!(matches!(a, Value::Native(_)));
    assert!(matches!(b, Value::Native(_)));
}

/// Call a wrapped value (function/object) with args + kwargs via NativeObject::call.
fn call_value(v: &Value, args: &[Value], kwargs: &[(String, Value)]) -> Value {
    match v {
        Value::Native(handle) => handle.0.call(args, kwargs).unwrap(),
        other => panic!("not callable: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Helen-program integration (port of test_helen_integration.py)
// ---------------------------------------------------------------------------

/// Run a Helen source snippet through the interpreter, printing captured
/// stdout, returning (interpret_result_ok, stdout).
fn run_helen(source: &str) -> (bool, String) {
    let full = format!(
        "import std.core.*\nimport std.str.*\nimport std.list.*\nimport std.dict.*\nimport std.math.*\nimport std.debug.*\n{source}"
    );
    let mut scanner = helen_core::lexer::Scanner::new(&full, "<test>");
    let tokens = scanner.scan_all();
    let mut parser = helen_parser::Parser::new(tokens);
    let program = parser.parse();
    assert!(parser.errors().is_empty(), "parse: {:?}", parser.errors());
    let mut interp = helen_interpreter::interpreter::Interpreter::new();
    let r = interp.interpret(&program);
    let out = interp.stdout.lock().unwrap().clone();
    (r.is_ok(), out)
}

#[test]
fn helen_import_python_module_math_sqrt() {
    let _ = helen_ffi::install();
    let (ok, out) = run_helen(
        r#"
        import "math" as math
        main {
            let result = math.sqrt(16)
            print(result)
        }
        "#,
    );
    assert!(ok, "interpret failed");
    assert_eq!(out.trim(), "4.0"); // Python math.sqrt returns float
}

#[test]
fn helen_import_and_call_functions() {
    let _ = helen_ffi::install();
    let (ok, out) = run_helen(
        r#"
        import "math" as math
        main {
            let x = math.sqrt(25)
            let y = math.pow(2, 10)
            print(x)
            print(y)
        }
        "#,
    );
    assert!(ok, "interpret failed");
    let lines: Vec<&str> = out.trim().lines().collect();
    assert_eq!(lines[0], "5.0"); // Python math.sqrt → float
    assert_eq!(lines[1], "1024.0"); // Python math.pow(2,10) → float
}

#[test]
fn helen_import_json_module_dumps() {
    let _ = helen_ffi::install();
    let (ok, out) = run_helen(
        r#"
        import "json" as json
        main {
            let data = {"name": "Alice", "age": 30}
            let json_str = json.dumps(data)
            print(json_str)
        }
        "#,
    );
    assert!(ok, "interpret failed");
    assert!(out.contains("\"name\""), "got: {out}");
    assert!(out.contains("Alice"), "got: {out}");
}

#[test]
fn helen_access_module_constant_pi() {
    let _ = helen_ffi::install();
    let (ok, out) = run_helen(
        r#"
        import "math" as math
        main {
            let pi = math.pi
            print(pi)
        }
        "#,
    );
    assert!(ok, "interpret failed");
    let pi: f64 = out.trim().parse().expect("pi is a float");
    assert!((pi - 3.14159).abs() < 1e-4, "got: {pi}");
}

#[test]
fn helen_nested_module_os_path() {
    let _ = helen_ffi::install();
    let (ok, _out) = run_helen(
        r#"
        import "os.path" as osp
        main {
            let joined = osp.join("a", "b")
            print(joined)
        }
        "#,
    );
    assert!(ok, "interpret failed");
}

// ---------------------------------------------------------------------------
// Custom LLM provider loader (M5.3 through M10)
// ---------------------------------------------------------------------------

#[test]
fn custom_provider_loader_registers_python_backed_protocol() {
    // Hermetic: point HOME at a temp dir with a fake `~/.helen/providers/`.
    let dir = std::env::temp_dir().join(format!("helen_ffi_providers_test_{}", std::process::id()));
    let providers = dir.join(".helen").join("providers");
    std::fs::create_dir_all(&providers).unwrap();
    std::fs::write(
        providers.join("my_provider.py"),
        r#"
class PlatformProtocol:
    name = "openai"
    def build_request_payload(self, base_payload, *, model_id, thinking_enabled=False, reasoning_effort=None):
        return base_payload
    def parse_response(self, response_data):
        return {"content": "", "reasoning_content": "", "tool_calls": [], "finish_reason": "stop", "usage": {}}

class MyProtocol(PlatformProtocol):
    name = "my-custom-v1"
    def build_request_payload(self, base_payload, *, model_id, thinking_enabled=False, reasoning_effort=None):
        base_payload["model"] = model_id
        return base_payload
    def parse_response(self, response_data):
        return {"content": "from my-custom-v1", "reasoning_content": "", "tool_calls": [], "finish_reason": "stop", "usage": {}}
"#,
    )
    .unwrap();

    let old_home = std::env::var("HOME").unwrap_or_default();
    std::env::set_var("HOME", &dir);
    let _ = helen_ffi::install();
    let registered = helen_ffi::custom_provider::load_custom_providers();
    std::env::set_var("HOME", old_home);

    assert!(
        registered.contains(&"my-custom-v1".to_string()),
        "expected my-custom-v1 registered, got: {registered:?}"
    );
    // The Python-backed adapter must be resolvable from the runtime registry.
    let got = helen_runtime::provider::custom_protocol_by_name("my-custom-v1");
    assert!(got.is_some(), "custom protocol not in runtime registry");
    let payload = got.as_ref().unwrap().build_request_payload(
        serde_json::json!({"temperature": 0.0}),
        "my-model",
        false,
        None,
    );
    assert_eq!(payload["model"], "my-model");

    // Exercise parse_response through the Python-backed adapter too.
    let parsed = got
        .as_ref()
        .unwrap()
        .parse_response(&serde_json::json!({"choices": [{"message": {"content": "hi"}}]}));
    assert_eq!(parsed["content"], "from my-custom-v1");

    // Cleanup.
    let _ = std::fs::remove_dir_all(&dir);
}
