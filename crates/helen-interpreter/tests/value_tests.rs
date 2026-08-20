//! Tests for value module — runtime value operations.

use helen_interpreter::value::Value;
use indexmap::IndexMap;
use num_bigint::BigInt;
use std::cell::RefCell;
use std::rc::Rc;

// ── Truthiness ──────────────────────────────────────────────────────────

#[test]
fn test_truthy_null() {
    assert!(!Value::Null.truthy());
}

#[test]
fn test_truthy_bool() {
    assert!(Value::Bool(true).truthy());
    assert!(!Value::Bool(false).truthy());
}

#[test]
fn test_truthy_int() {
    assert!(Value::Int(BigInt::from(42)).truthy());
    assert!(Value::Int(BigInt::from(-1)).truthy());
    assert!(!Value::Int(BigInt::from(0)).truthy());
}

#[test]
fn test_truthy_float() {
    assert!(Value::Float(1.234).truthy());
    assert!(Value::Float(-0.5).truthy());
    assert!(!Value::Float(0.0).truthy());
}

#[test]
fn test_truthy_str() {
    assert!(Value::Str(Rc::from("hello")).truthy());
    assert!(!Value::Str(Rc::from("")).truthy());
}

#[test]
fn test_truthy_list() {
    let empty = Value::List(Rc::new(RefCell::new(vec![])));
    let non_empty = Value::List(Rc::new(RefCell::new(vec![Value::Int(BigInt::from(1))])));
    assert!(!empty.truthy());
    assert!(non_empty.truthy());
}

#[test]
fn test_truthy_tuple() {
    let empty = Value::Tuple(Rc::new(RefCell::new(vec![])));
    let non_empty = Value::Tuple(Rc::new(RefCell::new(vec![Value::Int(BigInt::from(1))])));
    assert!(!empty.truthy());
    assert!(non_empty.truthy());
}

#[test]
fn test_truthy_map() {
    let empty = Value::Map(Rc::new(RefCell::new(IndexMap::new())));
    let mut m = IndexMap::new();
    m.insert(Value::Int(BigInt::from(1)), Value::Str(Rc::from("one")));
    let non_empty = Value::Map(Rc::new(RefCell::new(m)));
    assert!(!empty.truthy());
    assert!(non_empty.truthy());
}

// ── Type names ──────────────────────────────────────────────────────────

#[test]
fn test_type_name_null() {
    assert_eq!(Value::Null.type_name(), "NoneType");
}

#[test]
fn test_type_name_bool() {
    assert_eq!(Value::Bool(true).type_name(), "bool");
}

#[test]
fn test_type_name_int() {
    assert_eq!(Value::Int(BigInt::from(42)).type_name(), "int");
}

#[test]
fn test_type_name_float() {
    assert_eq!(Value::Float(1.234).type_name(), "float");
}

#[test]
fn test_type_name_str() {
    assert_eq!(Value::Str(Rc::from("hello")).type_name(), "str");
}

#[test]
fn test_type_name_list() {
    assert_eq!(
        Value::List(Rc::new(RefCell::new(vec![]))).type_name(),
        "list"
    );
}

#[test]
fn test_type_name_tuple() {
    assert_eq!(
        Value::Tuple(Rc::new(RefCell::new(vec![]))).type_name(),
        "tuple"
    );
}

#[test]
fn test_type_name_map() {
    assert_eq!(
        Value::Map(Rc::new(RefCell::new(IndexMap::new()))).type_name(),
        "dict"
    );
}

// ── Display (to_display) ────────────────────────────────────────────────

#[test]
fn test_display_null_top_level() {
    assert_eq!(Value::Null.to_display(true), "None");
}

#[test]
fn test_display_bool_top_level() {
    assert_eq!(Value::Bool(true).to_display(true), "true");
    assert_eq!(Value::Bool(false).to_display(true), "false");
}

#[test]
fn test_display_bool_nested() {
    assert_eq!(Value::Bool(true).to_display(false), "True");
    assert_eq!(Value::Bool(false).to_display(false), "False");
}

#[test]
fn test_display_int() {
    assert_eq!(Value::Int(BigInt::from(42)).to_display(true), "42");
    assert_eq!(Value::Int(BigInt::from(-7)).to_display(true), "-7");
}

#[test]
fn test_display_float() {
    assert_eq!(Value::Float(1.234).to_display(true), "1.234");
    assert_eq!(Value::Float(0.0).to_display(true), "0.0");
}

#[test]
fn test_display_str() {
    assert_eq!(Value::Str(Rc::from("hello")).to_display(true), "hello");
}

#[test]
fn test_display_list_top_level() {
    let list = Value::List(Rc::new(RefCell::new(vec![
        Value::Int(BigInt::from(1)),
        Value::Str(Rc::from("two")),
    ])));
    assert_eq!(list.to_display(true), "[1, 'two']");
}

#[test]
fn test_display_map_top_level() {
    let mut m = IndexMap::new();
    m.insert(Value::Str(Rc::from("k")), Value::Int(BigInt::from(42)));
    let map = Value::Map(Rc::new(RefCell::new(m)));
    assert_eq!(map.to_display(true), "{'k': 42}");
}

// ── Python repr ─────────────────────────────────────────────────────────

#[test]
fn test_python_repr_null() {
    assert_eq!(Value::Null.python_repr(), "None");
}

#[test]
fn test_python_repr_bool() {
    assert_eq!(Value::Bool(true).python_repr(), "True");
    assert_eq!(Value::Bool(false).python_repr(), "False");
}

#[test]
fn test_python_repr_int() {
    assert_eq!(Value::Int(BigInt::from(42)).python_repr(), "42");
}

