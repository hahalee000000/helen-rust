//! The Helen interpreter — statements, expressions, calls, exceptions.
//!
//! Byte-faithful port of `helen/interpreter/interpreter.py` plus the
//! `exception_mixin` and `pattern_mixin` (v1.44.0). LLM/agent/spawn and
//! import machinery are stubbed or minimal in M3 and completed in later
//! milestones.

// `ExceptionValue` mirrors Python's exception object: message, class name,
// span, cause chain and metadata — necessarily a large `Err` payload.
#![allow(clippy::result_large_err)]

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};

use helen_core::ast::{
    Access, AgentDecl, Binary, Call, CallArg, Expr, ForStmt, FunctionDecl, IfStmt, ImportStmt,
    Index, Lambda, Pipe, Program, Spawn, Stmt, ThrowStmt, Unary, VarDecl,
};
use helen_core::ast_printer::py_str_float;
use helen_core::source::SourceSpan;
use helen_core::tokens::{LiteralValue, Token, TokenType};
use helen_semantic::types::{type_compatible, Type};

use crate::closure::{compute_free_variables, Closure};
use crate::environment::Environment;
use crate::exceptions::{error_matches, resolve_exception, ExceptionValue, Flow};
use crate::interpreter_builtins::{
    builtin_abs, builtin_bool, builtin_dict, builtin_float, builtin_input, builtin_int,
    builtin_isinstance, builtin_len, builtin_list, builtin_max, builtin_min,
    builtin_multiline_input, builtin_print, builtin_range, builtin_str, builtin_type, check_type,
    cmp_values, format_keys, is_mutable_type, is_number, num_f64, py_mod, to_f64,
    type_from_typenode, BuiltinImpl, ListMethodKind, ListMethodValue, MapMethodKind,
    MapMethodValue,
};
use crate::llm_runtime::{LlmRuntime, MockLlmRuntime};
use crate::value::{BuiltinFn, ChannelMethodValue, ChannelMsg, StoreMethodValue, Value};

/// Runtime type-check results used by `_call_function`/`_call_closure`.
fn type_of_value(v: &Value) -> Type {
    match v {
        Value::Null => Type::Null,
        Value::Bool(_) => Type::Bool,
        Value::Int(_) => Type::Int,
        Value::Float(_) => Type::Float,
        Value::Str(_) => Type::Str,
        _ => Type::Any,
    }
}

/// The Helen interpreter.
pub struct Interpreter {
    pub environment: Rc<RefCell<Environment>>,
    pub functions: HashMap<String, Rc<FunctionDecl>>,
    pub agents: HashMap<String, Rc<AgentDecl>>,
    pub protocols: HashMap<String, Rc<helen_core::ast::ProtocolDecl>>,
    pub impls: HashMap<(String, String), Rc<helen_core::ast::ImplDecl>>,
    /// v1.10: names of `shared let` variables (cross-agent visibility).
    pub shared_vars: HashSet<String>,
    /// v1.39: maps imported function name -> its defining module env.
    pub function_module_envs: HashMap<String, Rc<RefCell<Environment>>>,
    /// Task 3.7b: `.helen`/data-file import resolution (fresh per instance).
    pub import_resolver: crate::import_resolver::ImportResolver,
    /// Absolute path of the file being interpreted (relative-import base).
    pub source_file: Option<String>,
    /// `.helen` files already registered (idempotency across imports).
    pub processed_files: HashSet<std::path::PathBuf>,
    pub program_args: Vec<String>,
    pub builtins: HashMap<String, Rc<BuiltinFn>>,
    /// v1.22: current agent (None at top level).
    pub current_agent: Option<Rc<AgentDecl>>,
    /// Task 3.6: LLM runtime backing `llm if` / `llm act`.
    pub llm_runtime: std::sync::Arc<dyn LlmRuntime>,
    /// v1.12: shared store instance cache (import reuse).
    pub shared_store_instances: HashMap<String, Value>,
    /// Captured stdout (Python redirects sys.stdout around `interpret`).
    pub stdout: std::sync::Arc<std::sync::Mutex<String>>,
    /// M8: session manager backing session/transcript stdlib functions.
    pub session_manager: std::sync::Arc<std::sync::Mutex<helen_runtime::SessionManager>>,
    /// M8: current session ID (empty when no session is active).
    pub session_id: String,
    /// AI-native observability (tracer, call_stack, llm_audit, last_error).
    pub observability: crate::observability::ObservabilityManager,
    /// Working memory for context management (P1).
    pub working_memory:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<Value>>>>,
    /// Call tracker for cancellable LLM calls (P3).
    pub call_tracker: std::sync::Arc<helen_runtime::call_tracking::CallTracker>,
    /// Data lineage tracker for cross-agent data flow (P5).
    pub data_lineage:
        std::sync::Arc<std::sync::Mutex<helen_runtime::data_lineage::DataLineageTracker>>,
    /// LLM call history (Python `_history`) — `{"role", "content"}` entries
    /// recorded by `llm if`/`llm act`; feeds `format_context_stats`.
    pub history: Rc<RefCell<Vec<serde_json::Value>>>,
}

/// Everything a spawned agent thread needs, deep-owned so it can be moved
/// across the thread boundary with single-owner semantics (see `visit_spawn`).
///
/// # SAFETY
/// `Send` is implemented manually: every `Rc` inside is a *fresh, uniquely
/// owned* allocation created by the parent (registries rebuilt via
/// `Rc::new(x.as_ref().clone())`, env via `snapshot()` deep-own, args via
/// `make_send_owned`). No allocation is shared with the parent thread.
/// Shared items use `Arc` (`llm_runtime`, `stdout`, the endpoint).
/// Python's GIL makes the equivalent Python code "safe"; Rust documents this
/// single-owner transfer as intentionally stricter.
struct SpawnPayload {
    env: Environment,
    functions: HashMap<String, Rc<FunctionDecl>>,
    agents: HashMap<String, Rc<AgentDecl>>,
    protocols: HashMap<String, Rc<helen_core::ast::ProtocolDecl>>,
    impls: HashMap<(String, String), Rc<helen_core::ast::ImplDecl>>,
    builtins: HashMap<String, Rc<BuiltinFn>>,
    shared_vars: std::collections::HashSet<String>,
    program_args: Vec<String>,
    source_file: Option<String>,
    args: HashMap<String, Value>,
    agent: Rc<AgentDecl>,
    span: SourceSpan,
    llm_runtime: std::sync::Arc<dyn LlmRuntime>,
    stdout: std::sync::Arc<std::sync::Mutex<String>>,
    endpoint: std::sync::Arc<helen_runtime::channel::ChannelEndpoint<ChannelMsg>>,
}

// SAFETY: see the struct doc — single-owner Rc discipline.
unsafe impl Send for SpawnPayload {}

/// The spawned-thread runner (Python `run_spawned`): builds a fresh
/// `Interpreter` from the snapshot, calls the agent, reports errors back over
/// the channel, and closes the endpoint.
fn run_spawned(p: SpawnPayload) {
    let mut interp = Interpreter::new();
    interp.environment = Rc::new(RefCell::new(p.env));
    interp.functions = p.functions;
    interp.agents = p.agents;
    interp.protocols = p.protocols;
    interp.impls = p.impls;
    interp.builtins = p.builtins;
    interp.shared_vars = p.shared_vars;
    interp.program_args = p.program_args;
    interp.source_file = p.source_file;
    interp.llm_runtime = p.llm_runtime;
    interp.stdout = p.stdout;

    let result = interp.call_agent(&p.agent, p.args, &p.span);
    if let Err(e) = result {
        // Report the error back over the channel (Python sends
        // {"__error__": true, "message": str(e)}).
        let mut m = indexmap::IndexMap::new();
        m.insert(Value::Str(Rc::from("__error__")), Value::Bool(true));
        m.insert(
            Value::Str(Rc::from("message")),
            Value::Str(Rc::from(e.message.clone())),
        );
        p.endpoint
            .send(ChannelMsg(Value::Map(Rc::new(RefCell::new(m)))));
    }
    p.endpoint.close();
}

impl Interpreter {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Self {
        let mut interp = Interpreter {
            environment: Rc::new(RefCell::new(Environment::new(None))),
            functions: HashMap::new(),
            agents: HashMap::new(),
            protocols: HashMap::new(),
            impls: HashMap::new(),
            shared_vars: HashSet::new(),
            function_module_envs: HashMap::new(),
            import_resolver: crate::import_resolver::ImportResolver::new(std::path::PathBuf::from(
                ".",
            )),
            source_file: None,
            processed_files: HashSet::new(),
            program_args: Vec::new(),
            builtins: HashMap::new(),
            current_agent: None,
            llm_runtime: std::sync::Arc::new(MockLlmRuntime::default()),
            shared_store_instances: HashMap::new(),
            stdout: std::sync::Arc::new(std::sync::Mutex::new(String::new())),
            session_manager: std::sync::Arc::new(std::sync::Mutex::new(
                helen_runtime::SessionManager::new(None),
            )),
            session_id: String::new(),
            observability: crate::observability::ObservabilityManager::new(),
            working_memory: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            call_tracker: std::sync::Arc::new(helen_runtime::call_tracking::CallTracker::new()),
            data_lineage: std::sync::Arc::new(std::sync::Mutex::new(
                helen_runtime::data_lineage::DataLineageTracker::new_in_memory(),
            )),
            history: Rc::new(RefCell::new(Vec::new())),
        };
        interp.register_core_builtins();
        interp
    }

    /// Inject a custom LLM runtime (tests use `MockLlmRuntime`).
    pub fn set_llm_runtime(&mut self, runtime: std::sync::Arc<dyn LlmRuntime>) {
        self.llm_runtime = runtime;
    }

    /// List all user-defined functions and agents (Python: `list_definitions`).
    pub fn list_definitions(&self) -> HashMap<String, Vec<String>> {
        let mut fns: Vec<String> = self.functions.keys().cloned().collect();
        fns.sort();
        let mut agents: Vec<String> = self.agents.keys().cloned().collect();
        agents.sort();
        let mut result = HashMap::new();
        result.insert("functions".to_string(), fns);
        result.insert("agents".to_string(), agents);
        result
    }

    /// Remove a function from the registry. Returns true if it existed.
    pub fn undefine_function(&mut self, name: &str) -> bool {
        self.functions.remove(name).is_some()
    }

    /// Remove an agent from the registry. Returns true if it existed.
    pub fn undefine_agent(&mut self, name: &str) -> bool {
        self.agents.remove(name).is_some()
    }

    /// Clear all user-defined functions and agents (keep stdlib).
    /// Python: `reset_definitions`.
    pub fn reset_definitions(&mut self) {
        self.functions.clear();
        self.agents.clear();
        self.current_agent = None;
        // Re-register stdlib builtins
        self.register_core_builtins();
    }

    /// Format context usage statistics for REPL display (P4).
    /// Simplified version - returns basic stats since full history management
    /// is not yet integrated in Rust.
    pub fn format_context_stats(&self) -> String {
        // Python: `HistoryManager.get_usage_stats(self._history, system_prompt)`
        // → `format_usage_stats`. Convert the recorded JSON entries to runtime
        // Messages and delegate to the runtime's fully-implemented formatter.
        let history: Vec<helen_runtime::transcript::Message> = self
            .history
            .borrow()
            .iter()
            .filter_map(|entry| {
                let role = entry.get("role")?.as_str()?;
                let content = entry.get("content")?.as_str()?;
                Some(helen_runtime::transcript::Message::new(
                    role,
                    serde_json::Value::String(content.to_string()),
                    Vec::new(),
                    None,
                    helen_runtime::transcript::generate_uuid(),
                    None,
                    0,
                    false,
                    false,
                    None,
                    String::new(),
                    String::new(),
                    Vec::new(),
                ))
            })
            .collect();

        let manager = helen_runtime::history::HistoryManager::new(None, None, None);
        let stats = manager.get_usage_stats(&history, None);
        manager.format_usage_stats(&stats)
    }

    fn register_builtin(&mut self, name: &str, func: BuiltinImpl) {
        let bf = Rc::new(BuiltinFn {
            name: name.to_string(),
            module: "core",
            func,
        });
        self.builtins.insert(name.to_string(), bf.clone());
    }

    /// Register all exports of a stdlib module into `self.builtins`
    /// (canonical name -> BuiltinFn). Idempotent.
    fn register_stdlib_module(&mut self, module: &str) {
        if module == "std.core" {
            return; // already registered in register_core_builtins
        }
        let Some(exports) = crate::stdlib::module_exports(module) else {
            return;
        };
        let tag = crate::stdlib::module_tag(module);
        for e in exports {
            let bf = Rc::new(BuiltinFn {
                name: e.name.to_string(),
                module: tag,
                func: e.func,
            });
            self.builtins.insert(e.name.to_string(), bf.clone());
        }
    }

