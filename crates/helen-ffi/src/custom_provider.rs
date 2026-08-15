//! Custom LLM provider loader (M5.3 through M10) — port of the custom half
//! of `helen/runtime/provider_protocol.py` (`_load_custom_providers`).
//!
//! Scans `~/.helen/providers/*.py`, execs each file in the embedded Python,
//! finds `PlatformProtocol` subclasses, and registers Python-backed adapters
//! into the helen-runtime custom protocol registry.
//!
//! Semantics (Python parity):
//! - Private / `__init__.py` files skipped.
//! - Built-in names cannot be overridden.
//! - A subclass missing an explicit `name` is skipped with a warning.
//! - Errors in user files are logged and skipped (no hard failure).

#![allow(deprecated)] // pyo3 0.23 IntoPy -> IntoPyObject migration
use pyo3::conversion::IntoPy;
use pyo3::types::{
    PyAnyMethods, PyBoolMethods, PyDict, PyDictMethods, PyFloatMethods, PyListMethods,
    PyStringMethods, PyTypeMethods,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `~/.helen/providers` (Python `_get_providers_dir`).
pub fn providers_dir() -> PathBuf {
    helen_runtime::config::get_helen_home().join("providers")
}

/// Scan `~/.helen/providers/*.py` and register custom protocols.
/// Returns the list of registered protocol names (errors skipped).
pub fn load_custom_providers() -> Vec<String> {
    let dir = providers_dir();
    let mut registered = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return registered;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == "py").unwrap_or(false))
        .filter(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            !name.starts_with('_') && !name.starts_with('.') && name != "__init__.py"
        })
        .collect();
    files.sort();
    for file in files {
        match load_one_provider_file(&file) {
            Ok(names) => registered.extend(names),
            Err(e) => {
                eprintln!("Custom provider {} failed to load: {e}", file.display());
            }
        }
    }
    registered
}

/// Load a single provider file and register its `PlatformProtocol`
/// subclasses. Returns the names registered from this file.
fn load_one_provider_file(filepath: &Path) -> Result<Vec<String>, String> {
    let code = std::fs::read_to_string(filepath)
        .map_err(|e| format!("cannot read {}: {e}", filepath.display()))?;

    pyo3::Python::with_gil(|py| {
        // Build a module namespace; inject the PlatformProtocol base class.
        let dict = pyo3::types::PyDict::new(py);
        // Minimal `PlatformProtocol` base (matches provider_protocol.py
        // default semantics — OpenAI-compatible passthrough).
        let base_code = r#"
class PlatformProtocol:
    name = "openai"
    def build_request_payload(self, base_payload, *, model_id, thinking_enabled=False, reasoning_effort=None):
        return base_payload
    def supports_tool_choice(self, value):
        return True
    def sanitize_messages(self, messages):
        return messages
    def parse_response(self, response_data):
        choice = response_data.get("choices", [{}])[0]
        message = choice.get("message", {})
        return {
            "content": message.get("content", ""),
            "reasoning_content": message.get("reasoning_content", ""),
            "tool_calls": message.get("tool_calls", []),
            "finish_reason": choice.get("finish_reason", "stop"),
            "usage": response_data.get("usage", {}),
        }
    def parse_streaming_delta(self, delta, context):
        return {
            "content": delta.get("content", ""),
            "reasoning_content": delta.get("reasoning_content", ""),
            "tool_calls": delta.get("tool_calls", []),
            "finish_reason": delta.get("finish_reason"),
        }
    def extract_streaming_usage(self, chunk):
        return chunk.get("usage")
    def parse_error(self, status_code, response_body):
        error = response_body.get("error", {})
        if isinstance(error, dict):
            return error.get("message", str(response_body))
        return str(response_body)
    def is_context_overflow_error(self, error_msg):
        markers = (
            "context length", "maximum context", "too many tokens",
            "reduce your prompt", "context overflow", "max_tokens",
        )
        return any(m in error_msg.lower() for m in markers)
"#;
        if let Err(e) = py.run(
            &std::ffi::CString::new(base_code).unwrap(),
            Some(&dict),
            None,
        ) {
            return Err(format!("cannot define PlatformProtocol base: {e}"));
        }
        // `__name__` required for class __module__ checks.
        let stem = filepath
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "provider".to_string());
        dict.set_item("__name__", format!("_helen_custom_provider_{stem}"))
            .unwrap();
        dict.set_item("__file__", filepath.to_string_lossy().as_ref())
            .unwrap();

        if let Err(e) = py.run(
            &std::ffi::CString::new(code.as_str()).unwrap(),
            Some(&dict),
            None,
        ) {
            return Err(e.to_string());
        }

        // Find PlatformProtocol subclasses defined in this module.
        let mut registered = Vec::new();
        // Base class object (injected above) for direct Rust-side subclass checks.
        let base_type: Option<pyo3::Bound<'_, pyo3::types::PyType>> =
            match dict.get_item("PlatformProtocol") {
                Ok(Some(b)) => b.downcast_into().ok(),
                _ => None,
            };
        for (_key, val) in dict.iter() {
            let Ok(class) = val.downcast::<pyo3::types::PyType>() else {
                continue;
            };
            // Skip the base itself.
            let class_name = class.name().map(|s| s.to_string()).unwrap_or_default();
            if class_name == "PlatformProtocol" {
                continue;
            }
            // issubclass(class, PlatformProtocol) — checked in Rust (no eval;
            // `dict[...]` in a Python eval hits PEP-585 generic subscription).
            let is_sub = match &base_type {
                Some(base) => class.is_subclass(base.as_any()).unwrap_or(false),
                None => false,
            };
            if !is_sub {
                continue;
            }
            // Explicit `name` must exist on the subclass itself.
            let name_val = class
                .getattr("__dict__")
                .ok()
                .and_then(|mapping| mapping.get_item("name").ok());
            let Some(name_val) = name_val else {
                continue; // no explicit name -> skipped (Python parity)
            };
            let Ok(protocol_name) = name_val.extract::<String>() else {
                continue;
            };
            // Built-in names cannot be overridden (registry enforces too).
            // Python parity: `detect_protocol` returns `protocol_class()`, an
            // INSTANCE. Instantiate the no-arg class here; skip on failure.
            let Ok(instance) = val.call0() else {
                eprintln!("Custom provider class {class_name} could not be instantiated; skipping");
                continue;
            };
            let adapter = PythonProtocolAdapter::new(instance.unbind());
            if helen_runtime::provider::register_custom_protocol(&protocol_name, Arc::new(adapter))
                .is_some()
            {
                registered.push(protocol_name);
            }
        }
        Ok(registered)
    })
}

