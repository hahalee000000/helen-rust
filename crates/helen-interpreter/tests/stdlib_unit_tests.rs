//! Unit tests for stdlib.rs functions to improve coverage.
//! Targets: dict operations, data operations, time operations, crypto operations.

#![allow(clippy::result_large_err)]

use std::cell::RefCell;
use std::rc::Rc;

use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::value::Value;

// Helper to create a test interpreter
fn make_interp() -> Interpreter {
    Interpreter::new()
}

// Helper to create a map value
fn make_map(pairs: Vec<(&str, Value)>) -> Value {
    let map: indexmap::IndexMap<Value, Value> = pairs
        .into_iter()
        .map(|(k, v)| (Value::Str(Rc::from(k)), v))
        .collect();
    Value::Map(Rc::new(RefCell::new(map)))
}

// Helper to create a list value
fn make_list(items: Vec<Value>) -> Value {
    Value::List(Rc::new(RefCell::new(items)))
}

// Helper to call a stdlib function
fn call_stdlib(
    module: &str,
    func_name: &str,
    interp: &mut Interpreter,
    args: &[Value],
) -> Result<Value, helen_interpreter::exceptions::ExceptionValue> {
    let export = helen_interpreter::stdlib::module_exports(module)
        .unwrap()
        .iter()
        .find(|e| e.name == func_name)
        .unwrap();
    (export.func)(interp, args)
}

// ============================================================================
// Dict operations tests
// ============================================================================

