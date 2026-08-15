//! Shared type utilities.
//!
//! Byte-faithful port of `helen/semantic/type_utils.py` (v1.44.0):
//! converts an AST type reference into a semantic [`Type`].

use crate::types::Type;
use helen_core::ast::{TypeRef, TypeRefKind};

/// Convert an AST `TypeRef` to a semantic [`Type`].
///
/// Mirrors Python `type_from_typenode`:
/// - `Optional`/`Union` kinds recurse structurally;
/// - plain names map case-insensitively (`int`/`integer`, …);
/// - unknown type names → `Any` (v1 lenient).
///
/// Python also handles `LiteralTypeNode` (`LiteralType(values)`) — the
/// current parser never produces one (the `_parse_type` docstring mentions
/// `Literal[...]` but no such code path exists), so no literal branch is
/// needed here.
pub fn from_type_ref(type_ref: &TypeRef) -> Type {
    match &type_ref.kind {
        TypeRefKind::Optional(inner) => Type::Optional(Box::new(from_type_ref(inner))),
        TypeRefKind::Union(members) => Type::Union(members.iter().map(from_type_ref).collect()),
        TypeRefKind::Simple => {
            let name = type_ref.name.to_lowercase();
            match name.as_str() {
                "int" | "integer" => Type::Int,
                "float" | "double" => Type::Float,
                "str" | "string" => Type::Str,
                "bool" | "boolean" => Type::Bool,
                "null" => Type::Null,
                "any" => Type::Any,
                "list" | "列表" => Type::List(Box::new(Type::Any)),
                "map" | "映射" => Type::Map(Box::new(Type::Any), Box::new(Type::Any)),
                // Unknown type names → AnyType (v1 lenient).
                _ => Type::Any,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helen_core::ast::TypeRef;
    use helen_core::source::SourceSpan;

    fn simple(name: &str) -> TypeRef {
        TypeRef {
            name: name.into(),
            span: SourceSpan::new("<test>", 1, 1, 1, 4),
            kind: TypeRefKind::Simple,
        }
    }

    fn optional(inner: TypeRef) -> TypeRef {
        let span = inner.span.clone();
        TypeRef {
            name: format!("optional<{}>", inner.name),
            span,
            kind: TypeRefKind::Optional(Box::new(inner)),
        }
    }

    fn union(members: Vec<TypeRef>) -> TypeRef {
        let span = SourceSpan::new("<test>", 1, 1, 1, 4);
        let name = format!(
            "union<{}>",
            members
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<_>>()
                .join("|")
        );
        TypeRef {
            name,
            span,
            kind: TypeRefKind::Union(members),
        }
    }

    #[test]
    fn none_maps_to_any() {
        assert_eq!(from_type_ref(&simple("__missing__")), Type::Any);
    }

    #[test]
    fn primitive_names() {
        assert_eq!(from_type_ref(&simple("int")), Type::Int);
        assert_eq!(from_type_ref(&simple("Integer")), Type::Int);
        assert_eq!(from_type_ref(&simple("float")), Type::Float);
        assert_eq!(from_type_ref(&simple("double")), Type::Float);
        assert_eq!(from_type_ref(&simple("str")), Type::Str);
        assert_eq!(from_type_ref(&simple("string")), Type::Str);
        assert_eq!(from_type_ref(&simple("bool")), Type::Bool);
        assert_eq!(from_type_ref(&simple("boolean")), Type::Bool);
        assert_eq!(from_type_ref(&simple("null")), Type::Null);
        assert_eq!(from_type_ref(&simple("any")), Type::Any);
    }

    #[test]
    fn containers_map_to_any_element() {
        assert_eq!(
            from_type_ref(&simple("list")),
            Type::List(Box::new(Type::Any))
        );
        assert_eq!(
            from_type_ref(&simple("map")),
            Type::Map(Box::new(Type::Any), Box::new(Type::Any))
        );
        assert_eq!(
            from_type_ref(&simple("列表")),
            Type::List(Box::new(Type::Any))
        );
        assert_eq!(
            from_type_ref(&simple("映射")),
            Type::Map(Box::new(Type::Any), Box::new(Type::Any))
        );
    }

    #[test]
    fn optional_recurses() {
        assert_eq!(
            from_type_ref(&optional(simple("str"))),
            Type::Optional(Box::new(Type::Str))
        );
    }

    #[test]
    fn union_recurses() {
        assert_eq!(
            from_type_ref(&union(vec![simple("str"), simple("int")])),
            Type::Union(vec![Type::Str, Type::Int])
        );
    }

    #[test]
    fn unknown_maps_to_any() {
        assert_eq!(from_type_ref(&simple("MyType")), Type::Any);
        // Case-insensitive: LIST → list.
        assert_eq!(
            from_type_ref(&simple("LIST")),
            Type::List(Box::new(Type::Any))
        );
    }
}