/// A `PlatformProtocol` backed by a Python object (delegates every method
/// through the embedded interpreter).
pub struct PythonProtocolAdapter {
    obj: pyo3::Py<pyo3::PyAny>,
}

impl PythonProtocolAdapter {
    pub fn new(obj: pyo3::Py<pyo3::PyAny>) -> Self {
        PythonProtocolAdapter { obj }
    }
}

impl helen_runtime::provider::PlatformProtocol for PythonProtocolAdapter {
    fn name(&self) -> &'static str {
        "custom"
    }

    fn build_request_payload(
        &self,
        base_payload: serde_json::Value,
        model_id: &str,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> serde_json::Value {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            let py_payload = json_to_python(py, &base_payload);
            let kwargs = PyDict::new(py);
            kwargs.set_item("model_id", model_id).unwrap();
            kwargs
                .set_item("thinking_enabled", thinking_enabled)
                .unwrap();
            match reasoning_effort {
                Some(r) => kwargs.set_item("reasoning_effort", r).unwrap(),
                None => kwargs.set_item("reasoning_effort", py.None()).unwrap(),
            }
            let result = {
                use pyo3::types::PyTuple;
                let arg_tuple = PyTuple::new(py, [py_payload.bind(py)])
                    .map_err(|e| e.to_string())
                    .and_then(|t| {
                        obj.call_method("build_request_payload", t, Some(&kwargs))
                            .map_err(|e| e.to_string())
                    });
                match arg_tuple {
                    Ok(r) => python_to_json(py, &r).unwrap_or(base_payload),
                    Err(e) => {
                        eprintln!("Custom provider build_request_payload failed: {e}");
                        base_payload
                    }
                }
            };
            result
        })
    }

    fn parse_response(&self, response_data: &serde_json::Value) -> serde_json::Value {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            let py_data = json_to_python(py, response_data);
            match obj.call_method1("parse_response", (py_data,)) {
                Ok(r) => python_to_json(py, &r).unwrap_or_else(|| serde_json::json!({})),
                Err(_) => serde_json::json!({}),
            }
        })
    }

    fn parse_streaming_delta(
        &self,
        delta: &serde_json::Value,
        _context: &mut serde_json::Value,
    ) -> serde_json::Value {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            let py_delta = json_to_python(py, delta);
            let py_ctx = pyo3::types::PyDict::new(py);
            match obj.call_method1("parse_streaming_delta", (py_delta, py_ctx)) {
                Ok(r) => python_to_json(py, &r).unwrap_or_else(|| serde_json::json!({})),
                Err(_) => serde_json::json!({}),
            }
        })
    }

    fn extract_streaming_usage(&self, chunk: &serde_json::Value) -> Option<serde_json::Value> {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            let py_chunk = json_to_python(py, chunk);
            match obj.call_method1("extract_streaming_usage", (py_chunk,)) {
                Ok(r) if !r.is_none() => python_to_json(py, &r),
                _ => None,
            }
        })
    }

    fn parse_error(&self, status_code: u16, response_body: &serde_json::Value) -> String {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            let py_body = json_to_python(py, response_body);
            match obj.call_method1("parse_error", (status_code, py_body)) {
                Ok(r) => r.to_string(),
                Err(_) => response_body.to_string(),
            }
        })
    }

    fn is_context_overflow_error(&self, error_msg: &str) -> bool {
        pyo3::Python::with_gil(|py| {
            let obj = self.obj.bind(py);
            match obj.call_method1("is_context_overflow_error", (error_msg,)) {
                Ok(r) => r.is_truthy().unwrap_or(false),
                Err(_) => false,
            }
        })
    }
}

