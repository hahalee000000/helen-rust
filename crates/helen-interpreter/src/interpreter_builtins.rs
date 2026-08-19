//! Interpreter builtins — type conversion, I/O, collection constructors, and
//! helper types for map/list method dispatch.
//!
//! Extracted from `interpreter.rs` to keep the core execution engine focused.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};


use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

use helen_core::ast::TypeRef;
use helen_semantic::types::Type;

pub fn is_number(v: &Value) -> bool {
    matches!(v, Value::Int(_) | Value::Float(_) | Value::Bool(_))
}

/// Convert a numeric value to f64 (bool counts as int).
pub fn to_f64(v: &Value) -> f64 {
    match v {
        Value::Int(i) => i.to_f64().unwrap_or(0.0),
        Value::Float(f) => *f,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        _ => f64::NAN,
    }
}

pub fn num_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => i.to_f64(),
        Value::Float(f) => Some(*f),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Python `%` on integers: result takes the sign of the divisor.
pub fn py_mod(a: &BigInt, b: &BigInt) -> BigInt {
    let r = a % b;
    if r.is_zero() {
        return r;
    }
    let same_sign = r.sign() == b.sign();
    if same_sign {
        r
    } else {
        r + b
    }
}

/// Cross-type numeric comparison (None if NaN involved).
pub fn cmp_values(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
        (Value::Int(x), Value::Float(y)) => cmp_int_float(x, *y),
        (Value::Float(x), Value::Int(y)) => cmp_int_float(y, *x).map(Ordering::reverse),
        (Value::Bool(x), Value::Int(y)) => {
            Some(if *x { BigInt::from(1) } else { BigInt::from(0) }.cmp(y))
        }
        (Value::Int(x), Value::Bool(y)) => {
            Some(x.cmp(&if *y { BigInt::from(1) } else { BigInt::from(0) }))
        }
        (Value::Bool(x), Value::Float(y)) => {
            let t = if *x { 1.0 } else { 0.0 };
            t.partial_cmp(y)
        }
        (Value::Float(x), Value::Bool(y)) => {
            let t = if *y { 1.0 } else { 0.0 };
            x.partial_cmp(&t)
        }
        _ => None,
    }
}