#[test]
fn test_python_repr_str() {
    assert_eq!(Value::Str(Rc::from("hello")).python_repr(), "'hello'");
}

#[test]
fn test_python_repr_list() {
    let list = Value::List(Rc::new(RefCell::new(vec![
        Value::Int(BigInt::from(1)),
        Value::Bool(true),
    ])));
    assert_eq!(list.python_repr(), "[1, True]");
}

#[test]
fn test_python_repr_tuple() {
    let tuple = Value::Tuple(Rc::new(RefCell::new(vec![
        Value::Int(BigInt::from(1)),
        Value::Int(BigInt::from(2)),
    ])));
    assert_eq!(tuple.python_repr(), "(1, 2)");
}

#[test]
fn test_python_repr_map() {
    let mut m = IndexMap::new();
    m.insert(Value::Int(BigInt::from(1)), Value::Str(Rc::from("one")));
    let map = Value::Map(Rc::new(RefCell::new(m)));
    assert_eq!(map.python_repr(), "{1: 'one'}");
}

// ── Python str ──────────────────────────────────────────────────────────

#[test]
fn test_python_str_null() {
    assert_eq!(Value::Null.python_str(), "None");
}

#[test]
fn test_python_str_bool() {
    assert_eq!(Value::Bool(true).python_str(), "True");
    assert_eq!(Value::Bool(false).python_str(), "False");
}

#[test]
fn test_python_str_str() {
    assert_eq!(Value::Str(Rc::from("hello")).python_str(), "hello");
}

// ── Deep clone ──────────────────────────────────────────────────────────

#[test]
fn test_clone_deep_list() {
    let original = Value::List(Rc::new(RefCell::new(vec![
        Value::Int(BigInt::from(1)),
        Value::Int(BigInt::from(2)),
    ])));
    let cloned = original.clone_deep();

    // Modify original
    if let Value::List(l) = &original {
        l.borrow_mut().push(Value::Int(BigInt::from(3)));
    }

    // Clone should be unchanged
    if let Value::List(l) = &cloned {
        assert_eq!(l.borrow().len(), 2);
    }
}

#[test]
fn test_clone_deep_map() {
    let mut m = IndexMap::new();
    m.insert(Value::Str(Rc::from("k")), Value::Int(BigInt::from(1)));
    let original = Value::Map(Rc::new(RefCell::new(m)));
    let cloned = original.clone_deep();

    // Modify original
    if let Value::Map(map) = &original {
        map.borrow_mut()
            .insert(Value::Str(Rc::from("k2")), Value::Int(BigInt::from(2)));
    }

    // Clone should be unchanged
    if let Value::Map(map) = &cloned {
        assert_eq!(map.borrow().len(), 1);
    }
}

#[test]
fn test_clone_deep_primitives() {
    // Primitives should just clone
    let int_val = Value::Int(BigInt::from(42));
    let cloned = int_val.clone_deep();
    assert_eq!(cloned, int_val);
}

// ── Clone owned ─────────────────────────────────────────────────────────

#[test]
fn test_clone_owned_str() {
    let original = Value::Str(Rc::from("hello"));
    let owned = original.clone_owned();
    assert_eq!(owned, original);
}

#[test]
fn test_clone_owned_list() {
    let original = Value::List(Rc::new(RefCell::new(vec![
        Value::Int(BigInt::from(1)),
        Value::Str(Rc::from("two")),
    ])));
    let owned = original.clone_owned();
    assert_eq!(owned, original);
}

#[test]
fn test_clone_owned_tuple() {
    let original = Value::Tuple(Rc::new(RefCell::new(vec![
        Value::Int(BigInt::from(1)),
        Value::Int(BigInt::from(2)),
    ])));
    let owned = original.clone_owned();
    assert_eq!(owned, original);
}

#[test]
fn test_clone_owned_map() {
    let mut m = IndexMap::new();
    m.insert(Value::Str(Rc::from("k")), Value::Int(BigInt::from(1)));
    let original = Value::Map(Rc::new(RefCell::new(m)));
    let owned = original.clone_owned();
    assert_eq!(owned, original);
}

// ── Exception factory ───────────────────────────────────────────────────

#[test]
fn test_exception_factory() {
    let exc = Value::exception("RuntimeError", "test error".to_string(), None);
    if let Value::Exception(e) = exc {
        assert_eq!(e.class_name, "RuntimeError");
        assert_eq!(e.message, "test error");
    } else {
        panic!("Expected Exception");
    }
}

// ── Mutable type check ──────────────────────────────────────────────────

#[test]
fn test_is_mutable_type() {
    assert!(!Value::Null.is_mutable_type());
    assert!(!Value::Bool(true).is_mutable_type());
    assert!(!Value::Int(BigInt::from(1)).is_mutable_type());
    assert!(!Value::Float(1.0).is_mutable_type());
    assert!(!Value::Str(Rc::from("s")).is_mutable_type());
    assert!(Value::List(Rc::new(RefCell::new(vec![]))).is_mutable_type());
    assert!(Value::Map(Rc::new(RefCell::new(IndexMap::new()))).is_mutable_type());
}

// ── as_bigint ───────────────────────────────────────────────────────────

#[test]
fn test_as_bigint_some() {
    let val = Value::Int(BigInt::from(42));
    assert!(val.as_bigint().is_some());
    assert_eq!(val.as_bigint().unwrap(), &BigInt::from(42));
}

#[test]
fn test_as_bigint_none() {
    assert!(Value::Null.as_bigint().is_none());
    assert!(Value::Float(1.0).as_bigint().is_none());
    assert!(Value::Str(Rc::from("1")).as_bigint().is_none());
}
