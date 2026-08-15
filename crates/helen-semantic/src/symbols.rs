//! Symbol table and scope management for the Helen language.
//!
//! Byte-faithful port of `helen/semantic/symbols.py` (v1.44.0): provides
//! hierarchical symbol resolution across nested scopes (global, agent,
//! function, block) with agent-boundary isolation.

use helen_core::ast::TypeRef;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

/// A named entity in the symbol table.
///
/// Mirrors Python's `Symbol` dataclass:
/// - `kind`: `'variable'`, `'function'`, `'agent'`, `'param'`, `'import'`,
///   `'const'` (Python uses `is_const` plus the kind string; `argv` is
///   registered with kind `"const"` and `is_const=True`).
/// - `type_node`: optional type annotation from the source.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub type_node: Option<Box<TypeRef>>,
    pub is_const: bool,
}

impl Symbol {
    pub fn new(name: &str, kind: &str) -> Self {
        Symbol {
            name: name.into(),
            kind: kind.into(),
            type_node: None,
            is_const: false,
        }
    }

    pub fn new_with_type(name: &str, kind: &str, type_node: TypeRef) -> Self {
        Symbol {
            name: name.into(),
            kind: kind.into(),
            type_node: Some(Box::new(type_node)),
            is_const: false,
        }
    }

    /// Python `Symbol.__repr__`:
    /// `Symbol({const }{name}{: type}, kind={kind})`.
    pub fn repr(&self) -> String {
        let const_flag = if self.is_const { "const " } else { "" };
        let type_str = match &self.type_node {
            Some(tn) => format!(": {}", tn.name),
            None => String::new(),
        };
        format!(
            "Symbol({const_flag}{}{type_str}, kind={})",
            self.name, self.kind
        )
    }
}

/// A single scope level in the symbol table hierarchy.
///
/// The scope stack is stored in a `Vec` inside [`SymbolTable`]; `index 0` is
/// the global scope, the last element is the current (innermost) scope.
#[derive(Debug, Clone)]
pub struct Scope {
    pub name: String,
    pub scope_type: String,
    pub symbols: HashMap<String, Symbol>,
}

impl Scope {
    fn new(name: &str, scope_type: &str) -> Self {
        Scope {
            name: name.into(),
            scope_type: scope_type.into(),
            symbols: HashMap::new(),
        }
    }

    /// Define a symbol in this scope.
    ///
    /// Returns the existing symbol if already defined (duplicate), else None.
    pub fn define(&mut self, name: &str, symbol: Symbol) -> Option<&Symbol> {
        match self.symbols.entry(name.into()) {
            Entry::Occupied(occ) => Some(occ.into_mut()),
            Entry::Vacant(vac) => {
                vac.insert(symbol);
                None
            }
        }
    }

    /// Remove a symbol from this scope. Returns the removed symbol, or None.
    pub fn undefine(&mut self, name: &str) -> Option<Symbol> {
        self.symbols.remove(name)
    }
}

