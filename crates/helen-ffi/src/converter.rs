//! TypeConverter (Task 10.3) — port of `helen/ffi/type_converter.py`.
//!
//! Converts between Helen `Value` and Python objects in both directions.
//! Complex Python objects are wrapped (not converted) as `PythonObject`
//! native handles; Helen containers convert recursively.

use helen_interpreter::native::NativeHandle;
use helen_interpreter::value::Value;

/// Convert a Helen `Value` to a Python object (via PyO3).
///
/// Port of `DefaultTypeConverter.helen_to_python`:
/// - null/bool/int/float/str → Python primitives
/// - list → list (recursive), map → dict (recursive), tuple → list
/// - Native objects: if they wrap a PyO3 object, return that object
///   unchanged (no copy); otherwise pass through as-is (error).
pub fn helen_to_python(py: pyo3::Python<'_>, value: &Value) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::conversion::IntoPyObject;
    use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => Ok(pyo3::types::PyBool::new(py, *b)
            .to_owned()
            .into_any()
            .unbind()),
        Value::Int(i) => {
            // Arbitrary-precision: build via Python int(str) so no overflow.
            let builtins = py.import("builtins")?;
            let int_cls = builtins.getattr("int")?;
            let obj = int_cls.call1((i.to_string(),))?;
            Ok(obj.unbind())
        }
        Value::Float(f) => Ok(f.into_pyobject(py)?.into_any().unbind()),
        Value::Str(s) => Ok(s.as_ref().into_pyobject(py)?.into_any().unbind()),
        Value::List(l) => {
            let items = l.borrow();
            let py_list = PyList::empty(py);
            for item in items.iter() {
                let converted = helen_to_python(py, item)?;
                py_list.append(converted)?;
            }
            Ok(py_list.into_any().unbind())
        }
        Value::Tuple(t) => {
            let items = t.borrow();
            let mut py_items = Vec::with_capacity(items.len());
            for item in items.iter() {
                py_items.push(helen_to_python(py, item)?);
            }
            Ok(PyTuple::new(py, py_items)?.into_any().unbind())
        }
        Value::Map(m) => {
            let dict = PyDict::new(py);
            let mb = m.borrow();
            for (k, v) in mb.iter() {
                let pk = helen_to_python(py, k)?;
                let pv = helen_to_python(py, v)?;
                dict.set_item(pk, pv)?;
            }
            Ok(dict.into_any().unbind())
        }
        Value::Native(handle) => {
            // Wrapped objects unwrap (Python `hasattr(value, 'unwrap')`).
            if let Some(obj) = handle.0.unwrap_object() {
                if let Ok(pyobj) = obj.downcast::<pyo3::Py<pyo3::PyAny>>() {
                    return Ok((*pyobj).clone_ref(py));
                }
            }
            Err(pyo3::exceptions::PyTypeError::new_err(
                "cannot convert native object to Python",
            ))
        }
        other => Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "cannot convert {} to Python",
            other.type_name()
        ))),
    }
}

/// Convert a Python object to a Helen `Value`.
///
/// Port of `DefaultTypeConverter.python_to_helen`:
/// - None → null; bool/int/float/str → Helen primitives
/// - tuple → list, list → list (recursive), dict → map (recursive)
/// - anything else → `PythonObject` wrapper (native handle).
#[allow(clippy::only_used_in_recursion)]
pub fn python_to_helen(py: pyo3::Python<'_>, obj: &pyo3::Bound<'_, pyo3::types::PyAny>) -> Value {
    use pyo3::types::{
        PyAnyMethods, PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyFloatMethods, PyInt,
        PyList, PyListMethods, PyString, PyStringMethods, PyTuple, PyTupleMethods,
    };

    // Order matters: bool is an int subclass in Python — check bool first.
    if obj.is_none() {
        return Value::Null;
    }
    if let Ok(b) = obj.downcast::<PyBool>() {
        return Value::Bool(b.is_true());
    }
    if let Ok(i) = obj.downcast::<PyInt>() {
        // Arbitrary precision: parse the decimal string repr.
        if let Ok(s) = i.str() {
            if let Ok(text) = s.to_str() {
                if let Ok(big) = text.parse::<num_bigint::BigInt>() {
                    return Value::Int(big);
                }
            }
        }
        // Fallback: try i64.
        if let Ok(v) = i.extract::<i64>() {
            return Value::Int(num_bigint::BigInt::from(v));
        }
    }
    if let Ok(f) = obj.downcast::<PyFloat>() {
        return Value::Float(f.value());
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        if let Ok(text) = s.to_str() {
            return Value::Str(std::rc::Rc::from(text));
        }
    }
    if let Ok(l) = obj.downcast::<PyList>() {
        let mut items = Vec::with_capacity(l.len());
        for item in l.iter() {
            items.push(python_to_helen(py, &item));
        }
        return Value::List(std::rc::Rc::new(std::cell::RefCell::new(items)));
    }
    if let Ok(t) = obj.downcast::<PyTuple>() {
        let mut items = Vec::with_capacity(t.len());
        for item in t.iter() {
            items.push(python_to_helen(py, &item));
        }
        return Value::List(std::rc::Rc::new(std::cell::RefCell::new(items)));
    }
    if let Ok(d) = obj.downcast::<PyDict>() {
        let mut map = indexmap::IndexMap::new();
        for (k, v) in d.iter() {
            let hk = python_to_helen(py, &k);
            let hv = python_to_helen(py, &v);
            map.insert(hk, hv);
        }
        return Value::Map(std::rc::Rc::new(std::cell::RefCell::new(map)));
    }
    // Complex objects are wrapped (Python `WrappedPythonObject`).
    let py_obj = obj.clone().unbind();
    Value::Native(NativeHandle(std::sync::Arc::new(
        crate::object::PythonObject::new(py_obj),
    )))
}
