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
use crate::shared_store::SharedStoreInstance;

/// A message payload transferred through a `Channel`.
///
/// Wraps `Value`; `unsafe impl Send` is justified by `make_send_owned`: every
/// value placed in a message is deep-owned first (fresh `Rc` allocations,
/// deep-copied stores), so no allocation inside a message is shared with the
/// sending interpreter. This mirrors Python's GIL-safe object sharing while
/// being intentionally stricter (single-owner transfer).
#[derive(Clone, Debug)]
pub struct ChannelMsg(pub Value);
// SAFETY: payloads are deep-owned at the send boundary (see make_send_owned).
// Nested Channel/SharedStore references are Arc-based (Send + Sync).
unsafe impl Send for ChannelMsg {}

impl helen_runtime::channel::Queueable for ChannelMsg {
    fn sentinel() -> Self {
        ChannelMsg(Value::Null)
    }
}

/// A bound shared-store method (`Counter.increment`).
#[derive(Clone, Debug)]
pub struct StoreMethodValue {
    pub store: std::sync::Arc<SharedStoreInstance>,
    pub name: String,
}

/// A bound channel-endpoint method (`mb.receive`).
#[derive(Clone, Debug)]
pub struct ChannelMethodValue {
    pub endpoint: std::sync::Arc<helen_runtime::channel::ChannelEndpoint<ChannelMsg>>,
    pub name: String,
}

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
    /// A tuple — immutable list-like; rendered `(a, b)` (Python str parity).
    Tuple(Rc<RefCell<Vec<Value>>>),
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
    /// A channel endpoint (v1.18 spawn) — shared across threads via Arc.
    Channel(std::sync::Arc<helen_runtime::channel::ChannelEndpoint<ChannelMsg>>),
    /// A shared store instance (v1.12).
    SharedStore(std::sync::Arc<SharedStoreInstance>),
    /// A bound shared-store method (`Counter.increment`).
    StoreMethod(Box<StoreMethodValue>),
    /// A bound channel-endpoint method (`mb.receive`).
    ChannelMethod(Box<ChannelMethodValue>),
    /// Read-only wrapper for reference types passed to agents (v1.12).
    ReadOnly(Rc<RefCell<Value>>),
    /// A native (foreign-language) object — Python FFI objects etc. (M10).
    Native(crate::native::NativeHandle),
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
            Value::Tuple(t) => !t.borrow().is_empty(),
            Value::Map(m) => !m.borrow().is_empty(),
            Value::Exception(_) => true,
            Value::BuiltinFn(_) | Value::UserFn(_) | Value::Agent(_) | Value::Closure(_) => true,
            Value::Range(_, _)
            | Value::MapMethod(_)
            | Value::ListMethod(_)
            | Value::Channel(_)
            | Value::SharedStore(_)
            | Value::StoreMethod(_)
            | Value::ChannelMethod(_)
            | Value::Native(_) => true,
            // v1.12: ReadOnlyView delegates truthiness to underlying data.
            Value::ReadOnly(r) => r.borrow().truthy(),
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
            Value::Tuple(_) => "tuple".into(),
            Value::Map(_) => "dict".into(),
            Value::Exception(e) => e.class_name.clone(),
            Value::BuiltinFn(b) => b.name.clone(),
            Value::UserFn(f) => f.name.clone(),
            Value::Agent(a) => a.name.clone(),
            Value::Closure(_) => "function".into(),
            Value::Range(_, _) => "range".into(),
            Value::MapMethod(_) | Value::ListMethod(_) => "method".into(),
            Value::Channel(_) => "ChannelEndpoint".into(),
            Value::SharedStore(_) => "SharedStore".into(),
            Value::StoreMethod(_) => "SharedStoreMethod".into(),
            Value::ChannelMethod(_) => "method".into(),
            Value::ReadOnly(r) => r.borrow().type_name(),
            Value::Native(n) => n.0.type_name(),
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
            Value::Tuple(t) => {
                let items: Vec<String> = t.borrow().iter().map(|v| v.python_repr()).collect();
                format!("({})", items.join(", "))
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
            Value::Channel(ep) => format!(
                "ChannelEndpoint({:?}, {})",
                ep.channel().name,
                if ep.is_main_thread() {
                    "main"
                } else {
                    "spawned"
                }
            ),
            Value::SharedStore(s) => format!(
                "<SharedStore {} with {} fields, {} methods>",
                s.name,
                s.field_order.len(),
                s.methods.len()
            ),
            Value::StoreMethod(sm) => {
                format!("<SharedStoreMethod {}.{}>", sm.store.name, sm.name)
            }
            Value::ChannelMethod(cm) => format!("<channel method {}.{}>", "endpoint", cm.name),
            // v1.12: ReadOnlyView stringifies as its underlying data.
            Value::ReadOnly(r) => r.borrow().python_str(),
            // M10: native objects stringify via their Python `str()`.
            Value::Native(n) => n.0.python_str(),
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
            Value::Tuple(t) => {
                let items: Vec<String> = t.borrow().iter().map(|v| v.python_repr()).collect();
                format!("({})", items.join(", "))
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
            Value::Channel(ep) => format!(
                "ChannelEndpoint({:?}, {})",
                ep.channel().name,
                if ep.is_main_thread() {
                    "main"
                } else {
                    "spawned"
                }
            ),
            Value::SharedStore(s) => format!(
                "<SharedStore {} with {} fields, {} methods>",
                s.name,
                s.field_order.len(),
                s.methods.len()
            ),
            Value::StoreMethod(sm) => {
                format!("<SharedStoreMethod {}.{}>", sm.store.name, sm.name)
            }
            Value::ChannelMethod(cm) => format!("<channel method {}.{}>", "endpoint", cm.name),
            // v1.12: ReadOnlyView repr wraps the data.
            Value::ReadOnly(r) => format!("ReadOnly({})", r.borrow().python_repr()),
            // M10: native objects repr via their Python `repr()`.
            Value::Native(n) => n.0.python_repr(),
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

    /// Deep-own clone: like `clone_deep` but also reallocates `Rc<str>`
    /// strings and deep-copies shared stores / channels. The result shares no
    /// `Rc` allocation with the source — safe to move across threads
    /// (single-owner transfer).
    pub fn clone_owned(&self) -> Value {
        match self {
            Value::Str(s) => Value::Str(Rc::from(s.as_ref())),
            Value::List(l) => {
                let copied = l.borrow().iter().map(Value::clone_owned).collect();
                Value::List(Rc::new(RefCell::new(copied)))
            }
            Value::Tuple(t) => {
                let copied = t.borrow().iter().map(Value::clone_owned).collect();
                Value::Tuple(Rc::new(RefCell::new(copied)))
            }
            Value::Map(m) => {
                let copied: IndexMap<Value, Value> = m
                    .borrow()
                    .iter()
                    .map(|(k, v)| (k.clone_owned(), v.clone_owned()))
                    .collect();
                Value::Map(Rc::new(RefCell::new(copied)))
            }
            Value::SharedStore(s) => Value::SharedStore(s.deep_copy()),
            Value::Channel(ep) => Value::Channel(ep.clone()), // Arc — shared, Send+Sync
            Value::BuiltinFn(f) => Value::BuiltinFn(Rc::new(f.as_ref().clone())),
            Value::UserFn(f) => Value::UserFn(Rc::new(f.as_ref().clone())),
            Value::Agent(a) => Value::Agent(Rc::new(a.as_ref().clone())),
            Value::MapMethod(mm) => {
                let inner: IndexMap<Value, Value> = mm
                    .map
                    .borrow()
                    .iter()
                    .map(|(k, v)| (k.clone_owned(), v.clone_owned()))
                    .collect();
                Value::MapMethod(Box::new(crate::interpreter::MapMethodValue {
                    kind: mm.kind.clone(),
                    map: Rc::new(RefCell::new(inner)),
                }))
            }
            Value::ListMethod(lm) => {
                let inner: Vec<Value> = lm.list.borrow().iter().map(Value::clone_owned).collect();
                Value::ListMethod(Box::new(crate::interpreter::ListMethodValue {
                    kind: lm.kind.clone(),
                    list: Rc::new(RefCell::new(inner)),
                }))
            }
            Value::ReadOnly(r) => Value::ReadOnly(Rc::new(RefCell::new(r.borrow().clone_owned()))),
            Value::StoreMethod(sm) => Value::StoreMethod(Box::new(StoreMethodValue {
                store: sm.store.deep_copy(),
                name: sm.name.clone(),
            })),
            Value::ChannelMethod(cm) => Value::ChannelMethod(Box::new(ChannelMethodValue {
                endpoint: cm.endpoint.clone(), // Arc — shared, Send+Sync
                name: cm.name.clone(),
            })),
            other => other.clone(),
        }
    }

    /// Build a deep-owned message payload for `Channel::send`.
    ///
    /// Function/closure references cannot be deep-owned; sending them is
    /// rejected with an error map (documented deviation: Python's GIL would
    /// share them; Rust is intentionally stricter).
    pub fn make_send_owned(&self) -> Value {
        match self {
            Value::Closure(_) => Value::Map(Rc::new(RefCell::new(IndexMap::from([
                (Value::Str(Rc::from("__error__")), Value::Bool(true)),
                (
                    Value::Str(Rc::from("message")),
                    Value::Str(Rc::from(
                        "cannot send a closure through a channel (single-owner transfer)",
                    )),
                ),
            ])))),
            other => other.clone_owned(),
        }
    }

    /// True for reference types wrapped in `ReadOnlyView` (Python
    /// `_is_mutable_type`: list / dict).
    pub fn is_mutable_type(&self) -> bool {
        matches!(self, Value::List(_) | Value::Map(_))
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
            (Value::Tuple(a), Value::Tuple(b)) => *a.borrow() == *b.borrow(),
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
            // Identity equality for channels/stores (Python object identity).
            (Value::Channel(a), Value::Channel(b)) => std::sync::Arc::ptr_eq(a, b),
            (Value::SharedStore(a), Value::SharedStore(b)) => std::sync::Arc::ptr_eq(a, b),
            (Value::StoreMethod(a), Value::StoreMethod(b)) => {
                std::sync::Arc::ptr_eq(&a.store, &b.store) && a.name == b.name
            }
            (Value::ChannelMethod(a), Value::ChannelMethod(b)) => {
                std::sync::Arc::ptr_eq(&a.endpoint, &b.endpoint) && a.name == b.name
            }
            // v1.12: ReadOnlyView equality delegates to the underlying data.
            (Value::ReadOnly(a), Value::ReadOnly(b)) => *a.borrow() == *b.borrow(),
            (Value::ReadOnly(a), other) => *a.borrow() == *other,
            (other, Value::ReadOnly(b)) => *other == *b.borrow(),
            // M10: native object identity (Python `is` — wrapper has no __eq__).
            (Value::Native(a), Value::Native(b)) => std::sync::Arc::ptr_eq(&a.0, &b.0),
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
            Value::Tuple(t) => {
                0xBu8.hash(state);
                let b = t.borrow();
                for v in b.iter() {
                    v.hash(state);
                }
            }
            Value::Channel(c) => {
                0xCu8.hash(state);
                std::ptr::hash(std::sync::Arc::as_ptr(c), state);
            }
            Value::SharedStore(s) => {
                0xDu8.hash(state);
                std::ptr::hash(std::sync::Arc::as_ptr(s), state);
            }
            Value::StoreMethod(sm) => {
                0xEu8.hash(state);
                std::ptr::hash(std::sync::Arc::as_ptr(&sm.store), state);
                sm.name.hash(state);
            }
            Value::ChannelMethod(cm) => {
                0x10u8.hash(state);
                std::ptr::hash(std::sync::Arc::as_ptr(&cm.endpoint), state);
                cm.name.hash(state);
            }
            Value::ReadOnly(r) => {
                0xFu8.hash(state);
                r.borrow().hash(state);
            }
            // M10: native objects hash by identity (Python `id`).
            Value::Native(n) => {
                0x11u8.hash(state);
                std::ptr::hash(std::sync::Arc::as_ptr(&n.0), state);
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
