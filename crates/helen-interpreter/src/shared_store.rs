//! Shared store runtime (Task 7.3) — port of `helen/interpreter/shared_store.py`.
//!
//! A `shared store` declaration creates an instance with typed fields and
//! methods. Within one interpreter the instance is shared (agent calls see
//! the same fields). At `spawn` the environment snapshot deep-copies the
//! instance (`__deepcopy__` parity): the spawned agent gets *independent*
//! fields — verified by `test_spawn_sharedstore_methods.py` where the child's
//! `set_value(999)` does not affect the parent's `get_value()`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;

use crate::value::Value;

/// A shared store instance (thread-safe field access; single-interpreter).
pub struct SharedStoreInstance {
    pub name: String,
    /// Field name -> current value (private state, Mutex-guarded).
    pub fields: Mutex<IndexMap<String, Value>>,
    /// Method name -> AST function declaration (immutable).
    pub methods: HashMap<String, std::rc::Rc<helen_core::ast::FunctionDecl>>,
    /// Stable field order for write-back.
    pub field_order: Vec<String>,
}

impl SharedStoreInstance {
    pub fn new(
        name: String,
        fields: IndexMap<String, Value>,
        methods: HashMap<String, std::rc::Rc<helen_core::ast::FunctionDecl>>,
    ) -> Self {
        let field_order = fields.keys().cloned().collect();
        SharedStoreInstance {
            name,
            fields: Mutex::new(fields),
            methods,
            field_order,
        }
    }

    /// `get_field` — thread-safe field read.
    pub fn get_field(&self, name: &str) -> Option<Value> {
        self.fields.lock().unwrap().get(name).cloned()
    }

    /// `set_field` — thread-safe field write.
    pub fn set_field(&self, name: &str, value: Value) -> Result<(), String> {
        let mut fields = self.fields.lock().unwrap();
        if fields.contains_key(name) {
            fields.insert(name.to_string(), value);
            Ok(())
        } else {
            Err(format!(
                "Shared store '{}' has no field '{}'",
                self.name, name
            ))
        }
    }

    /// Deep copy (spawn isolation) — `__deepcopy__` parity.
    ///
    /// Fields are cloned via `Value::clone_owned` (fresh Rc containers so the
    /// copy shares no allocation with the original); methods are re-cloned as
    /// fresh Rc references to the same AST nodes.
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn deep_copy(&self) -> Arc<SharedStoreInstance> {
        let fields: IndexMap<String, Value> = self
            .fields
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone_owned()))
            .collect();
        let methods = self
            .methods
            .iter()
            .map(|(k, v)| (k.clone(), std::rc::Rc::new(v.as_ref().clone())))
            .collect();
        Arc::new(SharedStoreInstance::new(self.name.clone(), fields, methods))
    }
}

impl std::fmt::Debug for SharedStoreInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<SharedStore {} with {} fields, {} methods>",
            self.name,
            self.field_order.len(),
            self.methods.len()
        )
    }
}
