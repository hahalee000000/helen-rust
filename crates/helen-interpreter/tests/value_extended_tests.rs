//! Extended tests for value module — clone_deep, clone_owned, make_send_owned, etc.

use helen_interpreter::value::Value;
use indexmap::IndexMap;
use num_bigint::BigInt;
use std::cell::RefCell;
use std::rc::Rc;

// ── clone_deep tests ────────────────────────────────────────────────────

#[test]
fn clone_deep_null() {
    let v = Value::Null;
    let cloned = v.clone_deep();
    assert_eq!(cloned.type_name(), "NoneType");
}

#[test]
fn clone_deep_int() {
    let v = Value::Int(BigInt::from(42));
    let cloned = v.clone_deep();
    match cloned {
        Value::Int(n) => assert_eq!(n, BigInt::from(42)),
        _ => panic!("expected Int"),
    }
}

#[test]
fn clone_deep_float() {
    let v = Value::Float(1.234);
    let cloned = v.clone_deep();
    match cloned {
        Value::Float(f) => assert_eq!(f, 1.234),
        _ => panic!("expected Float"),
    }
}

#[test]
fn clone_deep_str() {
    let v = Value::Str(Rc::from("hello"));
    let cloned = v.clone_deep();
    match cloned {
        Value::Str(s) => assert_eq!(s.as_ref(), "hello"),
        _ => panic!("expected Str"),
    }
}

#[test]
fn clone_deep_bool() {
    let v = Value::Bool(true);
    let cloned = v.clone_deep();
    match cloned {
        Value::Bool(b) => assert!(b),
        _ => panic!("expected Bool"),
    }
}