    /// Execute a stdlib module import (v1.34/v1.38): the three forms are
    /// `import std.X.*`, `import std.X.{a,b}`, and `import std.X as NS`.
    fn import_stdlib_module(
        &mut self,
        module: &str,
        imported_names: Option<&[String]>,
        namespace: Option<&str>,
        span: &SourceSpan,
    ) -> Result<(), ExceptionValue> {
        // std.core exports live in the `builtins` map; other modules are
        // registered on first use (idempotent).
        if module != "std.core" && crate::stdlib::module_exports(module).is_none() {
            return Err(self.runtime_error(
                Some(span),
                &format!(
                    "Unknown stdlib module '{module}'. Available: std.core, std.str, std.list, std.dict, std.math, std.debug, std.concurrency"
                ),
            ));
        }
        self.register_stdlib_module(module);

        let all_names: Vec<String> = if module == "std.core" {
            crate::stdlib::CORE_EXPORTS
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            crate::stdlib::module_exports(module)
                .unwrap()
                .iter()
                .map(|e| e.name.to_string())
                .collect()
        };

        if let Some(ns) = namespace {
            // Namespace import: `import std.X as NS` — a module object (map)
            // of name -> BuiltinFn, defined const (Python: is_const=True).
            let mut m = indexmap::IndexMap::new();
            for name in &all_names {
                if let Some(bf) = self.builtins.get(name) {
                    m.insert(
                        Value::Str(Rc::from(name.as_str())),
                        Value::BuiltinFn(bf.clone()),
                    );
                }
            }
            let module_obj = Value::Map(Rc::new(RefCell::new(m)));
            self.environment.borrow_mut().define(ns, module_obj, true);
            return Ok(());
        }

        let star = imported_names
            .map(|n| n.iter().any(|x| x == "*"))
            .unwrap_or(true);
        if star {
            // Wildcard: bind all exports + Chinese aliases (v1.39).
            for name in &all_names {
                if let Some(bf) = self.builtins.get(name) {
                    self.environment
                        .borrow_mut()
                        .define(name, Value::BuiltinFn(bf.clone()), true);
                }
            }
            let aliases: Vec<(String, String)> = helen_semantic::stdlib::all_aliases();
            for (alias, canonical) in aliases {
                if all_names.contains(&canonical) {
                    if let Some(bf) = self.builtins.get(&canonical) {
                        self.environment.borrow_mut().define(
                            &alias,
                            Value::BuiltinFn(bf.clone()),
                            true,
                        );
                    }
                }
            }
            return Ok(());
        }

        // Selective: `import std.X.{a, b}` — error on unknown names.
        let names = imported_names.unwrap_or_default();
        for name in names {
            if !all_names.contains(name) {
                return Err(self.runtime_error(
                    Some(span),
                    &format!(
                        "Function '{name}' not found in module '{module}'. Available: {}",
                        all_names.join(", ")
                    ),
                ));
            }
        }
        for name in names {
            if let Some(bf) = self.builtins.get(name) {
                self.environment
                    .borrow_mut()
                    .define(name, Value::BuiltinFn(bf.clone()), true);
            }
        }
        Ok(())
    }

    /// Set the file being interpreted (relative-import base). The file's
    /// directory becomes the resolver's `base_dir`; its path is used as the
    /// `from_file` anchor when resolving relative imports.
    pub fn set_source_file(&mut self, path: &str) {
        self.source_file = Some(path.to_string());
        let dir = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        self.import_resolver =
            crate::import_resolver::ImportResolver::new(if dir.as_os_str().is_empty() {
                std::path::PathBuf::from(".")
            } else {
                dir
            });
    }

    /// `import "path"` — `.helen` file or data file (Task 3.7b).
    ///
    /// `.helen`: parse + register functions/agents/consts/shared stores into
    /// the interpreter namespaces (main block NOT executed); aliased form
    /// exposes the direct file's symbols via a module object map.
    fn import_file(&mut self, imp: &ImportStmt) -> Result<(), ExceptionValue> {
        use crate::import_resolver::ResolvedImport;

        let from_file = self.source_file.as_ref().map(std::path::PathBuf::from);
        let direct_path = match self
            .import_resolver
            .resolve(&imp.module_path, from_file.as_deref())
        {
            Ok(ResolvedImport::Data { alias, value }) => {
                let name = imp.alias.clone().unwrap_or(alias);
                self.environment.borrow_mut().define(&name, value, true);
                return Ok(());
            }
            Ok(ResolvedImport::Python) => {
                // M10: fall back to the FFI runtime when a Python import hook
                // is registered (helen-ffi crate with `python-ffi` feature).
                // Mirror Python's `_import_python_module`: alias defaults to
                // the last dotted component; `.py` suffix is stripped.
                let module_name = imp
                    .module_path
                    .strip_suffix(".py")
                    .unwrap_or(&imp.module_path);
                if let Some(hook) = crate::native::python_import_hook() {
                    let value = match hook(module_name) {
                        Ok(v) => v,
                        Err(msg) => {
                            return Err(self.runtime_error(
                                Some(&imp.span),
                                &format!("Cannot import Python module '{module_name}': {msg}"),
                            ))
                        }
                    };
                    let alias = imp.alias.clone().unwrap_or_else(|| {
                        module_name
                            .rsplit('.')
                            .next()
                            .unwrap_or(module_name)
                            .to_string()
                    });
                    self.environment.borrow_mut().define(&alias, value, true);
                    return Ok(());
                }
                return Err(self.runtime_error(
                    Some(&imp.span),
                    &format!(
                        "Failed to import '{}': Python module imports are not \
                         supported by the Rust runtime (compile with the \
                         `python-ffi` feature)",
                        imp.module_path
                    ),
                ));
            }
            Ok(ResolvedImport::Helen { path }) => path,
            Err(msg) => return Err(self.runtime_error(Some(&imp.span), &msg)),
        };

        // Register every loaded `.helen` file (direct + transitive) once.
        let order: Vec<std::path::PathBuf> = self.import_resolver.load_order().to_vec();
        for abs in &order {
            if !self.processed_files.insert(abs.clone()) {
                continue;
            }
            self.register_helen_file(abs)?;
        }

        // Aliased import: expose the DIRECT file's symbols via a module map.
        if let Some(alias) = &imp.alias {
            let module = self.build_module_object(&direct_path)?;
            self.environment.borrow_mut().define(alias, module, true);
        }
        Ok(())
    }

    /// Register one `.helen` file's symbols into the interpreter:
    /// consts/shared-lets -> module env (+ global env), shared stores ->
    /// containers, functions -> `self.functions` + `function_module_envs`,
    /// agents -> `self.agents`.
    fn register_helen_file(&mut self, abs: &std::path::Path) -> Result<(), ExceptionValue> {
        let Some(reg) = self.import_resolver.file(abs).cloned() else {
            return Ok(());
        };
        let module_env = Rc::new(RefCell::new(Environment::new(Some(
            self.environment.clone(),
        ))));

        // 0. This file's own stdlib imports bind into its module env
        //    (Python `_get_file_module_env` step 3), so the file's functions
        //    see exactly the symbols their file declares.
        for i in &reg.imports {
            if i.is_stdlib_module {
                let module = i.module_name.clone().unwrap_or_default();
                let names = i.imported_names.clone();
                let ns = i.namespace.clone();
                let span = i.span.clone();
                self.with_scope(Some(module_env.clone()), |s| {
                    s.import_stdlib_module(&module, names.as_deref(), ns.as_deref(), &span)
                })?;
            }
        }

        // 1. Consts / shared lets: define (Null placeholder), then evaluate
        //    each initializer in the module env scope (so later consts see
        //    earlier ones). All land in the global env too (NameError-guard),
        //    shared lets also join `shared_vars`.
        for v in &reg.data {
            module_env
                .borrow_mut()
                .define(&v.name, Value::Null, !v.mutable);
        }
        for v in &reg.data {
            let value = if let Some(init) = &v.initializer {
                self.with_scope(Some(module_env.clone()), |s| s.eval_expr(init))?
            } else {
                Value::Null
            };
            module_env
                .borrow_mut()
                .define(&v.name, value.clone(), !v.mutable);
            if self.environment.borrow().get(&v.name).is_none() {
                self.environment
                    .borrow_mut()
                    .define(&v.name, value, !v.mutable);
            }
            if v.shared {
                self.shared_vars.insert(v.name.clone());
            }
        }

        // 2. Shared stores -> SharedStoreInstance (fields + methods).
        for ss in &reg.shared_stores {
            self.register_shared_store(ss, module_env.clone())?;
        }

        // 3. Functions -> self.functions + their file's module env.
        for f in &reg.functions {
            self.functions.insert(f.name.clone(), Rc::new(f.clone()));
            self.function_module_envs
                .insert(f.name.clone(), module_env.clone());
        }

        // 4. Agents -> self.agents.
        for a in &reg.agents {
            self.agents.insert(a.name.clone(), Rc::new(a.clone()));
        }
        Ok(())
    }

    /// Register a shared store declaration (Python `_visit_shared_container`):
    /// evaluate field initializers in `module_env`, bind methods, define the
    /// store as a const in the module env + global env, and add it to
    /// `shared_vars` so agents (and spawned children) see it.
    #[allow(clippy::arc_with_non_send_sync)]
    fn register_shared_store(
        &mut self,
        ss: &helen_core::ast::SharedStoreDecl,
        module_env: Rc<RefCell<Environment>>,
    ) -> Result<(), ExceptionValue> {
        let mut fields = indexmap::IndexMap::new();
        for f in &ss.fields {
            let val = if let Some(init) = &f.initializer {
                self.with_scope(Some(module_env.clone()), |s| s.eval_expr(init))?
            } else {
                Value::Null
            };
            fields.insert(f.name.clone(), val);
        }
        let mut methods = HashMap::new();
        for m in &ss.methods {
            methods.insert(m.name.clone(), Rc::new(m.clone()));
        }
        let instance = std::sync::Arc::new(crate::shared_store::SharedStoreInstance::new(
            ss.name.clone(),
            fields,
            methods,
        ));
        let value = Value::SharedStore(instance);
        module_env
            .borrow_mut()
            .define(&ss.name, value.clone(), true);
        if self.environment.borrow().get(&ss.name).is_none() {
            self.environment.borrow_mut().define(&ss.name, value, true);
        }
        self.shared_vars.insert(ss.name.clone());
        Ok(())
    }

    /// Build the module-object map for an aliased `.helen` import: the
    /// direct file's functions/agents/consts are exposed as `m.symbol`
    /// entries (`__type__` marker mirrors Python's module dict).
    fn build_module_object(&mut self, abs: &std::path::Path) -> Result<Value, ExceptionValue> {
        let reg = self.import_resolver.file(abs).cloned().unwrap_or_default();
        let mut map = indexmap::IndexMap::new();
        map.insert(
            Value::Str(std::rc::Rc::from("__type__")),
            Value::Str(std::rc::Rc::from("module")),
        );
        for f in &reg.functions {
            map.insert(
                Value::Str(std::rc::Rc::from(f.name.as_str())),
                Value::UserFn(Rc::new(f.clone())),
            );
        }
        for a in &reg.agents {
            map.insert(
                Value::Str(std::rc::Rc::from(a.name.as_str())),
                Value::Agent(Rc::new(a.clone())),
            );
        }
        for v in &reg.data {
            let val = self
                .environment
                .borrow()
                .get(&v.name)
                .unwrap_or(Value::Null);
            map.insert(Value::Str(std::rc::Rc::from(v.name.as_str())), val);
        }
        Ok(Value::Map(Rc::new(RefCell::new(map))))
    }

    fn register_core_builtins(&mut self) {
        // M3: core subset of std.core — bound into the global environment so
        // `import std.core.*` programs resolve them (import machinery is
        // Task 3.7; this mirrors its observable binding for core names).
        let names = [
            "print",
            "len",
            "str",
            "int",
            "float",
            "bool",
            "type",
            "isinstance",
            "range",
            "abs",
            "min",
            "max",
            "list",
            "dict",
            "input",
            "multiline_input",
        ];
        for n in names {
            self.register_builtin(
                n,
                match n {
                    "print" => builtin_print,
                    "len" => builtin_len,
                    "str" => builtin_str,
                    "int" => builtin_int,
                    "float" => builtin_float,
                    "bool" => builtin_bool,
                    "type" => builtin_type,
                    "isinstance" => builtin_isinstance,
                    "range" => builtin_range,
                    "abs" => builtin_abs,
                    "min" => builtin_min,
                    "max" => builtin_max,
                    "list" => builtin_list,
                    "input" => builtin_input,
                    "multiline_input" => builtin_multiline_input,
                    _ => builtin_dict,
                },
            );
        }
    }

    // ------------------------------------------------------------------
    // Entry points
    // ------------------------------------------------------------------

    /// `interpret(program)` — execute and unwrap top-level sentinels.
    pub fn interpret(&mut self, program: &Program) -> Result<Option<Value>, ExceptionValue> {
        let flow = self.execute_stmts(&program.statements)?;
        Ok(match flow {
            Flow::Return(v) => v.filter(|v| !matches!(v, Value::Null)),
            Flow::Break | Flow::Continue => None,
            // Python's `null` literal IS None — the interpret boundary must
            // collapse Value::Null → None for exact reference parity
            // (verified: `main { null }` → None in the Python reference).
            Flow::Normal(v) => v.filter(|v| !matches!(v, Value::Null)),
        })
    }

    /// Execute a list of statements, propagating sentinels.
    pub fn execute_stmts(&mut self, stmts: &[Stmt]) -> Result<Flow, ExceptionValue> {
        let mut result = Flow::Normal(None);
        for stmt in stmts {
            let step = self.execute_stmt(stmt)?;
            match step {
                Flow::Normal(_) => result = step,
                _ => return Ok(step),
            }
        }
        Ok(result)
    }

