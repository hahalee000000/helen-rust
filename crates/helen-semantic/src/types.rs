//! Helen type system.
//!
//! Byte-faithful port of `helen/semantic/types.py` (v1.44.0): the type
//! hierarchy, `type_compatible`, and `type_of_literal`.
//!
//! Python uses distinct classes (`IntType`, `OptionalType`, …) whose
//! `==` is class-identity for leaf types and structural/set-based for
//! composites. The Rust enum below mirrors that:
//! - leaf variants compare by variant identity;
//! - `Optional` / `List` / `Map` compare structurally;
//! - `Union` and `Literal` compare as **sets** (order-independent),
//!   exactly like Python's `UnionType.__eq__` / `LiteralType.__eq__`.

use helen_core::ast_printer::{py_repr_value, py_str_repr};
use helen_core::tokens::LiteralValue;

/// A Helen semantic type.
#[derive(Debug, Clone)]
pub enum Type {
    /// Dynamic-mode default type — accepts any value (`AnyType`).
    Any,
    /// Boolean type (`BoolType`).
    Bool,
    /// Numeric base type, accepts int or float (`NumberType`).
    Number,
    /// Integer subtype of `Number` (`IntType`).
    Int,
    /// Float subtype of `Number` (`FloatType`).
    Float,
    /// String type (`StringType`).
    Str,
    /// Null type (`NullType`).
    Null,
    /// Optional type `T?` = T | null (`OptionalType(inner)`).
    Optional(Box<Type>),
    /// Generic list `List[T]` (`ListType(element_type)`).
    List(Box<Type>),
    /// Generic map `Map[K, V]` (`MapType(key_type, value_type)`).
    Map(Box<Type>, Box<Type>),
    /// Union `A | B | C` (`UnionType(members)`).
    Union(Vec<Type>),
    /// Literal type `Literal[values]` (`LiteralType(values)`).
    Literal(Vec<LiteralValue>),
    /// Agent reference type (`AgentType(agent_name)`).
    Agent(String),
}

impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        use Type::*;
        match (self, other) {
            (Any, Any)
            | (Bool, Bool)
            | (Number, Number)
            | (Int, Int)
            | (Float, Float)
            | (Str, Str)
            | (Null, Null) => true,
            (Optional(a), Optional(b)) => a == b,
            (List(a), List(b)) => a == b,
            (Map(ak, av), Map(bk, bv)) => ak == bk && av == bv,
            // Python: UnionType == is set-based (order independent).
            (Union(a), Union(b)) => types_set_equal(a, b),
            // Python: LiteralType == is set-based (order independent).
            (Literal(a), Literal(b)) => literal_values_set_equal(a, b),
            (Agent(a), Agent(b)) => a == b,
            _ => false,
        }
    }
}

