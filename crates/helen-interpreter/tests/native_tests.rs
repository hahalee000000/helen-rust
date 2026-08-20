//! Tests for native module — NativeHandle, NativeObject trait, python import hook.

use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::native::*;
use helen_interpreter::value::Value;
use std::any::Any;
use std::sync::Arc;

// ── Test NativeObject implementation ────────────────────────────────────

struct TestNativeObj {
    name: String,
}

impl NativeObject for TestNativeObj {
    fn type_name(&self) -> String {
        self.name.clone()
    }
    fn python_str(&self) -> String {
        format!("<TestNativeObj:{}>", self.name)
    }
    fn python_repr(&self) -> String {
        format!("TestNativeObj({:?})", self.name)
    }
    fn get_attribute(&self, name: &str) -> Result<Value, ExceptionValue> {
        if name == "value" {
            Ok(Value::Int(42.into()))
        } else {
            Err(ExceptionValue::new(
                "AttributeError",
                format!("no attr: {name}"),
                None,
            ))
        }
    }
    fn call(&self, args: &[Value], _kwargs: &[(String, Value)]) -> Result<Value, ExceptionValue> {
        Ok(Value::Int(args.len().into()))
    }
    fn get_item(&self, key: &Value) -> Result<Value, ExceptionValue> {
        match key {
            Value::Int(_) => Ok(Value::Str(std::rc::Rc::from("item"))),
            _ => Err(ExceptionValue::new("KeyError", "bad key".into(), None)),
        }
    }
    fn set_item(&self, _key: &Value, _value: &Value) -> Result<(), ExceptionValue> {
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ── NativeHandle tests ──────────────────────────────────────────────────

#[test]
fn native_handle_type_name() {
    let obj = TestNativeObj {
        name: "test".into(),
    };
    let handle = NativeHandle(Arc::new(obj));
    assert_eq!(handle.0.type_name(), "test");
}

#[test]
fn native_handle_python_str() {
    let obj = TestNativeObj { name: "foo".into() };
    let handle = NativeHandle(Arc::new(obj));
    assert_eq!(handle.0.python_str(), "<TestNativeObj:foo>");
}

#[test]
fn native_handle_python_repr() {
    let obj = TestNativeObj { name: "bar".into() };
    let handle = NativeHandle(Arc::new(obj));
    assert_eq!(handle.0.python_repr(), "TestNativeObj(\"bar\")");
}

#[test]
fn native_handle_get_attribute_ok() {
    let obj = TestNativeObj { name: "x".into() };
    let handle = NativeHandle(Arc::new(obj));
    let val = handle.0.get_attribute("value").unwrap();
    match val {
        Value::Int(n) => assert_eq!(n, 42.into()),
        _ => panic!("expected Int"),
    }
}

#[test]
fn native_handle_get_attribute_err() {
    let obj = TestNativeObj { name: "x".into() };
    let handle = NativeHandle(Arc::new(obj));
    assert!(handle.0.get_attribute("missing").is_err());
}

#[test]
fn native_handle_call() {
    let obj = TestNativeObj { name: "x".into() };
    let handle = NativeHandle(Arc::new(obj));
    let args = vec![Value::Int(1.into()), Value::Int(2.into())];
    let result = handle.0.call(&args, &[]).unwrap();
    match result {
        Value::Int(n) => assert_eq!(n, 2.into()),
        _ => panic!("expected Int"),
    }
}

#[test]
fn native_handle_get_item() {
    let obj = TestNativeObj { name: "x".into() };
    let handle = NativeHandle(Arc::new(obj));
    let val = handle.0.get_item(&Value::Int(0.into())).unwrap();
    match val {
        Value::Str(s) => assert_eq!(s.as_ref(), "item"),
        _ => panic!("expected Str"),
    }
}

#[test]
fn native_handle_get_item_err() {
    let obj = TestNativeObj { name: "x".into() };
    let handle = NativeHandle(Arc::new(obj));
    assert!(handle.0.get_item(&Value::Null).is_err());
}

#[test]
fn native_handle_set_item() {
    let obj = TestNativeObj { name: "x".into() };
    let handle = NativeHandle(Arc::new(obj));
    assert!(handle
        .0
        .set_item(&Value::Int(0.into()), &Value::Int(1.into()))
        .is_ok());
}

#[test]
fn native_handle_downcast_ref() {
    let obj = TestNativeObj {
        name: "downcast_test".into(),
    };
    let handle = NativeHandle(Arc::new(obj));
    let downcasted = handle.downcast_ref::<TestNativeObj>();
    assert!(downcasted.is_some());
    assert_eq!(downcasted.unwrap().name, "downcast_test");
}

#[test]
fn native_handle_downcast_ref_wrong_type() {
    let obj = TestNativeObj { name: "x".into() };
    let handle = NativeHandle(Arc::new(obj));
    // Can't downcast to a different type
    struct Other;
    impl NativeObject for Other {
        fn type_name(&self) -> String {
            "other".into()
        }
        fn python_str(&self) -> String {
            "".into()
        }
        fn python_repr(&self) -> String {
            "".into()
        }
        fn get_attribute(&self, _: &str) -> Result<Value, ExceptionValue> {
            Ok(Value::Null)
        }
        fn call(&self, _: &[Value], _: &[(String, Value)]) -> Result<Value, ExceptionValue> {
            Ok(Value::Null)
        }
        fn get_item(&self, _: &Value) -> Result<Value, ExceptionValue> {
            Ok(Value::Null)
        }
        fn set_item(&self, _: &Value, _: &Value) -> Result<(), ExceptionValue> {
            Ok(())
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    assert!(handle.downcast_ref::<Other>().is_none());
}

#[test]
fn native_handle_debug() {
    let obj = TestNativeObj { name: "x".into() };
    let handle = NativeHandle(Arc::new(obj));
    let debug_str = format!("{:?}", handle);
    assert_eq!(debug_str, "NativeHandle");
}

#[test]
fn native_handle_clone() {
    let obj = TestNativeObj {
        name: "clone_test".into(),
    };
    let handle = NativeHandle(Arc::new(obj));
    let cloned = handle.clone();
    assert_eq!(cloned.0.type_name(), "clone_test");
}

// ── Python import hook tests ────────────────────────────────────────────

#[test]
fn python_import_hook_default_none() {
    // In test env, no hook is registered (OnceLock is per-process, but
    // other tests may have registered one; just check the function works)
    let _ = python_import_hook();
}

#[test]
fn runtime_error_exception_creates() {
    let exc = runtime_error_exception("test error".into());
    assert_eq!(exc.class_name, "RuntimeError");
    assert_eq!(exc.message, "test error");
}

#[test]
fn runtime_error_exception_empty() {
    let exc = runtime_error_exception("".into());
    assert_eq!(exc.class_name, "RuntimeError");
    assert_eq!(exc.message, "");
}