// -- JSON <-> Python helpers -------------------------------------------------

#[allow(deprecated, clippy::result_large_err)] // pyo3 0.23 IntoPy -> IntoPyObject migration
fn json_to_python(py: pyo3::Python<'_>, value: &serde_json::Value) -> pyo3::PyObject {
    use pyo3::types::{PyDict, PyList};
    match value {
        serde_json::Value::Null => py.None(),
        serde_json::Value::Bool(b) => b.into_py(py),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py(py)
            } else if let Some(u) = n.as_u64() {
                u.into_py(py)
            } else if let Some(f) = n.as_f64() {
                f.into_py(py)
            } else {
                py.None()
            }
        }
        serde_json::Value::String(s) => s.into_py(py),
        serde_json::Value::Array(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(json_to_python(py, item)).unwrap();
            }
            list.into_any().unbind()
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, json_to_python(py, v)).unwrap();
            }
            dict.into_any().unbind()
        }
    }
}

#[allow(clippy::only_used_in_recursion, clippy::result_large_err)]
fn python_to_json(
    py: pyo3::Python<'_>,
    obj: &pyo3::Bound<'_, pyo3::types::PyAny>,
) -> Option<serde_json::Value> {
    use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};
    if obj.is_none() {
        return Some(serde_json::Value::Null);
    }
    if let Ok(b) = obj.downcast::<PyBool>() {
        return Some(serde_json::Value::Bool(b.is_true()));
    }
    if let Ok(i) = obj.downcast::<PyInt>() {
        if let Ok(v) = i.extract::<i64>() {
            return Some(serde_json::json!(v));
        }
        if let Ok(v) = i.extract::<u64>() {
            return Some(serde_json::json!(v));
        }
    }
    if let Ok(f) = obj.downcast::<PyFloat>() {
        return Some(serde_json::json!(f.value()));
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        return s
            .to_str()
            .ok()
            .map(|s| serde_json::Value::String(s.to_string()));
    }
    if let Ok(l) = obj.downcast::<PyList>() {
        let mut items = Vec::with_capacity(l.len());
        for item in l.iter() {
            items.push(python_to_json(py, &item)?);
        }
        return Some(serde_json::Value::Array(items));
    }
    if let Ok(d) = obj.downcast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in d.iter() {
            let ks = k.str().ok()?.to_str().ok()?.to_string();
            map.insert(ks, python_to_json(py, &v)?);
        }
        return Some(serde_json::Value::Object(map));
    }
    None
}

// Silence unused import when feature toggles off (whole module is feature-gated).
#[allow(unused)]
fn _noop() {}