/// Set equality for a slice of `Type` (order independent), as Python's
/// `set(self.members) == set(other.members)`.
fn types_set_equal(a: &[Type], b: &[Type]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut used = vec![false; b.len()];
    for x in a {
        let mut found = false;
        for (i, y) in b.iter().enumerate() {
            if !used[i] && x == y {
                used[i] = true;
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

/// Set equality for a slice of `LiteralValue` (order independent).
fn literal_values_set_equal(a: &[LiteralValue], b: &[LiteralValue]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut used = vec![false; b.len()];
    for x in a {
        let mut found = false;
        for (i, y) in b.iter().enumerate() {
            if !used[i] && x == y {
                used[i] = true;
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

impl Type {
    /// Human-readable type name (`Type.name` in Python).
    pub fn name(&self) -> String {
        match self {
            Type::Any => "AnyType".into(),
            Type::Bool => "BoolType".into(),
            Type::Number => "NumberType".into(),
            Type::Int => "IntType".into(),
            Type::Float => "FloatType".into(),
            Type::Str => "StringType".into(),
            Type::Null => "NullType".into(),
            Type::Optional(inner) => format!("{}?", inner.name()),
            Type::List(elem) => format!("List[{}]", elem.name()),
            Type::Map(k, v) => format!("Map[{}, {}]", k.name(), v.name()),
            Type::Union(members) => members
                .iter()
                .map(|m| m.name())
                .collect::<Vec<_>>()
                .join(" | "),
            Type::Literal(values) => {
                let vals = values
                    .iter()
                    .map(py_repr_value)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Literal[{vals}]")
            }
            Type::Agent(n) => format!("Agent({n})"),
        }
    }

    /// Python `__repr__` of the type (used in diagnostics/tests).
    pub fn repr(&self) -> String {
        match self {
            Type::Any => "AnyType".into(),
            Type::Bool => "BoolType".into(),
            Type::Number => "NumberType".into(),
            Type::Int => "IntType".into(),
            Type::Float => "FloatType".into(),
            Type::Str => "StringType".into(),
            Type::Null => "NullType".into(),
            Type::Optional(inner) => format!("OptionalType({})", inner.repr()),
            Type::List(elem) => format!("ListType({})", elem.repr()),
            Type::Map(k, v) => format!("MapType({}, {})", k.repr(), v.repr()),
            Type::Union(members) => format!(
                "UnionType([{}])",
                members
                    .iter()
                    .map(|m| m.repr())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Type::Literal(values) => format!(
                "LiteralType([{}])",
                values
                    .iter()
                    .map(py_repr_value)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Type::Agent(n) => format!("AgentType({})", py_str_repr(n)),
        }
    }
}

/// Check if `actual` can be assigned to `expected`.
///
/// Byte-faithful port of Python `type_compatible` — including its quirks:
/// - "same class" check uses Python `type(a) is type(b)`, so e.g. any two
///   `OptionalType` instances are mutually "compatible" regardless of inner;
/// - number subtyping: `Int`→`Number`, `Float`→`Number`; `Int` excludes
///   `Float`; `Float` accepts every number;
/// - `Literal` actual: compatible iff **all** its values are compatible;
///   an empty literal is compatible with everything.
pub fn type_compatible(actual: &Type, expected: &Type) -> bool {
    use Type::*;
    // Anything is compatible with AnyType.
    if matches!(expected, Any) {
        return true;
    }
    // AnyType actual is compatible with everything (dynamic type).
    if matches!(actual, Any) {
        return true;
    }
    // Same type (Python: `type(actual) is type(expected)`).
    if std::mem::discriminant(actual) == std::mem::discriminant(expected) {
        return true;
    }
    // Number subtype rules.
    if matches!(actual, Int | Float | Number) && matches!(expected, Number) {
        // IntType accepts only IntType (not FloatType).
        if matches!(expected, Int) {
            return matches!(actual, Int | Number);
        }
        // FloatType and base NumberType accept all numbers.
        return true;
    }
    // LiteralType → underlying type: every value must be compatible.
    if let Literal(values) = actual {
        if values.is_empty() {
            return true;
        }
        return values
            .iter()
            .all(|v| type_compatible(&type_of_literal(v), expected));
    }
    // NullType → OptionalType[T].
    if matches!(expected, Optional(_)) && matches!(actual, Null) {
        return true;
    }
    // T → OptionalType[T].
    if let Optional(inner) = expected {
        if type_compatible(actual, inner) {
            return true;
        }
    }
    // T → UnionType if T is one of the members.
    if let Union(members) = expected {
        return members.iter().any(|m| type_compatible(actual, m));
    }
    // NullType compatible check for OptionalType (final redundant clause,
    // kept for byte-faithfulness to the Python source).
    if let Optional(inner) = expected {
        return matches!(actual, Null) || type_compatible(actual, inner);
    }
    false
}

/// Infer the Helen type from a literal value.
///
/// Byte-faithful port of Python `type_of_literal`. List/map literals are
/// intentionally `Any` (v1: no container inference).
pub fn type_of_literal(value: &LiteralValue) -> Type {
    match value {
        LiteralValue::Null => Type::Null,
        LiteralValue::Bool(_) => Type::Bool,
        LiteralValue::Int(_) => Type::Int,
        LiteralValue::Float(_) => Type::Float,
        LiteralValue::Str(_) => Type::Str,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helen_core::tokens::LiteralValue;

    fn int() -> Type {
        Type::Int
    }
    fn string() -> Type {
        Type::Str
    }
    fn opt(t: Type) -> Type {
        Type::Optional(Box::new(t))
    }

    #[test]
    fn names_match_python() {
        assert_eq!(Type::Int.name(), "IntType");
        assert_eq!(Type::Float.name(), "FloatType");
        assert_eq!(Type::Str.name(), "StringType");
        assert_eq!(Type::Null.name(), "NullType");
        assert_eq!(opt(Type::Str).name(), "StringType?");
        assert_eq!(Type::List(Box::new(int())).name(), "List[IntType]");
        assert_eq!(
            Type::Map(Box::new(string()), Box::new(int())).name(),
            "Map[StringType, IntType]"
        );
        assert_eq!(
            Type::Union(vec![string(), Type::Null]).name(),
            "StringType | NullType"
        );
        let lit = Type::Literal(vec![
            LiteralValue::Int(42.into()),
            LiteralValue::Str("hello".into()),
        ]);
        assert!(lit.name().contains("Literal"));
        assert_eq!(Type::Agent("Worker".into()).name(), "Agent(Worker)");
    }

    #[test]
    fn repr_matches_python() {
        assert_eq!(Type::Bool.repr(), "BoolType");
        assert_eq!(opt(string()).repr(), "OptionalType(StringType)");
        assert_eq!(
            Type::Union(vec![string(), Type::Null]).repr(),
            "UnionType([StringType, NullType])"
        );
        assert_eq!(
            Type::Literal(vec![
                LiteralValue::Int(42.into()),
                LiteralValue::Str("hello".into())
            ])
            .repr(),
            "LiteralType([42, 'hello'])"
        );
        assert_eq!(Type::Agent("MyAgent".into()).repr(), "AgentType('MyAgent')");
    }

    #[test]
    fn equality_is_class_based_and_set_based() {
        assert_eq!(Type::Bool, Type::Bool);
        assert_ne!(Type::Bool, Type::Number);
        assert_eq!(opt(string()), opt(string()));
        assert_ne!(opt(string()), opt(int()));
        // Union equality is order independent.
        assert_eq!(
            Type::Union(vec![string(), int()]),
            Type::Union(vec![int(), string()])
        );
        assert_ne!(
            Type::Union(vec![string(), int()]),
            Type::Union(vec![Type::Bool, int()])
        );
        // Literal equality is order independent.
        assert_eq!(
            Type::Literal(vec![
                LiteralValue::Int(42.into()),
                LiteralValue::Str("hello".into())
            ]),
            Type::Literal(vec![
                LiteralValue::Str("hello".into()),
                LiteralValue::Int(42.into())
            ])
        );
    }

    #[test]
    fn type_of_literal_mapping() {
        assert_eq!(type_of_literal(&LiteralValue::Int(42.into())), Type::Int);
        assert_eq!(type_of_literal(&LiteralValue::Float(2.71)), Type::Float);
        assert_eq!(
            type_of_literal(&LiteralValue::Str("hello".into())),
            Type::Str
        );
        assert_eq!(type_of_literal(&LiteralValue::Bool(true)), Type::Bool);
        assert_eq!(type_of_literal(&LiteralValue::Bool(false)), Type::Bool);
        assert_eq!(type_of_literal(&LiteralValue::Null), Type::Null);
    }

    #[test]
    fn same_type_compatible() {
        assert!(type_compatible(&Type::Str, &Type::Str));
        assert!(type_compatible(&Type::Bool, &Type::Bool));
        assert!(type_compatible(&Type::Number, &Type::Number));
    }

    #[test]
    fn any_accepts_all_and_vice_versa() {
        assert!(type_compatible(&Type::Str, &Type::Any));
        assert!(type_compatible(&Type::Number, &Type::Any));
        assert!(type_compatible(&Type::Bool, &Type::Any));
        assert!(type_compatible(&Type::Any, &Type::Str));
    }

    #[test]
    fn number_subtyping() {
        assert!(type_compatible(&Type::Int, &Type::Number));
        assert!(type_compatible(&Type::Float, &Type::Number));
        assert!(!type_compatible(&Type::Str, &Type::Number));
        assert!(!type_compatible(&Type::Bool, &Type::Str));
        assert!(!type_compatible(&Type::Float, &Type::Int));
        assert!(type_compatible(&Type::Int, &Type::Int));
    }

    #[test]
    fn literal_type_compatibility() {
        let lit = Type::Literal(vec![LiteralValue::Int(42.into())]);
        assert!(type_compatible(&lit, &Type::Number));
        let lit_s = Type::Literal(vec![LiteralValue::Str("hello".into())]);
        assert!(type_compatible(&lit_s, &Type::Str));
        let mixed = Type::Literal(vec![
            LiteralValue::Int(42.into()),
            LiteralValue::Str("oops".into()),
        ]);
        assert!(!type_compatible(&mixed, &Type::Number));
        assert!(type_compatible(&Type::Literal(vec![]), &Type::Str));
    }

    #[test]
    fn optional_and_union_compatibility() {
        assert!(type_compatible(&Type::Null, &opt(string())));
        assert!(type_compatible(&string(), &opt(string())));
        assert!(type_compatible(&Type::Int, &opt(Type::Number)));
        let u = Type::Union(vec![string(), int()]);
        assert!(type_compatible(&string(), &u));
        assert!(type_compatible(&int(), &u));
        assert!(!type_compatible(&Type::Bool, &u));
        assert!(!type_compatible(&Type::Number, &string()));
    }

    #[test]
    fn optional_quirk_same_class_is_compatible() {
        // Python: `type(actual) is type(expected)` short-circuits before any
        // inner-type check — OptionalType(Int) is "compatible" with
        // OptionalType(Str).
        assert!(type_compatible(&opt(int()), &opt(string())));
        assert!(type_compatible(
            &Type::Union(vec![int()]),
            &Type::Union(vec![string()])
        ));
    }
}