/// Hierarchical symbol table supporting nested scopes.
///
/// Manages a stack of scopes starting with a global scope (index 0),
/// mirroring Python's `SymbolTable` (scope stack with parent pointers).
#[derive(Debug, Default)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable {
            scopes: vec![Scope::new("global", "global")],
        }
    }

    /// Push a new nested scope (Python `enter_scope`).
    pub fn enter_scope(&mut self, name: &str, scope_type: &str) {
        self.scopes.push(Scope::new(name, scope_type));
    }

    /// Pop the current scope (Python `exit_scope`). Global scope is never
    /// popped.
    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a symbol in the current scope.
    ///
    /// Returns the existing symbol if already defined (duplicate), else None.
    pub fn define(&mut self, name: &str, symbol: Symbol) -> Option<&Symbol> {
        self.scopes.last_mut().unwrap().define(name, symbol)
    }

    /// Resolve a name by searching the scope chain upward (including global).
    pub fn resolve(&self, name: &str) -> Option<&Symbol> {
        self.scopes.iter().rev().find_map(|s| s.symbols.get(name))
    }

    /// Resolve a name only in the current scope (no upward search).
    pub fn resolve_local(&self, name: &str) -> Option<&Symbol> {
        self.scopes.last().and_then(|s| s.symbols.get(name))
    }

    /// Resolve a name walking up the scope chain, **stopping at global**.
    ///
    /// Mirrors Python `Scope.resolve_in_chain`: finds the first match in the
    /// current scope or any parent scope up to (but not including) the global
    /// scope. Returns None if the symbol is only in the global scope.
    pub fn resolve_in_chain(&self, name: &str) -> Option<&Symbol> {
        for i in (1..self.scopes.len()).rev() {
            if let Some(sym) = self.scopes[i].symbols.get(name) {
                return Some(sym);
            }
        }
        None
    }

    /// Remove a symbol from the current scope. Returns the removed symbol.
    pub fn undefine(&mut self, name: &str) -> Option<Symbol> {
        self.scopes.last_mut().and_then(|s| s.symbols.remove(name))
    }

    /// Remove a symbol from the **global** scope (Python
    /// `SymbolTable.global_scope.undefine`). Returns the removed symbol.
    pub fn global_undefine(&mut self, name: &str) -> bool {
        self.scopes
            .first_mut()
            .and_then(|s| s.symbols.remove(name))
            .is_some()
    }

    /// Current scope nesting depth (global = 0).
    pub fn depth(&self) -> usize {
        self.scopes.len() - 1
    }

    /// Whether we are at the global scope level.
    pub fn in_global_scope(&self) -> bool {
        self.scopes.len() == 1
    }

    /// Type of the current scope (`'global'`, `'agent'`, `'function'`,
    /// `'block'`, `'catch'`, …).
    pub fn current_scope_type(&self) -> &str {
        &self.scopes.last().unwrap().scope_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helen_core::ast::TypeRefKind;
    use helen_core::source::SourceSpan;

    #[test]
    fn symbol_repr() {
        let s = Symbol::new("x", "variable");
        assert_eq!(s.repr(), "Symbol(x, kind=variable)");
        let c = Symbol {
            name: "MAX".into(),
            kind: "const".into(),
            type_node: Some(Box::new(TypeRef {
                name: "int".into(),
                span: SourceSpan::new("<test>", 1, 1, 1, 4),
                kind: TypeRefKind::Simple,
            })),
            is_const: true,
        };
        assert_eq!(c.repr(), "Symbol(const MAX: int, kind=const)");
    }

    #[test]
    fn define_duplicate_returns_existing() {
        let mut st = SymbolTable::new();
        assert!(st.define("x", Symbol::new("x", "variable")).is_none());
        let dup = st.define("x", Symbol::new("x", "function"));
        assert!(dup.is_some());
        assert_eq!(dup.unwrap().kind, "variable");
        // Duplicate was not inserted: the existing symbol is still resolved.
        assert_eq!(st.resolve("x").unwrap().kind, "variable");
    }

    #[test]
    fn resolve_walks_scope_chain() {
        let mut st = SymbolTable::new();
        st.define("g", Symbol::new("g", "variable"));
        st.enter_scope("fn", "function");
        st.define("f", Symbol::new("f", "param"));
        assert_eq!(st.resolve("g").unwrap().name, "g");
        assert_eq!(st.resolve("f").unwrap().name, "f");
        assert_eq!(st.depth(), 1);
        assert_eq!(st.current_scope_type(), "function");
        assert!(!st.in_global_scope());
        st.exit_scope();
        assert_eq!(st.depth(), 0);
        assert!(st.in_global_scope());
        assert!(st.resolve("f").is_none());
        assert!(st.resolve("g").is_some());
    }

    #[test]
    fn resolve_in_chain_stops_at_global() {
        let mut st = SymbolTable::new();
        st.define("g", Symbol::new("g", "variable"));
        st.enter_scope("fn", "function");
        // `g` lives only in global scope → resolve_in_chain must miss it.
        assert!(st.resolve_in_chain("g").is_none());
        st.define("f", Symbol::new("f", "param"));
        assert!(st.resolve_in_chain("f").is_some());
        st.enter_scope("block", "block");
        st.define("b", Symbol::new("b", "variable"));
        assert!(st.resolve_in_chain("b").is_some());
        assert!(st.resolve_in_chain("f").is_some());
        assert!(st.resolve_in_chain("g").is_none());
    }

    #[test]
    fn resolve_local_only_current_scope() {
        let mut st = SymbolTable::new();
        st.enter_scope("fn", "function");
        st.define("x", Symbol::new("x", "variable"));
        assert!(st.resolve_local("x").is_some());
        st.enter_scope("block", "block");
        // x is in the parent function scope, not the current block scope.
        assert!(st.resolve_local("x").is_none());
        assert!(st.resolve("x").is_some());
    }

    #[test]
    fn undefine_removes_from_current_scope() {
        let mut st = SymbolTable::new();
        st.define("x", Symbol::new("x", "variable"));
        assert!(st.undefine("x").is_some());
        assert!(st.undefine("x").is_none());
        assert!(st.resolve("x").is_none());
    }

    #[test]
    fn exit_scope_never_pops_global() {
        let mut st = SymbolTable::new();
        st.exit_scope();
        st.exit_scope();
        assert!(st.in_global_scope());
        assert_eq!(st.depth(), 0);
    }
}