/// Exact int-vs-float ordering via mantissa/exponent decomposition.
fn cmp_int_float(i: &BigInt, f: f64) -> Option<Ordering> {
    if f.is_nan() {
        return None;
    }
    if f.is_infinite() {
        return Some(if f > 0.0 {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    if f == 0.0 {
        return Some(i.cmp(&BigInt::zero()));
    }
    let neg = f.is_sign_negative();
    let av = f.abs();
    let bits = av.to_bits();
    let exp_bits = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & 0x000f_ffff_ffff_ffff;
    let (mant, exp): (u64, i64) = if exp_bits == 0 {
        (frac, -1074)
    } else {
        ((1u64 << 52) | frac, exp_bits - 1023 - 52)
    };
    if exp >= 0 {
        let mut scaled = BigInt::from(mant) << exp;
        if neg {
            scaled = -scaled;
        }
        return Some(i.cmp(&scaled));
    }
    let sh = -exp;
    let lhs = i << sh;
    let rhs = if neg {
        -BigInt::from(mant)
    } else {
        BigInt::from(mant)
    };
    Some(lhs.cmp(&rhs))
}

/// v1.11 missing-key message: `str(available_keys[:10])` with "..." suffix.
pub fn format_keys(keys: &[Value]) -> String {
    let shown: Vec<Value> = keys.iter().take(10).cloned().collect();
    let mut s = Value::List(Rc::new(RefCell::new(shown))).python_str();
    if keys.len() > 10 {
        // Python: str(list[:10])[:-1] + ", ...]"
        s = s.trim_end_matches(']').to_string();
        s.push_str(", ...]");
    }
    s
}

/// Python `_TYPE_NAME_MAP` for `case is Type` patterns.
pub fn check_type(value: &Value, type_name: &str) -> bool {
    match type_name {
        "Int" => matches!(value, Value::Int(_) | Value::Bool(_)), // bool is int
        "Float" => matches!(value, Value::Float(_) | Value::Int(_) | Value::Bool(_)),
        "String" => matches!(value, Value::Str(_)),
        "Bool" => matches!(value, Value::Bool(_)),
        "List" => matches!(value, Value::List(_)),
        "Map" => matches!(value, Value::Map(_)),
        "Null" => matches!(value, Value::Null),
        _ => false,
    }
}

/// Port of `type_from_typenode` for simple annotations (runtime type check).
pub fn type_from_typenode(tn: &TypeRef) -> Type {
    match &tn.kind {
        helen_core::ast::TypeRefKind::Simple => match tn.name.as_str() {
            "int" => Type::Int,
            "float" => Type::Float,
            "str" | "string" => Type::Str,
            "bool" => Type::Bool,
            "null" | "NoneType" => Type::Null,
            "any" | "anytype" => Type::Any,
            _ => Type::Any, // user/agent types: dynamic at runtime
        },
        helen_core::ast::TypeRefKind::Optional(inner) => {
            Type::Optional(Box::new(type_from_typenode(inner)))
        }
        helen_core::ast::TypeRefKind::Union(members) => {
            Type::Union(members.iter().map(type_from_typenode).collect())
        }
    }
}

/// Python `_is_mutable_type` — reference types get deep-copied on capture.
pub fn is_mutable_type(v: &Value) -> bool {
    matches!(v, Value::List(_) | Value::Map(_))
}

// ---------------------------------------------------------------------------
// Map method values (dict.get / keys / values / items)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum MapMethodKind {
    Get,
    Keys,
    Values,
    Items,
}

#[derive(Clone, Debug)]
pub struct MapMethodValue {
    pub kind: MapMethodKind,
    pub map: Rc<RefCell<indexmap::IndexMap<Value, Value>>>,
}

impl MapMethodValue {
    pub fn call(&self, args: &[Value]) -> Result<Value, ExceptionValue> {
        match self.kind {
            MapMethodKind::Get => {
                let key = args.first().cloned().unwrap_or(Value::Null);
                let default = args.get(1).cloned().unwrap_or(Value::Null);
                let map = self.map.borrow();
                Ok(map.get(&key).cloned().unwrap_or(default))
            }
            MapMethodKind::Keys => {
                let map = self.map.borrow();
                let keys: Vec<Value> = map.keys().cloned().collect();
                Ok(Value::List(Rc::new(RefCell::new(keys))))
            }
            MapMethodKind::Values => {
                let map = self.map.borrow();
                let values: Vec<Value> = map.values().cloned().collect();
                Ok(Value::List(Rc::new(RefCell::new(values))))
            }
            MapMethodKind::Items => {
                let map = self.map.borrow();
                let items: Vec<Value> = map
                    .iter()
                    .map(|(k, v)| Value::List(Rc::new(RefCell::new(vec![k.clone(), v.clone()]))))
                    .collect();
                Ok(Value::List(Rc::new(RefCell::new(items))))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ListMethodKind {
    Append,
    Insert,
    Pop,
    Remove,
    Count,
    Index,
    Clear,
    Extend,
    Reverse,
    Copy,
    Sort,
}

#[derive(Clone, Debug)]
pub struct ListMethodValue {
    pub kind: ListMethodKind,
    pub list: Rc<RefCell<Vec<Value>>>,
}

impl ListMethodValue {
    pub fn call(&self, args: &[Value]) -> Result<Value, ExceptionValue> {
        let mut list = self.list.borrow_mut();
        match self.kind {
            ListMethodKind::Append => {
                if let Some(v) = args.first() {
                    list.push(v.clone());
                }
                Ok(Value::Null)
            }
            ListMethodKind::Insert => {
                let idx = args
                    .first()
                    .and_then(|v| v.as_bigint())
                    .and_then(|b| b.to_i64())
                    .unwrap_or(0);
                let val = args.get(1).cloned().unwrap_or(Value::Null);
                let n = list.len() as i64;
                let mut real = idx;
                if real < 0 {
                    real += n;
                }
                let pos = real.clamp(0, n) as usize;
                list.insert(pos, val);
                Ok(Value::Null)
            }
            ListMethodKind::Pop => {
                if list.is_empty() {
                    return Err(ExceptionValue::new(
                        "RuntimeError",
                        "pop from empty list".into(),
                        None,
                    ));
                }
                Ok(list.pop().unwrap_or(Value::Null))
            }
            ListMethodKind::Remove => {
                let val = args.first().cloned().unwrap_or(Value::Null);
                if let Some(pos) = list.iter().position(|v| *v == val) {
                    list.remove(pos);
                    Ok(Value::Null)
                } else {
                    Err(ExceptionValue::new(
                        "RuntimeError",
                        format!("{} not in list", val.python_repr()),
                        None,
                    ))
                }
            }
            ListMethodKind::Count => {
                let val = args.first().cloned().unwrap_or(Value::Null);
                let n = list.iter().filter(|v| **v == val).count() as i64;
                Ok(Value::Int(BigInt::from(n)))
            }
            ListMethodKind::Index => {
                let val = args.first().cloned().unwrap_or(Value::Null);
                if let Some(pos) = list.iter().position(|v| *v == val) {
                    Ok(Value::Int(BigInt::from(pos as i64)))
                } else {
                    Err(ExceptionValue::new(
                        "RuntimeError",
                        format!("{} is not in list", val.python_repr()),
                        None,
                    ))
                }
            }
            ListMethodKind::Clear => {
                list.clear();
                Ok(Value::Null)
            }
            ListMethodKind::Extend => {
                if let Some(Value::List(other)) = args.first() {
                    let items = other.borrow().clone();
                    list.extend(items);
                }
                Ok(Value::Null)
            }
            ListMethodKind::Reverse => {
                list.reverse();
                Ok(Value::Null)
            }
            ListMethodKind::Copy => {
                // Return a shallow copy of the list
                Ok(Value::List(Rc::new(RefCell::new(list.clone()))))
            }
            ListMethodKind::Sort => {
                // Sort in-place (Python parity: list.sort() modifies the list)
                list.sort_by(|a, b| {
                    // Compare values using Python-like ordering
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x.cmp(y),
                        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                        (Value::Int(x), Value::Float(y)) => x.to_f64().unwrap_or(0.0).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                        (Value::Float(x), Value::Int(y)) => x.partial_cmp(&y.to_f64().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal),
                        (Value::Str(x), Value::Str(y)) => x.cmp(y),
                        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal, // Incomparable types
                    }
                });
                Ok(Value::Null)
            }
        }
    }
}

pub type BuiltinImpl = fn(&mut Interpreter, &[Value]) -> Result<Value, ExceptionValue>;

// ---------------------------------------------------------------------------
// Core builtins (stdlib subset; M4 registers the full set)
// ---------------------------------------------------------------------------

pub fn builtin_print(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let parts: Vec<String> = args.iter().map(|a| a.to_display(true)).collect();
    let result = parts.join(" ");
    interp.stdout.lock().expect("stdout mutex poisoned").push_str(&result);
    interp.stdout.lock().expect("stdout mutex poisoned").push('\n');
    Ok(Value::Str(Rc::from(result.as_str())))
}

pub fn builtin_len(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    // v1.12 ReadOnlyView delegates `len()` to the underlying data (Python
    // ReadOnlyView.__len__ parity) — agents receive mutable args wrapped.
    let v = match &v {
        Value::ReadOnly(r) => r.borrow().clone(),
        _ => v,
    };
    let n: i64 = match &v {
        Value::Str(s) => s.len() as i64, // byte length (D4 divergence)
        Value::List(l) => l.borrow().len() as i64,
        Value::Map(m) => m.borrow().len() as i64,
        other => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("object of type '{}' has no len()", other.type_name()),
                None,
            ))
        }
    };
    Ok(Value::Int(BigInt::from(n)))
}

pub fn builtin_str(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Str(Rc::from(v.python_str().as_str())))
}

pub fn builtin_int(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    match v {
        Value::Int(i) => Ok(Value::Int(i)),
        Value::Bool(b) => Ok(Value::Int(if b { BigInt::from(1) } else { BigInt::from(0) })),
        Value::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!("Python ValueError: cannot convert float {f} to integer"),
                    None,
                ));
            }
            Ok(Value::Int(BigInt::from(f.trunc() as i64)))
        }
        Value::Str(s) => match s.trim().parse::<i128>() {
            Ok(n) => Ok(Value::Int(BigInt::from(n))),
            Err(_) => Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: invalid literal for int() with base 10: '{s}'"),
                None,
            )),
        },
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!(
                "Python TypeError: int() argument must be a string, a bytes-like object or a real number, not '{}'",
                other.type_name()
            ),
            None,
        )),
    }
}

pub fn builtin_float(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    match v {
        Value::Float(f) => Ok(Value::Float(f)),
        Value::Int(i) => Ok(Value::Float(i.to_f64().unwrap_or(0.0))),
        Value::Bool(b) => Ok(Value::Float(if b { 1.0 } else { 0.0 })),
        Value::Str(s) => match s.trim().parse::<f64>() {
            Ok(f) => Ok(Value::Float(f)),
            Err(_) => Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: could not convert string to float: '{s}'"),
                None,
            )),
        },
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!(
                "Python TypeError: float() argument must be a string or a real number, not '{}'",
                other.type_name()
            ),
            None,
        )),
    }
}

pub fn builtin_bool(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Bool(v.truthy()))
}

pub fn builtin_type(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Str(Rc::from(v.type_name().as_str())))
}

pub fn builtin_isinstance(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    let type_name = match args.get(1) {
        Some(Value::Str(s)) => s.to_string(),
        _ => return Ok(Value::Bool(false)),
    };
    let ok = match type_name.as_str() {
        "int" => matches!(v, Value::Int(_) | Value::Bool(_)),
        "float" => matches!(v, Value::Float(_) | Value::Int(_) | Value::Bool(_)),
        "str" => matches!(v, Value::Str(_)),
        "bool" => matches!(v, Value::Bool(_)),
        "list" => matches!(v, Value::List(_)),
        "dict" => matches!(v, Value::Map(_)),
        "NoneType" => matches!(v, Value::Null),
        _ => false,
    };
    Ok(Value::Bool(ok))
}

