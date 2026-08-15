//! Helen runtime value model.
//!
//! Byte-faithful port of the value semantics in `helen/interpreter/interpreter.py`
//! (v1.44.0) plus the stdlib display rules from `helen/stdlib/__init__.py`:
//!
//! - Python truthiness (0 / 0.0 / "" / [] / {} / null are falsy).
//! - Python `==` semantics: `1 == 1.0`, `true == 1`, `[1] == [1.0]`,
//!   `{1:"a"} == {1.0:"a"}` (order-independent dict equality).
//! - Display parity (D11): top-level `print(x)` uses Python `str()` with
//!   bools lowered to `true`/`false`; nested elements use Python `repr()`
//!   (`True`/`False`/`None`, single-quoted strings).
//! - Map keys are structural (D5): `1` and `1.0` collide exactly as in Python.
//!
//! Known deliberate divergence: strings are native UTF-8 bytes (byte-based
//! `len`/index/slice) whereas the Python reference uses codepoint semantics.
//! Recorded in `wiki/rust/migration-notes.md`.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use indexmap::IndexMap;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

use helen_core::ast::{AgentDecl, FunctionDecl};
use helen_core::ast_printer::{py_str_float, py_str_repr};

use crate::closure::Closure;
use crate::exceptions::ExceptionValue;

/// A stdlib builtin function (M4 registers the full 378; the core subset is
/// available from the start). Holds a name and an implementation pointer.
#[derive(Clone)]
pub struct BuiltinFn {
    pub name: String,
    pub module: &'static str,
    pub func: fn(&mut crate::interpreter::Interpreter, &[Value]) -> Result<Value, ExceptionValue>,
}

impl std::fmt::Debug for BuiltinFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<builtin {}>", self.name)
    }
}

/// A Helen runtime value.
#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    /// Arbitrary-precision integer (D3) — no overflow path.
    Int(BigInt),
    Float(f64),
    /// Native UTF-8 bytes; `len`/index/slice are byte-based (D4).
    Str(Rc<str>),
    List(Rc<RefCell<Vec<Value>>>),
    /// Insertion-ordered map with arbitrary structural keys (D5).
    Map(Rc<RefCell<IndexMap<Value, Value>>>),
    /// A thrown/raised Helen exception (catch binds this to the error var).
    Exception(Box<ExceptionValue>),
    /// A stdlib builtin function.
    BuiltinFn(Rc<BuiltinFn>),
    /// A user-defined function referenced as a first-class value.
    UserFn(Rc<FunctionDecl>),
    /// An agent referenced as a first-class value.
    Agent(Rc<AgentDecl>),
    /// A closure (lambda + captured environment).
    Closure(Rc<Closure>),
    /// A `start..end` range pattern (internal match marker).
    Range(Box<Value>, Box<Value>),
    /// A bound dict method (`m.get`, `m.keys`, ...).
    MapMethod(Box<crate::interpreter::MapMethodValue>),
    /// A bound list method (`l.append`, `l.pop`, ...).
    ListMethod(Box<crate::interpreter::ListMethodValue>),
}