#[test]
fn test_dict_keys_empty() {
    let mut interp = make_interp();
    let map = make_map(vec![]);
    let result = call_stdlib("std.dict", "keys", &mut interp, &[map]).unwrap();
    if let Value::List(list) = result {
        assert_eq!(list.borrow().len(), 0);
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_dict_keys_multiple() {
    let mut interp = make_interp();
    let map = make_map(vec![
        ("a", Value::Int(1.into())),
        ("b", Value::Int(2.into())),
        ("c", Value::Int(3.into())),
    ]);
    let result = call_stdlib("std.dict", "keys", &mut interp, &[map]).unwrap();
    if let Value::List(list) = result {
        assert_eq!(list.borrow().len(), 3);
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_dict_values_multiple() {
    let mut interp = make_interp();
    let map = make_map(vec![
        ("x", Value::Str(Rc::from("hello"))),
        ("y", Value::Bool(true)),
    ]);
    let result = call_stdlib("std.dict", "values", &mut interp, &[map]).unwrap();
    if let Value::List(list) = result {
        assert_eq!(list.borrow().len(), 2);
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_dict_entries_multiple() {
    let mut interp = make_interp();
    let map = make_map(vec![
        ("k1", Value::Int(10.into())),
        ("k2", Value::Int(20.into())),
    ]);
    let result = call_stdlib("std.dict", "entries", &mut interp, &[map]).unwrap();
    if let Value::List(list) = result {
        assert_eq!(list.borrow().len(), 2);
        // Each entry should be a (key, value) tuple
        for entry in list.borrow().iter() {
            if let Value::Tuple(pair) = entry {
                assert_eq!(pair.borrow().len(), 2);
            } else {
                panic!("Expected tuple pair");
            }
        }
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_dict_get_with_default() {
    let mut interp = make_interp();
    let map = make_map(vec![("key", Value::Int(42.into()))]);
    let result = call_stdlib(
        "std.dict",
        "get",
        &mut interp,
        &[map, Value::Str(Rc::from("key")), Value::Int(0.into())],
    )
    .unwrap();
    if let Value::Int(n) = result {
        assert_eq!(n, 42.into());
    } else {
        panic!("Expected int");
    }
}

#[test]
fn test_dict_get_missing_with_default() {
    let mut interp = make_interp();
    let map = make_map(vec![("key", Value::Int(42.into()))]);
    let result = call_stdlib(
        "std.dict",
        "get",
        &mut interp,
        &[map, Value::Str(Rc::from("missing")), Value::Int(99.into())],
    )
    .unwrap();
    if let Value::Int(n) = result {
        assert_eq!(n, 99.into());
    } else {
        panic!("Expected int");
    }
}

#[test]
fn test_dict_has_key_true() {
    let mut interp = make_interp();
    let map = make_map(vec![("exists", Value::Null)]);
    let result = call_stdlib(
        "std.dict",
        "has_key",
        &mut interp,
        &[map, Value::Str(Rc::from("exists"))],
    )
    .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_dict_has_key_false() {
    let mut interp = make_interp();
    let map = make_map(vec![("exists", Value::Null)]);
    let result = call_stdlib(
        "std.dict",
        "has_key",
        &mut interp,
        &[map, Value::Str(Rc::from("missing"))],
    )
    .unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn test_dict_merge_two_maps() {
    let mut interp = make_interp();
    let map1 = make_map(vec![("a", Value::Int(1.into()))]);
    let map2 = make_map(vec![("b", Value::Int(2.into()))]);
    let result = call_stdlib("std.dict", "merge", &mut interp, &[map1, map2]).unwrap();
    if let Value::Map(m) = result {
        assert_eq!(m.borrow().len(), 2);
    } else {
        panic!("Expected map");
    }
}

#[test]
fn test_dict_set_key_new() {
    let mut interp = make_interp();
    let map = make_map(vec![("a", Value::Int(1.into()))]);
    let result = call_stdlib(
        "std.dict",
        "set_key",
        &mut interp,
        &[map, Value::Str(Rc::from("b")), Value::Int(2.into())],
    )
    .unwrap();
    if let Value::Map(m) = result {
        assert_eq!(m.borrow().len(), 2);
    } else {
        panic!("Expected map");
    }
}

#[test]
fn test_dict_remove_key_existing() {
    let mut interp = make_interp();
    let map = make_map(vec![
        ("a", Value::Int(1.into())),
        ("b", Value::Int(2.into())),
    ]);
    let result = call_stdlib(
        "std.dict",
        "remove_key",
        &mut interp,
        &[map, Value::Str(Rc::from("a"))],
    )
    .unwrap();
    if let Value::Map(m) = result {
        assert_eq!(m.borrow().len(), 1);
    } else {
        panic!("Expected map");
    }
}

// ============================================================================
// Data operations tests (JSON, CSV)
// ============================================================================

#[test]
fn test_data_json_parse_valid() {
    let mut interp = make_interp();
    let json_str = Value::Str(Rc::from(r#"{"key": "value", "num": 42}"#));
    let result = call_stdlib("std.data", "json_parse", &mut interp, &[json_str]).unwrap();
    if let Value::Map(m) = result {
        assert_eq!(m.borrow().len(), 2);
    } else {
        panic!("Expected map");
    }
}

#[test]
fn test_data_json_parse_invalid() {
    let mut interp = make_interp();
    let json_str = Value::Str(Rc::from("not valid json"));
    let result = call_stdlib("std.data", "json_parse", &mut interp, &[json_str]);
    assert!(result.is_err());
}

#[test]
fn test_data_json_stringify_map() {
    let mut interp = make_interp();
    let map = make_map(vec![("key", Value::Str(Rc::from("value")))]);
    let result = call_stdlib("std.data", "json_stringify", &mut interp, &[map]).unwrap();
    if let Value::Str(s) = result {
        assert!(s.contains("key"));
        assert!(s.contains("value"));
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_data_json_stringify_list() {
    let mut interp = make_interp();
    let list = make_list(vec![Value::Int(1.into()), Value::Int(2.into())]);
    let result = call_stdlib("std.data", "json_stringify", &mut interp, &[list]).unwrap();
    if let Value::Str(s) = result {
        assert!(s.contains("1"));
        assert!(s.contains("2"));
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_data_csv_parse_simple() {
    let mut interp = make_interp();
    let csv = Value::Str(Rc::from("name,age\nAlice,30\nBob,25"));
    let result = call_stdlib("std.data", "csv_parse", &mut interp, &[csv]).unwrap();
    if let Value::List(rows) = result {
        assert_eq!(rows.borrow().len(), 3); // 3 rows total (header + 2 data rows)
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_data_csv_stringify_simple() {
    let mut interp = make_interp();
    let rows = make_list(vec![
        make_list(vec![
            Value::Str(Rc::from("name")),
            Value::Str(Rc::from("age")),
        ]),
        make_list(vec![
            Value::Str(Rc::from("Alice")),
            Value::Str(Rc::from("30")),
        ]),
    ]);
    let result = call_stdlib("std.data", "csv_stringify", &mut interp, &[rows]).unwrap();
    if let Value::Str(s) = result {
        assert!(s.contains("name"));
        assert!(s.contains("age"));
        assert!(s.contains("Alice"));
    } else {
        panic!("Expected string");
    }
}

// ============================================================================
// Time operations tests
// ============================================================================

#[test]
fn test_time_now_returns_string() {
    let mut interp = make_interp();
    let result = call_stdlib("std.time", "now", &mut interp, &[]).unwrap();
    if let Value::Str(s) = result {
        assert!(!s.is_empty());
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_time_time_func_returns_float() {
    let mut interp = make_interp();
    let result = call_stdlib("std.time", "time", &mut interp, &[]).unwrap();
    if let Value::Float(_) = result {
        // OK
    } else {
        panic!("Expected float");
    }
}

#[test]
fn test_time_date_format() {
    let mut interp = make_interp();
    let date_str = Value::Str(Rc::from("2024-01-15"));
    let fmt = Value::Str(Rc::from("%Y/%m/%d"));
    let result = call_stdlib("std.time", "date_format", &mut interp, &[date_str, fmt]).unwrap();
    if let Value::Str(s) = result {
        assert_eq!(s.as_ref(), "2024/01/15");
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_time_date_parse() {
    let mut interp = make_interp();
    let date_str = Value::Str(Rc::from("2024-03-20"));
    let fmt = Value::Str(Rc::from("%Y-%m-%d"));
    let result = call_stdlib("std.time", "date_parse", &mut interp, &[date_str, fmt]).unwrap();
    if let Value::Str(s) = result {
        assert!(s.contains("2024"));
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_time_date_year() {
    let mut interp = make_interp();
    let date_str = Value::Str(Rc::from("2024-06-15"));
    let result = call_stdlib("std.time", "date_year", &mut interp, &[date_str]).unwrap();
    if let Value::Int(n) = result {
        assert_eq!(n, 2024.into());
    } else {
        panic!("Expected int");
    }
}

#[test]
fn test_time_date_month() {
    let mut interp = make_interp();
    let date_str = Value::Str(Rc::from("2024-06-15"));
    let result = call_stdlib("std.time", "date_month", &mut interp, &[date_str]).unwrap();
    if let Value::Int(n) = result {
        assert_eq!(n, 6.into());
    } else {
        panic!("Expected int");
    }
}

#[test]
fn test_time_date_day() {
    let mut interp = make_interp();
    let date_str = Value::Str(Rc::from("2024-06-15"));
    let result = call_stdlib("std.time", "date_day", &mut interp, &[date_str]).unwrap();
    if let Value::Int(n) = result {
        assert_eq!(n, 15.into());
    } else {
        panic!("Expected int");
    }
}

// ============================================================================
// Crypto operations tests
// ============================================================================

#[test]
fn test_crypto_md5() {
    let mut interp = make_interp();
    let input = Value::Str(Rc::from("hello"));
    let result = call_stdlib("std.crypto", "md5", &mut interp, &[input]).unwrap();
    if let Value::Str(s) = result {
        assert_eq!(s.len(), 32); // MD5 hex digest is 32 chars
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_crypto_sha256() {
    let mut interp = make_interp();
    let input = Value::Str(Rc::from("test"));
    let result = call_stdlib("std.crypto", "sha256", &mut interp, &[input]).unwrap();
    if let Value::Str(s) = result {
        assert_eq!(s.len(), 64); // SHA256 hex digest is 64 chars
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_crypto_random_range() {
    let mut interp = make_interp();
    let result = call_stdlib("std.crypto", "random", &mut interp, &[]).unwrap();
    if let Value::Float(f) = result {
        assert!((0.0..1.0).contains(&f));
    } else {
        panic!("Expected float");
    }
}

#[test]
fn test_crypto_randint() {
    let mut interp = make_interp();
    let min = Value::Int(1.into());
    let max = Value::Int(10.into());
    let result = call_stdlib("std.crypto", "randint", &mut interp, &[min, max]).unwrap();
    if let Value::Int(n) = result {
        let n_val: i64 = n.try_into().unwrap();
        assert!((1..=10).contains(&n_val));
    } else {
        panic!("Expected int");
    }
}

#[test]
fn test_crypto_choice() {
    let mut interp = make_interp();
    let list = make_list(vec![
        Value::Str(Rc::from("a")),
        Value::Str(Rc::from("b")),
        Value::Str(Rc::from("c")),
    ]);
    let result = call_stdlib("std.crypto", "choice", &mut interp, &[list]).unwrap();
    if let Value::Str(s) = result {
        let s_str: &str = s.as_ref();
        assert!(s_str == "a" || s_str == "b" || s_str == "c");
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_crypto_shuffle() {
    let mut interp = make_interp();
    let list = make_list(vec![
        Value::Int(1.into()),
        Value::Int(2.into()),
        Value::Int(3.into()),
    ]);
    let result = call_stdlib("std.crypto", "shuffle", &mut interp, &[list]).unwrap();
    if let Value::List(l) = result {
        assert_eq!(l.borrow().len(), 3);
    } else {
        panic!("Expected list");
    }
}