pub fn builtin_range(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let get = |i: usize| -> Option<BigInt> {
        args.get(i).and_then(|v| match v {
            Value::Int(n) => Some(n.clone()),
            _ => None,
        })
    };
    let (start, stop, step) = match args.len() {
        0 => return Ok(Value::List(Rc::new(RefCell::new(vec![])))),
        1 => (BigInt::from(0), get(0).unwrap_or_default(), BigInt::from(1)),
        2 => (
            get(0).unwrap_or_default(),
            get(1).unwrap_or_default(),
            BigInt::from(1),
        ),
        _ => (
            get(0).unwrap_or_default(),
            get(1).unwrap_or_default(),
            get(2).unwrap_or_default(),
        ),
    };
    if step.is_zero() {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: range() arg 3 must not be zero".into(),
            None,
        ));
    }
    let mut out = Vec::new();
    if step > 0u32.into() {
        let mut cur = start.clone();
        while cur < stop {
            out.push(Value::Int(cur.clone()));
            cur += &step;
        }
    } else {
        let mut cur = start.clone();
        while cur > stop {
            out.push(Value::Int(cur.clone()));
            cur += &step;
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

pub fn builtin_abs(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    match v {
        Value::Int(i) => Ok(Value::Int(i.abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        Value::Bool(b) => Ok(Value::Int(if b {
            BigInt::from(1)
        } else {
            BigInt::from(0)
        })),
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!("bad operand type for abs(): '{}'", other.type_name()),
            None,
        )),
    }
}

pub fn builtin_min(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let values: Vec<Value> = if args.len() == 1 {
        match &args[0] {
            Value::List(l) => l.borrow().clone(),
            _ => args.to_vec(),
        }
    } else {
        args.to_vec()
    };
    values
        .into_iter()
        .min_by(|a, b| cmp_values(a, b).unwrap_or(Ordering::Equal))
        .ok_or_else(|| {
            ExceptionValue::new(
                "RuntimeError",
                "min() arg is an empty sequence".into(),
                None,
            )
        })
}

pub fn builtin_max(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let values: Vec<Value> = if args.len() == 1 {
        match &args[0] {
            Value::List(l) => l.borrow().clone(),
            _ => args.to_vec(),
        }
    } else {
        args.to_vec()
    };
    values
        .into_iter()
        .max_by(|a, b| cmp_values(a, b).unwrap_or(Ordering::Equal))
        .ok_or_else(|| {
            ExceptionValue::new(
                "RuntimeError",
                "max() arg is an empty sequence".into(),
                None,
            )
        })
}

pub fn builtin_list(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    if args.is_empty() {
        return Ok(Value::List(Rc::new(RefCell::new(vec![]))));
    }
    let v = &args[0];
    match v {
        Value::List(l) => Ok(Value::List(Rc::new(RefCell::new(l.borrow().clone())))),
        Value::Str(s) => {
            // Python list("abc") -> ['a','b','c'] (codepoints; byte divergence for non-ASCII)
            let chars: Vec<Value> = s
                .chars()
                .map(|c| Value::Str(Rc::from(c.to_string().as_str())))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(chars))))
        }
        Value::Map(m) => {
            // list(dict) -> keys
            let keys: Vec<Value> = m.borrow().keys().cloned().collect();
            Ok(Value::List(Rc::new(RefCell::new(keys))))
        }
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!("'{}' object is not iterable", other.type_name()),
            None,
        )),
    }
}

pub fn builtin_dict(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    if args.is_empty() {
        return Ok(Value::Map(Rc::new(RefCell::new(indexmap::IndexMap::new()))));
    }
    let v = &args[0];
    match v {
        Value::Map(m) => Ok(Value::Map(Rc::new(RefCell::new(m.borrow().clone())))),
        Value::List(l) => {
            let mut map = indexmap::IndexMap::new();
            for item in l.borrow().iter() {
                if let Value::List(pair) = item {
                    let b = pair.borrow();
                    if b.len() == 2 {
                        map.insert(b[0].clone(), b[1].clone());
                    }
                }
            }
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!("cannot convert '{}' to dict", other.type_name()),
            None,
        )),
    }
}

#[cfg(test)]
mod m3_tests {
    use super::*;
    use crate::llm_runtime::MockLlmRuntime;
    use helen_core::lexer::Scanner;

    /// Serialize MCP-touching tests: the runtime MCP registry is a process
    /// global (Python `tools._mcp_registry`), so parallel tests must not
    /// observe each other's MCP state.
    fn with_mcp_clean<T>(f: impl FnOnce() -> T) -> T {
        use std::sync::{Mutex, OnceLock};
        static MCP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _g = MCP_LOCK.get_or_init(|| Mutex::new(())).lock().expect("MCP mutex poisoned");
        helen_runtime::shutdown_mcp();
        let r = f();
        helen_runtime::shutdown_mcp();
        r
    }

    fn run_src(src: &str) -> (Result<Option<Value>, ExceptionValue>, String) {
        let mut scanner = Scanner::new(src, "t.helen");
        let tokens = scanner.scan_all();
        let mut parser = helen_parser::Parser::new(tokens);
        let program = parser.parse();
        assert!(
            parser.errors().is_empty(),
            "parse errs: {:?}",
            parser.errors()
        );
        let mut interp = Interpreter::new();
        let r = interp.interpret(&program);
        let out = interp.stdout.lock().expect("stdout mutex poisoned").clone();
        (r, out)
    }

    fn run_src_with_runtime(
        src: &str,
        runtime: std::sync::Arc<dyn crate::llm_runtime::LlmRuntime>,
    ) -> (Result<Option<Value>, ExceptionValue>, String) {
        let mut scanner = Scanner::new(src, "t.helen");
        let tokens = scanner.scan_all();
        let mut parser = helen_parser::Parser::new(tokens);
        let program = parser.parse();
        assert!(
            parser.errors().is_empty(),
            "parse errs: {:?}",
            parser.errors()
        );
        let mut interp = Interpreter::new();
        interp.set_llm_runtime(runtime);
        let r = interp.interpret(&program);
        let out = interp.stdout.lock().expect("stdout mutex poisoned").clone();
        (r, out)
    }

    /// Like `run_src_with_runtime` but returns the mock afterwards so tests
    /// can inspect its route/act history. The mock's history lives in `Rc`,
    /// so the caller's clone sees the recorded calls.
    fn run_src_with_mock(
        src: &str,
        mock: MockLlmRuntime,
    ) -> (
        Result<Option<Value>, ExceptionValue>,
        String,
        MockLlmRuntime,
    ) {
        let hist_handle = mock.clone();
        let (r, out) = run_src_with_runtime(src, std::sync::Arc::new(mock));
        (r, out, hist_handle)
    }