#[test]
fn clone_deep_list() {
    let v = Value::List(Rc::new(RefCell::new(vec![
        Value::Int(BigInt::from(1)),
        Value::Int(BigInt::from(2)),
    ])));
    let cloned = v.clone_deep();
    match cloned {
        Value::List(l) => {
            let l = l.borrow();
            assert_eq!(l.len(), 2);
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn clone_deep_map() {
    let mut m = IndexMap::new();
    m.insert(Value::Str(Rc::from("key")), Value::Int(BigInt::from(42)));
    let v = Value::Map(Rc::new(RefCell::new(m)));
    let cloned = v.clone_deep();
    match cloned {
        Value::Map(map) => {
            let map = map.borrow();
            assert_eq!(map.len(), 1);
        }
        _ => panic!("expected Map"),
    }
}

#[test]
fn clone_deep_tuple() {
    let v = Value::Tuple(Rc::new(RefCell::new(vec![
        Value::Int(BigInt::from(1)),
        Value::Str(Rc::from("two")),
    ])));
    let cloned = v.clone_deep();
    match cloned {
        Value::Tuple(t) => {
            let t = t.borrow();
            assert_eq!(t.len(), 2);
        }
        _ => panic!("expected Tuple"),
    }
}

// ── clone_owned tests ───────────────────────────────────────────────────

#[test]
fn clone_owned_null() {
    let v = Value::Null;
    let cloned = v.clone_owned();
    assert_eq!(cloned.type_name(), "NoneType");
}

#[test]
fn clone_owned_int() {
    let v = Value::Int(BigInt::from(42));
    let cloned = v.clone_owned();
    match cloned {
        Value::Int(n) => assert_eq!(n, BigInt::from(42)),
        _ => panic!("expected Int"),
    }
}

#[test]
fn clone_owned_str() {
    let v = Value::Str(Rc::from("test"));
    let cloned = v.clone_owned();
    match cloned {
        Value::Str(s) => assert_eq!(s.as_ref(), "test"),
        _ => panic!("expected Str"),
    }
}

#[test]
fn clone_owned_list() {
    let v = Value::List(Rc::new(RefCell::new(vec![Value::Int(BigInt::from(1))])));
    let cloned = v.clone_owned();
    match cloned {
        Value::List(l) => {
            let l = l.borrow();
            assert_eq!(l.len(), 1);
        }
        _ => panic!("expected List"),
    }
}

// ── make_send_owned tests ───────────────────────────────────────────────

#[test]
fn make_send_owned_null() {
    let v = Value::Null;
    let owned = v.make_send_owned();
    assert_eq!(owned.type_name(), "NoneType");
}

#[test]
fn make_send_owned_int() {
    let v = Value::Int(BigInt::from(99));
    let owned = v.make_send_owned();
    match owned {
        Value::Int(n) => assert_eq!(n, BigInt::from(99)),
        _ => panic!("expected Int"),
    }
}

#[test]
fn make_send_owned_str() {
    let v = Value::Str(Rc::from("send"));
    let owned = v.make_send_owned();
    match owned {
        Value::Str(s) => assert_eq!(s.as_ref(), "send"),
        _ => panic!("expected Str"),
    }
}

#[test]
fn make_send_owned_bool() {
    let v = Value::Bool(false);
    let owned = v.make_send_owned();
    match owned {
        Value::Bool(b) => assert!(!b),
        _ => panic!("expected Bool"),
    }
}

// ── is_mutable_type tests ───────────────────────────────────────────────

#[test]
fn is_mutable_type_list() {
    let v = Value::List(Rc::new(RefCell::new(vec![])));
    assert!(v.is_mutable_type());
}

#[test]
fn is_mutable_type_map() {
    let v = Value::Map(Rc::new(RefCell::new(IndexMap::new())));
    assert!(v.is_mutable_type());
}

#[test]
fn is_mutable_type_tuple() {
    let v = Value::Tuple(Rc::new(RefCell::new(vec![])));
    assert!(!v.is_mutable_type()); // Tuples are immutable in Helen
}

#[test]
fn is_mutable_type_int() {
    let v = Value::Int(BigInt::from(1));
    assert!(!v.is_mutable_type());
}

#[test]
fn is_mutable_type_str() {
    let v = Value::Str(Rc::from("test"));
    assert!(!v.is_mutable_type());
}

#[test]
fn is_mutable_type_bool() {
    let v = Value::Bool(true);
    assert!(!v.is_mutable_type());
}

#[test]
fn is_mutable_type_null() {
    let v = Value::Null;
    assert!(!v.is_mutable_type());
}

#[test]
fn is_mutable_type_float() {
    let v = Value::Float(1.0);
    assert!(!v.is_mutable_type());
}

// ── as_bigint tests ─────────────────────────────────────────────────────

#[test]
fn as_bigint_some() {
    let v = Value::Int(BigInt::from(42));
    let result = v.as_bigint();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), &BigInt::from(42));
}

#[test]
fn as_bigint_none_for_str() {
    let v = Value::Str(Rc::from("not an int"));
    assert!(v.as_bigint().is_none());
}

#[test]
fn as_bigint_none_for_null() {
    let v = Value::Null;
    assert!(v.as_bigint().is_none());
}

#[test]
fn as_bigint_none_for_float() {
    let v = Value::Float(1.234);
    assert!(v.as_bigint().is_none());
}

// ── exception factory tests ─────────────────────────────────────────────

#[test]
fn exception_creates_runtime_error() {
    let exc = Value::exception("RuntimeError", "test error".to_string(), None);
    match exc {
        Value::Exception(e) => {
            assert_eq!(e.class_name, "RuntimeError");
            assert_eq!(e.message, "test error");
        }
        _ => panic!("expected Exception"),
    }
}

#[test]
fn exception_creates_value_error() {
    let exc = Value::exception("ValueError", "bad value".to_string(), None);
    match exc {
        Value::Exception(e) => {
            assert_eq!(e.class_name, "ValueError");
            assert_eq!(e.message, "bad value");
        }
        _ => panic!("expected Exception"),
    }
}

// ── Display formatting edge cases ───────────────────────────────────────

#[test]
fn display_empty_list() {
    let v = Value::List(Rc::new(RefCell::new(vec![])));
    let display = v.to_display(true);
    assert_eq!(display, "[]");
}

#[test]
fn display_empty_map() {
    let v = Value::Map(Rc::new(RefCell::new(IndexMap::new())));
    let display = v.to_display(true);
    assert_eq!(display, "{}");
}

#[test]
fn display_empty_tuple() {
    let v = Value::Tuple(Rc::new(RefCell::new(vec![])));
    let display = v.to_display(true);
    assert_eq!(display, "()");
}

#[test]
fn display_nested_list() {
    let inner = Value::List(Rc::new(RefCell::new(vec![Value::Int(BigInt::from(1))])));
    let outer = Value::List(Rc::new(RefCell::new(vec![inner])));
    let display = outer.to_display(true);
    assert!(display.contains("["));
}

#[test]
fn python_str_null() {
    let v = Value::Null;
    assert_eq!(v.python_str(), "None");
}

#[test]
fn python_str_bool_true() {
    let v = Value::Bool(true);
    assert_eq!(v.python_str(), "True");
}

#[test]
fn python_str_bool_false() {
    let v = Value::Bool(false);
    assert_eq!(v.python_str(), "False");
}

#[test]
fn python_repr_null() {
    let v = Value::Null;
    assert_eq!(v.python_repr(), "None");
}

#[test]
fn python_repr_str() {
    let v = Value::Str(Rc::from("hello"));
    let repr = v.python_repr();
    assert!(repr.contains("hello"));
}