impl Value {
    /// Python truthiness: null / false / 0 / 0.0 / "" / [] / {} are falsy.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => !i.is_zero(),
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.borrow().is_empty(),
            Value::Map(m) => !m.borrow().is_empty(),
            Value::Exception(_) => true,
            Value::BuiltinFn(_) | Value::UserFn(_) | Value::Agent(_) | Value::Closure(_) => true,
            Value::Range(_, _) | Value::MapMethod(_) | Value::ListMethod(_) => true,
        }
    }

    /// Python `type(x).__name__` for the `type()` builtin and error messages.
    pub fn type_name(&self) -> String {
        match self {
            Value::Null => "NoneType".into(),
            Value::Bool(_) => "bool".into(),
            Value::Int(_) => "int".into(),
            Value::Float(_) => "float".into(),
            Value::Str(_) => "str".into(),
            Value::List(_) => "list".into(),
            Value::Map(_) => "dict".into(),
            Value::Exception(e) => e.class_name.clone(),
            Value::BuiltinFn(b) => b.name.clone(),
            Value::UserFn(f) => f.name.clone(),
            Value::Agent(a) => a.name.clone(),
            Value::Closure(_) => "function".into(),
            Value::Range(_, _) => "range".into(),
            Value::MapMethod(_) | Value::ListMethod(_) => "method".into(),
        }
    }

    /// Display parity (D11).
    ///
    /// `top_level=true` mirrors `print(arg)`: bools lower to `true`/`false`,
    /// everything else uses Python `str()`. Nested values use Python `repr()`
    /// (`True`/`False`/`None`, quoted strings, floats unchanged).
    pub fn to_display(&self, top_level: bool) -> String {
        if top_level {
            match self {
                Value::Bool(b) => return if *b { "true".into() } else { "false".into() },
                _ => return self.python_str(),
            }
        }
        self.python_repr()
    }

    /// Python `str(value)`. Containers render their elements with `repr()`.
    pub fn python_str(&self) -> String {
        match self {
            Value::Null => "None".into(),
            Value::Bool(b) => {
                if *b {
                    "True".into()
                } else {
                    "False".into()
                }
            }
            Value::Int(i) => i.to_string(),
            Value::Float(f) => py_str_float(*f),
            Value::Str(s) => s.to_string(),
            Value::List(l) => {
                let items: Vec<String> = l.borrow().iter().map(|v| v.python_repr()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Map(m) => {
                let items: Vec<String> = m
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.python_repr(), v.python_repr()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            Value::Exception(e) => e.to_display_string(),
            Value::BuiltinFn(b) => format!("<built-in function {}>", b.name),
            Value::UserFn(f) => format!("<function {}>", f.name),
            Value::Agent(a) => format!("<agent {}>", a.name),
            Value::Closure(_) => "<function <lambda>>".into(),
            Value::Range(a, b) => format!("{}..{}", a.python_str(), b.python_str()),
            Value::MapMethod(_) | Value::ListMethod(_) => "<dict method>".into(),
        }
    }

    /// Python `repr(value)`.
    pub fn python_repr(&self) -> String {
        match self {
            Value::Null => "None".into(),
            Value::Bool(b) => {
                if *b {
                    "True".into()
                } else {
                    "False".into()
                }
            }
            Value::Int(i) => i.to_string(),
            Value::Float(f) => py_str_float(*f),
            Value::Str(s) => py_str_repr(s),
            Value::List(l) => {
                let items: Vec<String> = l.borrow().iter().map(|v| v.python_repr()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Map(m) => {
                let items: Vec<String> = m
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.python_repr(), v.python_repr()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            Value::Exception(e) => e.to_display_string(),
            Value::BuiltinFn(b) => format!("<built-in function {}>", b.name),
            Value::UserFn(f) => format!("<function {}>", f.name),
            Value::Agent(a) => format!("<agent {}>", a.name),
            Value::Closure(_) => "<function <lambda>>".into(),
            Value::Range(a, b) => format!("{}..{}", a.python_str(), b.python_str()),
            Value::MapMethod(_) | Value::ListMethod(_) => "<dict method>".into(),
        }
    }

    /// Deep copy (Python `copy.deepcopy`) — used for spawn/env snapshots.
    pub fn clone_deep(&self) -> Value {
        match self {
            Value::List(l) => {
                let copied = l.borrow().iter().map(Value::clone_deep).collect();
                Value::List(Rc::new(RefCell::new(copied)))
            }
            Value::Map(m) => {
                let copied: IndexMap<Value, Value> = m
                    .borrow()
                    .iter()
                    .map(|(k, v)| (k.clone_deep(), v.clone_deep()))
                    .collect();
                Value::Map(Rc::new(RefCell::new(copied)))
            }
            other => other.clone(),
        }
    }

    /// Create a Helen exception value (Python `raise exc_class(message, span)`).
    pub fn exception(
        class_name: &str,
        message: String,
        span: Option<helen_core::source::SourceSpan>,
    ) -> Value {
        Value::Exception(Box::new(ExceptionValue::new(class_name, message, span)))
    }

    /// Integer value if this is an Int (used by `range` step etc.).
    pub fn as_bigint(&self) -> Option<&BigInt> {
        match self {
            Value::Int(i) => Some(i),
            _ => None,
        }
    }
}

/// Exact Python int-vs-float comparison: `i == f` (mathematical equality).
fn float_eq_int(f: f64, i: &BigInt) -> bool {
    if f.is_nan() || f.is_infinite() {
        return false;
    }
    if f == 0.0 {
        return i.is_zero();
    }
    let neg = f.is_sign_negative();
    let av = f.abs();
    let bits = av.to_bits();
    let exp_bits = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & 0x000f_ffff_ffff_ffff;
    // f = mant * 2^exp with integer mant, exactly.
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
        return *i == scaled;
    }
    // i == sign*mant*2^exp  <=>  i * 2^-exp == sign*mant
    let sh = -exp;
    let rhs = if neg {
        -BigInt::from(mant)
    } else {
        BigInt::from(mant)
    };
    (i << sh) == rhs
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            // f64 PartialEq: NaN != NaN (matches Python).
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::List(a), Value::List(b)) => *a.borrow() == *b.borrow(),
            (Value::Map(a), Value::Map(b)) => *a.borrow() == *b.borrow(),
            // Python bool is an int subclass: true == 1, false == 0.
            (Value::Bool(b), Value::Int(i)) => {
                *i == if *b { BigInt::from(1) } else { BigInt::from(0) }
            }
            (Value::Int(i), Value::Bool(b)) => {
                *i == if *b { BigInt::from(1) } else { BigInt::from(0) }
            }
            (Value::Bool(b), Value::Float(f)) => {
                let t = if *b { 1.0 } else { 0.0 };
                *f == t
            }
            (Value::Float(f), Value::Bool(b)) => {
                let t = if *b { 1.0 } else { 0.0 };
                *f == t
            }
            (Value::Int(i), Value::Float(f)) => float_eq_int(*f, i),
            (Value::Float(f), Value::Int(i)) => float_eq_int(*f, i),
            (Value::Exception(a), Value::Exception(b)) => {
                a.class_name == b.class_name && a.message == b.message && a.span == b.span
            }
            (Value::BuiltinFn(a), Value::BuiltinFn(b)) => a.name == b.name,
            (Value::UserFn(a), Value::UserFn(b)) => a.name == b.name,
            (Value::Agent(a), Value::Agent(b)) => a.name == b.name,
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            (Value::Range(a, b), Value::Range(c, d)) => a == c && b == d,
            _ => false,
        }
    }
}

// NOTE: `Eq` is implemented for map-key ergonomics. The one contract
// violation is NaN (NaN != NaN), which deliberately mirrors Python dict
// behaviour: `{nan: 1}[nan]` raises KeyError.
impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Null => 0u8.hash(state),
            Value::Bool(b) => (if *b { 1i64 } else { 0i64 }).hash(state),
            Value::Int(i) => {
                // Hash through i64 when possible so Int(1) collides with
                // Float(1.0)/Bool(true) exactly as Python's dict does.
                if let Some(v) = i.to_i64() {
                    v.hash(state);
                } else {
                    i.hash(state);
                }
            }
            Value::Float(f) => {
                // Python hash parity: hash(1.0) == hash(1), hash(-0.0) == hash(0).
                if *f == 0.0 {
                    0i64.hash(state);
                } else if f.is_finite() && f.fract() == 0.0 && f.abs() < 9.223372036854776e18 {
                    (*f as i64).hash(state);
                } else {
                    f.to_bits().hash(state);
                }
            }
            Value::Str(s) => s.hash(state),
            // Lists/maps are unhashable in Python; we never reach here at
            // runtime (the interpreter rejects unhashable map keys first).
            Value::List(l) => {
                0x1u8.hash(state);
                let b = l.borrow();
                for v in b.iter() {
                    v.hash(state);
                }
            }
            Value::Map(m) => {
                0x2u8.hash(state);
                let mb = m.borrow();
                for (k, v) in mb.iter() {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Value::Exception(e) => {
                0x3u8.hash(state);
                e.class_name.hash(state);
                e.message.hash(state);
            }
            Value::BuiltinFn(b) => {
                0x4u8.hash(state);
                b.name.hash(state);
            }
            Value::UserFn(f) => {
                0x5u8.hash(state);
                f.name.hash(state);
            }
            Value::Agent(a) => {
                0x6u8.hash(state);
                a.name.hash(state);
            }
            Value::Closure(c) => {
                0x7u8.hash(state);
                std::ptr::hash(Rc::as_ptr(c), state);
            }
            Value::Range(a, b) => {
                0x8u8.hash(state);
                a.hash(state);
                b.hash(state);
            }
            Value::MapMethod(m) => {
                0x9u8.hash(state);
                std::ptr::hash(Rc::as_ptr(&m.map), state);
            }
            Value::ListMethod(m) => {
                0xAu8.hash(state);
                std::ptr::hash(Rc::as_ptr(&m.list), state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(n: i64) -> Value {
        Value::Int(BigInt::from(n))
    }
    fn lst(items: Vec<Value>) -> Value {
        Value::List(Rc::new(RefCell::new(items)))
    }
    fn mp(pairs: Vec<(Value, Value)>) -> Value {
        let mut m = IndexMap::new();
        for (k, v) in pairs {
            m.insert(k, v);
        }
        Value::Map(Rc::new(RefCell::new(m)))
    }

    #[test]
    fn truthiness_matches_python() {
        assert!(!Value::Null.truthy());
        assert!(!Value::Bool(false).truthy());
        assert!(Value::Bool(true).truthy());
        assert!(!int(0).truthy());
        assert!(int(5).truthy());
        assert!(!Value::Float(0.0).truthy());
        assert!(Value::Float(0.1).truthy());
        assert!(Value::Float(f64::NAN).truthy()); // nan != 0 -> True
        assert!(!Value::Str("".into()).truthy());
        assert!(Value::Str("a".into()).truthy());
        assert!(!lst(vec![]).truthy());
        assert!(lst(vec![int(0)]).truthy());
        assert!(!mp(vec![]).truthy());
        assert!(mp(vec![(int(1), int(2))]).truthy());
    }

    #[test]
    fn equality_int_float_bool_cross_type() {
        assert!(int(1) == Value::Float(1.0));
        assert!(int(0) == Value::Float(0.0));
        assert!(int(1) == Value::Bool(true));
        assert!(int(0) == Value::Bool(false));
        assert!(Value::Float(1.0) == Value::Bool(true));
        assert!(int(2) != Value::Float(2.5));
        assert!(int(1) != Value::Str("1".into()));
        assert!(Value::Null != int(0));
    }

    #[test]
    fn equality_large_int_float_exact() {
        // 2**100 == float(2**100) is True in Python.
        let big: BigInt = BigInt::from(1u8) << 100;
        assert!(Value::Int(big.clone()) == Value::Float(big.to_f64().unwrap()));
        // 2**53 + 1 != 2.0**53 (float rounds down).
        let p53p1 = (BigInt::from(1u8) << 53) + BigInt::from(1u8);
        assert!(Value::Int(p53p1) != Value::Float(9007199254740992.0));
        // negatives
        assert!(Value::Int(BigInt::from(-2)) == Value::Float(-2.0));
        assert!(Value::Int(BigInt::from(-1)) != Value::Float(1.0));
        // nan never equal
        assert!(Value::Float(f64::NAN) != Value::Float(f64::NAN));
    }

    #[test]
    fn equality_containers_structural() {
        assert!(lst(vec![int(1), int(2)]) == lst(vec![Value::Float(1.0), Value::Float(2.0)]));
        assert!(lst(vec![lst(vec![int(1)])]) == lst(vec![lst(vec![Value::Float(1.0)])]));
        assert!(lst(vec![int(1)]) != lst(vec![int(1), int(2)]));
        // dict equality is order-independent with structural keys
        assert!(
            mp(vec![(int(1), Value::Str("a".into()))])
                == mp(vec![(Value::Float(1.0), Value::Str("a".into()))])
        );
        let d1 = mp(vec![
            (int(1), Value::Str("a".into())),
            (int(2), Value::Str("b".into())),
        ]);
        let d2 = mp(vec![
            (int(2), Value::Str("b".into())),
            (int(1), Value::Str("a".into())),
        ]);
        assert!(d1 == d2);
        assert!(mp(vec![]) == mp(vec![]));
    }

    #[test]
    fn map_key_collision_int_float_and_bool() {
        let mut m = IndexMap::new();
        m.insert(int(1), Value::Str("a".into()));
        m.insert(Value::Float(1.0), Value::Str("b".into()));
        // Python: {1: 'a', 1.0: 'b'} -> {1: 'b'}
        assert_eq!(m.len(), 1);
        assert!(m.contains_key(&int(1)));
        assert_eq!(m.get(&int(1)), Some(&Value::Str("b".into())));

        let mut m2 = IndexMap::new();
        m2.insert(Value::Bool(true), Value::Str("a".into()));
        m2.insert(int(1), Value::Str("b".into()));
        // Python: {True: 'a', 1: 'b'} -> {True: 'b'}
        assert_eq!(m2.len(), 1);
        assert!(m2.contains_key(&Value::Bool(true)));
        assert_eq!(m2.get(&Value::Bool(true)), Some(&Value::Str("b".into())));
    }

    #[test]
    fn display_top_level_and_nested() {
        assert_eq!(Value::Bool(true).to_display(true), "true");
        assert_eq!(Value::Bool(false).to_display(true), "false");
        assert_eq!(Value::Null.to_display(true), "None");
        assert_eq!(int(42).to_display(true), "42");
        assert_eq!(Value::Float(3.5).to_display(true), "3.5");
        assert_eq!(Value::Float(3.0).to_display(true), "3.0");
        assert_eq!(Value::Float(1e20).to_display(true), "1e+20");
        assert_eq!(Value::Float(1.5e-05).to_display(true), "1.5e-05");
        assert_eq!(Value::Float(1e-4).to_display(true), "0.0001");
        assert_eq!(Value::Float(1e16).to_display(true), "1e+16");
        assert_eq!(Value::Str("hello".into()).to_display(true), "hello");
        // nested repr: bools/None upper-case, strings quoted
        let l = lst(vec![
            int(1),
            Value::Str("a".into()),
            Value::Bool(true),
            Value::Null,
            Value::Float(3.5),
            lst(vec![int(2)]),
        ]);
        assert_eq!(l.to_display(true), "[1, 'a', True, None, 3.5, [2]]");
        let d = mp(vec![
            (Value::Str("a".into()), int(1)),
            (int(2), Value::Str("b".into())),
        ]);
        assert_eq!(d.to_display(true), "{'a': 1, 2: 'b'}");
        // nested strings use repr escapes
        let s = Value::Str("a'b\"c\\d\ne".into());
        assert_eq!(s.to_display(false), "'a\\'b\"c\\\\d\\ne'");
    }

    #[test]
    fn str_vs_repr_scalar() {
        // top-level str of a str is the string itself; repr adds quotes
        assert_eq!(Value::Str("abc".into()).to_display(true), "abc");
        assert_eq!(Value::Str("abc".into()).python_repr(), "'abc'");
        assert_eq!(Value::Bool(true).python_repr(), "True");
        assert_eq!(Value::Null.python_repr(), "None");
    }

    #[test]
    fn type_names_match_python() {
        assert_eq!(Value::Null.type_name(), "NoneType");
        assert_eq!(Value::Bool(true).type_name(), "bool");
        assert_eq!(int(1).type_name(), "int");
        assert_eq!(Value::Float(1.0).type_name(), "float");
        assert_eq!(Value::Str("x".into()).type_name(), "str");
        assert_eq!(lst(vec![]).type_name(), "list");
        assert_eq!(mp(vec![]).type_name(), "dict");
        assert_eq!(
            Value::exception("TimeoutError", "slow".into(), None).type_name(),
            "TimeoutError"
        );
    }

    #[test]
    fn clone_deep_copies_containers() {
        let inner = lst(vec![int(1)]);
        let outer = lst(vec![inner.clone(), int(2)]);
        let copy = outer.clone_deep();
        if let (Value::List(o), Value::List(c)) = (&outer, &copy) {
            assert!(!Rc::ptr_eq(o, c));
            let ob = o.borrow();
            let cb = c.borrow();
            if let (Value::List(oi), Value::List(ci)) = (&ob[0], &cb[0]) {
                assert!(!Rc::ptr_eq(oi, ci));
            } else {
                panic!("expected inner list");
            }
            assert!(ob[0] == cb[0]);
            assert!(ob[1] == cb[1]);
        } else {
            panic!("expected lists");
        }
    }
}