    /// Execute a single statement.
    pub fn execute_stmt(&mut self, stmt: &Stmt) -> Result<Flow, ExceptionValue> {
        // Coverage: record executed line (only when enabled — cheap check).
        if self.observability.coverage.is_enabled() {
            let span = stmt.span();
            let file = if span.file.is_empty() {
                self.source_file.as_deref()
            } else {
                Some(span.file.as_str())
            };
            self.observability
                .coverage
                .record_line(file, span.start_line);
            // Register/record function declarations for coverage.
            match stmt {
                Stmt::FunctionDecl(f) => {
                    self.observability
                        .coverage
                        .record_function(file, f.span.start_line, &f.name);
                }
                Stmt::AgentDecl(a) => {
                    self.observability
                        .coverage
                        .record_function(file, a.span.start_line, &a.name);
                }
                Stmt::FnBlock(fb) => {
                    self.observability.coverage.record_function(
                        file,
                        fb.span.start_line,
                        "<fn_block>",
                    );
                }
                _ => {}
            }
        }
        match stmt {
            Stmt::VarDecl(v) => {
                let value = self.visit_var_decl(v)?;
                Ok(Flow::Normal(value))
            }
            Stmt::If(s) => self.visit_if(s),
            Stmt::For(s) => self.visit_for(s),
            Stmt::While(s) => self.visit_while(s),
            Stmt::Break(_) => Ok(Flow::Break),
            Stmt::Continue(_) => Ok(Flow::Continue),
            Stmt::Return(r) => {
                let value = if let Some(v) = &r.value {
                    Some(self.eval_expr(v)?)
                } else {
                    None
                };
                Ok(Flow::Return(value))
            }
            Stmt::Expr(e) => {
                // HLD 3.5.1: an expression statement yields its value to the
                // enclosing flow (Python `visit_expr_stmt` returns the value,
                // so `interpret(main { 42 })` is `42`). M13 Tier C caught this.
                let v = self.eval_expr(&e.expression)?;
                Ok(Flow::Normal(Some(v)))
            }
            Stmt::FunctionDecl(f) => {
                self.visit_function_decl(f);
                Ok(Flow::Normal(None))
            }
            Stmt::FnBlock(fb) => self.execute_stmts(&fb.body),
            Stmt::MainBlock(mb) => self.with_scope(None, |s| s.execute_stmts(&mb.body)),
            Stmt::AgentDecl(a) => {
                let rc = Rc::new(a.clone());
                self.current_agent = Some(rc.clone());
                self.agents.insert(a.name.clone(), rc);
                Ok(Flow::Normal(None))
            }
            Stmt::ProtocolDecl(p) => {
                self.protocols.insert(p.name.clone(), Rc::new(p.clone()));
                Ok(Flow::Normal(None))
            }
            Stmt::ImplDecl(im) => {
                let rc = Rc::new(im.clone());
                self.impls
                    .insert((im.protocol_name.clone(), im.struct_name.clone()), rc);
                for m in &im.methods {
                    self.functions.insert(m.name.clone(), Rc::new(m.clone()));
                }
                Ok(Flow::Normal(None))
            }
            Stmt::Try(t) => self.visit_try(t),
            Stmt::Throw(t) => Err(self.make_throw(t)?),
            Stmt::Assert(a) => {
                self.visit_assert(a)?;
                Ok(Flow::Normal(None))
            }
            Stmt::Match(m) => self.visit_match_stmt(m),
            Stmt::Alias(a) => {
                self.visit_alias(a);
                Ok(Flow::Normal(None))
            }
            Stmt::Import(imp) => {
                // v1.34/v1.38 stdlib module imports: wildcard, selective,
                // and namespace (`as NS`) forms. Non-stdlib imports resolve
                // `.helen` files / data files (Task 3.7b).
                if imp.is_stdlib_module {
                    let module = imp.module_name.clone().unwrap_or_default();
                    let names = imp.imported_names.clone();
                    let ns = imp.namespace.clone();
                    let span = imp.span.clone();
                    self.import_stdlib_module(&module, names.as_deref(), ns.as_deref(), &span)?;
                } else {
                    self.import_file(imp)?;
                }
                Ok(Flow::Normal(None))
            }
            Stmt::SharedStoreDecl(ss) => {
                self.register_shared_store(ss, self.environment.clone())?;
                Ok(Flow::Normal(None))
            }
            Stmt::LlmIf(li) => self.visit_llm_if(li),
            Stmt::LlmBranch(lb) => {
                let b = lb.clone();
                if let Some(c) = &b.condition {
                    let v = self.eval_expr(c)?;
                    if !v.truthy() {
                        return Ok(Flow::Normal(None));
                    }
                }
                self.execute_stmts(&b.body)
            }
            Stmt::Case(_)
            | Stmt::CatchClause(_)
            | Stmt::CatchAll(_)
            | Stmt::FinallyBlock(_)
            | Stmt::PromptDef(_)
            | Stmt::Declaration(_)
            | Stmt::AgentParam(_)
            | Stmt::ContextConfig(_) => Ok(Flow::Normal(None)),
        }
    }