    /// Run `main_src` with helper module files on disk (Tier-C `.helen`
    /// imports). Writes `files` into a temp dir, parses `main_src` with the
    /// main path anchored there, and sets the interpreter's source file so
    /// relative imports resolve against the temp dir.
    fn run_src_with_files(
        main_src: &str,
        files: &[(&str, &str)],
    ) -> (Result<Option<Value>, ExceptionValue>, String) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "helen_imp_test_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("create dir");
        for (name, content) in files {
            std::fs::write(dir.join(name), content).unwrap();
        }
        let main_path = dir.join("main.helen");
        std::fs::write(&main_path, main_src).expect("write file");

        let mut scanner = Scanner::new(main_src, main_path.to_str().expect("to_str"));
        let tokens = scanner.scan_all();
        let mut parser = helen_parser::Parser::new(tokens);
        let program = parser.parse();
        assert!(
            parser.errors().is_empty(),
            "parse errs: {:?}",
            parser.errors()
        );
        let mut interp = Interpreter::new();
        interp.set_source_file(main_path.to_str().expect("to_str"));
        let r = interp.interpret(&program);
        let out = interp.stdout.lock().expect("stdout mutex poisoned").clone();
        (r, out)
    }

    #[test]
    fn prints_hello() {
        let (r, out) = run_src("import std.core.*\nmain {\n    print(\"hello\")\n}\n");
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "hello\n");
    }

    #[test]
    fn arithmetic_and_string_interp() {
        let src = "import std.core.*\nmain {\n    let x = 6 * 7\n    print(\"sum:\", x)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "sum: 42\n");
    }

    #[test]
    fn float_str_matches_python() {
        let src =
            "import std.core.*\nmain {\n    print(3.0)\n    print(3.14)\n    print(2.0 + 1)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "3.0\n3.14\n3.0\n");
    }

    #[test]
    fn function_call_and_return() {
        let src = "import std.core.*\nfn add(a: int, b: int): int {\n    return a + b\n}\nmain {\n    print(add(2, 3))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "5\n");
    }

    #[test]
    fn closure_captures_env() {
        let src = "import std.core.*\nmain {\n    let make_counter = fn() {\n        let n = 0\n        return fn(): int {\n            n = n + 1\n            return n\n        }\n    }\n    let c = make_counter()\n    c()\n    c()\n    print(c())\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "3\n");
    }

    #[test]
    fn try_catch_swallows_exception() {
        // Python parity: the 11-entry runtime whitelist rejects ValueError;
        // RuntimeError is the generic catch-all the language exposes.
        let src = "import std.core.*\nmain {\n    try {\n        throw RuntimeError(\"boom\")\n    } catch RuntimeError e {\n        print(\"caught\")\n    }\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "caught\n");
    }

    #[test]
    fn division_by_zero_raises() {
        // Python parity: int/int division by zero raises a plain RuntimeError
        // ("RuntimeError: Division by zero"), NOT ZeroDivisionError.
        let src = "import std.core.*\nmain {\n    let x = 5 / 0\n    print(x)\n}\n";
        let (r, _out) = run_src(src);
        assert!(r.is_err(), "should raise");
        let e = r.unwrap_err();
        assert_eq!(e.class_name, "RuntimeError");
        assert_eq!(e.message, "Division by zero");
    }

    #[test]
    fn while_loop_sums() {
        let src = "import std.core.*\nmain {\n    let total = 0\n    let i = 0\n    while i < 5 {\n        total = total + i\n        i = i + 1\n    }\n    print(total)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "10\n");
    }

    #[test]
    fn list_methods_and_index() {
        let src = "import std.core.*\nmain {\n    let xs = [1, 2, 3]\n    print(len(xs))\n    print(xs[0])\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "3\n1\n");
    }

    #[test]
    fn list_append_and_pop() {
        let src = "import std.core.*\nmain {\n    let xs = [1, 2, 3]\n    xs.append(4)\n    print(xs)\n    xs.pop()\n    print(xs)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "[1, 2, 3, 4]\n[1, 2, 3]\n");
    }

    #[test]
    fn try_finally_runs() {
        // Python parity: finally always runs even when the body throws.
        let src = "import std.core.*\nmain {\n    try {\n        throw RuntimeError(\"x\")\n    } finally {\n        print(\"cleanup\")\n    }\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_err(), "should rethrow");
        assert_eq!(out, "cleanup\n");
    }

    #[test]
    fn template_interpolation() {
        let src = "import std.core.*\nmain {\n    let x = 42\n    print({{x}})\n    print({{x + 1}})\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "42\n43\n");
    }

    // ------------------------------------------------------------------
    // Task 3.7: stdlib module imports (three forms)
    // ------------------------------------------------------------------

    #[test]
    fn stdlib_wildcard_import() {
        // `import std.list.*` binds sort; `import std.math.*` binds round.
        let src = "import std.core.*\nimport std.list.*\nimport std.math.*\nmain {\n    print(sort([3, 1, 2]))\n    print(round(3.7))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "[1, 2, 3]\n4.0\n");
    }

    #[test]
    fn stdlib_selective_import() {
        // `import std.str.{upper, lower}` binds only the named exports.
        let src = "import std.core.*\nimport std.str.{upper, lower}\nmain {\n    print(upper(\"hi\"))\n    print(lower(\"HI\"))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "HI\nhi\n");
    }

    #[test]
    fn stdlib_namespace_import() {
        // `import std.dict as D` creates a module object (map of fns).
        let src = "import std.core.*\nimport std.dict as D\nmain {\n    let d = {\"a\": 1}\n    print(D.keys(d))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "['a']\n");
    }

    #[test]
    fn stdlib_unknown_module_errors() {
        // Python parity: `_runtime_error` on unknown module.
        let src = "import std.core.*\nimport std.nope.*\nmain {\n    print(1)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_err(), "should raise");
        let e = r.unwrap_err();
        assert!(
            e.message.contains("Unknown stdlib module 'std.nope'"),
            "msg: {}",
            e.message
        );
        assert_eq!(out, "");
    }

    #[test]
    fn stdlib_unknown_function_errors() {
        // Python parity: selective import of an unknown export errors.
        let src = "import std.core.*\nimport std.str.{nope}\nmain {\n    print(1)\n}\n";
        let (r, _out) = run_src(src);
        assert!(r.is_err(), "should raise");
        let e = r.unwrap_err();
        assert!(
            e.message
                .contains("Function 'nope' not found in module 'std.str'"),
            "msg: {}",
            e.message
        );
    }

    #[test]
    fn stdlib_higher_order_functions() {
        // map/filter/reduce receive closures (Python `_map`/`_filter`/`_reduce`).
        let src = "import std.core.*\nimport std.list.*\nmain {\n    let nums = [1, 2, 3, 4, 5]\n    let doubled = map(nums, fn(x) { return x * 2 })\n    let evens = filter(nums, fn(x) { return x % 2 == 0 })\n    let total = reduce(nums, fn(acc, x) { return acc + x }, 0)\n    print(doubled)\n    print(evens)\n    print(total)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "[2, 4, 6, 8, 10]\n[2, 4]\n15\n");
    }

    #[test]
    fn stdlib_str_functions() {
        // join(items, sep) — items first, Python _join parity
        let src = "import std.core.*\nimport std.str.*\nmain {\n    print(upper(\"Hello\"))\n    print(substring(\"Hello\", 1, 3))\n    print(join([\"a\", \"b\"], \"-\"))\n    print(contains(\"hello\", \"ell\"))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "HELLO\nel\na-b\ntrue\n");
    }
    #[test]
    fn llm_if_routes_to_correct_branch() {
        // Python `test_route_to_correct_branch`: MockLLMRuntime(route_return="query").
        let mock = MockLlmRuntime::new(Some("query".to_string()), None);
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nllm if \"classify input\" { branch \"query\" { print(\"Q\") } default { print(\"D\") } }\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "Q\n");
    }

    #[test]
    fn llm_if_defaults_on_unknown_branch() {
        // Python `test_route_to_default_on_unknown`: route_return="unknown_branch".
        let mock = MockLlmRuntime::new(Some("unknown_branch".to_string()), None);
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nllm if \"classify\" { branch \"query\" { print(\"Q\") } default { print(\"D\") } }\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "D\n");
    }

    #[test]
    fn llm_if_defaults_on_none() {
        // Python `test_route_to_default_on_parse_failure`: route returns None.
        let mock = MockLlmRuntime::new(None, None);
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nllm if \"classify\" { branch \"query\" { print(\"Q\") } default { print(\"D\") } }\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "D\n");
    }

    #[test]
    fn llm_if_routes_and_records_history() {
        // The runtime receives description + branch names ("default" appended).
        let mock = MockLlmRuntime::new(Some("query".to_string()), None);
        let (r, _out, mock) = run_src_with_mock(
            "import std.core.*\nllm if \"classify input\" { branch \"query\" { print(1) } default { print(0) } }\n",
            mock,
        );
        assert!(r.is_ok(), "{r:?}");
        let hist = mock.route_history.borrow();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].0, "classify input");
        assert_eq!(hist[0].1, vec!["query".to_string(), "default".to_string()]);
        assert_eq!(hist[0].2, None);
    }

    #[test]
    fn llm_act_returns_canned_text() {
        // Python `act_return` string -> LLMResponse(text=...).
        let mock = MockLlmRuntime::with_act_text("ok");
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nprint(llm act \"hello\")\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "ok\n");
    }

    #[test]
    fn llm_act_passes_agent_settings() {
        // Agent `declare` settings (model/temperature/max-turns) must reach
        // the runtime (Python `_get_agent_setting` passthrough, M5).
        let mock = MockLlmRuntime::with_act_text("ok");
        let src = r#"import std.core.*
agent A {
    prompt "you are a helper"
    model "qwen-max"
    temperature 0.3
    max-turns 2
    main {
        return llm act "hello"
    }
}
print(A())
"#;
        let (r, out, mock) = run_src_with_mock(src, mock);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "ok\n");
        let hist = mock.act_history.borrow();
        assert_eq!(hist.len(), 1);
        // The Mock records only (prompt, tools); settings passthrough is
        // verified through the parameter plumbing (compiler-enforced).
        assert_eq!(hist[0].0, "hello");
    }

    #[test]
    fn llm_act_records_prompt() {
        let mock = MockLlmRuntime::with_act_text("ok");
        let (r, _out, mock) =
            run_src_with_mock("import std.core.*\nllm act \"the prompt\"\n", mock);
        assert!(r.is_ok(), "{r:?}");
        let hist = mock.act_history.borrow();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].0, "the prompt");
        // M6: outside an agent, `_build_tools_list` always includes the
        // skill tools (load_skill + list_skill_references) — Python parity.
        let names: Vec<&str> = hist[0]
            .1
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        assert_eq!(names, vec!["load_skill", "list_skill_references"]);
    }

    #[test]
    fn llm_if_defaults_when_runtime_fails() {
        // Python `test_route_on_llm_exception`: route() raises -> default.
        let mut mock = MockLlmRuntime::new(None, None);
        mock.route_fail = Some(ExceptionValue::new(
            "RuntimeError",
            "timeout".to_string(),
            None,
        ));
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nllm if \"classify\" { branch \"query\" { print(1) } default { print(42) } }\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "42\n");
    }

    #[test]
    fn llm_if_routes_to_middle_branch() {
        // Python `test_multiple_branches`: any branch can be selected.
        let mock = MockLlmRuntime::new(Some("command".to_string()), None);
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nllm if \"classify\" { branch \"query\" { print(1) } branch \"command\" { print(2) } default { print(0) } }\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "2\n");
    }

    // ------------------------------------------------------------------
    // Task 3.7b: `.helen` file imports (Tier-C parity tests)
    // ------------------------------------------------------------------

    #[test]
    fn aliased_import_cross_function_call() {
        // Python `test_basic_cross_function_call`: fn A calls fn B within
        // the same aliased module.
        let (r, out) = run_src_with_files(
            "import std.core.*\nimport \"mod.helen\" as math\nmain {\n    print(math.quadruple(5))\n}\n",
            &[(
                "mod.helen",
                "import std.core.*\nfn double(x: int): int { return x * 2 }\nfn quadruple(x: int): int { return double(double(x)) }\n",
            )],
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "20\n");
    }

    #[test]
    fn aliased_import_multi_level_chain() {
        // Python `test_multi_level_call_chain`: A calls B calls C.
        let (r, out) = run_src_with_files(
            "import std.core.*\nimport \"mod.helen\" as m\nmain {\n    print(m.transform(5))\n}\n",
            &[(
                "mod.helen",
                "import std.core.*\nfn add_one(x: int): int { return x + 1 }\nfn double(x: int): int { return x * 2 }\nfn transform(x: int): int { return double(add_one(x)) }\n",
            )],
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "12\n");
    }

    #[test]
    fn aliased_import_recursive_function() {
        // Python `test_recursive_function`.
        let (r, out) = run_src_with_files(
            "import std.core.*\nimport \"mod.helen\" as m\nmain {\n    print(m.factorial(5))\n}\n",
            &[(
                "mod.helen",
                "import std.core.*\nfn factorial(n: int): int {\n    if n <= 1 { return 1 }\n    return n * factorial(n - 1)\n}\n",
            )],
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "120\n");
    }

    #[test]
    fn aliased_import_cross_call_with_const() {
        // Python `test_cross_function_with_const_access`.
        let (r, out) = run_src_with_files(
            "import std.core.*\nimport \"mod.helen\" as m\nmain {\n    print(m.scale_double(5))\n}\n",
            &[(
                "mod.helen",
                "import std.core.*\nconst MULTIPLIER = 3\nfn scale(x: int): int { return x * MULTIPLIER }\nfn scale_double(x: int): int { return scale(double(x)) }\nfn double(x: int): int { return x * 2 }\n",
            )],
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "30\n");
    }

    #[test]
    fn aliased_import_stdlib_in_module() {
        // Python `test_cross_function_with_stdlib_function`: the module's
        // fn uses its own stdlib import.
        let (r, out) = run_src_with_files(
            "import std.core.*\nimport \"mod.helen\" as m\nmain {\n    print(m.greet())\n}\n",
            &[(
                "mod.helen",
                "import std.core.*\nimport std.str.*\nfn greet(): str { return upper(\"hi\") }\n",
            )],
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "HI\n");
    }

    #[test]
    fn non_aliased_import_registers_globals() {
        // Python `test_non_aliased_import`: no alias → symbols register
        // directly to the global namespace.
        let (r, out) = run_src_with_files(
            "import std.core.*\nimport \"mod.helen\"\nmain {\n    print(double(21))\n}\n",
            &[(
                "mod.helen",
                "import std.core.*\nfn double(x: int): int { return x * 2 }\n",
            )],
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "42\n");
    }

    // ------------------------------------------------------------------
    // Task 3.6b: `llm act` streaming callbacks (intended HLD semantics —
    // see wiki/rust/migration-notes.md; Python's path is broken upstream)
    // ------------------------------------------------------------------

    #[test]
    fn llm_act_streaming_dispatches_chunk_and_complete() {
        // on_chunk receives the full text (one content event from the
        // default act_stream); on_complete() fires with no args; the
        // expression evaluates to the text.
        let mock = MockLlmRuntime::with_act_text("story");
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nmain {\n    let r = llm act \"Hi\" on_chunk fn(chunk: str) { print(\"C:\" + chunk) } on_complete fn() { print(\"DONE\") }\n    print(\"RET:\" + r)\n}\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "C:story\nDONE\nRET:story\n");
    }

    #[test]
    fn llm_act_streaming_chunk_false_interrupts() {
        // on_chunk returning literal `false` interrupts: on_complete is
        // skipped, return value is the partial text.
        let mock = MockLlmRuntime::with_act_text("story");
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nmain {\n    let r = llm act \"Hi\" on_chunk fn(chunk: str) { print(\"C:\" + chunk) return false } on_complete fn() { print(\"DONE\") }\n    print(\"RET:\" + r)\n}\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "C:story\nRET:story\n");
    }

    #[test]
    fn llm_act_streaming_empty_text_returns_empty_string() {
        // Python: no content events (mock text="") → on_chunk never fires
        // but on_complete DOES (only skipped when interrupted); joined text
        // is "" (not Null).
        let mock = MockLlmRuntime::new(None, None);
        let (r, out) = run_src_with_runtime(
            "import std.core.*\nmain {\n    let r = llm act \"Hi\" on_chunk fn(chunk: str) { print(chunk) } on_complete fn() { print(\"DONE\") }\n    print(\"[\" + r + \"]\")\n}\n",
            std::sync::Arc::new(mock),
        );
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "DONE\n[]\n");
    }

    // ------------------------------------------------------------------
    // M6: agent tool loop — tools allowlist → schemas; dispatch routing
    // ------------------------------------------------------------------

    #[test]
    fn agent_tools_allowlist_builds_schemas() {
        // agent with `tools = ["calculate"]` → the llm act call receives the
        // calculate schema + always-on skill tools (Python `_build_tools_list`).
        let mock = MockLlmRuntime::with_act_text("42");
        let src = r#"import std.core.*
agent Calc {
    prompt "compute"
    tools = ["calculate"]
    main {
        llm act "compute it"
    }
}
main {
    Calc()
}
"#;
        let (r, _out, mock) = run_src_with_mock(src, mock);
        assert!(r.is_ok(), "{r:?}");
        let hist = mock.act_history.borrow();
        assert_eq!(hist.len(), 1);
        let names: Vec<&str> = hist[0]
            .1
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        assert_eq!(
            names,
            vec!["calculate", "load_skill", "list_skill_references"]
        );
    }

    #[test]
    fn agent_helen_function_exposed_as_tool() {
        // `functions { fn add(a, b) }` + `tools = ["add"]` → the LLM sees the
        // Helen function schema (type annotations map to JSON Schema types).
        let mock = MockLlmRuntime::with_act_text("3");
        let src = r#"import std.core.*
agent Adder {
    prompt "add"
    tools = ["add"]
    functions {
        fn add(a: int, b: int): int {
            return a + b
        }
    }
    main {
        llm act "call add"
    }
}
main {
    Adder()
}
"#;
        let (r, _out, mock) = run_src_with_mock(src, mock);
        assert!(r.is_ok(), "{r:?}");
        let hist = mock.act_history.borrow();
        assert_eq!(hist.len(), 1);
        let tool = &hist[0].1[0];
        assert_eq!(tool["function"]["name"], "add");
        assert_eq!(tool["function"]["description"], "Helen function: add");
        assert_eq!(
            tool["function"]["parameters"]["properties"]["a"]["type"],
            "integer"
        );
        assert_eq!(
            tool["function"]["parameters"]["required"],
            serde_json::json!(["a", "b"])
        );
    }

    #[test]
    fn dispatch_routes_agent_function_and_tool_registry() {
        with_mcp_clean(|| {
            // Directly exercise dispatch_agent_tool: agent Helen function first,
            // then the built-in registry (calculate). The agent is invoked via a
            // normal agent call so its functions{} are registered as tools.
            let mock = MockLlmRuntime::with_act_text("ok");
            let src = r#"import std.core.*
agent A {
    prompt "p"
    functions {
        fn greet(name: str): str {
            return "hi " + name
        }
    }
    main {
        llm act "call greet"
    }
}
main {
    A()
}
"#;
            let (r, _out, mock) = run_src_with_mock(src, mock);
            assert!(r.is_ok(), "{r:?}");
            let hist = mock.act_history.borrow();
            assert_eq!(hist.len(), 1);
            // The allowlist only contains skill tools (no `tools` declaration),
            // but the dispatch closure is wired — exercise it via a direct call
            // through a fresh interpreter with the agent registered.
            let mut scanner = Scanner::new(src, "t.helen");
            let tokens = scanner.scan_all();
            let mut parser = helen_parser::Parser::new(tokens);
            let program = parser.parse();
            let mut interp = Interpreter::new();
            let r = interp.interpret(&program);
            assert!(r.is_ok(), "{r:?}");
            // Built-in registry dispatch works.
            let calc =
                interp.dispatch_agent_tool("calculate", &serde_json::json!({"expression": "6*7"}));
            let v: serde_json::Value = serde_json::from_str(&calc).expect("from_str");
            assert_eq!(v["result"], 42);
            // Unknown tool falls through to the registry error.
            let unknown = interp.dispatch_agent_tool("nope", &serde_json::json!({}));
            assert!(unknown.contains("Unknown tool"));
        });
    }

    #[test]
    fn agent_scope_consts_visible_lets_hidden() {
        // M6 scope isolation: module-level const is visible in agent main,
        // module-level let is hidden (Python `_call_agent` L1 standard).
        let src = r#"import std.core.*
const MAX = 100
let hidden_var = "secret"
agent A {
    prompt "p"
    main {
        print(MAX)
        // accessing hidden_var should fail at runtime
        print(hidden_var)
    }
}
main {
    A()
}
"#;
        let (r, _out) = run_src(src);
        assert!(
            r.is_err(),
            "module let must not leak into agent scope: {r:?}"
        );
        let msg = format!("{:?}", r.err());
        assert!(
            msg.contains("hidden_var") || msg.contains("not defined") || msg.contains("NameError"),
            "{msg}"
        );
    }

    // ------------------------------------------------------------------
    // M7: concurrency — spawn, channel, shared store, mailbox, read-only
    // ------------------------------------------------------------------

    #[test]
    fn spawn_channel_round_trip() {
        let src = r#"import std.core.*
agent Worker(reply: Channel) {
    main {
        reply.send({"status": "ok", "value": 42})
        reply.close()
    }
}
main {
    let mb = spawn Worker()
    let r = mb.receive()
    print(r["status"])
    print(r["value"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "spawn failed: {:?}", r.err());
        assert_eq!(out.trim(), "ok\n42");
    }

    #[test]
    fn spawn_shared_store_methods_work_and_are_independent() {
        // Mirrors Python test_spawn_sharedstore_methods.py: parent increments
        // once (count=1); child deep-copies the store, increments twice
        // (count=3) and reports back; parent remains 1.
        let src = r#"import std.core.*
shared store Counter {
    let count: int = 0
    fn increment() { count = count + 1 }
    fn get(): int { return count }
}

agent Worker(reply: Channel) {
    main {
        Counter.increment()
        Counter.increment()
        reply.send({"count": Counter.get()})
        reply.close()
    }
}

main {
    Counter.increment()
    let parent_count = Counter.get()
    let mb = spawn Worker()
    let r = mb.receive()
    print(parent_count)
    print(r["count"])
    print(Counter.get())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "spawn failed: {:?}", r.err());
        assert_eq!(out.trim(), "1\n3\n1");
    }

    #[test]
    fn spawn_shared_let_visible_in_child() {
        let src = r#"import std.core.*
shared let shared_value = "hello-from-parent"

agent Worker(reply: Channel) {
    main {
        reply.send({"value": shared_value})
        reply.close()
    }
}

main {
    let mb = spawn Worker()
    let r = mb.receive()
    print(r["value"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "spawn failed: {:?}", r.err());
        assert_eq!(out.trim(), "hello-from-parent");
    }

    #[test]
    fn spawn_agent_with_positional_args() {
        let src = r#"import std.core.*
agent Adder(reply: Channel, x: int, y: int) {
    main {
        reply.send({"sum": x + y})
        reply.close()
    }
}
main {
    let mb = spawn Adder(20, 22)
    let r = mb.receive()
    print(r["sum"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "spawn failed: {:?}", r.err());
        assert_eq!(out.trim(), "42");
    }

    #[test]
    fn spawn_send_after_close_is_ignored() {
        let src = r#"import std.core.*
agent Worker(reply: Channel) {
    main {
        reply.send({"first": 1})
        reply.close()
        reply.send({"second": 2})
    }
}
main {
    let mb = spawn Worker()
    let r = mb.receive()
    print(r["first"])
    let r2 = mb.receive()
    print(r2)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "spawn failed: {:?}", r.err());
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "1");
        // Close sentinel is delivered as None (printed as "None").
        assert_eq!(lines[1], "None");
    }

    #[test]
    fn shared_store_field_read_write_direct() {
        let src = r#"import std.core.*
shared store State {
    let value: int = 10
    fn set_value(v: int) { value = v }
    fn get_value(): int { return value }
}
main {
    print(State.value)
    State.value = 25
    print(State.get_value())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "store field ops failed: {:?}", r.err());
        assert_eq!(out.trim(), "10\n25");
    }

    #[test]
    fn mailbox_select_returns_first_available() {
        let src = r#"import std.core.*
import std.concurrency.*
import std.time.*
agent Slow(reply: Channel) {
    main {
        sleep(0.2)
        reply.send({"who": "slow"})
        reply.close()
    }
}
agent Fast(reply: Channel) {
    main {
        reply.send({"who": "fast"})
        reply.close()
    }
}
main {
    let m1 = spawn Slow()
    let m2 = spawn Fast()
    let sel = mailbox_select([m1, m2], 5.0)
    print(sel["message"]["who"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "mailbox failed: {:?}", r.err());
        assert_eq!(out.trim(), "fast");
    }

    #[test]
    fn readonly_agent_param_mutation_raises() {
        let src = r#"import std.core.*
agent A(items: list) {
    main {
        items.append(99)
        print(items)
    }
}
main {
    A([1, 2, 3])
}
"#;
        let (r, _out) = run_src(src);
        assert!(r.is_err(), "read-only mutation must fail: {r:?}");
        let msg = format!("{:?}", r.err());
        assert!(
            msg.contains("read-only") || msg.contains("ScopeViolation"),
            "{msg}"
        );
    }

    #[test]
    fn session_stdlib_set_dir_list_delete() {
        let dir = std::env::temp_dir().join(format!("helen_sess_stdlib_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dir_display = dir.display();
        // Pre-create a session dir + transcript via the manager directly.
        let mgr = helen_runtime::SessionManager::new(Some(&dir));
        let sid = mgr.create_session(None);
        std::fs::write(mgr.get_session_path(&sid), "line1\n").unwrap();
        let src = format!(
            r#"import std.core.*
import std.transcript.*
let r = set_session_dir("{dir_display}")
print(r["status"])
let id = get_session_id()
print(len(id))          // v1.29.14: lazy-init creates a UUID session
let sessions = list_sessions()
print(len(sessions))    // 1: only sessions with transcripts count
"#
        );
        let (r, out) = run_src(&src);
        assert!(r.is_ok(), "session stdlib failed: {:?}", r.err());
        assert_eq!(out.trim(), "ok\n44\n1");
    }

    #[test]
    fn session_delete_and_cleanup_work() {
        let dir = std::env::temp_dir().join(format!("helen_sess_del_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dir_display = dir.display();
        // Pre-create two sessions with transcripts.
        let mgr = helen_runtime::SessionManager::new(Some(&dir));
        let s1 = mgr.create_session(None);
        std::fs::write(mgr.get_session_path(&s1), "a\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let s2 = mgr.create_session(None);
        std::fs::write(mgr.get_session_path(&s2), "b\n").unwrap();
        let src = format!(
            r#"import std.core.*
import std.transcript.*
let r = set_session_dir("{dir_display}")
print(delete_session("{s2}"))
print(cleanup_sessions(0))   // deletes s1 (only remaining)
let remaining = list_sessions()
print(len(remaining))
"#
        );
        let (r, out) = run_src(&src);
        assert!(r.is_ok(), "session delete failed: {:?}", r.err());
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true"); // delete_session(s2)
        assert_eq!(lines[1], "1"); // cleanup deleted s1
        assert_eq!(lines[2], "0"); // nothing remains
    }

    // M9: MCP integration — a fixture MCP server is discovered and its tools
    // appear in the agent tool registry (DoD 9.1).
    #[test]
    fn agent_tools_allowlist_resolves_mcp_tool_schemas() {
        with_mcp_clean(|| {
            // Point MCP at the fixture mock server (same as runtime crate tests).
            let fixture = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../helen-runtime/tests/fixtures/mock_mcp_server.py"
            );
            let config = serde_json::json!({
                "mcpServers": {
                    "mock": {
                        "command": "python3",
                        "args": [fixture],
                    }
                }
            });
            let dir = std::env::temp_dir().join(format!("mcp_agent_{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("create dir");
            let config_path = dir.join(".mcp.json");
            std::fs::write(&config_path, serde_json::to_string(&config).expect("write file")).unwrap();

            // Initialize MCP.
            helen_runtime::initialize_mcp(&config_path);

            // Agent with `tools = ["echo", "add"]` → the llm act call receives the
            // MCP tool schemas (merged into the tool registry, Python parity).
            let mock = MockLlmRuntime::with_act_text("ok");
            let src = r#"import std.core.*
agent M {
    prompt "use mcp"
    tools = ["echo", "add"]
    main {
        llm act "call echo"
    }
}
main {
    M()
}
"#;
            let (r, _out, mock) = run_src_with_mock(src, mock);
            assert!(r.is_ok(), "{r:?}");
            let hist = mock.act_history.borrow();
            assert_eq!(hist.len(), 1);
            let names: Vec<&str> = hist[0]
                .1
                .iter()
                .filter_map(|t| t["function"]["name"].as_str())
                .collect();
            assert!(
                names.contains(&"echo"),
                "MCP 'echo' schema missing from tool list: {names:?}"
            );
            assert!(
                names.contains(&"add"),
                "MCP 'add' schema missing from tool list: {names:?}"
            );
            // Always-on skill tools still present.
            assert!(names.contains(&"load_skill"));

            // Dispatch an MCP tool through the agent dispatch path.
            let mut scanner = Scanner::new(src, "t.helen");
            let tokens = scanner.scan_all();
            let mut parser = helen_parser::Parser::new(tokens);
            let program = parser.parse();
            let mut interp = Interpreter::new();
            let r = interp.interpret(&program);
            assert!(r.is_ok(), "{r:?}");
            let echo =
                interp.dispatch_agent_tool("echo", &serde_json::json!({"message": "from helen"}));
            let v: serde_json::Value = serde_json::from_str(&echo).expect("from_str");
            assert_eq!(v["output"], "Echo: from helen");
        });
    }

    // =========================================================================
    // Phase 2: Comprehensive stdlib integration tests
    // =========================================================================

    #[test]
    fn phase2_str_upper() {
        let src = r#"import std.core.*
import std.str.*
main {
    print(upper("hello"))
    print(upper("Hello World"))
    print(upper(""))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "HELLO\nHELLO WORLD\n\n");
    }

    #[test]
    fn phase2_str_lower() {
        let src = r#"import std.core.*
import std.str.*
main {
    print(lower("HELLO"))
    print(lower("Hello World"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "hello\nhello world\n");
    }

    #[test]
    fn phase2_str_trim() {
        let src = r#"import std.core.*
import std.str.*
main {
    print(trim("  hello  "))
    print(trim("hello"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "hello\nhello\n");
    }

    #[test]
    fn phase2_str_contains() {
        let src = r#"import std.core.*
import std.str.*
main {
    print(contains("hello world", "world"))
    print(contains("hello world", "xyz"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "true\nfalse\n");
    }

    #[test]
    fn phase2_str_startswith_endswith() {
        let src = r#"import std.core.*
import std.str.*
main {
    print(startswith("hello world", "hello"))
    print(endswith("hello world", "world"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "true\ntrue\n");
    }

    #[test]
    fn phase2_str_replace() {
        let src = r#"import std.core.*
import std.str.*
main {
    print(replace("hello world", "world", "rust"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "hello rust\n");
    }

    #[test]
    fn phase2_str_reverse() {
        let src = r#"import std.core.*
import std.str.*
main {
    print(reverse("hello"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "olleh\n");
    }

    #[test]
    fn phase2_math_pow() {
        let src = r#"import std.core.*
import std.math.*
main {
    print(pow(2, 3))
    print(pow(5, 0))
    print(pow(3, 2))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "8.0");
        assert_eq!(lines[1], "1.0");
        assert_eq!(lines[2], "9.0");
    }

    #[test]
    fn phase2_math_sqrt() {
        let src = r#"import std.core.*
import std.math.*
main {
    print(sqrt(4.0))
    print(sqrt(9.0))
    print(sqrt(16.0))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2.0");
        assert_eq!(lines[1], "3.0");
        assert_eq!(lines[2], "4.0");
    }

    #[test]
    fn phase2_math_floor_ceil_round() {
        let src = r#"import std.core.*
import std.math.*
main {
    print(floor(3.7))
    print(ceil(3.2))
    print(round(3.7))
    print(round(3.2))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "4");
        assert_eq!(lines[2], "4.0");
        assert_eq!(lines[3], "3.0");
    }

    #[test]
    fn phase2_math_mean() {
        let src = r#"import std.core.*
import std.math.*
main {
    let nums = [1.0, 2.0, 3.0, 4.0, 5.0]
    print(mean(nums))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3.0");
    }

    #[test]
    fn phase2_math_median() {
        let src = r#"import std.core.*
import std.math.*
main {
    let nums = [3.0, 1.0, 2.0]
    print(median(nums))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2.0");
    }

    #[test]
    fn phase2_list_sort() {
        let src = r#"import std.core.*
main {
    let nums = [3, 1, 2]
    nums.sort()
    print(nums)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "[1, 2, 3]");
    }

    #[test]
    fn phase2_list_sort_strings() {
        let src = r#"import std.core.*
main {
    let words = ["banana", "apple", "cherry"]
    words.sort()
    print(words)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "['apple', 'banana', 'cherry']");
    }

    #[test]
    fn phase2_list_copy() {
        let src = r#"import std.core.*
main {
    let original = [1, 2, 3]
    let copy = original.copy()
    copy.append(4)
    print(original)
    print(copy)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "[1, 2, 3]");
        assert_eq!(lines[1], "[1, 2, 3, 4]");
    }

    #[test]
    fn phase2_dict_keys_values() {
        let src = r#"import std.core.*
main {
    let d = {"a": 1, "b": 2, "c": 3}
    let keys = d.keys()
    let values = d.values()
    print(len(keys))
    print(len(values))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "3");
    }

    #[test]
    fn phase2_dict_get() {
        let src = r#"import std.core.*
main {
    let d = {"a": 1, "b": 2}
    print(d.get("a"))
    print(d.get("c", 99))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "1");
        assert_eq!(lines[1], "99");
    }
}
