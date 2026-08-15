//! Runtime environment: a chain of lexical scopes.
//!
//! Byte-faithful port of `helen/interpreter/environment.py` (v1.44.0).
//! The Python flat-cache and environment-pool are performance
//! optimizations; Rust's HashMap lookup makes them unnecessary, so only
//! the observable semantics are ported.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::exceptions::ConstAssignmentError;
use crate::value::Value;

/// Error from `assign`: either the variable is const or not found.
#[derive(Debug)]
pub enum AssignError {
    NotFound(String),
    Const(ConstAssignmentError),
}

/// A single lexical scope in the environment chain.
#[derive(Debug)]
pub struct Environment {
    pub parent: Option<Rc<RefCell<Environment>>>,
    store: HashMap<String, Value>,
    consts: HashSet<String>,
}

impl Environment {
    pub fn new(parent: Option<Rc<RefCell<Environment>>>) -> Self {
        Environment {
            parent,
            store: HashMap::new(),
            consts: HashSet::new(),
        }
    }

    /// `define(name, value, is_const)` — always targets this scope.
    pub fn define(&mut self, name: &str, value: Value, is_const: bool) {
        self.store.insert(name.to_string(), value);
        if is_const {
            self.consts.insert(name.to_string());
        }
    }

    /// `lookup(name)` — walk the chain. Returns None if not found.
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.store.get(name) {
            return Some(v.clone());
        }
        if let Some(p) = &self.parent {
            return p.borrow().get(name);
        }
        None
    }

    /// `assign(name, value)` — update in the defining scope.
    pub fn assign(&mut self, name: &str, value: Value) -> Result<(), AssignError> {
        if self.store.contains_key(name) {
            if self.consts.contains(name) {
                return Err(AssignError::Const(ConstAssignmentError {
                    name: name.to_string(),
                    span: None,
                }));
            }
            self.store.insert(name.to_string(), value);
            return Ok(());
        }
        if let Some(p) = &self.parent {
            return p.borrow_mut().assign(name, value);
        }
        Err(AssignError::NotFound(name.to_string()))
    }

    /// `is_const(name)` — walk the chain.
    pub fn is_const(&self, name: &str) -> bool {
        if self.store.contains_key(name) {
            return self.consts.contains(name);
        }
        if let Some(p) = &self.parent {
            return p.borrow().is_const(name);
        }
        false
    }

    /// `enter_scope()` — create a child scope of `parent` (Python
    /// `Environment.enter_scope` creates the child with `parent=self`).
    pub fn child(parent: Rc<RefCell<Environment>>) -> Rc<RefCell<Environment>> {
        Rc::new(RefCell::new(Environment::new(Some(parent))))
    }

    /// `__contains__(name)` — defined anywhere in the chain.
    pub fn contains(&self, name: &str) -> bool {
        self.store.contains_key(name)
            || self
                .parent
                .as_ref()
                .map(|p| p.borrow().contains(name))
                .unwrap_or(false)
    }

    /// Names defined directly in this scope.
    pub fn local_names(&self) -> Vec<String> {
        self.store.keys().cloned().collect()
    }

    /// Raw access to this scope's store (used by `shared let` export).
    pub fn store_ref(&self) -> &HashMap<String, Value> {
        &self.store
    }

    /// `snapshot()` — deep copy of the entire chain (spawn isolation).
    ///
    /// Python v1.18: ALL values are deep-copied with no exceptions.
    pub fn snapshot(&self) -> Environment {
        let parent_snapshot = self
            .parent
            .as_ref()
            .map(|p| Rc::new(RefCell::new(p.borrow().snapshot())));
        let mut new_env = Environment::new(parent_snapshot);
        for (k, v) in &self.store {
            new_env.store.insert(k.clone(), v.clone_owned());
        }
        new_env.consts = self.consts.clone();
        new_env
    }
}

impl Default for Environment {
    fn default() -> Self {
        Environment::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn int(n: i64) -> Value {
        Value::Int(num_bigint::BigInt::from(n))
    }

    #[test]
    fn define_get_assign_walk_chain() {
        let mut global = Environment::new(None);
        global.define("a", int(1), false);
        let child = Rc::new(RefCell::new(Environment::new(Some(Rc::new(RefCell::new(
            global,
        ))))));
        child.borrow_mut().define("b", int(2), false);
        assert_eq!(child.borrow().get("a"), Some(int(1)));
        assert_eq!(child.borrow().get("b"), Some(int(2)));
        // assign updates the defining scope
        child.borrow_mut().assign("a", int(10)).unwrap();
        assert_eq!(child.borrow().get("a"), Some(int(10)));
        // child still shadows nothing here
        assert!(child.borrow().get("missing").is_none());
    }

    #[test]
    fn const_protection() {
        let mut env = Environment::new(None);
        env.define("MAX", int(100), true);
        assert!(env.is_const("MAX"));
        match env.assign("MAX", int(5)) {
            Err(AssignError::Const(e)) => assert_eq!(e.name, "MAX"),
            _ => panic!("expected ConstAssignmentError"),
        }
        // get still returns the original value
        assert_eq!(env.get("MAX"), Some(int(100)));
    }

    #[test]
    fn assign_not_found_errors() {
        let mut env = Environment::new(None);
        match env.assign("nope", int(1)) {
            Err(AssignError::NotFound(n)) => assert_eq!(n, "nope"),
            _ => panic!("expected NotFound"),
        }
    }

    #[test]
    fn shadowing_define_targets_innermost() {
        let mut global = Environment::new(None);
        global.define("x", int(1), false);
        let child = Rc::new(RefCell::new(Environment::new(Some(Rc::new(RefCell::new(
            global,
        ))))));
        child.borrow_mut().define("x", int(2), false);
        assert_eq!(child.borrow().get("x"), Some(int(2)));
        // global x unchanged
        let child_borrow = child.borrow();
        let g = child_borrow.parent.as_ref().unwrap().borrow();
        assert_eq!(g.get("x"), Some(int(1)));
    }

    #[test]
    fn contains_walks_chain() {
        let mut global = Environment::new(None);
        global.define("a", int(1), false);
        let child = Rc::new(RefCell::new(Environment::new(Some(Rc::new(RefCell::new(
            global,
        ))))));
        child.borrow_mut().define("b", int(2), false);
        assert!(child.borrow().contains("a"));
        assert!(child.borrow().contains("b"));
        assert!(!child.borrow().contains("c"));
    }

    #[test]
    fn snapshot_deep_copies_chain() {
        let mut global = Environment::new(None);
        global.define("a", int(1), true);
        let child = Rc::new(RefCell::new(Environment::new(Some(Rc::new(RefCell::new(
            global,
        ))))));
        let list_val = Value::List(Rc::new(RefCell::new(vec![int(1)])));
        child.borrow_mut().define("lst", list_val, false);

        let snap = child.borrow().snapshot();
        // deep copy: mutating the original list must not affect the snapshot
        if let Value::List(l) = child.borrow().get("lst").unwrap() {
            l.borrow_mut().push(int(99));
        }
        if let Value::List(l) = snap.get("lst").unwrap() {
            assert_eq!(l.borrow().len(), 1);
        } else {
            panic!("expected list");
        }
        // consts copied
        let snap_global = snap.parent.as_ref().unwrap().borrow();
        assert!(snap_global.is_const("a"));
        assert_eq!(snap_global.get("a"), Some(int(1)));
    }
}