    /// Evaluate an expression.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, ExceptionValue> {
        match expr {
            Expr::Literal(l) => Ok(self.value_from_literal(&l.value)),
            Expr::Variable(v) => self.visit_variable(&v.name, &v.span),
            Expr::Binary(b) => self.visit_binary(b),
            Expr::Pipe(p) => self.visit_pipe(p),
            Expr::Unary(u) => self.visit_unary(u),
            Expr::Grouping(g) => self.eval_expr(&g.expression),
            Expr::Call(c) => self.visit_call(c),
            Expr::Index(i) => self.visit_index(i),
            Expr::Access(a) => self.visit_access(a),
            Expr::List(l) => {
                let mut items = Vec::with_capacity(l.elements.len());
                for e in &l.elements {
                    items.push(self.eval_expr(e)?);
                }
                Ok(Value::List(Rc::new(RefCell::new(items))))
            }
            Expr::Map(m) => {
                let mut map = indexmap::IndexMap::new();
                for e in &m.entries {
                    let k = self.eval_expr(&e.key)?;
                    let v = self.eval_expr(&e.value)?;
                    map.insert(k, v);
                }
                Ok(Value::Map(Rc::new(RefCell::new(map))))
            }
            Expr::TemplateRef(t) => self.eval_expr(&t.expression),
            Expr::Lambda(l) => self.visit_lambda(l),
            Expr::MatchExpr(m) => self.visit_match_expr(m),
            Expr::Type(_) | Expr::OptionalType(_) | Expr::UnionType(_) | Expr::LiteralType(_) => {
                Ok(Value::Null)
            }
            Expr::RangePattern(r) => {
                let start = self.eval_expr(&r.start)?;
                let end = self.eval_expr(&r.end)?;
                Ok(Value::Range(Box::new(start), Box::new(end)))
            }
            Expr::WildcardPattern(_) => Ok(Value::Null),
            Expr::VariablePattern(vp) => {
                // Not reachable via normal expression evaluation; handled in match.
                self.visit_variable(&vp.name, &vp.span)
            }
            Expr::TypePattern(_) => Ok(Value::Null),
            Expr::LlmAct(la) => self.visit_llm_act(la),
            Expr::Spawn(sp) => self.visit_spawn(sp),
        }
    }

    /// `visit_spawn_expr` (v1.18) — spawn an agent in a new thread and return
    /// the main-thread `Channel` endpoint.
    ///
    /// Port of `interpreter.py:visit_spawn_expr`:
    /// 1. Resolve the agent; evaluate user args.
    /// 2. Optional `resume("<session_id>")` (carried; loaded in M8).
    /// 3. Create a bidirectional channel; auto-inject the spawned endpoint
    ///    as the agent's last argument.
    /// 4. Deep-own snapshot of the environment (Python `environment.snapshot()`
    ///    — `deepcopy`), fresh single-owner registries, then run the agent in
    ///    a new OS thread with a fresh `Interpreter`.
    /// 5. Errors are sent back as `{"__error__": true, "message": ...}`.
    /// 6. Return the main endpoint.
    fn visit_spawn(&mut self, sp: &Spawn) -> Result<Value, ExceptionValue> {
        use helen_runtime::channel::{Channel, ChannelEndpoint};
        use std::sync::Arc;

        let call = sp.call.as_ref();

        // 1. Resolve the agent being called.
        let agent_name = match call.callee.as_ref() {
            Expr::Variable(v) => v.name.clone(),
            _ => {
                return Err(self.runtime_error(Some(&sp.span), "spawn requires an agent call"));
            }
        };
        let agent = self.agents.get(&agent_name).cloned().ok_or_else(|| {
            self.runtime_error(
                Some(&sp.span),
                &format!("Undefined agent '{agent_name}' in spawn"),
            )
        })?;

        // 2. Evaluate user-provided args (the Channel param is auto-injected).
        let mut arg_values: Vec<Value> = Vec::new();
        for arg in &call.arguments {
            arg_values.push(self.eval_expr(&arg.value)?);
        }

        // v1.27: resume("<session_id>"). Session loading is M8 (session
        // manager); we validate and carry the id.
        let _resume_session_id: Option<String> = if let Some(rs) = &sp.resume_session {
            match self.eval_expr(rs)? {
                Value::Str(s) => Some(s.to_string()),
                other => {
                    return Err(self.runtime_error(
                        Some(&sp.span),
                        &format!(
                            "spawn resume() requires a string session_id, got {}",
                            other.type_name()
                        ),
                    ));
                }
            }
        } else {
            None
        };

        // 3. Bidirectional channel.
        let channel = Arc::new(Channel::<ChannelMsg>::new(format!("spawn_{agent_name}")));
        let main_endpoint = Arc::new(ChannelEndpoint::new(channel.clone(), true));
        let spawned_endpoint = Arc::new(ChannelEndpoint::new(channel, false));

        // Auto-inject the spawned endpoint. Python `visit_spawn_expr` binds
        // user args positionally to the agent's non-Channel params (the last
        // param is the auto-injected Channel). Deep-own the evaluated args
        // (single-owner transfer to the new thread).
        let mut agent_args: HashMap<String, Value> = HashMap::new();
        let data_params: Vec<&helen_core::ast::AgentParam> = agent
            .params
            .iter()
            .filter(|p| p.type_annotation.as_ref().map(|t| t.name.as_str()) != Some("Channel"))
            .collect();
        let channel_param = agent
            .params
            .iter()
            .find(|p| p.type_annotation.as_ref().map(|t| t.name.as_str()) == Some("Channel"))
            .or_else(|| agent.params.last());
        // Unbound non-Channel params default to Null.
        for p in agent.params.iter() {
            if p.type_annotation.as_ref().map(|t| t.name.as_str()) != Some("Channel") {
                agent_args.insert(p.name.clone(), Value::Null);
            }
        }
        for (i, p) in data_params.iter().enumerate() {
            if i < arg_values.len() {
                agent_args.insert(p.name.clone(), arg_values[i].make_send_owned());
            }
        }
        if let Some(cp) = channel_param {
            agent_args.insert(cp.name.clone(), Value::Channel(spawned_endpoint.clone()));
        }

        // 4. Deep-owned environment snapshot + fresh single-owner registries.
        let env_snapshot = self.environment.borrow().snapshot();
        let functions: HashMap<String, Rc<FunctionDecl>> = self
            .functions
            .iter()
            .map(|(k, v)| (k.clone(), Rc::new(v.as_ref().clone())))
            .collect();
        let agents: HashMap<String, Rc<AgentDecl>> = self
            .agents
            .iter()
            .map(|(k, v)| (k.clone(), Rc::new(v.as_ref().clone())))
            .collect();
        let protocols: HashMap<String, Rc<helen_core::ast::ProtocolDecl>> = self
            .protocols
            .iter()
            .map(|(k, v)| (k.clone(), Rc::new(v.as_ref().clone())))
            .collect();
        let impls: HashMap<(String, String), Rc<helen_core::ast::ImplDecl>> = self
            .impls
            .iter()
            .map(|(k, v)| (k.clone(), Rc::new(v.as_ref().clone())))
            .collect();
        let builtins: HashMap<String, Rc<BuiltinFn>> = self
            .builtins
            .iter()
            .map(|(k, v)| (k.clone(), Rc::new(v.as_ref().clone())))
            .collect();
        let shared_vars: std::collections::HashSet<String> = self.shared_vars.clone();
        let program_args: Vec<String> = self.program_args.clone();
        let source_file = self.source_file.clone();
        let llm_runtime = self.llm_runtime.clone();
        let stdout = self.stdout.clone();

        let payload = SpawnPayload {
            env: env_snapshot,
            functions,
            agents,
            protocols,
            impls,
            builtins,
            shared_vars,
            program_args,
            source_file,
            args: agent_args,
            agent,
            span: sp.span.clone(),
            llm_runtime,
            stdout,
            endpoint: spawned_endpoint,
        };

        std::thread::Builder::new()
            .name(format!("spawn-{agent_name}"))
            .spawn(move || run_spawned(payload))
            .map_err(|_| self.runtime_error(Some(&sp.span), "failed to spawn agent thread"))?;

        Ok(Value::Channel(main_endpoint))
    }

    /// `call_method` on a `ChannelEndpoint` (Python `ChannelEndpoint.call_method`).
    fn call_channel_method(
        &mut self,
        ep: &std::sync::Arc<helen_runtime::channel::ChannelEndpoint<ChannelMsg>>,
        name: &str,
        args: &[Value],
        span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        match name {
            // send / 发送
            "send" | "发送" => {
                if let Some(msg) = args.first() {
                    ep.send(ChannelMsg(msg.make_send_owned()));
                } else {
                    ep.send(ChannelMsg(Value::Null));
                }
                Ok(Value::Null)
            }
            // receive / 接收 — optional float timeout (seconds).
            "receive" | "接收" => {
                let timeout = match args.first() {
                    Some(Value::Float(f)) if f.is_finite() && *f >= 0.0 => {
                        Some(std::time::Duration::from_secs_f64(*f))
                    }
                    Some(Value::Int(i)) => match i.to_f64() {
                        Some(f) if f.is_finite() && f >= 0.0 => {
                            Some(std::time::Duration::from_secs_f64(f))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                match ep.receive(timeout) {
                    Some(msg) => Ok(msg.0),
                    None => Ok(Value::Null),
                }
            }
            // try_receive / 尝试接收
            "try_receive" | "尝试接收" => match ep.try_receive() {
                Some(msg) => Ok(msg.0),
                None => Ok(Value::Null),
            },
            // cancel / 取消
            "cancel" | "取消" => {
                ep.cancel();
                Ok(Value::Null)
            }
            // close / 关闭
            "close" | "关闭" => {
                ep.close();
                Ok(Value::Null)
            }
            // is_closed / 已关闭 / is_channel_closed
            "is_closed" | "已关闭" | "is_channel_closed" => Ok(Value::Bool(ep.is_closed())),
            other => {
                Err(self.runtime_error(Some(span), &format!("Channel has no method '{other}'")))
            }
        }
    }

    /// Execute a shared-store method (Python `SharedStoreMethod.__call__`).
    ///
    /// Serializes on the store lock, binds fields as locals, binds params
    /// (with defaults), executes the body, then writes modified fields back.
    fn call_store_method(
        &mut self,
        store: &std::sync::Arc<crate::shared_store::SharedStoreInstance>,
        name: &str,
        args: &[Value],
        span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        let method = store.methods.get(name).cloned().ok_or_else(|| {
            self.runtime_error(
                Some(span),
                &format!("Shared store '{}' has no method '{name}'", store.name),
            )
        })?;

        // Snapshot current fields (serialized under the store lock).
        let fields = store.fields.lock().expect("mutex poisoned").clone();

        // Execution environment: a child of the CALLING interpreter's env so
        // stdlib/consts resolve in the caller (Python v1.39.3).
        let call_env = Environment::child(self.environment.clone());
        {
            let mut env = call_env.borrow_mut();
            for (fname, fval) in &fields {
                env.define(fname, fval.clone(), false);
            }
            for (i, p) in method.params.iter().enumerate() {
                if let Some(a) = args.get(i) {
                    env.define(&p.name, a.clone(), false);
                } else if let Some(d) = &p.default_value {
                    let dv = self.eval_expr(d)?;
                    env.define(&p.name, dv, false);
                } else {
                    env.define(&p.name, Value::Null, false);
                }
            }
        }

        let result = self.with_scope(Some(call_env.clone()), |s| {
            s.execute_stmts(&method.body.body)
        })?;

        // Write back any field modifications (Python `method_env.lookup`).
        {
            let cb = call_env.borrow();
            let mut fl = store.fields.lock().expect("mutex poisoned");
            for fname in &store.field_order {
                if let Some(v) = cb.get(fname) {
                    fl.insert(fname.clone(), v);
                }
            }
        }

        match result {
            Flow::Return(v) => Ok(v.unwrap_or(Value::Null)),
            Flow::Normal(v) => Ok(v.unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }

    fn value_from_literal(&self, lit: &LiteralValue) -> Value {
        match lit {
            LiteralValue::Null => Value::Null,
            LiteralValue::Bool(b) => Value::Bool(*b),
            LiteralValue::Str(s) => Value::Str(Rc::from(s.as_str())),
            LiteralValue::Int(i) => Value::Int(i.clone()),
            LiteralValue::Float(f) => Value::Float(*f),
        }
    }

    /// `_push_scope` — run `f` with a fresh child scope (or a given env).
    fn with_scope<T>(
        &mut self,
        set_to: Option<Rc<RefCell<Environment>>>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let old = self.environment.clone();
        self.environment = set_to.unwrap_or_else(|| Environment::child(old.clone()));
        let r = f(self);
        self.environment = old;
        r
    }

    // ------------------------------------------------------------------
    // Variable / statements
    // ------------------------------------------------------------------

    fn visit_var_decl(&mut self, v: &VarDecl) -> Result<Option<Value>, ExceptionValue> {
        let is_const = !v.mutable;
        let is_lambda_init =
            matches!(&v.initializer, Some(e) if matches!(e.as_ref(), Expr::Lambda(_)));

        if is_lambda_init {
            self.environment
                .borrow_mut()
                .define(&v.name, Value::Null, false);
        }

        let mut value = Value::Null;
        if let Some(init) = &v.initializer {
            value = self.eval_expr(init)?;
        }

        if is_lambda_init {
            if let Value::Closure(c) = &value {
                let mut c: crate::closure::Closure = c.as_ref().clone();
                c.self_name = Some(v.name.clone());
                value = Value::Closure(Rc::new(c));
            }
        }

        self.environment
            .borrow_mut()
            .define(&v.name, value, is_const);
        if v.shared {
            self.shared_vars.insert(v.name.clone());
        }
        Ok(Some(if v.initializer.is_some() {
            // v1.45/HlD 3.5.1: `let` yields its initializer value to the
            // enclosing flow (Python `visit_var_decl` returns the value).
            self.environment
                .borrow()
                .get(&v.name)
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        }))
    }

    fn visit_if(&mut self, s: &IfStmt) -> Result<Flow, ExceptionValue> {
        let condition = self.eval_expr(&s.condition)?;
        // Coverage: record branch taken (0=false, 1=true).
        if self.observability.coverage.is_enabled() {
            let file = if s.span.file.is_empty() {
                self.source_file.as_deref()
            } else {
                Some(s.span.file.as_str())
            };
            self.observability.coverage.record_branch(
                file,
                s.span.start_line,
                if condition.truthy() { 1 } else { 0 },
            );
        }
        if condition.truthy() {
            return self.execute_stmt(&s.then_branch);
        }
        if let Some(else_branch) = &s.else_branch {
            return self.execute_stmt(else_branch);
        }
        Ok(Flow::Normal(None))
    }

    fn visit_for(&mut self, s: &ForStmt) -> Result<Flow, ExceptionValue> {
        let iterable = self.eval_expr(&s.iterable)?;
        let items: Vec<Value> = match &iterable {
            Value::List(l) => l.borrow().clone(),
            _ => {
                return Err(self.runtime_error(
                    Some(&s.span),
                    &format!("Cannot iterate over {}", iterable.type_name()),
                ))
            }
        };

        let mut result = Flow::Normal(None);
        for item in items {
            let step = self.with_scope(None, |interp| {
                if let Some(iter) = &s.iterator {
                    interp
                        .environment
                        .borrow_mut()
                        .define(&iter.name, item.clone(), false);
                }
                interp.execute_stmt(&s.body)
            })?;
            match step {
                Flow::Break => return Ok(result),
                Flow::Continue => continue,
                Flow::Return(_) => return Ok(step),
                Flow::Normal(v) => result = Flow::Normal(v),
            }
        }
        Ok(result)
    }

    fn visit_while(&mut self, s: &helen_core::ast::WhileStmt) -> Result<Flow, ExceptionValue> {
        let mut result = Flow::Normal(None);
        loop {
            let condition = self.eval_expr(&s.condition)?;
            if !condition.truthy() {
                break;
            }
            let step = self.execute_stmt(&s.body)?;
            match step {
                Flow::Break => break,
                Flow::Continue => continue,
                Flow::Return(_) => return Ok(step),
                Flow::Normal(v) => result = Flow::Normal(v),
            }
        }
        Ok(result)
    }

    fn visit_function_decl(&mut self, f: &FunctionDecl) {
        self.functions.insert(f.name.clone(), Rc::new(f.clone()));
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn visit_variable(&mut self, name: &str, span: &SourceSpan) -> Result<Value, ExceptionValue> {
        if let Some(v) = self.environment.borrow().get(name) {
            return Ok(v);
        }
        if let Some(f) = self.functions.get(name) {
            return Ok(Value::UserFn(f.clone()));
        }
        if let Some(a) = self.agents.get(name) {
            return Ok(Value::Agent(a.clone()));
        }
        Err(self.runtime_error(Some(span), &format!("Undefined variable '{name}'")))
    }

    fn visit_binary(&mut self, b: &Binary) -> Result<Value, ExceptionValue> {
        let op = b.operator.kind;

        if op == TokenType::Assign {
            let right = self.eval_expr(&b.right)?;
            match b.left.as_ref() {
                Expr::Variable(v) => {
                    if self.environment.borrow().is_const(&v.name) {
                        return Err(ExceptionValue::new(
                            "ConstAssignmentError",
                            format!("cannot assign to const variable '{}'", v.name),
                            Some(b.span.clone()),
                        ));
                    }
                    match self.environment.borrow_mut().assign(&v.name, right.clone()) {
                        Ok(_) => Ok(right),
                        Err(_) => Err(self.runtime_error(
                            Some(&b.span),
                            &format!("Undefined variable '{}'", v.name),
                        )),
                    }
                }
                Expr::Index(i) => {
                    let target = self.eval_expr(&i.target)?;
                    let index = self.eval_expr(&i.index)?;
                    self.assign_index(&target, &index, right, &b.span)
                }
                Expr::Access(a) => {
                    let target = self.eval_expr(&a.target)?;
                    self.assign_access(&target, &a.property, right, &b.span)
                }
                _ => Err(self.runtime_error(Some(&b.span), "Invalid assignment target")),
            }
        } else if op == TokenType::And {
            let left = self.eval_expr(&b.left)?;
            if !left.truthy() {
                return Ok(Value::Bool(false));
            }
            let right = self.eval_expr(&b.right)?;
            Ok(Value::Bool(right.truthy()))
        } else if op == TokenType::Or {
            let left = self.eval_expr(&b.left)?;
            if left.truthy() {
                return Ok(Value::Bool(true));
            }
            let right = self.eval_expr(&b.right)?;
            Ok(Value::Bool(right.truthy()))
        } else {
            let left = self.eval_expr(&b.left)?;
            let right = self.eval_expr(&b.right)?;
            self.binary_op(op, left, right, b)
        }
    }

    fn binary_op(
        &self,
        op: TokenType,
        left: Value,
        right: Value,
        node: &Binary,
    ) -> Result<Value, ExceptionValue> {
        use TokenType::*;
        match op {
            Plus => self.add(left, right, &node.operator),
            Minus | Star | Slash | Percent => {
                self.check_number(&node.operator, &[&left, &right])?;
                self.num_binop(op, left, right, node)
            }
            EqualEqual => Ok(Value::Bool(left == right)),
            BangEqual => Ok(Value::Bool(left != right)),
            Greater | GreaterEqual | Less | LessEqual => {
                self.check_number(&node.operator, &[&left, &right])?;
                let ord = cmp_values(&left, &right);
                match ord {
                    None => Ok(Value::Bool(false)), // NaN comparisons are false
                    Some(o) => Ok(Value::Bool(match op {
                        Greater => o == Ordering::Greater,
                        GreaterEqual => o != Ordering::Less,
                        Less => o == Ordering::Less,
                        LessEqual => o != Ordering::Greater,
                        _ => unreachable!(),
                    })),
                }
            }
            _ => Err(self.runtime_error(
                Some(&node.span),
                &format!("Unknown operator '{}'", node.operator.lexeme),
            )),
        }
    }

    /// Python `_add`: string concat if either side is a string; list concat
    /// for two lists; numeric addition otherwise.
    fn add(&self, left: Value, right: Value, _op: &Token) -> Result<Value, ExceptionValue> {
        if let Value::Str(_) = &left {
            let s = format!("{}{}", left.python_str(), right.python_str());
            return Ok(Value::Str(Rc::from(s.as_str())));
        }
        if let Value::Str(_) = &right {
            let s = format!("{}{}", left.python_str(), right.python_str());
            return Ok(Value::Str(Rc::from(s.as_str())));
        }
        if let (Value::Int(a), Value::Int(b)) = (&left, &right) {
            return Ok(Value::Int(a + b));
        }
        if is_number(&left) && is_number(&right) {
            let a = to_f64(&left);
            let b = to_f64(&right);
            return Ok(Value::Float(a + b));
        }
        if let (Value::List(a), Value::List(b)) = (&left, &right) {
            let mut items = a.borrow().clone();
            items.extend(b.borrow().iter().cloned());
            return Ok(Value::List(Rc::new(RefCell::new(items))));
        }
        Err(ExceptionValue::new(
            "RuntimeError",
            format!("Cannot add {} and {}", left.type_name(), right.type_name()),
            None,
        ))
    }

    fn num_binop(
        &self,
        op: TokenType,
        left: Value,
        right: Value,
        node: &Binary,
    ) -> Result<Value, ExceptionValue> {
        let zero: Value = Value::Int(BigInt::from(0));
        let right_is_zero = match &right {
            Value::Int(i) => i.is_zero(),
            Value::Float(f) => *f == 0.0,
            _ => false,
        };
        match op {
            TokenType::Slash => {
                if right_is_zero {
                    return Err(self.runtime_error(Some(&node.span), "Division by zero"));
                }
                // Python int/int always yields float.
                let a = to_f64(&left);
                let b = to_f64(&right);
                Ok(Value::Float(a / b))
            }
            TokenType::Percent => {
                if right_is_zero {
                    return Err(self.runtime_error(Some(&node.span), "Modulo by zero"));
                }
                match (&left, &right) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(py_mod(a, b))),
                    _ => {
                        let a = to_f64(&left);
                        let b = to_f64(&right);
                        Ok(Value::Float(a.rem_euclid(b)))
                    }
                }
            }
            TokenType::Minus | TokenType::Star => match (&left, &right) {
                (Value::Int(a), Value::Int(b)) => {
                    if op == TokenType::Minus {
                        Ok(Value::Int(a - b))
                    } else {
                        Ok(Value::Int(a * b))
                    }
                }
                _ => {
                    let a = to_f64(&left);
                    let b = to_f64(&right);
                    if op == TokenType::Minus {
                        Ok(Value::Float(a - b))
                    } else {
                        Ok(Value::Float(a * b))
                    }
                }
            },
            _ => {
                let _ = zero;
                Err(self.runtime_error(
                    Some(&node.span),
                    &format!("Unknown operator '{}'", node.operator.lexeme),
                ))
            }
        }
    }

    fn visit_unary(&mut self, u: &Unary) -> Result<Value, ExceptionValue> {
        let operand = self.eval_expr(&u.operand)?;
        match u.operator.kind {
            TokenType::Bang => Ok(Value::Bool(!operand.truthy())),
            TokenType::Minus => {
                self.check_number(&u.operator, &[&operand])?;
                match &operand {
                    Value::Int(i) => Ok(Value::Int(-i)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    Value::Bool(b) => Ok(Value::Int(if *b {
                        BigInt::from(-1)
                    } else {
                        BigInt::from(0)
                    })),
                    _ => unreachable!(),
                }
            }
            _ => Err(self.runtime_error(
                Some(&u.span),
                &format!("Unknown unary operator '{}'", u.operator.lexeme),
            )),
        }
    }

    /// `value |> fn` — desugars to `fn(value)`.
    fn visit_pipe(&mut self, p: &Pipe) -> Result<Value, ExceptionValue> {
        let value = self.eval_expr(&p.value)?;
        if let Expr::Variable(v) = p.function.as_ref() {
            if let Some(f) = self.functions.get(&v.name).cloned() {
                let parent_env = self.function_module_envs.get(&v.name).cloned();
                return self.call_function(&f, vec![value], parent_env, &p.span);
            }
            if let Some(a) = self.agents.get(&v.name).cloned() {
                let mut args = HashMap::new();
                if let Some(first) = a.params.first() {
                    args.insert(first.name.clone(), value);
                }
                return self.call_agent(&a, args, &p.span);
            }
        }
        let func = self.eval_expr(&p.function)?;
        match func {
            Value::Closure(c) => self.call_closure(&c, vec![value], &p.span),
            Value::BuiltinFn(b) => (b.func)(self, &[value]),
            Value::UserFn(f) => self.call_function(&f, vec![value], None, &p.span),
            Value::Agent(a) => {
                let mut args = HashMap::new();
                if let Some(first) = a.params.first() {
                    args.insert(first.name.clone(), value);
                }
                self.call_agent(&a, args, &p.span)
            }
            _ => {
                let name = match p.function.as_ref() {
                    Expr::Variable(v) => v.name.clone(),
                    other => format!("{:?}", std::mem::discriminant(other)),
                };
                Err(self.runtime_error(Some(&p.span), &format!("'{name}' is not callable")))
            }
        }
    }

    fn visit_call(&mut self, c: &Call) -> Result<Value, ExceptionValue> {
        let callee_name = match c.callee.as_ref() {
            Expr::Variable(v) => Some(v.name.clone()),
            _ => None,
        };

        // 1. Registered function
        if let Some(name) = &callee_name {
            if let Some(f) = self.functions.get(name).cloned() {
                let args = self.eval_args(&c.arguments)?;
                let parent_env = self.function_module_envs.get(name).cloned();
                return self.call_function(&f, args, parent_env, &c.span);
            }
        }
        // 2. Registered agent
        if let Some(name) = &callee_name {
            if let Some(a) = self.agents.get(name).cloned() {
                let mut agent_args = HashMap::new();
                for (i, arg) in c.arguments.iter().enumerate() {
                    let value = self.eval_expr(&arg.value)?;
                    if let Some(n) = &arg.name {
                        agent_args.insert(n.clone(), value);
                    } else if i < a.params.len() {
                        agent_args.insert(a.params[i].name.clone(), value);
                    } else {
                        return Err(self.runtime_error(
                            Some(&c.span),
                            &format!(
                                "too many positional arguments for agent '{name}' \
                                 (expected at most {})",
                                a.params.len()
                            ),
                        ));
                    }
                }
                return self.call_agent(&a, agent_args, &c.span);
            }
        }

        // 3. Evaluate callee and dispatch
        let callee = self.eval_expr(&c.callee)?;
        let args = self.eval_args(&c.arguments)?;
        match callee {
            Value::UserFn(f) => {
                let parent_env = self.function_module_envs.get(&f.name).cloned();
                self.call_function(&f, args, parent_env, &c.span)
            }
            Value::Closure(cl) => self.call_closure(&cl, args, &c.span),
            Value::BuiltinFn(b) => (b.func)(self, &args),
            Value::Agent(a) => {
                let mut agent_args = HashMap::new();
                for (i, arg) in args.into_iter().enumerate() {
                    if i < a.params.len() {
                        agent_args.insert(a.params[i].name.clone(), arg);
                    }
                }
                self.call_agent(&a, agent_args, &c.span)
            }
            Value::MapMethod(m) => m.call(&args),
            Value::ListMethod(m) => m.call(&args),
            Value::ChannelMethod(cm) => {
                self.call_channel_method(&cm.endpoint, &cm.name, &args, &c.span)
            }
            Value::StoreMethod(sm) => self.call_store_method(&sm.store, &sm.name, &args, &c.span),
            // M10: native callable (Python function/class/callable object).
            Value::Native(n) => {
                let (pos, kwargs) = self.split_args(&c.arguments)?;
                n.0.call(&pos, &kwargs)
                    .map_err(|e| self.runtime_error(Some(&c.span), &e.message))
            }
            other => Err(self.runtime_error(
                Some(&c.span),
                &format!("'{}' is not callable", other.type_name()),
            )),
        }
    }

    /// Evaluate call arguments, splitting positional and keyword args
    /// (M10 FFI calls pass kwargs through to Python).
    #[allow(clippy::type_complexity)]
    fn split_args(
        &mut self,
        arguments: &[CallArg],
    ) -> Result<(Vec<Value>, Vec<(String, Value)>), ExceptionValue> {
        let mut pos = Vec::new();
        let mut kwargs = Vec::new();
        for a in arguments {
            let v = self.eval_expr(&a.value)?;
            match &a.name {
                Some(name) => kwargs.push((name.clone(), v)),
                None => pos.push(v),
            }
        }
        Ok((pos, kwargs))
    }

    fn eval_args(&mut self, arguments: &[CallArg]) -> Result<Vec<Value>, ExceptionValue> {
        let mut out = Vec::with_capacity(arguments.len());
        for a in arguments {
            out.push(self.eval_expr(&a.value)?);
        }
        Ok(out)
    }

    /// Generic call dispatch for a value (used by stdlib higher-order
    /// functions like `map`/`filter`/`reduce`/`sort` that receive closures).
    pub fn call_value(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, ExceptionValue> {
        match callee {
            Value::UserFn(f) => {
                let parent_env = self.function_module_envs.get(&f.name).cloned();
                self.call_function(&f, args, parent_env, &self.fake_span())
            }
            Value::Closure(cl) => self.call_closure(&cl, args, &self.fake_span()),
            Value::BuiltinFn(b) => (b.func)(self, &args),
            Value::Agent(a) => {
                let mut agent_args = HashMap::new();
                for (i, arg) in args.into_iter().enumerate() {
                    if i < a.params.len() {
                        agent_args.insert(a.params[i].name.clone(), arg);
                    }
                }
                self.call_agent(&a, agent_args, &self.fake_span())
            }
            Value::MapMethod(m) => m.call(&args),
            Value::ListMethod(m) => m.call(&args),
            Value::ChannelMethod(cm) => {
                self.call_channel_method(&cm.endpoint, &cm.name, &args, &self.fake_span())
            }
            Value::StoreMethod(sm) => {
                self.call_store_method(&sm.store, &sm.name, &args, &self.fake_span())
            }
            // M10: native callable invoked with positional args only.
            Value::Native(n) => {
                n.0.call(&args, &[])
                    .map_err(|e| self.runtime_error(None, &e.message))
            }
            other => {
                Err(self.runtime_error(None, &format!("'{}' is not callable", other.type_name())))
            }
        }
    }

    /// A zero-width span for synthetic calls (stdlib higher-order functions).
    fn fake_span(&self) -> SourceSpan {
        SourceSpan::new("", 0, 0, 0, 0)
    }

    fn visit_index(&mut self, i: &Index) -> Result<Value, ExceptionValue> {
        let target = self.eval_expr(&i.target)?;
        let index = self.eval_expr(&i.index)?;
        match &target {
            Value::List(l) => match &index {
                Value::Int(idx) => {
                    let items = l.borrow();
                    let n = items.len() as i64;
                    let mut real = idx.to_i64().unwrap_or(i64::MAX);
                    if real < 0 {
                        real += n;
                    }
                    if real < 0 || real >= n {
                        return Err(self.runtime_error(Some(&i.span), "list index out of range"));
                    }
                    Ok(items[real as usize].clone())
                }
                other => Err(self.runtime_error(
                    Some(&i.span),
                    &format!("List index must be integer, got {}", other.type_name()),
                )),
            },
            Value::Map(m) => {
                let map = m.borrow();
                if let Some(v) = map.get(&index) {
                    return Ok(v.clone());
                }
                // v1.11 detailed missing-key error
                let available: Vec<Value> = map.keys().cloned().collect();
                let keys_str = format_keys(&available);
                Err(self.runtime_error(
                    Some(&i.span),
                    &format!(
                        "Map key {} not found. Available keys: {}",
                        index.python_repr(),
                        keys_str
                    ),
                ))
            }
            Value::Str(s) => self.index_string(s, &index, &i.span),
            // v1.12: ReadOnlyView __getitem__ delegates; nested mutables wrap.
            Value::ReadOnly(r) => {
                let inner = r.borrow().clone();
                match &inner {
                    Value::List(l) => match &index {
                        Value::Int(idx) => {
                            let items = l.borrow();
                            let n = items.len() as i64;
                            let mut real = idx.to_i64().unwrap_or(i64::MAX);
                            if real < 0 {
                                real += n;
                            }
                            if real < 0 || real >= n {
                                return Err(
                                    self.runtime_error(Some(&i.span), "list index out of range")
                                );
                            }
                            let v = items[real as usize].clone();
                            Ok(if v.is_mutable_type() {
                                Value::ReadOnly(Rc::new(RefCell::new(v)))
                            } else {
                                v
                            })
                        }
                        other => Err(self.runtime_error(
                            Some(&i.span),
                            &format!("List index must be integer, got {}", other.type_name()),
                        )),
                    },
                    Value::Map(m) => {
                        let map = m.borrow();
                        if let Some(v) = map.get(&index) {
                            let v = v.clone();
                            return Ok(if v.is_mutable_type() {
                                Value::ReadOnly(Rc::new(RefCell::new(v)))
                            } else {
                                v
                            });
                        }
                        Err(self.runtime_error(
                            Some(&i.span),
                            &format!("Map key {} not found", index.python_repr()),
                        ))
                    }
                    other => Err(self.runtime_error(
                        Some(&i.span),
                        &format!("Type {} does not support indexing", other.type_name()),
                    )),
                }
            }
            // M10: native object __getitem__ (Python `obj[key]`).
            Value::Native(n) => {
                n.0.get_item(&index)
                    .map_err(|e| self.runtime_error(Some(&i.span), &e.message))
            }
            other => Err(self.runtime_error(
                Some(&i.span),
                &format!("Type {} does not support indexing", other.type_name()),
            )),
        }
    }

    /// Byte-based string indexing (D4). ASCII matches Python exactly.
    fn index_string(
        &self,
        s: &str,
        index: &Value,
        span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        let idx = match index {
            Value::Int(i) => i,
            other => {
                return Err(self.runtime_error(
                    Some(span),
                    &format!("String index must be integer, got {}", other.type_name()),
                ))
            }
        };
        let bytes = s.as_bytes();
        let n = bytes.len() as i64;
        let mut real = idx.to_i64().unwrap_or(i64::MAX);
        if real < 0 {
            real += n;
        }
        if real < 0 || real >= n {
            return Err(self.runtime_error(Some(span), "string index out of range"));
        }
        let pos = real as usize;
        // UTF-8 boundary validation (divergence from Python for non-ASCII)
        if !s.is_char_boundary(pos) {
            return Err(self.runtime_error(Some(span), "string index is not a character boundary"));
        }
        let end = pos + 1;
        let ch = s
            .get(pos..end)
            .ok_or_else(|| self.runtime_error(Some(span), "string index out of range"))?;
        Ok(Value::Str(Rc::from(ch)))
    }

    fn assign_index(
        &self,
        target: &Value,
        index: &Value,
        right: Value,
        span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        match target {
            Value::List(l) => {
                let idx = match index {
                    Value::Int(i) => i,
                    _ => {
                        return Err(self.runtime_error(
                            Some(span),
                            &format!("List index must be integer, got {}", index.type_name()),
                        ))
                    }
                };
                let mut items = l.borrow_mut();
                let n = items.len() as i64;
                let mut real = idx.to_i64().unwrap_or(i64::MAX);
                if real < 0 {
                    real += n;
                }
                if real < 0 || real >= n {
                    return Err(self.runtime_error(Some(span), "list index out of range"));
                }
                items[real as usize] = right.clone();
                Ok(right)
            }
            Value::Map(m) => {
                m.borrow_mut().insert(index.clone(), right.clone());
                Ok(right)
            }
            // M10: native object __setitem__ (Python `obj[key] = value`).
            Value::Native(n) => {
                n.0.set_item(index, &right)
                    .map_err(|e| self.runtime_error(Some(span), &e.message))?;
                Ok(right)
            }
            // v1.12: mutation through a ReadOnlyView raises ScopeViolationError.
            Value::ReadOnly(_) => Err(ExceptionValue::new(
                "ScopeViolationError",
                String::from("cannot modify a read-only value (agent parameter isolation)"),
                Some((*span).clone()),
            )),
            other => Err(self.runtime_error(
                Some(span),
                &format!("Type {} does not support indexing", other.type_name()),
            )),
        }
    }

    fn visit_access(&mut self, a: &Access) -> Result<Value, ExceptionValue> {
        // v1.42: static agent function access: AgentName.fn_name
        if let Expr::Variable(v) = a.target.as_ref() {
            if let Some(agent) = self.agents.get(&v.name) {
                if let Some(f) = agent.functions.iter().find(|f| f.name == a.property) {
                    let wrapper = Rc::new(f.clone());
                    return Ok(Value::UserFn(wrapper));
                }
                return Err(self.runtime_error(
                    Some(&a.span),
                    &format!(
                        "Agent '{}' has no function '{}' in its functions{{}} block",
                        agent.name, a.property
                    ),
                ));
            }
        }

        let target = self.eval_expr(&a.target)?;
        let prop = &a.property;
        match &target {
            Value::Map(m) => {
                let map = m.borrow();
                if (prop == "get" || prop == "keys" || prop == "values" || prop == "items")
                    && !map.contains_key(&Value::Str(Rc::from(prop.as_str())))
                {
                    // dict method access
                    return Ok(Value::MapMethod(Box::new(MapMethodValue {
                        kind: match prop.as_str() {
                            "get" => MapMethodKind::Get,
                            "keys" => MapMethodKind::Keys,
                            "values" => MapMethodKind::Values,
                            _ => MapMethodKind::Items,
                        },
                        map: m.clone(),
                    })));
                }
                if let Some(v) = map.get(&Value::Str(Rc::from(prop.as_str()))) {
                    return Ok(v.clone());
                }
                Err(self.runtime_error(Some(&a.span), &format!("Property '{prop}' not found")))
            }
            Value::Exception(ex) => {
                if prop == "message" {
                    return Ok(Value::Str(Rc::from(ex.message.as_str())));
                }
                if let Some(v) = ex.fields.get(prop) {
                    return Ok(v.clone());
                }
                Err(self.runtime_error(
                    Some(&a.span),
                    &format!("'{}' has no property '{}'", ex.class_name, prop),
                ))
            }
            Value::List(l) => {
                // Native list methods (Python `getattr(list, prop)`): append,
                // insert, pop, remove, count, index, clear, extend, reverse.
                let kind = match prop.as_str() {
                    "append" => Some(ListMethodKind::Append),
                    "insert" => Some(ListMethodKind::Insert),
                    "pop" => Some(ListMethodKind::Pop),
                    "remove" => Some(ListMethodKind::Remove),
                    "count" => Some(ListMethodKind::Count),
                    "index" => Some(ListMethodKind::Index),
                    "clear" => Some(ListMethodKind::Clear),
                    "extend" => Some(ListMethodKind::Extend),
                    "reverse" => Some(ListMethodKind::Reverse),
                    "copy" => Some(ListMethodKind::Copy),
                    "sort" => Some(ListMethodKind::Sort),
                    _ => None,
                };
                if let Some(kind) = kind {
                    return Ok(Value::ListMethod(Box::new(ListMethodValue {
                        kind,
                        list: l.clone(),
                    })));
                }
                Err(self.runtime_error(
                    Some(&a.span),
                    &format!("'list' has no property '{}'", a.property),
                ))
            }
            Value::Channel(ep) => {
                // ChannelEndpoint methods (Python `ChannelEndpoint.call_method`).
                let methods = [
                    "send",
                    "receive",
                    "try_receive",
                    "cancel",
                    "close",
                    "is_closed",
                    "is_channel_closed",
                    "发送",
                    "接收",
                    "尝试接收",
                    "取消",
                    "关闭",
                    "已关闭",
                ];
                if methods.contains(&prop.as_str()) {
                    Ok(Value::ChannelMethod(Box::new(ChannelMethodValue {
                        endpoint: ep.clone(),
                        name: prop.clone(),
                    })))
                } else {
                    Err(self
                        .runtime_error(Some(&a.span), &format!("Channel has no method '{prop}'")))
                }
            }
            Value::SharedStore(store) => {
                // Methods first (Python __getattr__: methods win over fields).
                if store.methods.contains_key(prop) {
                    return Ok(Value::StoreMethod(Box::new(StoreMethodValue {
                        store: store.clone(),
                        name: prop.clone(),
                    })));
                }
                if let Some(v) = store.get_field(prop) {
                    return Ok(v);
                }
                Err(self.runtime_error(
                    Some(&a.span),
                    &format!(
                        "Shared store '{}' has no field or method '{prop}'",
                        store.name
                    ),
                ))
            }
            Value::ReadOnly(_) => {
                // v1.12: ReadOnlyView has no __getattr__ delegation — method
                // access raises (Python: AttributeError), which is exactly the
                // mutation guard. Reads go through __getitem__/__iter__.
                Err(self.runtime_error(
                    Some(&a.span),
                    &format!(
                        "ReadOnlyView has no attribute '{prop}' \
                         (read-only wrapper for agent params)"
                    ),
                ))
            }
            // M10: native object __getattr__ (Python `module.attr`, `obj.attr`).
            Value::Native(n) => {
                n.0.get_attribute(prop)
                    .map_err(|e| self.runtime_error(Some(&a.span), &e.message))
            }
            other => Err(self.runtime_error(
                Some(&a.span),
                &format!("'{}' has no property '{}'", other.type_name(), a.property),
            )),
        }
    }

    fn assign_access(
        &self,
        target: &Value,
        prop: &str,
        right: Value,
        span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        if let Value::Map(m) = target {
            m.borrow_mut()
                .insert(Value::Str(Rc::from(prop)), right.clone());
            return Ok(right);
        }
        // Shared-store field assignment: `Store.field = value`.
        if let Value::SharedStore(store) = target {
            store
                .set_field(prop, right.clone())
                .map_err(|e| self.runtime_error(Some(span), &e))?;
            return Ok(right);
        }
        // v1.12: ReadOnlyView blocks attribute mutation.
        if let Value::ReadOnly(_) = target {
            return Err(ExceptionValue::new(
                "ScopeViolationError",
                String::from("cannot modify a read-only value (agent parameter isolation)"),
                Some((*span).clone()),
            ));
        }
        // M10: native object __setattr__ (Python `obj.attr = value`).
        if let Value::Native(n) = target {
            n.0.set_item(&Value::Str(Rc::from(prop)), &right)
                .map_err(|e| self.runtime_error(Some(span), &e.message))?;
            return Ok(right);
        }
        Err(self.runtime_error(
            Some(span),
            &format!(
                "Type {} does not support attribute assignment",
                target.type_name()
            ),
        ))
    }

    fn visit_lambda(&mut self, l: &Lambda) -> Result<Value, ExceptionValue> {
        let free_vars = compute_free_variables(l);
        let mut captured_env = Environment::new(None);
        for var in &free_vars {
            if let Some(value) = self.environment.borrow().get(var) {
                let value = if is_mutable_type(&value) {
                    value.clone_deep()
                } else {
                    value
                };
                captured_env.define(var, value, false);
            }
        }
        let closure = Closure::new(Rc::new(l.clone()), Rc::new(RefCell::new(captured_env)));
        Ok(Value::Closure(Rc::new(closure)))
    }

    // ------------------------------------------------------------------
    // Try / throw / assert
    // ------------------------------------------------------------------

    fn visit_try(&mut self, t: &helen_core::ast::TryStmt) -> Result<Flow, ExceptionValue> {
        let mut caught = false;
        let mut exc_to_rethrow: Option<ExceptionValue> = None;
        let mut result = Flow::Normal(None);

        let try_result = self.with_scope(None, |s| s.execute_stmts(&t.body));
        match try_result {
            Ok(flow) => result = flow,
            Err(exc) => {
                caught = true;
                exc_to_rethrow = Some(exc.clone());

                for clause in &t.catch_clauses {
                    let type_name = &clause.error_type.name;
                    if error_matches(&exc, type_name) {
                        let catch_result = self.with_scope(None, |s| {
                            s.environment.borrow_mut().define(
                                &clause.error_name,
                                Value::Exception(Box::new(exc.clone())),
                                false,
                            );
                            s.execute_stmts(&clause.body)
                        });
                        match catch_result {
                            Ok(Flow::Return(_)) => return catch_result,
                            Ok(flow) => {
                                result = flow;
                            }
                            Err(_) => return catch_result,
                        }
                        caught = false;
                        break;
                    }
                }

                if caught {
                    if let Some(ca) = &t.catch_all {
                        let catch_result = self.with_scope(None, |s| s.execute_stmts(&ca.body));
                        match catch_result {
                            Ok(Flow::Return(_)) => return catch_result,
                            Ok(flow) => {
                                result = flow;
                            }
                            Err(_) => return catch_result,
                        }
                        caught = false;
                    }
                }
            }
        }

        // finally
        if let Some(fb) = &t.finally_block {
            let finally_result = self.with_scope(None, |s| s.execute_stmts(&fb.body));
            if let Err(e) = &finally_result {
                return Err(e.clone()); // thrown-from-finally replaces prior error
            }
        }

        if caught {
            if let Some(exc) = exc_to_rethrow {
                return Err(exc);
            }
        }
        Ok(result)
    }

    /// `throw ExceptionType` or `throw ExceptionType(message)`.
    fn make_throw(&mut self, t: &ThrowStmt) -> Result<ExceptionValue, ExceptionValue> {
        let type_name = &t.exception_type.name;
        let class_name = match resolve_exception(type_name) {
            Some(n) => n.to_string(),
            None => {
                return Err(self.runtime_error(
                    Some(&t.span),
                    &format!("'{type_name}' is not a valid exception type"),
                ))
            }
        };

        let message_value = if let Some(m) = &t.message {
            let v = self.eval_expr(m)?;
            match v {
                Value::Str(s) => s.to_string(),
                other => other.python_str(),
            }
        } else {
            ExceptionValue::default_message(&class_name)
        };

        Ok(self.build_exception(&class_name, message_value, t.span.clone()))
    }

    /// Port of `exc_class(message, node.span)` — the positional-argument
    /// quirks per class (AgentError receives the message as agent_name).
    fn build_exception(
        &self,
        class_name: &str,
        message: String,
        span: SourceSpan,
    ) -> ExceptionValue {
        match class_name {
            "AgentError" => ExceptionValue {
                class_name: class_name.to_string(),
                message: format!("Agent '{message}' failed"),
                span: None,
                fields: indexmap::IndexMap::new(),
            },
            "LLMOutputContractError" => ExceptionValue {
                class_name: class_name.to_string(),
                message: format!("Agent '{message}' output does not match contract: "),
                span: None,
                fields: indexmap::IndexMap::new(),
            },
            _ => ExceptionValue::new(class_name, message, Some(span)),
        }
    }

    fn visit_assert(&mut self, a: &helen_core::ast::AssertStmt) -> Result<(), ExceptionValue> {
        let condition = self.eval_expr(&a.condition)?;
        if condition.truthy() {
            return Ok(());
        }
        let message = if let Some(m) = &a.message {
            let v = self.eval_expr(m)?;
            match v {
                Value::Str(s) => s.to_string(),
                other => other.python_str(),
            }
        } else {
            "Assertion failed".to_string()
        };
        Err(ExceptionValue::new(
            "AssertionError",
            message,
            Some(a.span.clone()),
        ))
    }

    // ------------------------------------------------------------------
    // Match / patterns
    // ------------------------------------------------------------------

    fn match_case(
        &mut self,
        subject: &Value,
        pattern: &Expr,
        guard: &Option<Box<Expr>>,
    ) -> Result<(bool, Vec<(String, Value)>), ExceptionValue> {
        let mut matched = false;
        let mut bindings: Vec<(String, Value)> = Vec::new();
        match pattern {
            Expr::WildcardPattern(_) => matched = true,
            Expr::VariablePattern(vp) => {
                matched = true;
                bindings.push((vp.name.clone(), subject.clone()));
            }
            Expr::TypePattern(tp) => {
                matched = check_type(subject, &tp.type_name);
                if matched {
                    if let Some(bn) = &tp.binding_name {
                        bindings.push((bn.clone(), subject.clone()));
                    }
                }
            }
            Expr::RangePattern(r) => {
                let start = self.eval_expr(&r.start)?;
                let end = self.eval_expr(&r.end)?;
                if let (Some(a), Some(b), Some(c)) =
                    (num_f64(subject), num_f64(&start), num_f64(&end))
                {
                    matched = a >= b && a <= c;
                }
            }
            other => {
                let pattern_value = self.eval_expr(other)?;
                matched = subject == &pattern_value;
            }
        }

        if matched {
            if let Some(g) = guard {
                let guard_ok = self.with_scope(None, |s| {
                    for (name, value) in &bindings {
                        s.environment
                            .borrow_mut()
                            .define(name, value.clone(), false);
                    }
                    s.eval_expr(g)
                })?;
                matched = guard_ok.truthy();
            }
        }
        Ok((matched, bindings))
    }

    fn visit_match_stmt(
        &mut self,
        m: &helen_core::ast::MatchStmtNode,
    ) -> Result<Flow, ExceptionValue> {
        let subject = self.eval_expr(&m.subject)?;
        for case in &m.cases {
            let (matched, bindings) = self.match_case(&subject, &case.pattern, &case.guard)?;
            if matched {
                let flow = self.with_scope(None, |s| {
                    for (name, value) in &bindings {
                        s.environment
                            .borrow_mut()
                            .define(name, value.clone(), false);
                    }
                    s.execute_stmts(&case.body)
                })?;
                return Ok(flow);
            }
        }
        if !m.default.is_empty() {
            let flow = self.with_scope(None, |s| s.execute_stmts(&m.default))?;
            return Ok(flow);
        }
        Ok(Flow::Normal(None))
    }

    fn visit_match_expr(
        &mut self,
        m: &helen_core::ast::MatchExprNode,
    ) -> Result<Value, ExceptionValue> {
        let subject = self.eval_expr(&m.subject)?;
        for case in &m.cases {
            let (matched, bindings) = self.match_case(&subject, &case.pattern, &case.guard)?;
            if matched {
                return self.with_scope(None, |s| {
                    for (name, value) in &bindings {
                        s.environment
                            .borrow_mut()
                            .define(name, value.clone(), false);
                    }
                    match case.body.first() {
                        Some(Stmt::Expr(e)) => s.eval_expr(&e.expression),
                        _ => {
                            let _ = s.execute_stmts(&case.body)?;
                            Ok(Value::Null)
                        }
                    }
                });
            }
        }
        if let Some(d) = &m.default_body {
            return self.eval_expr(d);
        }
        Ok(Value::Null)
    }

    // ------------------------------------------------------------------
    // Calls
    // ------------------------------------------------------------------

    /// Call a top-level Helen function with fully-resolved positional args.
    ///
    /// M11: made `pub` so the Python bridge (`helen-python-bridge`) can call
    /// functions from Python (`HelenFunctionWrapper.__call__` parity).
    pub fn call_function(
        &mut self,
        func: &FunctionDecl,
        args: Vec<Value>,
        parent_env: Option<Rc<RefCell<Environment>>>,
        _span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        // Runtime parameter type checking
        for (i, param) in func.params.iter().enumerate() {
            if i < args.len() {
                if let Some(tn) = &param.type_annotation {
                    let expected = type_from_typenode(tn);
                    let actual = type_of_value(&args[i]);
                    if !matches!(actual, Type::Any) && !type_compatible(&actual, &expected) {
                        return Err(ExceptionValue::new(
                            "RuntimeError",
                            format!(
                                "argument {} type '{}' is not compatible with parameter type '{}'",
                                i + 1,
                                actual.name(),
                                expected.name()
                            ),
                            None,
                        ));
                    }
                }
            }
        }

        let call_env = match &parent_env {
            Some(pe) => Rc::new(RefCell::new(Environment::new(Some(pe.clone())))),
            None => Environment::child(self.environment.clone()),
        };

        // Bind parameters
        {
            let mut env = call_env.borrow_mut();
            for (i, param) in func.params.iter().enumerate() {
                if i < args.len() {
                    env.define(&param.name, args[i].clone(), false);
                } else if let Some(dv) = &param.default_value {
                    let default_val = if parent_env.is_some() {
                        // evaluate default in module env
                        self.with_scope(parent_env.clone(), |s| s.eval_expr(dv))?
                    } else {
                        self.eval_expr(dv)?
                    };
                    env.define(&param.name, default_val, false);
                } else {
                    env.define(&param.name, Value::Null, false);
                }
            }
        }

        let flow = self.with_scope(Some(call_env), |s| s.execute_stmts(&func.body.body))?;
        match flow {
            Flow::Return(v) => Ok(v.unwrap_or(Value::Null)),
            Flow::Normal(v) => Ok(v.unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }

    pub fn call_closure(
        &mut self,
        closure: &Closure,
        args: Vec<Value>,
        _span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        let lambda = closure.lambda.clone();

        // Runtime parameter type checking (same as call_function)
        for (i, param) in lambda.params.iter().enumerate() {
            if i < args.len() {
                if let Some(tn) = &param.type_annotation {
                    let expected = type_from_typenode(tn);
                    let actual = type_of_value(&args[i]);
                    if !matches!(actual, Type::Any) && !type_compatible(&actual, &expected) {
                        return Err(ExceptionValue::new(
                            "RuntimeError",
                            format!(
                                "argument {} type '{}' is not compatible with parameter type '{}'",
                                i + 1,
                                actual.name(),
                                expected.name()
                            ),
                            None,
                        ));
                    }
                }
            }
        }

        // New scope with the CAPTURED environment as parent (closures!)
        let call_env = Environment::child(closure.captured_env.clone());

        // v1.18 recursive closure support
        if let Some(self_name) = &closure.self_name {
            call_env.borrow_mut().define(
                self_name,
                Value::Closure(Rc::new(closure.clone())),
                false,
            );
        }

        // Bind parameters
        {
            let mut env = call_env.borrow_mut();
            for (i, param) in lambda.params.iter().enumerate() {
                if i < args.len() {
                    env.define(&param.name, args[i].clone(), false);
                } else if let Some(dv) = &param.default_value {
                    let default_val = self.eval_expr(dv)?;
                    env.define(&param.name, default_val, false);
                } else {
                    env.define(&param.name, Value::Null, false);
                }
            }
        }

        let flow = self.with_scope(Some(call_env), |s| s.execute_stmts(&lambda.body.body))?;
        match flow {
            Flow::Return(v) => Ok(v.unwrap_or(Value::Null)),
            Flow::Normal(v) => Ok(v.unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }

    /// Minimal agent call for M3: fresh isolated environment, bind params,
    /// execute the agent's main block (if any). LLM interaction lands in 3.6.
    /// Call an agent by name with a keyword-argument map (HLD 3.5.2).
    ///
    /// M11: made `pub` so the Python bridge (`helen-python-bridge`) can call
    /// agents from Python (`HelenAgentWrapper.__call__` parity).
    pub fn call_agent(
        &mut self,
        agent: &AgentDecl,
        args: HashMap<String, Value>,
        _span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        let prev_agent = self.current_agent.clone();
        self.current_agent = Some(Rc::new(agent.clone()));

        // v1.12 isolation: agent gets a completely isolated env (HLD 3.5.2).
        // - stdlib builtins are injected (Python `stdlib_cache` loop)
        // - module-level consts injected read-only (L1 standard)
        // - module-level `let` hidden (unless L0 open)
        // - shared lets injected writable (Python `_shared_vars` loop)
        let call_env = Rc::new(RefCell::new(Environment::new(None)));
        {
            let mut env = call_env.borrow_mut();
            // 1. Builtins.
            for (name, bf) in &self.builtins {
                env.define(name, Value::BuiltinFn(bf.clone()), true);
            }
            // 2. Module-level consts + shared lets (walk the whole chain).
            let shared: std::collections::HashSet<String> = self.shared_vars.clone();
            let mut cur = Some(self.environment.clone());
            while let Some(scope) = cur {
                let borrowed = scope.borrow();
                // Collect names to inject (const).
                let mut to_inject: Vec<(String, Value)> = Vec::new();
                for (name, value) in borrowed.store_ref() {
                    if borrowed.is_const(name) {
                        to_inject.push((name.clone(), value.clone()));
                    }
                }

                drop(borrowed);
                for (name, value) in to_inject {
                    if env.get(&name).is_none() {
                        env.define(&name, value, true);
                    }
                }
                let next = scope.borrow().parent.clone();
                drop(scope);
                cur = next;
            }
            // 3. Shared let variables (writable).
            for name in &shared {
                if let Some(v) = self.environment.borrow().get(name) {
                    if env.get(name).is_none() {
                        env.define(name, v, false);
                    }
                }
            }
        }

        // 4. Bind params. v1.12: wrap mutable reference types (list, dict)
        //    in a ReadOnlyView so the agent cannot modify the caller's
        //    data (L1 isolation; Python ReadOnlyView). Missing params with
        //    a default are evaluated in the agent's isolated env (Python
        //    `_call_agent` parity); missing without a default bind Null.
        //    Values are computed first (defaults may evaluate expressions,
        //    which need `self.environment` unborrowed), then defined.
        let mut param_values: Vec<(String, Value)> = Vec::new();
        for p in &agent.params {
            let value = if let Some(v) = args.get(&p.name).cloned() {
                v
            } else if let Some(dv) = &p.default_value {
                // Evaluate in the agent's isolated env (env swap, like
                // Python's `self.environment = call_env`).
                let old_env = self.environment.clone();
                self.environment = call_env.clone();
                let r = self.eval_expr(dv);
                self.environment = old_env;
                r?
            } else {
                Value::Null
            };
            let bound = if value.is_mutable_type() {
                Value::ReadOnly(Rc::new(RefCell::new(value)))
            } else {
                value
            };
            param_values.push((p.name.clone(), bound));
        }
        {
            let mut env = call_env.borrow_mut();
            for (name, bound) in param_values {
                env.define(&name, bound, false);
            }
        }

        let result = self.with_scope(Some(call_env), |s| {
            // HLD 3.5.3: register the agent's functions { } block into the
            // function registry (Python registers agent.functions into
            // self._functions before running main). Saved/restored so they
            // do not leak into the caller's scope.
            let prev_functions: HashMap<String, Rc<FunctionDecl>> = agent
                .functions
                .iter()
                .filter_map(|f| {
                    s.functions
                        .get(&f.name)
                        .cloned()
                        .map(|v| (f.name.clone(), v))
                })
                .collect();
            let mut restored: Vec<String> = Vec::new();
            for f in &agent.functions {
                if !s.functions.contains_key(&f.name) {
                    restored.push(f.name.clone());
                }
                s.functions.insert(f.name.clone(), Rc::new(f.clone()));
            }
            // Define variables from the functions { } block (let/const
            // declarations) in the agent's isolated env, sequential order.
            for var_node in &agent.function_vars {
                let value = if let Some(init) = &var_node.initializer {
                    match s.eval_expr(init) {
                        Ok(v) => v,
                        Err(e) => {
                            for name in &restored {
                                s.functions.remove(name);
                            }
                            for (n, v) in &prev_functions {
                                s.functions.insert(n.clone(), v.clone());
                            }
                            return Err(e);
                        }
                    }
                } else {
                    Value::Null
                };
                if s.environment.borrow().get(&var_node.name).is_none() {
                    s.environment
                        .borrow_mut()
                        .define(&var_node.name, value, var_node.mutable);
                }
            }
            let r = if let Some(logic) = &agent.logic {
                match logic.as_ref() {
                    Stmt::MainBlock(mb) => s.execute_stmts(&mb.body),
                    other => s.execute_stmt(other),
                }
            } else {
                // Agent without explicit logic but with a prompt:
                // auto-execute an LLM call (Python `_call_agent` branch).
                let Some(prompt_def) = &agent.prompt else {
                    return Ok(Flow::Normal(Some(Value::Null)));
                };
                let rendered = s.render_prompt_template(&prompt_def.content);
                if rendered.trim().is_empty() {
                    return Ok(Flow::Normal(Some(Value::Null)));
                }
                let tools = s.build_tools_list();
                let s_ptr = s as *mut Interpreter;
                let dispatch_fn = move |name: &str, args: &serde_json::Value| -> String {
                    // SAFETY: `s` is the unique owner and lives for the
                    // duration of this synchronous `act` call.
                    let interp = unsafe { &mut *s_ptr };
                    interp.dispatch_agent_tool(name, args)
                };
                let model = s.agent_setting("model");
                let temperature: f64 = s
                    .agent_setting("temperature")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1.0);
                let max_turns: usize = s
                    .agent_setting("max-turns")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
                let max_tokens: Option<u64> =
                    s.agent_setting("max-tokens").and_then(|v| v.parse().ok());
                let thinking_enabled = s
                    .agent_setting("thinking-mode")
                    .map(|v| v == "true")
                    .unwrap_or(false);
                let reasoning_effort = s.agent_setting("reasoning-effort");
                match s.llm_runtime.act(
                    &rendered,
                    &tools,
                    model.as_deref(),
                    temperature,
                    max_turns,
                    max_tokens,
                    &[],
                    None,
                    Some(&dispatch_fn),
                    thinking_enabled,
                    reasoning_effort.as_deref(),
                ) {
                    Ok(resp) => Ok(Flow::Normal(Some(match resp.text {
                        Some(t) => Value::Str(Rc::from(t.as_str())),
                        None => Value::Null,
                    }))),
                    Err(e) => Err(e),
                }
            };
            // Restore function registry (do not leak agent functions).
            for name in &restored {
                s.functions.remove(name);
            }
            for (n, v) in &prev_functions {
                s.functions.insert(n.clone(), v.clone());
            }
            r
        });

        self.current_agent = prev_agent;
        match result? {
            Flow::Return(v) => Ok(v.unwrap_or(Value::Null)),
            Flow::Normal(v) => Ok(v.unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }

    // ------------------------------------------------------------------
    // Task 6.2: Agent tool loop — build tools list + dispatch (port of
    // `LlmMixin._build_tools_list` / `_create_dispatch_fn` / `_function_to_tool_schema`)
    // ------------------------------------------------------------------

    /// `_function_to_tool_schema` — convert a Helen FunctionDecl to an
    /// OpenAI tool schema. Type annotations map to JSON Schema types.
    fn function_to_tool_schema(&self, fn_decl: &FunctionDecl) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required: Vec<String> = Vec::new();
        for param in &fn_decl.params {
            let param_type = match param
                .type_annotation
                .as_ref()
                .map(|t| t.name.to_lowercase())
                .as_deref()
            {
                Some("int") | Some("integer") => "integer",
                Some("float") | Some("number") => "number",
                Some("bool") | Some("boolean") => "boolean",
                Some("list") | Some("array") => "array",
                Some("map") | Some("dict") | Some("object") => "object",
                _ => "string",
            };
            properties.insert(param.name.clone(), serde_json::json!({"type": param_type}));
            if param.default_value.is_none() {
                required.push(param.name.clone());
            }
        }
        serde_json::json!({
            "type": "function",
            "function": {
                "name": fn_decl.name,
                "description": format!("Helen function: {}", fn_decl.name),
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": required,
                },
            },
        })
    }

    /// `_build_tools_list` — build the tool schemas for `llm act` from the
    /// agent's `tools = [...]` allowlist. Two-layer authorization:
    ///   - each name resolves to a Helen function (functions{} block) OR a
    ///     built-in tool (registry)
    ///   - load_skill + list_skill_references are always included (unless
    ///     sandbox isolation, which only gets skill tools)
    ///   - no `tools` declaration -> skill tools only
    fn build_tools_list(&self) -> Vec<serde_json::Value> {
        let skill_tools = |names: &[&str]| -> Vec<serde_json::Value> {
            helen_runtime::get_tool_schemas(
                &names.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            )
        };

        let Some(agent) = &self.current_agent else {
            // No agent context: skill tools only.
            return skill_tools(&["load_skill", "list_skill_references"]);
        };
        // v1.12: sandbox (L3) — only skill tools.
        if agent.isolation_level == "sandbox" {
            return skill_tools(&["load_skill", "list_skill_references"]);
        }

        // 1. Read declared tools allowlist.
        let mut declared_tools: Option<Vec<String>> = None;
        for decl in &agent.declarations {
            let Some(tools_expr) = &decl.tools else {
                continue;
            };
            if let Expr::Literal(lit) = tools_expr {
                // tools = ["web_search", "read_file", ...]
                // Parser stores the list as `\x1fname\x1ename\x1f` (items
                // joined with \x1e, wrapped in \x1f markers).
                if let LiteralValue::Str(s) = &lit.value {
                    let inner = s.trim_matches('\u{1f}');
                    let names: Vec<String> = if inner.is_empty() {
                        Vec::new()
                    } else {
                        inner
                            .split('\u{1e}')
                            .map(|x| x.trim().to_string())
                            .filter(|x| !x.is_empty())
                            .collect()
                    };
                    declared_tools = Some(names);
                }
            } else if let Expr::List(ll) = tools_expr {
                let mut names = Vec::new();
                for it in &ll.elements {
                    if let Expr::Literal(sl) = it {
                        if let LiteralValue::Str(s) = &sl.value {
                            names.push(s.to_string());
                        }
                    }
                }
                declared_tools = Some(names);
            } else if let Expr::Variable(v) = tools_expr {
                // tools = CONST_NAME — look up the const value.
                if let Some(Value::List(l)) = self.environment.borrow().get(&v.name) {
                    declared_tools = Some(
                        l.borrow()
                            .iter()
                            .map(|x| x.python_str())
                            .collect::<Vec<_>>(),
                    );
                } else {
                    declared_tools = Some(Vec::new());
                }
            }
            break;
        }

        // 2. No tools declared -> LLM gets nothing (skill tools added below).
        let Some(declared) = declared_tools else {
            return skill_tools(&["load_skill", "list_skill_references"]);
        };

        // 3. Resolve each allowlist name to a Helen fn or a registry tool.
        let mut tools: Vec<serde_json::Value> = Vec::new();
        let mut tool_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for name in declared {
            // 3a. Helen function in functions{} block.
            let mut fn_schema: Option<serde_json::Value> = None;
            for fn_decl in &agent.functions {
                if fn_decl.name == name {
                    fn_schema = Some(self.function_to_tool_schema(fn_decl));
                    break;
                }
            }
            if let Some(schema) = fn_schema {
                if tool_names.insert(name.clone()) {
                    tools.push(schema);
                }
                continue;
            }
            // 3b. Built-in tool in the runtime registry.
            let schemas = helen_runtime::get_tool_schemas(std::slice::from_ref(&name));
            for schema in schemas {
                let tname = schema["function"]["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if tool_names.insert(tname.clone()) {
                    tools.push(schema);
                }
            }
        }

        // 4. Always include skill tools (HLD 3.6.5 Tier 2/3 disclosure).
        for t in ["load_skill", "list_skill_references"] {
            if !tool_names.contains(t) {
                tools.extend(skill_tools(&[t]));
            }
        }
        tools
    }

    /// `_execute_agent_function` — execute an agent's Helen function with
    /// JSON args (from the LLM tool call). Creates a fresh child scope,
    /// binds params (by name), evaluates defaults, executes the body.
    fn execute_agent_function(
        &mut self,
        fn_decl: &FunctionDecl,
        args: &serde_json::Value,
        span: Option<&SourceSpan>,
    ) -> Result<Value, ExceptionValue> {
        let call_env = Environment::child(self.environment.clone());
        {
            let mut env = call_env.borrow_mut();
            for param in &fn_decl.params {
                if let Some(a) = args.get(&param.name) {
                    env.define(&param.name, crate::stdlib::json_to_value(a), false);
                } else if let Some(dv) = &param.default_value {
                    let default_val =
                        self.with_scope(Some(call_env.clone()), |s| s.eval_expr(dv))?;
                    env.define(&param.name, default_val, false);
                } else {
                    return Err(ExceptionValue::new(
                        "RuntimeError",
                        format!("Missing required argument: {}", param.name),
                        span.cloned(),
                    ));
                }
            }
        }
        let flow = self.with_scope(Some(call_env), |s| s.execute_stmts(&fn_decl.body.body))?;
        match flow {
            Flow::Return(v) => Ok(v.unwrap_or(Value::Null)),
            Flow::Normal(v) => Ok(v.unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }

    /// `_create_dispatch_fn` — dispatch a tool call: agent Helen function
    /// first, then the built-in tool registry. Returns JSON string (Python
    /// `json.dumps(result)` semantics for the LLM loop).
    pub(crate) fn dispatch_agent_tool(&mut self, name: &str, args: &serde_json::Value) -> String {
        // 1. Agent Helen function.
        let agent_fns: Vec<FunctionDecl> = self
            .current_agent
            .as_ref()
            .map(|a| a.functions.clone())
            .unwrap_or_default();
        for fn_decl in &agent_fns {
            if fn_decl.name == name {
                let result = self.execute_agent_function(fn_decl, args, None);
                return match result {
                    Ok(v) => match crate::stdlib::value_to_json(&v) {
                        Ok(j) => j.to_string(),
                        Err(_) => v.python_str(),
                    },
                    Err(e) => serde_json::json!({
                        "error": format!("Helen function '{name}' failed: {}", e.message)
                    })
                    .to_string(),
                };
            }
        }
        // 2. Built-in tool registry.
        helen_runtime::dispatch_tool(name, args)
    }

    // ------------------------------------------------------------------
    // Alias / LLM stubs
    // ------------------------------------------------------------------

    fn visit_alias(&mut self, a: &helen_core::ast::AliasStmt) {
        if let Some(v) = self.environment.borrow().get(&a.canonical) {
            self.environment
                .borrow_mut()
                .define(&a.alias_name, v, false);
        }
    }

    /// `llm if` (HLD 3.6.5/3.6.6): build the branch-name list, route through
    /// the LLM runtime, and execute the selected branch (or default) in a
    /// fresh scope. Port of `LlmMixin.visit_llm_if_stmt`.
    fn visit_llm_if(
        &mut self,
        li: &helen_core::ast::LlmIfStmtNode,
    ) -> Result<Flow, ExceptionValue> {
        // Branch names (Python: LiteralNode -> str(value); no condition ->
        // "default"). Non-literal conditions fall back to a debug form.
        let branch_names: Vec<String> = li
            .branches
            .iter()
            .map(|b| match &b.condition {
                Some(cond) => self.branch_name(cond),
                None => "default".to_string(),
            })
            .collect();

        // Evaluate the description expression to a string.
        let desc_val = self.eval_expr(&li.description)?;
        let desc_str = desc_val.python_str();

        // Route. A runtime failure surfaces a warning and uses the default
        // branch (Python prints to stderr and sets selected = None).
        let selected = self
            .llm_runtime
            .route(&desc_str, &branch_names, None)
            .unwrap_or_default();
        // HLD 3.6.6: the returned branch must be one of the known names.
        let selected = match &selected {
            Some(s) if branch_names.iter().any(|b| b == s) => selected,
            _ => None,
        };

        // Record history (Python `_add_to_history` parity).
        {
            let mut h = self.history.borrow_mut();
            h.push(serde_json::json!({"role": "user", "content": format!("[route] {desc_str}")}));
            h.push(serde_json::json!({
                "role": "assistant",
                "content": format!("[routed to: {}]", selected.as_deref().unwrap_or("default"))
            }));
        }

        // Execute the matching branch in a fresh scope.
        for (idx, b) in li.branches.iter().enumerate() {
            if b.condition.is_some() && selected.as_deref() == Some(branch_names[idx].as_str()) {
                return self.with_scope(None, |s| s.execute_stmts(&b.body));
            }
        }
        // No match -> default branch (condition is None).
        for b in &li.branches {
            if b.condition.is_none() {
                return self.with_scope(None, |s| s.execute_stmts(&b.body));
            }
        }
        Ok(Flow::Normal(None))
    }

    /// Render a prompt template by replacing `{{var}}` (and nested
    /// `{{a.b}}` attribute paths) with environment values. Port of Python
    /// `_render_prompt_template_legacy` (`_PROMPT_VAR_RE`).
    fn render_prompt_template(&self, template: &str) -> String {
        let mut out = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(open) = rest.find("{{") {
            out.push_str(&rest[..open]);
            let after = &rest[open + 2..];
            let Some(close) = after.find("}}") else {
                out.push_str(&rest[open..]);
                return out;
            };
            let var_path = after[..close].trim();
            let parts: Vec<&str> = var_path.split('.').collect();
            let mut value: Option<Value> = if let Some(first) = parts.first() {
                self.environment.borrow().get(first)
            } else {
                None
            };
            for part in &parts[1..] {
                value = match value {
                    Some(Value::Map(m)) => m.borrow().get(&Value::Str(Rc::from(*part))).cloned(),
                    _ => None,
                };
            }
            match value {
                Some(v) => out.push_str(&v.python_str()),
                None => {
                    // Keep original placeholder if not found.
                    out.push_str("{{");
                    out.push_str(var_path);
                    out.push_str("}}");
                }
            }
            rest = &after[close + 2..];
        }
        out.push_str(rest);
        out
    }

    /// Look up an agent declaration setting (Python `_get_agent_setting`).
    /// Settings come from agent `declare` lines e.g. `model "qwen-max"`.
    fn agent_setting(&self, name: &str) -> Option<String> {
        let agent = self.current_agent.clone()?;
        for decl in &agent.declarations {
            let expr = match name {
                "model" => decl.model.as_ref(),
                "tools" => decl.tools.as_ref(),
                "memory" => decl.memory.as_ref(),
                "temperature" => decl.temperature.as_ref(),
                "max-turns" => decl.max_turns.as_ref(),
                "max-tokens" => decl.max_tokens.as_ref(),
                "thinking-mode" => decl.thinking_mode.as_ref(),
                "reasoning-effort" => decl.reasoning_effort.as_ref(),
                "provider" => decl.provider.as_ref(),
                _ => None,
            };
            if let Some(helen_core::ast::Expr::Literal(lit)) = expr {
                match &lit.value {
                    LiteralValue::Str(s) => return Some(s.clone()),
                    LiteralValue::Int(i) => return Some(i.to_string()),
                    LiteralValue::Float(f) => return Some(py_str_float(*f)),
                    LiteralValue::Bool(b) => return Some(b.to_string()),
                    LiteralValue::Null => return Some("null".into()),
                }
            }
        }
        None
    }

    /// Stringify an `llm if` branch condition the way Python's
    /// `str(LiteralNode.value)` does.
    fn branch_name(&self, cond: &helen_core::ast::Expr) -> String {
        if let helen_core::ast::Expr::Literal(lit) = cond {
            match &lit.value {
                LiteralValue::Str(s) => s.clone(),
                LiteralValue::Int(i) => i.to_string(),
                LiteralValue::Float(f) => py_str_float(*f),
                LiteralValue::Bool(b) => {
                    if *b {
                        "True".to_string()
                    } else {
                        "False".to_string()
                    }
                }
                LiteralValue::Null => "None".to_string(),
            }
        } else {
            format!("{cond:?}")
        }
    }

    /// `llm act` (HLD 3.6.5): evaluate the prompt, call the LLM runtime, and
    /// return the response text (Null when the runtime yields no text).
    /// Port of `LlmMixin.visit_llm_act_expr` (sync path, no callbacks).
    fn visit_llm_act(&mut self, la: &helen_core::ast::LlmAct) -> Result<Value, ExceptionValue> {
        // Evaluate the prompt expression to a string (Python: `_stringify`
        // for non-str values).
        let prompt = if let Some(p) = &la.prompt {
            self.eval_expr(p)?.python_str()
        } else {
            String::new()
        };

        // Bare form: if prompt is empty and we're inside an agent context,
        // use the rendered agent prompt as the user message (Python parity).
        let prompt = if prompt.is_empty() && self.current_agent.is_some() {
            if let Some(agent) = &self.current_agent {
                if let Some(prompt_def) = &agent.prompt {
                    self.render_prompt_template(&prompt_def.content)
                } else {
                    prompt
                }
            } else {
                prompt
            }
        } else {
            prompt
        };
        // Extract agent settings (Python `_get_agent_setting`): model,
        // temperature, max-turns. M5: full signature passthrough.
        let model = self.agent_setting("model");
        let temperature: f64 = self
            .agent_setting("temperature")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        let max_turns: usize = self
            .agent_setting("max-turns")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let max_tokens: Option<u64> = self
            .agent_setting("max-tokens")
            .and_then(|v| v.parse().ok());
        let thinking_enabled = self
            .agent_setting("thinking-mode")
            .map(|v| v == "true")
            .unwrap_or(false);
        let reasoning_effort = self.agent_setting("reasoning-effort");

        // M6: build the tools list from the agent's `tools` allowlist
        // (Python `_build_tools_list`): agent Helen functions + registry
        // tools + always load_skill/list_skill_references.
        let tools = self.build_tools_list();

        // M6: dispatch closure (Python `_create_dispatch_fn`) — route agent
        // Helen functions first, then the built-in tool registry.
        let self_ptr = self as *mut Interpreter;
        let dispatch_fn = move |name: &str, args: &serde_json::Value| -> String {
            // SAFETY: `self` (the interpreter) is the unique owner and lives
            // for the duration of this synchronous `act` call; the closure is
            // only invoked from inside `act`, never after `self` is dropped.
            let interp = unsafe { &mut *self_ptr };
            interp.dispatch_agent_tool(name, args)
        };

        let has_streaming = la.on_chunk.is_some() || la.on_complete.is_some();

        // True streaming path: use act_stream() when callbacks are present
        if has_streaming {
            use std::cell::RefCell;
            let full_text = Rc::new(RefCell::new(String::new()));
            let interrupted = Rc::new(RefCell::new(false));

            // Prepare callbacks for streaming
            let chunk_fn_opt = if let Some(oc) = &la.on_chunk {
                Some(self.eval_expr(oc)?)
            } else {
                None
            };
            let complete_fn_opt = if let Some(oc) = &la.on_complete {
                Some(self.eval_expr(oc)?)
            } else {
                None
            };

            // Streaming event handler — uses unsafe pointer to self (same pattern as dispatch_fn)
            let self_ptr_stream = self as *mut Interpreter;
            let full_text_clone = full_text.clone();
            let interrupted_clone = interrupted.clone();
            let mut on_event = move |event: serde_json::Value| -> bool {
                if *interrupted_clone.borrow() {
                    return false;
                }

                // Check for content chunks
                if let Some(event_type) = event.get("type").and_then(|t| t.as_str()) {
                    if event_type == "content" {
                        if let Some(content) = event.get("content").and_then(|c| c.as_str()) {
                            if !content.is_empty() {
                                full_text_clone.borrow_mut().push_str(content);
                                // Call Helen on_chunk callback
                                if let Some(ref chunk_fn) = chunk_fn_opt {
                                    // SAFETY: same as dispatch_fn — self lives for the duration of act_stream
                                    let interp = unsafe { &mut *self_ptr_stream };
                                    match interp.call_value(
                                        chunk_fn.clone(),
                                        vec![Value::Str(Rc::from(content))],
                                    ) {
                                        Ok(result) => {
                                            // Python checks `chunk_result is False` (identity)
                                            if matches!(result, Value::Bool(false)) {
                                                *interrupted_clone.borrow_mut() = true;
                                                return false;
                                            }
                                        }
                                        Err(_) => {
                                            // Callback error — continue streaming
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                true
            };

            // Call act_stream
            let stream_result = self.llm_runtime.act_stream(
                &prompt,
                model.as_deref(),
                temperature,
                None,
                &tools,
                max_turns,
                &[],
                Some(&dispatch_fn),
                &mut on_event,
                thinking_enabled,
                reasoning_effort.as_deref(),
            );

            // Record history
            {
                let mut h = self.history.borrow_mut();
                h.push(serde_json::json!({"role": "user", "content": prompt}));
                let text = full_text.borrow().clone();
                if !text.is_empty() {
                    h.push(serde_json::json!({"role": "assistant", "content": text}));
                }
            }

            // Call on_complete if not interrupted
            if !*interrupted.borrow() {
                if let Some(complete_fn) = complete_fn_opt {
                    let _ = self.call_value(complete_fn, vec![]);
                }
            }

            // Check for streaming errors
            stream_result?;

            let final_text = full_text.borrow().clone();
            return Ok(Value::Str(Rc::from(final_text.as_str())));
        }

        // Non-streaming path: use act()
        let response = self.llm_runtime.act(
            &prompt,
            &tools,
            model.as_deref(),
            temperature,
            max_turns,
            max_tokens,
            &[],
            None,
            Some(&dispatch_fn),
            thinking_enabled,
            reasoning_effort.as_deref(),
        )?;

        // Record history (Python `_add_to_history` parity).
        {
            let mut h = self.history.borrow_mut();
            h.push(serde_json::json!({"role": "user", "content": prompt}));
            if let Some(t) = &response.text {
                h.push(serde_json::json!({"role": "assistant", "content": t}));
            }
        }

        match response.text {
            Some(t) => Ok(Value::Str(Rc::from(t.as_str()))),
            None => Ok(Value::Null),
        }
    }

    // ------------------------------------------------------------------
    // Numeric helpers
    // ------------------------------------------------------------------

    fn runtime_error(&self, span: Option<&SourceSpan>, message: &str) -> ExceptionValue {
        ExceptionValue::new("RuntimeError", message.to_string(), span.cloned())
    }

    fn check_number(&self, op: &Token, values: &[&Value]) -> Result<(), ExceptionValue> {
        for v in values {
            if !is_number(v) {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!(
                        "Operator '{}' requires numbers, got {}",
                        op.lexeme,
                        v.type_name()
                    ),
                    Some(op.span()),
                ));
            }
        }
        Ok(())
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Interpreter::new()
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------
