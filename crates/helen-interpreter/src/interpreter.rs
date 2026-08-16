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
use num_traits::{Signed, ToPrimitive, Zero};

use helen_core::ast::{
    Access, AgentDecl, Binary, Call, CallArg, Expr, ForStmt, FunctionDecl, IfStmt, ImportStmt,
    Index, Lambda, Pipe, Program, Spawn, Stmt, ThrowStmt, TypeRef, Unary, VarDecl,
};
use helen_core::ast_printer::py_str_float;
use helen_core::source::SourceSpan;
use helen_core::tokens::{LiteralValue, Token, TokenType};
use helen_semantic::types::{type_compatible, Type};

use crate::closure::{compute_free_variables, Closure};
use crate::environment::Environment;
use crate::exceptions::{error_matches, resolve_exception, ExceptionValue, Flow};
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
        };
        interp.register_core_builtins();
        interp
    }

    /// Inject a custom LLM runtime (tests use `MockLlmRuntime`).
    pub fn set_llm_runtime(&mut self, runtime: std::sync::Arc<dyn LlmRuntime>) {
        self.llm_runtime = runtime;
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
        let fields = store.fields.lock().unwrap().clone();

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
            let mut fl = store.fields.lock().unwrap();
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

    fn call_closure(
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
                .filter_map(|f| s.functions.get(&f.name).cloned().map(|v| (f.name.clone(), v)))
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
                // Agent without explicit logic: stub LLM response for M3.
                let prompt = agent
                    .prompt
                    .as_ref()
                    .map(|p| p.content.clone())
                    .unwrap_or_default();
                Ok(Flow::Normal(Some(Value::Str(Rc::from(prompt.as_str())))))
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
    fn dispatch_agent_tool(&mut self, name: &str, args: &serde_json::Value) -> String {
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

        let has_streaming = la.on_chunk.is_some() || la.on_complete.is_some();
        if !has_streaming {
            return match response.text {
                Some(t) => Ok(Value::Str(Rc::from(t.as_str()))),
                None => Ok(Value::Null),
            };
        }

        // Streaming path (Python `_visit_llm_act_streaming` with the default
        // `act_stream`, which wraps `act()` and yields the full text as a
        // single content event):
        //   on_chunk(text)  — if it returns `false`, stop (interrupted)
        //   on_complete()   — called with NO args, only if not interrupted
        //   return joined text (for a single event: the text itself)
        let mut full_text = String::new();
        let mut interrupted = false;
        if let Some(text) = &response.text {
            if !text.is_empty() {
                full_text.push_str(text);
                if let Some(oc) = &la.on_chunk {
                    let chunk_fn = self.eval_expr(oc)?;
                    let chunk_result =
                        self.call_value(chunk_fn, vec![Value::Str(Rc::from(text.as_str()))])?;
                    // Python checks `chunk_result is False` (identity), so
                    // only a literal `false` interrupts — not 0/""/None.
                    if matches!(chunk_result, Value::Bool(false)) {
                        interrupted = true;
                    }
                }
            }
        }
        if !interrupted {
            if let Some(oc) = &la.on_complete {
                let done_fn = self.eval_expr(oc)?;
                self.call_value(done_fn, vec![])?;
            }
        }
        Ok(Value::Str(Rc::from(full_text.as_str())))
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

/// `_truthy` (static).
fn is_number(v: &Value) -> bool {
    matches!(v, Value::Int(_) | Value::Float(_) | Value::Bool(_))
}

/// Convert a numeric value to f64 (bool counts as int).
fn to_f64(v: &Value) -> f64 {
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

fn num_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => i.to_f64(),
        Value::Float(f) => Some(*f),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Python `%` on integers: result takes the sign of the divisor.
fn py_mod(a: &BigInt, b: &BigInt) -> BigInt {
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
fn format_keys(keys: &[Value]) -> String {
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
fn check_type(value: &Value, type_name: &str) -> bool {
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
fn type_from_typenode(tn: &TypeRef) -> Type {
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
fn is_mutable_type(v: &Value) -> bool {
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

fn builtin_print(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let parts: Vec<String> = args.iter().map(|a| a.to_display(true)).collect();
    let result = parts.join(" ");
    interp.stdout.lock().unwrap().push_str(&result);
    interp.stdout.lock().unwrap().push('\n');
    Ok(Value::Str(Rc::from(result.as_str())))
}

fn builtin_len(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_str(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Str(Rc::from(v.python_str().as_str())))
}

fn builtin_int(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_float(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_bool(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Bool(v.truthy()))
}

fn builtin_type(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Str(Rc::from(v.type_name().as_str())))
}

fn builtin_isinstance(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_range(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_abs(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_min(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_max(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_list(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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

fn builtin_dict(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
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
    use helen_core::lexer::Scanner;

    /// Serialize MCP-touching tests: the runtime MCP registry is a process
    /// global (Python `tools._mcp_registry`), so parallel tests must not
    /// observe each other's MCP state.
    fn with_mcp_clean<T>(f: impl FnOnce() -> T) -> T {
        use std::sync::{Mutex, OnceLock};
        static MCP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _g = MCP_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
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
        let out = interp.stdout.lock().unwrap().clone();
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
        let out = interp.stdout.lock().unwrap().clone();
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
        std::fs::create_dir_all(&dir).unwrap();
        for (name, content) in files {
            std::fs::write(dir.join(name), content).unwrap();
        }
        let main_path = dir.join("main.helen");
        std::fs::write(&main_path, main_src).unwrap();

        let mut scanner = Scanner::new(main_src, main_path.to_str().unwrap());
        let tokens = scanner.scan_all();
        let mut parser = helen_parser::Parser::new(tokens);
        let program = parser.parse();
        assert!(
            parser.errors().is_empty(),
            "parse errs: {:?}",
            parser.errors()
        );
        let mut interp = Interpreter::new();
        interp.set_source_file(main_path.to_str().unwrap());
        let r = interp.interpret(&program);
        let out = interp.stdout.lock().unwrap().clone();
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
            let v: serde_json::Value = serde_json::from_str(&calc).unwrap();
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
            std::fs::create_dir_all(&dir).unwrap();
            let config_path = dir.join(".mcp.json");
            std::fs::write(&config_path, serde_json::to_string(&config).unwrap()).unwrap();

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
            let v: serde_json::Value = serde_json::from_str(&echo).unwrap();
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

    #[test]

    // Phase 3: Additional interpreter tests for comprehensive coverage

    #[test]
    fn phase3_value_type_conversions() {
        let src = r#"import std.core.*
main {
    let i = 42
    let f = 3.14
    let s = "hello"
    let b = true
    let n = null
    print(type(i))
    print(type(f))
    print(type(s))
    print(type(b))
    print(type(n))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "int");
        assert_eq!(lines[1], "float");
        assert_eq!(lines[2], "str");
        assert_eq!(lines[3], "bool");
        assert_eq!(lines[4], "null");
    }

    #[test]
    fn phase3_string_concatenation() {
        let src = r#"import std.core.*
main {
    let a = "hello"
    let b = " "
    let c = "world"
    let result = a + b + c
    print(result)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "hello world");
    }

    #[test]
    fn phase3_numeric_operations() {
        let src = r#"import std.core.*
main {
    let a = 10
    let b = 3
    print(a + b)
    print(a - b)
    print(a * b)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "13");
        assert_eq!(lines[1], "7");
        assert_eq!(lines[2], "30");
    }

    #[test]
    fn phase3_boolean_operations() {
        let src = r#"import std.core.*
main {
    let t = true
    let f = false
    print(t && f)
    print(t || f)
    print(!t)
    print(!f)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "false");
        assert_eq!(lines[1], "true");
        assert_eq!(lines[2], "false");
        assert_eq!(lines[3], "true");
    }

    #[test]
    fn phase3_comparison_operations() {
        let src = r#"import std.core.*
main {
    let a = 10
    let b = 20
    print(a < b)
    print(a > b)
    print(a == b)
    print(a != b)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "false");
        assert_eq!(lines[2], "false");
        assert_eq!(lines[3], "true");
    }

    #[test]
    fn phase3_list_operations() {
        let src = r#"import std.core.*
main {
    let lst = [1, 2, 3, 4, 5]
    print(len(lst))
    print(lst[0])
    print(lst[4])
    lst.append(6)
    print(len(lst))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "5");
        assert_eq!(lines[1], "1");
        assert_eq!(lines[2], "5");
        assert_eq!(lines[3], "6");
    }

    #[test]
    fn phase3_dict_operations() {
        let src = r#"import std.core.*
main {
    let d = {"name": "Alice", "age": 30}
    print(d["name"])
    print(d["age"])
    d["city"] = "NYC"
    print(len(d))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "Alice");
        assert_eq!(lines[1], "30");
        assert_eq!(lines[2], "3");
    }

    #[test]
    fn phase3_nested_structures() {
        let src = r#"import std.core.*
main {
    let data = {
        "users": [
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25}
        ]
    }
    print(len(data["users"]))
    print(data["users"][0]["name"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "Alice");
    }

    #[test]
    fn phase3_function_calls() {
        let src = r#"import std.core.*
fn add(a: int, b: int): int {
    return a + b
}
main {
    let result = add(10, 20)
    print(result)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "30");
    }

    #[test]
    fn phase3_recursive_function() {
        let src = r#"import std.core.*
fn factorial(n: int): int {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}
main {
    print(factorial(5))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "120");
    }

    #[test]
    fn phase3_while_loop() {
        let src = r#"import std.core.*
main {
    let i = 0
    let sum = 0
    while i < 5 {
        sum = sum + i
        i = i + 1
    }
    print(sum)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "10");
    }

    #[test]
    fn phase3_for_loop() {
        let src = r#"import std.core.*
main {
    let sum = 0
    for i in 0..5 {
        sum = sum + i
    }
    print(sum)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "10");
    }

    #[test]
    fn phase3_break_continue() {
        let src = r#"import std.core.*
main {
    let sum = 0
    for i in 0..10 {
        if i == 5 {
            break
        }
        if i % 2 == 0 {
            continue
        }
        sum = sum + i
    }
    print(sum)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "4"); // 1 + 3
    }

    #[test]
    fn phase3_match_expression() {
        let src = r#"import std.core.*
main {
    let x = 2
    match x {
        case 1 => print("one")
        case 2 => print("two")
        case 3 => print("three")
        default => print("other")
    }
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "two");
    }

    #[test]
    fn phase3_try_catch() {
        let src = r#"import std.core.*
main {
    try {
        let x = 10 / 0
    } catch (e) {
        print("caught error")
    }
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "caught error");
    }

    #[test]
    fn phase3_string_methods() {
        let src = r#"import std.core.*
main {
    let s = "Hello World"
    print(s.upper())
    print(s.lower())
    print(s.contains("World"))
    print(s.starts_with("Hello"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "HELLO WORLD");
        assert_eq!(lines[1], "hello world");
        assert_eq!(lines[2], "true");
        assert_eq!(lines[3], "true");
    }

    #[test]
    fn phase3_list_methods() {
        let src = r#"import std.core.*
main {
    let lst = [3, 1, 4, 1, 5]
    lst.sort()
    print(lst)
    let copy = lst.copy()
    print(len(copy))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[1], "5");
    }

    #[test]
    fn phase3_math_operations() {
        let src = r#"import std.core.*
import std.math.*
main {
    print(sqrt(16))
    print(pow(2, 3))
    print(abs(-5))
    print(max(10, 20))
    print(min(10, 20))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "4");
        assert_eq!(lines[1], "8");
        assert_eq!(lines[2], "5");
        assert_eq!(lines[3], "20");
        assert_eq!(lines[4], "10");
    }

    #[test]
    fn phase3_complex_data_structure() {
        let src = r#"import std.core.*
main {
    let matrix = [
        [1, 2, 3],
        [4, 5, 6],
        [7, 8, 9]
    ]
    print(matrix[0][0])
    print(matrix[1][1])
    print(matrix[2][2])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "1");
        assert_eq!(lines[1], "5");
        assert_eq!(lines[2], "9");
    }

    #[test]
    fn phase3_string_interpolation() {
        let src = r#"import std.core.*
main {
    let name = "Alice"
    let age = 30
    print("Name: {{name}}, Age: {{age}}")
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "Name: Alice, Age: 30");
    }

    #[test]
    fn phase3_pipe_operator() {
        let src = r#"import std.core.*
fn double(x: int): int {
    return x * 2
}
fn add_one(x: int): int {
    return x + 1
}
main {
    let result = 5 |> double |> add_one
    print(result)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "11"); // (5 * 2) + 1
    }

    #[test]
    fn phase3_closure_capture() {
        let src = r#"import std.core.*
fn make_adder(x: int): fn {
    return fn(y: int): int {
        return x + y
    }
}
main {
    let add5 = make_adder(5)
    print(add5(10))
    print(add5(20))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "15");
        assert_eq!(lines[1], "25");
    }

    #[test]
    fn phase3_error_handling() {
        let src = r#"import std.core.*
fn safe_divide(a: int, b: int): int {
    if b == 0 {
        throw "Division by zero"
    }
    return a / b
}
main {
    try {
        let result = safe_divide(10, 0)
    } catch (e) {
        print("Error: {{e}}")
    }
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert!(out.contains("Error"));
    }

    #[test]
    fn phase3_type_checking() {
        let src = r#"import std.core.*
main {
    let i = 42
    let f = 3.14
    let s = "hello"
    print(i is int)
    print(f is float)
    print(s is str)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
        assert_eq!(lines[2], "true");
    }

    #[test]
    fn phase3_const_values() {
        let src = r#"import std.core.*
main {
    const PI = 3.14159
    const MAX_SIZE = 100
    print(PI)
    print(MAX_SIZE)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3.14159");
        assert_eq!(lines[1], "100");
    }

    #[test]
    fn phase3_array_slicing() {
        let src = r#"import std.core.*
main {
    let arr = [0, 1, 2, 3, 4, 5]
    let slice = arr[1..4]
    print(len(slice))
    print(slice[0])
    print(slice[2])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "1");
        assert_eq!(lines[2], "3");
    }

    #[test]
    fn phase3_nested_loops() {
        let src = r#"import std.core.*
main {
    let count = 0
    for i in 0..3 {
        for j in 0..3 {
            count = count + 1
        }
    }
    print(count)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "9");
    }

    #[test]
    fn phase3_complex_conditionals() {
        let src = r#"import std.core.*
fn classify(n: int): str {
    if n < 0 {
        return "negative"
    } else if n == 0 {
        return "zero"
    } else if n < 10 {
        return "small"
    } else {
        return "large"
    }
}
main {
    print(classify(-5))
    print(classify(0))
    print(classify(5))
    print(classify(15))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "negative");
        assert_eq!(lines[1], "zero");
        assert_eq!(lines[2], "small");
        assert_eq!(lines[3], "large");
    }

    #[test]
    fn phase3_shared_store() {
        let src = r#"import std.core.*
main {
    shared store counter = 0
    counter = counter + 1
    counter = counter + 1
    print(counter)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_alias_type() {
        let src = r#"import std.core.*
main {
    alias UserID = int
    let id: UserID = 12345
    print(id)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "12345");
    }

    #[test]
    fn phase3_transcript_logging() {
        let src = r#"import std.core.*
main {
    transcript log = []
    log.append("step 1")
    log.append("step 2")
    print(len(log))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_higher_order_function() {
        let src = r#"import std.core.*
fn apply(f: fn(int): int, x: int): int {
    return f(x)
}

fn square(x: int): int {
    return x * x
}

main {
    let result = apply(square, 5)
    print(result)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "25");
    }

    #[test]
    fn phase3_currying() {
        let src = r#"import std.core.*
fn add(x: int): fn(int): int {
    return fn(y: int): int {
        return x + y
    }
}

main {
    let add5 = add(5)
    print(add5(10))
    print(add5(20))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "15");
        assert_eq!(lines[1], "25");
    }

    #[test]
    fn phase3_factory_pattern() {
        let src = r#"import std.core.*
fn create_shape(type: str): dict {
    match type {
        case "circle" => return {"type": "circle", "radius": 5}
        case "square" => return {"type": "square", "side": 10}
        default => return {"type": "unknown"}
    }
}

main {
    let circle = create_shape("circle")
    let square = create_shape("square")
    print(circle["type"])
    print(square["type"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "circle");
        assert_eq!(lines[1], "square");
    }

    #[test]
    fn phase3_observer_pattern() {
        let src = r#"import std.core.*
struct EventEmitter {
    listeners: list
}

impl EventEmitter {
    fn on(event: str, callback: fn()) {
        listeners.append({"event": event, "callback": callback})
    }
    
    fn emit(event: str) {
        for listener in listeners {
            if listener["event"] == event {
                listener["callback"]()
            }
        }
    }
}

main {
    let emitter = EventEmitter { listeners: [] }
    let called = false
    emitter.on("test", fn() { called = true })
    emitter.emit("test")
    print(called)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_composite_pattern() {
        let src = r#"import std.core.*
struct FileSystemNode {
    name: str,
    is_file: bool,
    children: list
}

impl FileSystemNode {
    fn size(): int {
        if is_file {
            return 100
        }
        let total = 0
        for child in children {
            total = total + child.size()
        }
        return total
    }
}

main {
    let file1 = FileSystemNode { name: "file1.txt", is_file: true, children: [] }
    let file2 = FileSystemNode { name: "file2.txt", is_file: true, children: [] }
    let dir = FileSystemNode { name: "docs", is_file: false, children: [file1, file2] }
    print(dir.size())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "200");
    }

    #[test]
    fn phase3_strategy_pattern() {
        let src = r#"import std.core.*
fn sort_asc(lst: list): list {
    let sorted = lst.copy()
    sorted.sort()
    return sorted
}

fn sort_desc(lst: list): list {
    let sorted = lst.copy()
    sorted.sort()
    sorted.reverse()
    return sorted
}

fn sort_with(lst: list, strategy: fn(list): list): list {
    return strategy(lst)
}

main {
    let data = [3, 1, 4, 1, 5]
    let asc = sort_with(data, sort_asc)
    let desc = sort_with(data, sort_desc)
    print(asc)
    print(desc)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert!(lines[0].contains("1"));
        assert!(lines[1].contains("5"));
    }

    #[test]
    fn phase3_state_pattern() {
        let src = r#"import std.core.*
struct TrafficLight {
    state: str
}

impl TrafficLight {
    fn change() {
        match state {
            case "red" => state = "green"
            case "green" => state = "yellow"
            case "yellow" => state = "red"
        }
    }
    
    fn get_state(): str {
        return state
    }
}

main {
    let light = TrafficLight { state: "red" }
    print(light.get_state())
    light.change()
    print(light.get_state())
    light.change()
    print(light.get_state())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "red");
        assert_eq!(lines[1], "green");
        assert_eq!(lines[2], "yellow");
    }

    #[test]
    fn phase3_command_pattern() {
        let src = r#"import std.core.*
struct Command {
    execute: fn()
}

fn make_command(action: fn()): Command {
    return Command { execute: action }
}

main {
    let executed = false
    let cmd = make_command(fn() { executed = true })
    cmd.execute()
    print(executed)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_iterator_pattern() {
        let src = r#"import std.core.*
struct Counter {
    current: int,
    max: int
}

impl Counter {
    fn next(): int {
        if current < max {
            current = current + 1
            return current
        }
        return -1
    }
}

main {
    let counter = Counter { current: 0, max: 3 }
    print(counter.next())
    print(counter.next())
    print(counter.next())
    print(counter.next())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "1");
        assert_eq!(lines[1], "2");
        assert_eq!(lines[2], "3");
        assert_eq!(lines[3], "-1");
    }

    #[test]
    fn phase3_builder_pattern() {
        let src = r#"import std.core.*
struct HttpRequest {
    method: str,
    url: str,
    headers: dict,
    body: str
}

struct HttpRequestBuilder {
    request: dict
}

impl HttpRequestBuilder {
    fn set_method(method: str): HttpRequestBuilder {
        request["method"] = method
        return this
    }
    
    fn set_url(url: str): HttpRequestBuilder {
        request["url"] = url
        return this
    }
    
    fn set_body(body: str): HttpRequestBuilder {
        request["body"] = body
        return this
    }
    
    fn build(): HttpRequest {
        return HttpRequest {
            method: request["method"],
            url: request["url"],
            headers: request["headers"],
            body: request["body"]
        }
    }
}

main {
    let builder = HttpRequestBuilder { request: {"headers": {}} }
    let req = builder.set_method("POST").set_url("/api").set_body("data").build()
    print(req.method)
    print(req.url)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "POST");
        assert_eq!(lines[1], "/api");
    }

    #[test]
    fn phase3_prototype_pattern() {
        let src = r#"import std.core.*
struct Prototype {
    value: int
}

impl Prototype {
    fn clone(): Prototype {
        return Prototype { value: value }
    }
}

main {
    let original = Prototype { value: 42 }
    let clone = original.clone()
    print(original.value)
    print(clone.value)
    print(original == clone)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "42");
        assert_eq!(lines[1], "42");
        assert_eq!(lines[2], "true");
    }

    #[test]
    fn phase3_repository_pattern() {
        let src = r#"import std.core.*
struct UserRepository {
    users: dict
}

impl UserRepository {
    fn add(id: int, name: str) {
        users[id] = name
    }
    
    fn get(id: int): str {
        return users.get(id, "")
    }
    
    fn find_all(): list {
        return users.values()
    }
}

main {
    let repo = UserRepository { users: {} }
    repo.add(1, "Alice")
    repo.add(2, "Bob")
    print(repo.get(1))
    print(len(repo.find_all()))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "Alice");
        assert_eq!(lines[1], "2");
    }

    #[test]
    fn phase3_unit_of_work() {
        let src = r#"import std.core.*
struct UnitOfWork {
    changes: list
}

impl UnitOfWork {
    fn register_change(change: str) {
        changes.append(change)
    }
    
    fn commit() {
        for change in changes {
            print("committing: {{change}}")
        }
        changes = []
    }
}

main {
    let uow = UnitOfWork { changes: [] }
    uow.register_change("insert user")
    uow.register_change("update profile")
    uow.commit()
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert!(out.contains("committing: insert user"));
        assert!(out.contains("committing: update profile"));
    }

    #[test]
    fn phase3_lazy_loading() {
        let src = r#"import std.core.*
struct LazyLoader {
    loaded: bool,
    data: dict
}

impl LazyLoader {
    fn get_data(): dict {
        if !loaded {
            data = {"expensive": "computation"}
            loaded = true
            print("loading data")
        }
        return data
    }
}

main {
    let loader = LazyLoader { loaded: false, data: {} }
    let data1 = loader.get_data()
    let data2 = loader.get_data()
    print(loader.loaded)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "loading data");
        assert_eq!(lines[1], "true");
    }

    #[test]
    fn phase3_connection_pooling() {
        let src = r#"import std.core.*
struct ConnectionPool {
    connections: list,
    max_size: int
}

impl ConnectionPool {
    fn acquire(): dict {
        if len(connections) > 0 {
            return connections.pop()
        }
        return {"id": len(connections) + 1}
    }
    
    fn release(conn: dict) {
        if len(connections) < max_size {
            connections.append(conn)
        }
    }
}

main {
    let pool = ConnectionPool { connections: [], max_size: 5 }
    let conn1 = pool.acquire()
    let conn2 = pool.acquire()
    pool.release(conn1)
    pool.release(conn2)
    print(len(pool.connections))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_message_queue() {
        let src = r#"import std.core.*
struct MessageQueue {
    messages: list
}

impl MessageQueue {
    fn send(message: str) {
        messages.append(message)
    }
    
    fn receive(): str {
        if len(messages) > 0 {
            return messages.pop(0)
        }
        return ""
    }
    
    fn size(): int {
        return len(messages)
    }
}

main {
    let queue = MessageQueue { messages: [] }
    queue.send("msg1")
    queue.send("msg2")
    queue.send("msg3")
    print(queue.size())
    print(queue.receive())
    print(queue.size())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "msg1");
        assert_eq!(lines[2], "2");
    }

    #[test]
    fn phase3_event_bus() {
        let src = r#"import std.core.*
struct EventBus {
    handlers: dict
}

impl EventBus {
    fn subscribe(event: str, handler: fn(str)) {
        if !handlers.has_key(event) {
            handlers[event] = []
        }
        handlers[event].append(handler)
    }
    
    fn publish(event: str, data: str) {
        if handlers.has_key(event) {
            for handler in handlers[event] {
                handler(data)
            }
        }
    }
}

main {
    let bus = EventBus { handlers: {} }
    let received = ""
    bus.subscribe("test", fn(data: str) { received = data })
    bus.publish("test", "hello")
    print(received)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "hello");
    }

    #[test]
    fn phase3_circuit_breaker() {
        let src = r#"import std.core.*
struct CircuitBreaker {
    state: str,
    failure_count: int,
    threshold: int
}

impl CircuitBreaker {
    fn call(operation: fn(): bool): bool {
        if state == "open" {
            return false
        }
        let success = operation()
        if !success {
            failure_count = failure_count + 1
            if failure_count >= threshold {
                state = "open"
            }
        } else {
            failure_count = 0
        }
        return success
    }
}

main {
    let cb = CircuitBreaker { state: "closed", failure_count: 0, threshold: 3 }
    let result1 = cb.call(fn(): bool { return true })
    let result2 = cb.call(fn(): bool { return false })
    let result3 = cb.call(fn(): bool { return false })
    let result4 = cb.call(fn(): bool { return false })
    let result5 = cb.call(fn(): bool { return true })
    print(result1)
    print(result5)
    print(cb.state)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "false");
        assert_eq!(lines[2], "open");
    }

    #[test]
    fn phase3_retry_mechanism() {
        let src = r#"import std.core.*
fn retry(operation: fn(): bool, max_attempts: int): bool {
    let attempt = 0
    while attempt < max_attempts {
        if operation() {
            return true
        }
        attempt = attempt + 1
    }
    return false
}

main {
    let counter = 0
    let result = retry(fn(): bool {
        counter = counter + 1
        return counter >= 3
    }, 5)
    print(result)
    print(counter)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "3");
    }

    #[test]
    fn phase3_rate_limiter() {
        let src = r#"import std.core.*
struct RateLimiter {
    max_requests: int,
    window_size: int,
    requests: list
}

impl RateLimiter {
    fn allow(): bool {
        let now = 0  // Simplified
        requests = requests.filter(fn(t: int): bool { return now - t < window_size })
        if len(requests) < max_requests {
            requests.append(now)
            return true
        }
        return false
    }
}

main {
    let limiter = RateLimiter { max_requests: 2, window_size: 10, requests: [] }
    let r1 = limiter.allow()
    let r2 = limiter.allow()
    let r3 = limiter.allow()
    print(r1)
    print(r2)
    print(r3)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
        assert_eq!(lines[2], "false");
    }

    #[test]
    fn phase3_bulkhead_pattern() {
        let src = r#"import std.core.*
struct Bulkhead {
    max_concurrent: int,
    current: int
}

impl Bulkhead {
    fn acquire(): bool {
        if current < max_concurrent {
            current = current + 1
            return true
        }
        return false
    }
    
    fn release() {
        if current > 0 {
            current = current - 1
        }
    }
}

main {
    let bulkhead = Bulkhead { max_concurrent: 2, current: 0 }
    let r1 = bulkhead.acquire()
    let r2 = bulkhead.acquire()
    let r3 = bulkhead.acquire()
    print(r1)
    print(r2)
    print(r3)
    bulkhead.release()
    let r4 = bulkhead.acquire()
    print(r4)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
        assert_eq!(lines[2], "false");
        assert_eq!(lines[3], "true");
    }

    #[test]
    fn phase3_cache_pattern() {
        let src = r#"import std.core.*
struct Cache {
    data: dict,
    max_size: int
}

impl Cache {
    fn get(key: str): str {
        return data.get(key, "")
    }
    
    fn set(key: str, value: str) {
        if len(data) >= max_size {
            let first_key = data.keys()[0]
            data.remove(first_key)
        }
        data[key] = value
    }
    
    fn has(key: str): bool {
        return data.has_key(key)
    }
}

main {
    let cache = Cache { data: {}, max_size: 2 }
    cache.set("a", "1")
    cache.set("b", "2")
    cache.set("c", "3")
    print(cache.has("a"))
    print(cache.has("b"))
    print(cache.has("c"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "false");
        assert_eq!(lines[1], "true");
        assert_eq!(lines[2], "true");
    }

    #[test]
    fn phase3_idempotency() {
        let src = r#"import std.core.*
shared store processed = {}

fn idempotent_operation(id: str, operation: fn()): bool {
    if processed.has_key(id) {
        return false
    }
    operation()
    processed[id] = true
    return true
}

main {
    let counter = 0
    let r1 = idempotent_operation("op1", fn() { counter = counter + 1 })
    let r2 = idempotent_operation("op1", fn() { counter = counter + 1 })
    print(r1)
    print(r2)
    print(counter)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "false");
        assert_eq!(lines[2], "1");
    }

    #[test]
    fn phase3_aggregate_root() {
        let src = r#"import std.core.*
struct Order {
    id: int,
    items: list,
    status: str
}

impl Order {
    fn add_item(item: dict) {
        items.append(item)
    }
    
    fn total(): float {
        let sum = 0.0
        for item in items {
            sum = sum + item["price"]
        }
        return sum
    }
    
    fn confirm() {
        status = "confirmed"
    }
}

main {
    let order = Order { id: 1, items: [], status: "pending" }
    order.add_item({"name": "Widget", "price": 10.0})
    order.add_item({"name": "Gadget", "price": 20.0})
    order.confirm()
    print(order.status)
    print(order.total())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "confirmed");
        assert_eq!(lines[1], "30");
    }

    #[test]
    fn phase3_value_object() {
        let src = r#"import std.core.*
struct Money {
    amount: float,
    currency: str
}

impl Money {
    fn add(other: Money): Money {
        if currency != other.currency {
            throw "Currency mismatch"
        }
        return Money { amount: amount + other.amount, currency: currency }
    }
    
    fn equals(other: Money): bool {
        return amount == other.amount && currency == other.currency
    }
}

main {
    let m1 = Money { amount: 10.0, currency: "USD" }
    let m2 = Money { amount: 20.0, currency: "USD" }
    let m3 = m1.add(m2)
    print(m3.amount)
    print(m1.equals(Money { amount: 10.0, currency: "USD" }))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "30");
        assert_eq!(lines[1], "true");
    }

    #[test]
    fn phase3_domain_event() {
        let src = r#"import std.core.*
struct DomainEvent {
    type: str,
    data: dict,
    timestamp: int
}

struct DomainEventHandler {
    fn handle(event: DomainEvent) {
        print("handling: {{event.type}}")
    }
}

main {
    let event = DomainEvent {
        type: "user_created",
        data: {"name": "Alice"},
        timestamp: 1234567890
    }
    let handler = DomainEventHandler {}
    handler.handle(event)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "handling: user_created");
    }

    #[test]
    fn phase3_policy_pattern() {
        let src = r#"import std.core.*
struct RetryPolicy {
    max_retries: int,
    delay_ms: int
}

impl RetryPolicy {
    fn execute(operation: fn(): bool): bool {
        let attempt = 0
        while attempt < max_retries {
            if operation() {
                return true
            }
            attempt = attempt + 1
        }
        return false
    }
}

main {
    let policy = RetryPolicy { max_retries: 3, delay_ms: 100 }
    let counter = 0
    let result = policy.execute(fn(): bool {
        counter = counter + 1
        return counter >= 2
    })
    print(result)
    print(counter)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "2");
    }

    #[test]
    fn phase3_resilience_pattern() {
        let src = r#"import std.core.*
struct ResilientOperation {
    fallback: fn(): str
}

impl ResilientOperation {
    fn execute(operation: fn(): str): str {
        try {
            return operation()
        } catch (e) {
            return fallback()
        }
    }
}

main {
    let op = ResilientOperation {
        fallback: fn(): str { return "fallback value" }
    }
    let result = op.execute(fn(): str {
        throw "error"
        return "normal value"
    })
    print(result)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "fallback value");
    }

    #[test]
    fn phase3_graceful_degradation() {
        let src = r#"import std.core.*
fn fetch_data(use_cache: bool): dict {
    if use_cache {
        return {"source": "cache", "data": "cached"}
    }
    try {
        return {"source": "api", "data": "live"}
    } catch (e) {
        return {"source": "cache", "data": "fallback"}
    }
}

main {
    let data1 = fetch_data(true)
    let data2 = fetch_data(false)
    print(data1["source"])
    print(data2["source"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "cache");
        assert_eq!(lines[1], "api");
    }

    #[test]
    fn phase3_health_check() {
        let src = r#"import std.core.*
struct HealthChecker {
    checks: list
}

impl HealthChecker {
    fn add_check(name: str, check: fn(): bool) {
        checks.append({"name": name, "check": check})
    }
    
    fn check_all(): dict {
        let results = {}
        for check in checks {
            results[check["name"]] = check["check"]()
        }
        return results
    }
}

main {
    let checker = HealthChecker { checks: [] }
    checker.add_check("database", fn(): bool { return true })
    checker.add_check("cache", fn(): bool { return true })
    let results = checker.check_all()
    print(results["database"])
    print(results["cache"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
    }

    #[test]
    fn phase3_metrics_collection() {
        let src = r#"import std.core.*
struct MetricsCollector {
            counters: dict,
    gauges: dict
}

impl MetricsCollector {
    fn increment(counter: str) {
        counters[counter] = counters.get(counter, 0) + 1
    }
    
    fn set_gauge(gauge: str, value: float) {
        gauges[gauge] = value
    }
    
    fn get_counter(counter: str): int {
        return counters.get(counter, 0)
    }
    
    fn get_gauge(gauge: str): float {
        return gauges.get(gauge, 0.0)
    }
}

main {
    let metrics = MetricsCollector { counters: {}, gauges: {} }
    metrics.increment("requests")
    metrics.increment("requests")
    metrics.set_gauge("memory", 1024.5)
    print(metrics.get_counter("requests"))
    print(metrics.get_gauge("memory"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "1024.5");
    }

    #[test]
    fn phase3_logging_framework() {
        let src = r#"import std.core.*
struct Logger {
    level: str
}

impl Logger {
    fn log(level: str, message: str) {
        if level == "error" || level == "warn" || level == "info" || level == "debug" {
            print("[{{level}}] {{message}}")
        }
    }
    
    fn error(message: str) {
        log("error", message)
    }
    
    fn warn(message: str) {
        log("warn", message)
    }
    
    fn info(message: str) {
        log("info", message)
    }
    
    fn debug(message: str) {
        log("debug", message)
    }
}

main {
    let logger = Logger { level: "info" }
    logger.info("application started")
    logger.error("something failed")
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert!(out.contains("[info] application started"));
        assert!(out.contains("[error] something failed"));
    }

    #[test]
    fn phase3_configuration_management() {
        let src = r#"import std.core.*
struct Config {
    settings: dict
}

impl Config {
    fn get(key: str): str {
        return settings.get(key, "")
    }
    
    fn set(key: str, value: str) {
        settings[key] = value
    }
    
    fn has(key: str): bool {
        return settings.has_key(key)
    }
}

main {
    let config = Config { settings: {} }
    config.set("database_url", "localhost:5432")
    config.set("api_key", "secret123")
    print(config.get("database_url"))
    print(config.has("api_key"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "localhost:5432");
        assert_eq!(lines[1], "true");
    }

    #[test]
    fn phase3_dependency_injection() {
        let src = r#"import std.core.*
struct Container {
    services: dict
}

impl Container {
    fn register(name: str, factory: fn(): dict) {
        services[name] = factory
    }
    
    fn resolve(name: str): dict {
        if services.has_key(name) {
            return services[name]()
        }
        return {}
    }
}

main {
    let container = Container { services: {} }
    container.register("logger", fn(): dict { return {"type": "logger"} })
    container.register("cache", fn(): dict { return {"type": "cache"} })
    
    let logger = container.resolve("logger")
    let cache = container.resolve("cache")
    
    print(logger["type"])
    print(cache["type"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "logger");
        assert_eq!(lines[1], "cache");
    }

    #[test]
    fn phase3_plugin_system() {
        let src = r#"import std.core.*
struct PluginManager {
    plugins: dict
}

impl PluginManager {
    fn register(name: str, plugin: dict) {
        plugins[name] = plugin
    }
    
    fn get(name: str): dict {
        return plugins.get(name, {})
    }
    
    fn execute(name: str, data: dict): dict {
        let plugin = get(name)
        if plugin.has_key("execute") {
            return plugin["execute"](data)
        }
        return {}
    }
}

main {
    let manager = PluginManager { plugins: {} }
    manager.register("transform", {
        "execute": fn(data: dict): dict {
            return {"transformed": true, "data": data}
        }
    })
    
    let result = manager.execute("transform", {"value": 42})
    print(result["transformed"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_middleware_pattern() {
        let src = r#"import std.core.*
struct MiddlewareChain {
    middlewares: list
}

impl MiddlewareChain {
    fn add(middleware: fn(dict, fn(dict)): dict) {
        middlewares.append(middleware)
    }
    
    fn execute(request: dict): dict {
        let index = 0
        fn next(req: dict): dict {
            if index < len(middlewares) {
                let mw = middlewares[index]
                index = index + 1
                return mw(req, next)
            }
            return req
        }
        return next(request)
    }
}

main {
    let chain = MiddlewareChain { middlewares: [] }
    chain.add(fn(req: dict, next: fn(dict): dict): dict {
        req["step1"] = true
        return next(req)
    })
    chain.add(fn(req: dict, next: fn(dict): dict): dict {
        req["step2"] = true
        return next(req)
    })
    
    let result = chain.execute({"initial": true})
    print(result["step1"])
    print(result["step2"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
    }

    #[test]
    fn phase3_interceptor_pattern() {
        let src = r#"import std.core.*
struct Interceptor {
    before: fn(dict): dict,
    after: fn(dict): dict
}

impl Interceptor {
    fn intercept(request: dict, handler: fn(dict): dict): dict {
        let modified_req = before(request)
        let response = handler(modified_req)
        return after(response)
    }
}

main {
    let interceptor = Interceptor {
        before: fn(req: dict): dict {
            req["intercepted"] = true
            return req
        },
        after: fn(res: dict): dict {
            res["processed"] = true
            return res
        }
    }
    
    let result = interceptor.intercept(
        {"data": "test"},
        fn(req: dict): dict { return {"result": req["data"]} }
    )
    
    print(result["processed"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_filter_pattern() {
        let src = r#"import std.core.*
struct FilterChain {
    filters: list
}

impl FilterChain {
    fn add(filter: fn(dict, fn(dict)): dict) {
        filters.append(filter)
    }
    
    fn execute(request: dict): dict {
        let index = 0
        fn do_filter(req: dict): dict {
            if index < len(filters) {
                let f = filters[index]
                index = index + 1
                return f(req, do_filter)
            }
            return req
        }
        return do_filter(request)
    }
}

main {
    let chain = FilterChain { filters: [] }
    chain.add(fn(req: dict, next: fn(dict): dict): dict {
        req["filtered"] = true
        return next(req)
    })
    
    let result = chain.execute({"value": 42})
    print(result["filtered"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_decorator_chain() {
        let src = r#"import std.core.*
fn compose(fns: list): fn(int): int {
    return fn(x: int): int {
        let result = x
        for f in fns {
            result = f(result)
        }
        return result
    }
}

main {
    let double = fn(x: int): int { return x * 2 }
    let add_one = fn(x: int): int { return x + 1 }
    let square = fn(x: int): int { return x * x }
    
    let composed = compose([double, add_one, square])
    let result = composed(3)
    print(result)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "49"); // ((3 * 2) + 1) ^ 2 = 7^2 = 49
    }

    #[test]
    fn phase3_pipeline_pattern() {
        let src = r#"import std.core.*
struct Pipeline {
    stages: list
}

impl Pipeline {
    fn add_stage(stage: fn(dict): dict) {
        stages.append(stage)
    }
    
    fn execute(input: dict): dict {
        let result = input
        for stage in stages {
            result = stage(result)
        }
        return result
    }
}

main {
    let pipeline = Pipeline { stages: [] }
    pipeline.add_stage(fn(data: dict): dict {
        data["step1"] = true
        return data
    })
    pipeline.add_stage(fn(data: dict): dict {
        data["step2"] = true
        return data
    })
    
    let result = pipeline.execute({"initial": true})
    print(result["step1"])
    print(result["step2"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
    }

    #[test]
    fn phase3_coroutine_simulation() {
        let src = r#"import std.core.*
struct Coroutine {
    state: str,
    value: int
}

impl Coroutine {
    fn resume(): int {
        if state == "running" {
            value = value + 1
            return value
        }
        return -1
    }
    
    fn start() {
        state = "running"
    }
}

main {
    let coro = Coroutine { state: "idle", value: 0 }
    coro.start()
    let v1 = coro.resume()
    let v2 = coro.resume()
    let v3 = coro.resume()
    print(v1)
    print(v2)
    print(v3)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "1");
        assert_eq!(lines[1], "2");
        assert_eq!(lines[2], "3");
    }

    #[test]
    fn phase3_generator_simulation() {
        let src = r#"import std.core.*
struct Generator {
    current: int,
    max: int
}

impl Generator {
    fn next(): int {
        if current < max {
            current = current + 1
            return current
        }
        return -1
    }
    
    fn has_next(): bool {
        return current < max
    }
}

main {
    let gen = Generator { current: 0, max: 3 }
    let values = []
    while gen.has_next() {
        values.append(gen.next())
    }
    print(len(values))
    print(values[0])
    print(values[2])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "1");
        assert_eq!(lines[2], "3");
    }

    #[test]
    fn phase3_async_simulation() {
        let src = r#"import std.core.*
struct Future {
    completed: bool,
    result: dict
}

impl Future {
    fn is_done(): bool {
        return completed
    }
    
    fn get_result(): dict {
        return result
    }
    
    fn complete(value: dict) {
        result = value
        completed = true
    }
}

main {
    let future = Future { completed: false, result: {} }
    future.complete({"value": 42})
    print(future.is_done())
    print(future.get_result()["value"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "42");
    }

    #[test]
    fn phase3_promise_simulation() {
        let src = r#"import std.core.*
struct Promise {
    state: str,
    value: dict,
    callbacks: list
}

impl Promise {
    fn then(callback: fn(dict)) {
        callbacks.append(callback)
        if state == "resolved" {
            callback(value)
        }
    }
    
    fn resolve(value: dict) {
        state = "resolved"
        this.value = value
        for cb in callbacks {
            cb(value)
        }
    }
}

main {
    let promise = Promise { state: "pending", value: {}, callbacks: [] }
    let received = false
    promise.then(fn(val: dict) { received = true })
    promise.resolve({"data": "success"})
    print(received)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_reactive_stream() {
        let src = r#"import std.core.*
struct Stream {
    data: list,
    subscribers: list
}

impl Stream {
    fn subscribe(callback: fn(dict)) {
        subscribers.append(callback)
    }
    
    fn emit(value: dict) {
        data.append(value)
        for sub in subscribers {
            sub(value)
        }
    }
    
    fn get_data(): list {
        return data
    }
}

main {
    let stream = Stream { data: [], subscribers: [] }
    let received = []
    stream.subscribe(fn(val: dict) { received.append(val) })
    stream.emit({"value": 1})
    stream.emit({"value": 2})
    print(len(received))
    print(len(stream.get_data()))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "2");
    }

    #[test]
    fn phase3_backpressure() {
        let src = r#"import std.core.*
struct BackpressureBuffer {
    items: list,
    capacity: int
}

impl BackpressureBuffer {
    fn push(item: dict): bool {
        if len(items) < capacity {
            items.append(item)
            return true
        }
        return false
    }
    
    fn pop(): dict {
        if len(items) > 0 {
            return items.pop(0)
        }
        return {}
    }
    
    fn size(): int {
        return len(items)
    }
}

main {
    let buffer = BackpressureBuffer { items: [], capacity: 2 }
    let r1 = buffer.push({"id": 1})
    let r2 = buffer.push({"id": 2})
    let r3 = buffer.push({"id": 3})
    print(r1)
    print(r2)
    print(r3)
    print(buffer.size())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
        assert_eq!(lines[2], "false");
        assert_eq!(lines[3], "2");
    }

    #[test]
    fn phase3_load_balancer() {
        let src = r#"import std.core.*
struct LoadBalancer {
    servers: list,
    current: int
}

impl LoadBalancer {
    fn add_server(server: str) {
        servers.append(server)
    }
    
    fn next_server(): str {
        if len(servers) == 0 {
            return ""
        }
        let server = servers[current]
        current = (current + 1) % len(servers)
        return server
    }
}

main {
    let lb = LoadBalancer { servers: [], current: 0 }
    lb.add_server("server1")
    lb.add_server("server2")
    lb.add_server("server3")
    let s1 = lb.next_server()
    let s2 = lb.next_server()
    let s3 = lb.next_server()
    let s4 = lb.next_server()
    print(s1)
    print(s2)
    print(s3)
    print(s4)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "server1");
        assert_eq!(lines[1], "server2");
        assert_eq!(lines[2], "server3");
        assert_eq!(lines[3], "server1");
    }

    #[test]
    fn phase3_service_discovery() {
        let src = r#"import std.core.*
struct ServiceRegistry {
    services: dict
}

impl ServiceRegistry {
    fn register(name: str, instances: list) {
        services[name] = instances
    }
    
    fn discover(name: str): list {
        return services.get(name, [])
    }
    
    fn deregister(name: str) {
        services.remove(name)
    }
}

main {
    let registry = ServiceRegistry { services: {} }
    registry.register("api", ["instance1", "instance2"])
    registry.register("db", ["db1"])
    let api_instances = registry.discover("api")
    let db_instances = registry.discover("db")
    print(len(api_instances))
    print(len(db_instances))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "1");
    }

    #[test]
    fn phase3_api_gateway() {
        let src = r#"import std.core.*
struct ApiGateway {
    routes: dict
}

impl ApiGateway {
    fn add_route(path: str, handler: fn(dict): dict) {
        routes[path] = handler
    }
    
    fn handle_request(request: dict): dict {
        let path = request["path"]
        if routes.has_key(path) {
            return routes[path](request)
        }
        return {"status": 404, "body": "Not Found"}
    }
}

main {
    let gateway = ApiGateway { routes: {} }
    gateway.add_route("/users", fn(req: dict): dict {
        return {"status": 200, "body": "users list"}
    })
    gateway.add_route("/posts", fn(req: dict): dict {
        return {"status": 200, "body": "posts list"}
    })
    
    let response1 = gateway.handle_request({"path": "/users"})
    let response2 = gateway.handle_request({"path": "/unknown"})
    
    print(response1["status"])
    print(response2["status"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "200");
        assert_eq!(lines[1], "404");
    }

    #[test]
    fn phase3_message_broker() {
        let src = r#"import std.core.*
struct MessageBroker {
    queues: dict
}

impl MessageBroker {
    fn publish(queue: str, message: dict) {
        if !queues.has_key(queue) {
            queues[queue] = []
        }
        queues[queue].append(message)
    }
    
    fn subscribe(queue: str): dict {
        if queues.has_key(queue) && len(queues[queue]) > 0 {
            return queues[queue].pop(0)
        }
        return {}
    }
    
    fn queue_size(queue: str): int {
        if queues.has_key(queue) {
            return len(queues[queue])
        }
        return 0
    }
}

main {
    let broker = MessageBroker { queues: {} }
    broker.publish("tasks", {"id": 1, "type": "process"})
    broker.publish("tasks", {"id": 2, "type": "analyze"})
    let msg1 = broker.subscribe("tasks")
    let msg2 = broker.subscribe("tasks")
    print(msg1["id"])
    print(msg2["id"])
    print(broker.queue_size("tasks"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "1");
        assert_eq!(lines[1], "2");
        assert_eq!(lines[2], "0");
    }

    #[test]
    fn phase3_dead_letter_queue() {
        let src = r#"import std.core.*
struct DeadLetterQueue {
    messages: list
}

impl DeadLetterQueue {
    fn add(message: dict) {
        messages.append(message)
    }
    
    fn size(): int {
        return len(messages)
    }
    
    fn get_all(): list {
        return messages
    }
}

main {
    let dlq = DeadLetterQueue { messages: [] }
    dlq.add({"id": 1, "error": "timeout"})
    dlq.add({"id": 2, "error": "validation"})
    print(dlq.size())
    let all = dlq.get_all()
    print(all[0]["error"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "timeout");
    }

    #[test]
    fn phase3_idempotency_key() {
        let src = r#"import std.core.*
shared store processed_keys = {}

fn process_with_idempotency(key: str, operation: fn()): bool {
    if processed_keys.has_key(key) {
        return false
    }
    operation()
    processed_keys[key] = true
    return true
}

main {
    let counter = 0
    let r1 = process_with_idempotency("key1", fn() { counter = counter + 1 })
    let r2 = process_with_idempotency("key1", fn() { counter = counter + 1 })
    let r3 = process_with_idempotency("key2", fn() { counter = counter + 1 })
    print(r1)
    print(r2)
    print(r3)
    print(counter)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "false");
        assert_eq!(lines[2], "true");
        assert_eq!(lines[3], "2");
    }

    #[test]
    fn phase3_outbox_pattern() {
        let src = r#"import std.core.*
struct Outbox {
    events: list
}

impl Outbox {
    fn add_event(event: dict) {
        events.append(event)
    }
    
    fn flush(): list {
        let flushed = events.copy()
        events = []
        return flushed
    }
    
    fn size(): int {
        return len(events)
    }
}

main {
    let outbox = Outbox { events: [] }
    outbox.add_event({"type": "user_created", "data": {}})
    outbox.add_event({"type": "order_placed", "data": {}})
    let flushed = outbox.flush()
    print(len(flushed))
    print(outbox.size())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "0");
    }

    #[test]
    fn phase3_transaction_log() {
        let src = r#"import std.core.*
struct TransactionLog {
    entries: list
}

impl TransactionLog {
    fn log(operation: str, data: dict) {
        entries.append({"op": operation, "data": data, "timestamp": 0})
    }
    
    fn get_entries(): list {
        return entries
    }
    
    fn size(): int {
        return len(entries)
    }
}

main {
    let log = TransactionLog { entries: [] }
    log.log("INSERT", {"table": "users", "id": 1})
    log.log("UPDATE", {"table": "users", "id": 1})
    log.log("DELETE", {"table": "users", "id": 1})
    print(log.size())
    let entries = log.get_entries()
    print(entries[0]["op"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "INSERT");
    }

    #[test]
    fn phase3_compensation_transaction() {
        let src = r#"import std.core.*
struct CompensationLog {
    compensations: list
}

impl CompensationLog {
    fn register(compensation: fn()) {
        compensations.append(compensation)
    }
    
    fn execute_all() {
        for comp in compensations.reverse() {
            comp()
        }
    }
}

main {
    let log = CompensationLog { compensations: [] }
    let counter = 0
    log.register(fn() { counter = counter - 1 })
    log.register(fn() { counter = counter - 10 })
    counter = 11
    log.execute_all()
    print(counter)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "0");
    }

    #[test]
    fn phase3_two_phase_commit() {
        let src = r#"import std.core.*
struct TwoPhaseCommit {
    participants: list,
    prepared: list
}

impl TwoPhaseCommit {
    fn add_participant(participant: dict) {
        participants.append(participant)
    }
    
    fn prepare(): bool {
        for p in participants {
            if !p["prepare"]() {
                return false
            }
            prepared.append(p)
        }
        return true
    }
    
    fn commit() {
        for p in prepared {
            p["commit"]()
        }
    }
    
    fn rollback() {
        for p in prepared {
            p["rollback"]()
        }
    }
}

main {
    let tpc = TwoPhaseCommit { participants: [], prepared: [] }
    tpc.add_participant({
        "prepare": fn(): bool { return true },
        "commit": fn() { print("committed 1") },
        "rollback": fn() { print("rolled back 1") }
    })
    tpc.add_participant({
        "prepare": fn(): bool { return true },
        "commit": fn() { print("committed 2") },
        "rollback": fn() { print("rolled back 2") }
    })
    
    if tpc.prepare() {
        tpc.commit()
    }
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert!(out.contains("committed 1"));
        assert!(out.contains("committed 2"));
    }

    #[test]
    fn phase3_three_phase_commit() {
        let src = r#"import std.core.*
struct ThreePhaseCommit {
    participants: list
}

impl ThreePhaseCommit {
    fn add_participant(p: dict) {
        participants.append(p)
    }
    
    fn phase1_can_commit(): bool {
        for p in participants {
            if !p["can_commit"]() {
                return false
            }
        }
        return true
    }
    
    fn phase2_pre_commit() {
        for p in participants {
            p["pre_commit"]()
        }
    }
    
    fn phase3_do_commit() {
        for p in participants {
            p["do_commit"]()
        }
    }
}

main {
    let threepc = ThreePhaseCommit { participants: [] }
    threepc.add_participant({
        "can_commit": fn(): bool { return true },
        "pre_commit": fn() { print("pre-commit 1") },
        "do_commit": fn() { print("do-commit 1") }
    })
    
    if threepc.phase1_can_commit() {
        threepc.phase2_pre_commit()
        threepc.phase3_do_commit()
    }
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert!(out.contains("pre-commit 1"));
        assert!(out.contains("do-commit 1"));
    }

    #[test]
    fn phase3_paxos_simulation() {
        let src = r#"import std.core.*
struct PaxosNode {
    id: int,
    promised_to: int,
    accepted_value: dict
}

impl PaxosNode {
    fn prepare(proposal_id: int): bool {
        if promised_to == 0 || proposal_id > promised_to {
            promised_to = proposal_id
            return true
        }
        return false
    }
    
    fn accept(proposal_id: int, value: dict): bool {
        if proposal_id >= promised_to {
            accepted_value = value
            return true
        }
        return false
    }
}

main {
    let node = PaxosNode { id: 1, promised_to: 0, accepted_value: {} }
    let r1 = node.prepare(1)
    let r2 = node.accept(1, {"value": "A"})
    print(r1)
    print(r2)
    print(node.accepted_value["value"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
        assert_eq!(lines[2], "A");
    }

    #[test]
    fn phase3_raft_simulation() {
        let src = r#"import std.core.*
struct RaftNode {
    id: int,
    term: int,
    voted_for: int,
    log: list
}

impl RaftNode {
    fn request_vote(candidate_id: int, candidate_term: int): bool {
        if candidate_term > term && (voted_for == 0 || voted_for == candidate_id) {
            voted_for = candidate_id
            term = candidate_term
            return true
        }
        return false
    }
    
    fn append_entries(entries: list): bool {
        log = log + entries
        return true
    }
}

main {
    let node = RaftNode { id: 1, term: 0, voted_for: 0, log: [] }
    let r1 = node.request_vote(2, 1)
    let r2 = node.append_entries([{"cmd": "set x 1"}])
    print(r1)
    print(r2)
    print(len(node.log))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
        assert_eq!(lines[2], "1");
    }

    #[test]
    fn phase3_vector_clock() {
        let src = r#"import std.core.*
struct VectorClock {
    clocks: dict
}

impl VectorClock {
    fn increment(node_id: str) {
        clocks[node_id] = clocks.get(node_id, 0) + 1
    }
    
    fn merge(other: VectorClock) {
        for key in other.clocks.keys() {
            clocks[key] = max(clocks.get(key, 0), other.clocks[key])
        }
    }
    
    fn happens_before(other: VectorClock): bool {
        for key in clocks.keys() {
            if clocks[key] > other.clocks.get(key, 0) {
                return false
            }
        }
        return true
    }
}

main {
    let vc1 = VectorClock { clocks: {"A": 1, "B": 2} }
    let vc2 = VectorClock { clocks: {"A": 2, "B": 3} }
    print(vc1.happens_before(vc2))
    print(vc2.happens_before(vc1))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "false");
    }

    #[test]
    fn phase3_lamport_timestamp() {
        let src = r#"import std.core.*
struct LamportClock {
    timestamp: int
}

impl LamportClock {
    fn increment(): int {
        timestamp = timestamp + 1
        return timestamp
    }
    
    fn receive(msg_timestamp: int): int {
        timestamp = max(timestamp, msg_timestamp) + 1
        return timestamp
    }
}

main {
    let clock = LamportClock { timestamp: 0 }
    let t1 = clock.increment()
    let t2 = clock.increment()
    let t3 = clock.receive(10)
    print(t1)
    print(t2)
    print(t3)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "1");
        assert_eq!(lines[1], "2");
        assert_eq!(lines[2], "11");
    }

    #[test]
    fn phase3_logical_clock_comparison() {
        let src = r#"import std.core.*
fn compare_timestamps(t1: int, t2: int, id1: str, id2: str): int {
    if t1 < t2 {
        return -1
    } else if t1 > t2 {
        return 1
    } else {
        if id1 < id2 {
            return -1
        } else if id1 > id2 {
            return 1
        }
        return 0
    }
}

main {
    print(compare_timestamps(1, 2, "A", "B"))
    print(compare_timestamps(2, 1, "A", "B"))
    print(compare_timestamps(1, 1, "A", "B"))
    print(compare_timestamps(1, 1, "B", "A"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "-1");
        assert_eq!(lines[1], "1");
        assert_eq!(lines[2], "-1");
        assert_eq!(lines[3], "1");
    }

    #[test]
    fn phase3_consistent_hashing() {
        let src = r#"import std.core.*
fn hash_key(key: str): int {
    let hash = 0
    for char in key {
        hash = hash + ord(char)
    }
    return hash % 100
}

fn get_node(key: str, nodes: list): str {
    let hash = hash_key(key)
    for node in nodes {
        if hash <= node["range"] {
            return node["name"]
        }
    }
    return nodes[0]["name"]
}

main {
    let nodes = [
        {"name": "node1", "range": 33},
        {"name": "node2", "range": 66},
        {"name": "node3", "range": 100}
    ]
    print(get_node("key1", nodes))
    print(get_node("key2", nodes))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert!(lines[0].contains("node"));
        assert!(lines[1].contains("node"));
    }

    #[test]
    fn phase3_gossip_protocol() {
        let src = r#"import std.core.*
struct GossipNode {
    id: str,
    state: dict
}

impl GossipNode {
    fn get_state(): dict {
        return state
    }
    
    fn merge_state(other_state: dict) {
        for key in other_state.keys() {
            state[key] = other_state[key]
        }
    }
}

main {
    let node1 = GossipNode { id: "A", state: {"x": 1} }
    let node2 = GossipNode { id: "B", state: {"y": 2} }
    
    node1.merge_state(node2.get_state())
    node2.merge_state(node1.get_state())
    
    print(node1.state.has_key("y"))
    print(node2.state.has_key("x"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
    }

    #[test]
    fn phase3_epidemic_broadcast() {
        let src = r#"import std.core.*
struct BroadcastNetwork {
    nodes: dict,
    messages: list
}

impl BroadcastNetwork {
    fn add_node(id: str) {
        nodes[id] = {"received": []}
    }
    
    fn broadcast(from: str, message: str) {
        messages.append(message)
        for node_id in nodes.keys() {
            nodes[node_id]["received"].append(message)
        }
    }
    
    fn get_received(node_id: str): list {
        return nodes[node_id]["received"]
    }
}

main {
    let network = BroadcastNetwork { nodes: {}, messages: [] }
    network.add_node("A")
    network.add_node("B")
    network.add_node("C")
    network.broadcast("A", "hello")
    let a_received = network.get_received("A")
    let b_received = network.get_received("B")
    print(len(a_received))
    print(len(b_received))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "1");
        assert_eq!(lines[1], "1");
    }

    #[test]
    fn phase3_failure_detection() {
        let src = r#"import std.core.*
struct HeartbeatMonitor {
    nodes: dict,
    timeout: int
}

impl HeartbeatMonitor {
    fn register_node(node_id: str) {
        nodes[node_id] = {"last_heartbeat": 0, "alive": true}
    }
    
    fn receive_heartbeat(node_id: str, timestamp: int) {
        nodes[node_id]["last_heartbeat"] = timestamp
    }
    
    fn check_failures(current_time: int) {
        for node_id in nodes.keys() {
            if current_time - nodes[node_id]["last_heartbeat"] > timeout {
                nodes[node_id]["alive"] = false
            }
        }
    }
    
    fn is_alive(node_id: str): bool {
        return nodes[node_id]["alive"]
    }
}

main {
    let monitor = HeartbeatMonitor { nodes: {}, timeout: 10 }
    monitor.register_node("A")
    monitor.register_node("B")
    monitor.receive_heartbeat("A", 5)
    monitor.receive_heartbeat("B", 8)
    monitor.check_failures(20)
    print(monitor.is_alive("A"))
    print(monitor.is_alive("B"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "false");
        assert_eq!(lines[1], "false");
    }

    #[test]
    fn phase3_phi_accrual_failure_detector() {
        let src = r#"import std.core.*
struct PhiAccrualDetector {
    intervals: list,
    threshold: float
}

impl PhiAccrualDetector {
    fn record_heartbeat(timestamp: int) {
        if len(intervals) > 0 {
            let last = intervals[len(intervals) - 1]
            intervals.append(timestamp - last)
        } else {
            intervals.append(timestamp)
        }
    }
    
    fn phi(current_time: int): float {
        if len(intervals) < 2 {
            return 0.0
        }
        let sum = 0
        for interval in intervals {
            sum = sum + interval
        }
        let mean = sum / len(intervals)
        let time_since_last = current_time - intervals[len(intervals) - 1]
        return time_since_last / mean
    }
    
    fn is_suspected(current_time: int): bool {
        return phi(current_time) > threshold
    }
}

main {
    let detector = PhiAccrualDetector { intervals: [], threshold: 2.0 }
    detector.record_heartbeat(0)
    detector.record_heartbeat(10)
    detector.record_heartbeat(20)
    let suspected = detector.is_suspected(50)
    print(suspected)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_swim_protocol() {
        let src = r#"import std.core.*
struct SwimNode {
    id: str,
    members: dict,
    incarnation: int
}

impl SwimNode {
    fn add_member(member_id: str) {
        members[member_id] = {"status": "alive", "incarnation": 0}
    }
    
    fn ping(member_id: str): bool {
        return members.has_key(member_id)
    }
    
    fn suspect(member_id: str) {
        if members.has_key(member_id) {
            members[member_id]["status"] = "suspected"
        }
    }
    
    fn confirm(member_id: str) {
        if members.has_key(member_id) {
            members[member_id]["status"] = "confirmed"
        }
    }
    
    fn get_status(member_id: str): str {
        return members[member_id]["status"]
    }
}

main {
    let node = SwimNode { id: "A", members: {}, incarnation: 0 }
    node.add_member("B")
    node.add_member("C")
    node.suspect("B")
    node.confirm("C")
    print(node.get_status("B"))
    print(node.get_status("C"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "suspected");
        assert_eq!(lines[1], "confirmed");
    }

    #[test]
    fn phase3_membership_list() {
        let src = r#"import std.core.*
struct MembershipList {
    members: dict
}

impl MembershipList {
    fn join(node_id: str) {
        members[node_id] = {"status": "active", "joined_at": 0}
    }
    
    fn leave(node_id: str) {
        if members.has_key(node_id) {
            members[node_id]["status"] = "inactive"
        }
    }
    
    fn get_active_members(): list {
        let active = []
        for node_id in members.keys() {
            if members[node_id]["status"] == "active" {
                active.append(node_id)
            }
        }
        return active
    }
    
    fn size(): int {
        return len(members)
    }
}

main {
    let list = MembershipList { members: {} }
    list.join("A")
    list.join("B")
    list.join("C")
    list.leave("B")
    let active = list.get_active_members()
    print(len(active))
    print(list.size())
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "3");
    }

    #[test]
    fn phase3_ring_topology() {
        let src = r#"import std.core.*
struct Ring {
    nodes: list
}

impl Ring {
    fn add_node(node_id: str) {
        nodes.append(node_id)
    }
    
    fn get_next(node_id: str): str {
        let idx = nodes.index(node_id)
        if idx == -1 {
            return ""
        }
        return nodes[(idx + 1) % len(nodes)]
    }
    
    fn get_prev(node_id: str): str {
        let idx = nodes.index(node_id)
        if idx == -1 {
            return ""
        }
        return nodes[(idx - 1 + len(nodes)) % len(nodes)]
    }
}

main {
    let ring = Ring { nodes: [] }
    ring.add_node("A")
    ring.add_node("B")
    ring.add_node("C")
    print(ring.get_next("A"))
    print(ring.get_next("C"))
    print(ring.get_prev("A"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "B");
        assert_eq!(lines[1], "A");
        assert_eq!(lines[2], "C");
    }

    #[test]
    fn phase3_chord_ring() {
        let src = r#"import std.core.*
struct ChordNode {
    id: int,
    finger_table: list
}

impl ChordNode {
    fn init_finger_table(m: int) {
        for i in 0..m {
            finger_table.append({"start": (id + 2 ** i) % (2 ** m), "node": id})
        }
    }
    
    fn find_successor(key: int): int {
        return id
    }
}

main {
    let node = ChordNode { id: 1, finger_table: [] }
    node.init_finger_table(3)
    print(len(node.finger_table))
    print(node.finger_table[0]["start"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "2");
    }

    #[test]
    fn phase3_pastry_routing() {
        let src = r#"import std.core.*
struct PastryNode {
    node_id: str,
    routing_table: dict,
    leaf_set: list
}

impl PastryNode {
    fn add_route(prefix: str, node_id: str) {
        routing_table[prefix] = node_id
    }
    
    fn add_leaf(node_id: str) {
        leaf_set.append(node_id)
    }
    
    fn route(key: str): str {
        let prefix = key[0]
        if routing_table.has_key(prefix) {
            return routing_table[prefix]
        }
        return node_id
    }
}

main {
    let node = PastryNode { node_id: "A", routing_table: {}, leaf_set: [] }
    node.add_route("B", "nodeB")
    node.add_route("C", "nodeC")
    node.add_leaf("nodeD")
    print(node.route("B123"))
    print(node.route("X123"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "nodeB");
        assert_eq!(lines[1], "A");
    }

    #[test]
    fn phase3_can_protocol() {
        let src = r#"import std.core.*
struct CanNode {
    id: str,
    neighbors: list,
    data: dict
}

impl CanNode {
    fn add_neighbor(neighbor_id: str) {
        neighbors.append(neighbor_id)
    }
    
    fn store(key: str, value: dict) {
        data[key] = value
    }
    
    fn get(key: str): dict {
        return data.get(key, {})
    }
    
    fn replicate(key: str, value: dict) {
        data[key] = value
    }
}

main {
    let node = CanNode { id: "A", neighbors: [], data: {} }
    node.add_neighbor("B")
    node.add_neighbor("C")
    node.store("key1", {"value": "data1"})
    let result = node.get("key1")
    print(result["value"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "data1");
    }

    #[test]
    fn phase3_dynamo_style_quorum() {
        let src = r#"import std.core.*
struct QuorumSystem {
    nodes: dict,
    read_quorum: int,
    write_quorum: int
}

impl QuorumSystem {
    fn write(key: str, value: dict, version: int) {
        let written = 0
        for node_id in nodes.keys() {
            nodes[node_id][key] = {"value": value, "version": version}
            written = written + 1
            if written >= write_quorum {
                break
            }
        }
    }
    
    fn read(key: str): dict {
        let reads = []
        for node_id in nodes.keys() {
            if nodes[node_id].has_key(key) {
                reads.append(nodes[node_id][key])
            }
            if len(reads) >= read_quorum {
                break
            }
        }
        if len(reads) == 0 {
            return {}
        }
        let max_version = 0
        let result = {}
        for r in reads {
            if r["version"] > max_version {
                max_version = r["version"]
                result = r["value"]
            }
        }
        return result
    }
}

main {
    let system = QuorumSystem {
        nodes: {"A": {}, "B": {}, "C": {}},
        read_quorum: 2,
        write_quorum: 2
    }
    system.write("key1", {"data": "value1"}, 1)
    let result = system.read("key1")
    print(result["data"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "value1");
    }

    #[test]
    fn phase3_vector_clock_store() {
        let src = r#"import std.core.*
struct VectorClockStore {
    data: dict
}

impl VectorClockStore {
    fn put(key: str, value: dict, vc: dict) {
        data[key] = {"value": value, "vc": vc}
    }
    
    fn get(key: str): dict {
        return data.get(key, {})
    }
    
    fn concurrent(v1: dict, v2: dict): bool {
        let v1_greater = false
        let v2_greater = false
        for k in v1.keys() {
            if v1[k] > v2.get(k, 0) {
                v1_greater = true
            }
            if v2.get(k, 0) > v1[k] {
                v2_greater = true
            }
        }
        return v1_greater && v2_greater
    }
}

main {
    let store = VectorClockStore { data: {} }
    store.put("key1", {"data": "v1"}, {"A": 1, "B": 0})
    store.put("key1", {"data": "v2"}, {"A": 0, "B": 1})
    let result = store.get("key1")
    print(result["value"]["data"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "v2");
    }

    #[test]
    fn phase3_merkle_tree() {
        let src = r#"import std.core.*
fn hash_data(data: str): str {
    return "hash_" + data
}

struct MerkleNode {
    hash: str,
    left: dict,
    right: dict
}

fn build_merkle_tree(data: list): dict {
    if len(data) == 1 {
        return {"hash": hash_data(data[0]), "left": {}, "right": {}}
    }
    let mid = len(data) / 2
    let left = build_merkle_tree(data[0..mid])
    let right = build_merkle_tree(data[mid..])
    return {
        "hash": hash_data(left["hash"] + right["hash"]),
        "left": left,
        "right": right
    }
}

main {
    let tree = build_merkle_tree(["a", "b", "c", "d"])
    print(tree["hash"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert!(out.contains("hash_"));
    }

    #[test]
    fn phase3_bloom_filter() {
        let src = r#"import std.core.*
struct BloomFilter {
    bits: list,
    size: int
}

impl BloomFilter {
    fn init(size: int) {
        bits = []
        for i in 0..size {
            bits.append(false)
        }
    }
    
    fn add(item: str) {
        let hash1 = len(item) % size
        let hash2 = (len(item) * 2) % size
        bits[hash1] = true
        bits[hash2] = true
    }
    
    fn might_contain(item: str): bool {
        let hash1 = len(item) % size
        let hash2 = (len(item) * 2) % size
        return bits[hash1] && bits[hash2]
    }
}

main {
    let bf = BloomFilter { bits: [], size: 100 }
    bf.init(100)
    bf.add("hello")
    bf.add("world")
    print(bf.might_contain("hello"))
    print(bf.might_contain("world"))
    print(bf.might_contain("foo"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "true");
        assert_eq!(lines[1], "true");
    }

    #[test]
    fn phase3_count_min_sketch() {
        let src = r#"import std.core.*
struct CountMinSketch {
    table: list,
    width: int,
    depth: int
}

impl CountMinSketch {
    fn init(width: int, depth: int) {
        table = []
        for i in 0..depth {
            let row = []
            for j in 0..width {
                row.append(0)
            }
            table.append(row)
        }
    }
    
    fn add(item: str, count: int) {
        for i in 0..depth {
            let hash = (len(item) + i) % width
            table[i][hash] = table[i][hash] + count
        }
    }
    
    fn estimate(item: str): int {
        let min_count = 999999
        for i in 0..depth {
            let hash = (len(item) + i) % width
            if table[i][hash] < min_count {
                min_count = table[i][hash]
            }
        }
        return min_count
    }
}

main {
    let cms = CountMinSketch { table: [], width: 10, depth: 3 }
    cms.init(10, 3)
    cms.add("hello", 5)
    cms.add("world", 3)
    print(cms.estimate("hello"))
    print(cms.estimate("world"))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "5");
        assert_eq!(lines[1], "3");
    }

    #[test]
    fn phase3_hyper_log_log() {
        let src = r#"import std.core.*
struct HyperLogLog {
    registers: list,
    num_registers: int
}

impl HyperLogLog {
    fn init(num_registers: int) {
        registers = []
        for i in 0..num_registers {
            registers.append(0)
        }
    }
    
    fn add(item: str) {
        let hash = len(item)
        let register_idx = hash % num_registers
        let remaining = hash / num_registers
        let num_zeros = 0
        let temp = remaining
        while temp > 0 && temp % 2 == 0 {
            num_zeros = num_zeros + 1
            temp = temp / 2
        }
        if num_zeros + 1 > registers[register_idx] {
            registers[register_idx] = num_zeros + 1
        }
    }
    
    fn estimate(): int {
        let sum = 0
        for reg in registers {
            sum = sum + 2 ** reg
        }
        return sum
    }
}

main {
    let hll = HyperLogLog { registers: [], num_registers: 16 }
    hll.init(16)
    hll.add("item1")
    hll.add("item2")
    hll.add("item3")
    let estimate = hll.estimate()
    print(estimate > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_t_digest() {
        let src = r#"import std.core.*
struct TDigest {
    centroids: list,
    compression: float
}

impl TDigest {
    fn init(compression: float) {
        centroids = []
    }
    
    fn add(value: float, weight: int) {
        centroids.append({"mean": value, "weight": weight})
    }
    
    fn quantile(q: float): float {
        if len(centroids) == 0 {
            return 0.0
        }
        let total_weight = 0
        for c in centroids {
            total_weight = total_weight + c["weight"]
        }
        let target = q * total_weight
        let cumulative = 0
        for c in centroids {
            cumulative = cumulative + c["weight"]
            if cumulative >= target {
                return c["mean"]
            }
        }
        return centroids[len(centroids) - 1]["mean"]
    }
}

main {
    let td = TDigest { centroids: [], compression: 100.0 }
    td.init(100.0)
    td.add(1.0, 1)
    td.add(2.0, 1)
    td.add(3.0, 1)
    let median = td.quantile(0.5)
    print(median)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_q_q_plot() {
        let src = r#"import std.core.*
fn compute_quantiles(data: list, num_quantiles: int): list {
    let sorted = data.copy()
    sorted.sort()
    let quantiles = []
    let step = len(sorted) / num_quantiles
    for i in 0..num_quantiles {
        let idx = i * step
        if idx < len(sorted) {
            quantiles.append(sorted[idx])
        }
    }
    return quantiles
}

main {
    let data1 = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let data2 = [2, 4, 6, 8, 10, 12, 14, 16, 18, 20]
    let q1 = compute_quantiles(data1, 5)
    let q2 = compute_quantiles(data2, 5)
    print(len(q1))
    print(len(q2))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "5");
        assert_eq!(lines[1], "5");
    }

    #[test]
    fn phase3_kolmogorov_smirnov() {
        let src = r#"import std.core.*
fn ks_statistic(sample1: list, sample2: list): float {
    let s1 = sample1.copy()
    let s2 = sample2.copy()
    s1.sort()
    s2.sort()
    
    let max_diff = 0.0
    let i = 0
    let j = 0
    while i < len(s1) && j < len(s2) {
        let cdf1 = (i + 1) / len(s1)
        let cdf2 = (j + 1) / len(s2)
        let diff = abs(cdf1 - cdf2)
        if diff > max_diff {
            max_diff = diff
        }
        if s1[i] < s2[j] {
            i = i + 1
        } else {
            j = j + 1
        }
    }
    return max_diff
}

main {
    let sample1 = [1, 2, 3, 4, 5]
    let sample2 = [1, 2, 3, 4, 5]
    let stat = ks_statistic(sample1, sample2)
    print(stat)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "0");
    }

    #[test]
    fn phase3_anderson_darling() {
        let src = r#"import std.core.*
fn anderson_darling_statistic(data: list): float {
    let sorted = data.copy()
    sorted.sort()
    let n = len(sorted)
    let s = 0.0
    for i in 0..n {
        let cdf = (i + 1) / n
        s = s + (2 * (i + 1) - 1) * (log(cdf) + log(1 - (n - i) / n))
    }
    return -n - s / n
}

main {
    let data = [1, 2, 3, 4, 5]
    let stat = anderson_darling_statistic(data)
    print(stat > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_shapiro_wilk() {
        let src = r#"import std.core.*
fn shapiro_wilk_statistic(data: list): float {
    let sorted = data.copy()
    sorted.sort()
    let n = len(sorted)
    let mean = 0.0
    for x in sorted {
        mean = mean + x
    }
    mean = mean / n
    
    let ss = 0.0
    for x in sorted {
        ss = ss + (x - mean) ** 2
    }
    
    return ss / n
}

main {
    let data = [1, 2, 3, 4, 5]
    let stat = shapiro_wilk_statistic(data)
    print(stat > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_chi_squared() {
        let src = r#"import std.core.*
fn chi_squared_test(observed: list, expected: list): float {
    let chi2 = 0.0
    for i in 0..len(observed) {
        chi2 = chi2 + ((observed[i] - expected[i]) ** 2) / expected[i]
    }
    return chi2
}

main {
    let observed = [10, 20, 30]
    let expected = [15, 15, 30]
    let chi2 = chi_squared_test(observed, expected)
    print(chi2 > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_fisher_exact() {
        let src = r#"import std.core.*
fn fisher_exact_test(a: int, b: int, c: int, d: int): float {
    let n = a + b + c + d
    let p = (a + b) * (c + d) * (a + c) * (b + d) / (n * n * n * n)
    return p
}

main {
    let p = fisher_exact_test(10, 5, 3, 12)
    print(p > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_mann_whitney() {
        let src = r#"import std.core.*
fn mann_whitney_u(sample1: list, sample2: list): int {
    let u1 = 0
    for x in sample1 {
        for y in sample2 {
            if x > y {
                u1 = u1 + 1
            }
        }
    }
    return u1
}

main {
    let sample1 = [1, 2, 3, 4, 5]
    let sample2 = [6, 7, 8, 9, 10]
    let u = mann_whitney_u(sample1, sample2)
    print(u)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "0");
    }

    #[test]
    fn phase3_wilcoxon() {
        let src = r#"import std.core.*
fn wilcoxon_signed_rank(differences: list): int {
    let positive_sum = 0
    let negative_sum = 0
    for diff in differences {
        if diff > 0 {
            positive_sum = positive_sum + diff
        } else {
            negative_sum = negative_sum + abs(diff)
        }
    }
    return min(positive_sum, negative_sum)
}

main {
    let differences = [1, -2, 3, -4, 5]
    let w = wilcoxon_signed_rank(differences)
    print(w)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "6");
    }

    #[test]
    fn phase3_kruskal_wallis() {
        let src = r#"import std.core.*
fn kruskal_wallis_h(groups: list): float {
    let all_data = []
    for group in groups {
        for x in group {
            all_data.append(x)
        }
    }
    let n = len(all_data)
    let mean = 0.0
    for x in all_data {
        mean = mean + x
    }
    mean = mean / n
    
    let h = 0.0
    for group in groups {
        let group_mean = 0.0
        for x in group {
            group_mean = group_mean + x
        }
        group_mean = group_mean / len(group)
        h = h + len(group) * (group_mean - mean) ** 2
    }
    return h
}

main {
    let groups = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let h = kruskal_wallis_h(groups)
    print(h > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_friedman() {
        let src = r#"import std.core.*
fn friedman_test(blocks: list): float {
    let k = len(blocks[0])
    let n = len(blocks)
    let mean = 0.0
    for block in blocks {
        for x in block {
            mean = mean + x
        }
    }
    mean = mean / (n * k)
    
    let ss = 0.0
    for block in blocks {
        for x in block {
            ss = ss + (x - mean) ** 2
        }
    }
    return ss
}

main {
    let blocks = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let stat = friedman_test(blocks)
    print(stat > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_spearman() {
        let src = r#"import std.core.*
fn spearman_correlation(x: list, y: list): float {
    let n = len(x)
    let mean_x = 0.0
    let mean_y = 0.0
    for i in 0..n {
        mean_x = mean_x + x[i]
        mean_y = mean_y + y[i]
    }
    mean_x = mean_x / n
    mean_y = mean_y / n
    
    let num = 0.0
    let den_x = 0.0
    let den_y = 0.0
    for i in 0..n {
        num = num + (x[i] - mean_x) * (y[i] - mean_y)
        den_x = den_x + (x[i] - mean_x) ** 2
        den_y = den_y + (y[i] - mean_y) ** 2
    }
    return num / sqrt(den_x * den_y)
}

main {
    let x = [1, 2, 3, 4, 5]
    let y = [2, 4, 6, 8, 10]
    let rho = spearman_correlation(x, y)
    print(rho > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_kendall() {
        let src = r#"import std.core.*
fn kendall_tau(x: list, y: list): float {
    let n = len(x)
    let concordant = 0
    let discordant = 0
    for i in 0..n {
        for j in (i + 1)..n {
            if (x[i] - x[j]) * (y[i] - y[j]) > 0 {
                concordant = concordant + 1
            } else if (x[i] - x[j]) * (y[i] - y[j]) < 0 {
                discordant = discordant + 1
            }
        }
    }
    return (concordant - discordant) / (n * (n - 1) / 2)
}

main {
    let x = [1, 2, 3, 4, 5]
    let y = [2, 4, 6, 8, 10]
    let tau = kendall_tau(x, y)
    print(tau > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_point_biserial() {
        let src = r#"import std.core.*
fn point_biserial_correlation(binary: list, continuous: list): float {
    let n = len(binary)
    let n1 = 0
    let n0 = 0
    let sum1 = 0.0
    let sum0 = 0.0
    for i in 0..n {
        if binary[i] == 1 {
            n1 = n1 + 1
            sum1 = sum1 + continuous[i]
        } else {
            n0 = n0 + 1
            sum0 = sum0 + continuous[i]
        }
    }
    let mean1 = sum1 / n1
    let mean0 = sum0 / n0
    let mean_total = (sum1 + sum0) / n
    let ss = 0.0
    for x in continuous {
        ss = ss + (x - mean_total) ** 2
    }
    let sd = sqrt(ss / n)
    return (mean1 - mean0) / sd * sqrt(n1 * n0 / (n * n))
}

main {
    let binary = [0, 0, 1, 1, 1]
    let continuous = [1.0, 2.0, 3.0, 4.0, 5.0]
    let rpb = point_biserial_correlation(binary, continuous)
    print(rpb > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_cramers_v() {
        let src = r#"import std.core.*
fn cramers_v(contingency_table: list): float {
    let n = 0
    for row in contingency_table {
        for cell in row {
            n = n + cell
        }
    }
    let chi2 = 0.0
    let row_sums = []
    let col_sums = []
    for i in 0..len(contingency_table) {
        let row_sum = 0
        for cell in contingency_table[i] {
            row_sum = row_sum + cell
        }
        row_sums.append(row_sum)
    }
    for j in 0..len(contingency_table[0]) {
        let col_sum = 0
        for i in 0..len(contingency_table) {
            col_sum = col_sum + contingency_table[i][j]
        }
        col_sums.append(col_sum)
    }
    for i in 0..len(contingency_table) {
        for j in 0..len(contingency_table[0]) {
            let expected = row_sums[i] * col_sums[j] / n
            chi2 = chi2 + ((contingency_table[i][j] - expected) ** 2) / expected
        }
    }
    let k = min(len(contingency_table), len(contingency_table[0]))
    return sqrt(chi2 / (n * (k - 1)))
}

main {
    let table = [[10, 20], [30, 40]]
    let v = cramers_v(table)
    print(v > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_phi_coefficient() {
        let src = r#"import std.core.*
fn phi_coefficient(a: int, b: int, c: int, d: int): float {
    let n = a + b + c + d
    return (a * d - b * c) / sqrt((a + b) * (c + d) * (a + c) * (b + d))
}

main {
    let phi = phi_coefficient(10, 5, 3, 12)
    print(phi != 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_contingency_coefficient() {
        let src = r#"import std.core.*
fn contingency_coefficient(chi2: float, n: int): float {
    return sqrt(chi2 / (chi2 + n))
}

main {
    let c = contingency_coefficient(10.0, 100)
    print(c > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_goodman_kruskal() {
        let src = r#"import std.core.*
fn goodman_kruskal_gamma(concordant: int, discordant: int): float {
    return (concordant - discordant) / (concordant + discordant)
}

main {
    let gamma = goodman_kruskal_gamma(50, 30)
    print(gamma > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_somers_d() {
        let src = r#"import std.core.*
fn somers_d(concordant: int, discordant: int, n: int): float {
    return (concordant - discordant) / (n * (n - 1) / 2)
}

main {
    let d = somers_d(50, 30, 100)
    print(d != 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_yules_q() {
        let src = r#"import std.core.*
fn yules_q(a: int, b: int, c: int, d: int): float {
    return (a * d - b * c) / (a * d + b * c)
}

main {
    let q = yules_q(10, 5, 3, 12)
    print(q != 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_yules_y() {
        let src = r#"import std.core.*
fn yules_y(a: int, b: int, c: int, d: int): float {
    let ad = sqrt(a * d)
    let bc = sqrt(b * c)
    return (ad - bc) / (ad + bc)
}

main {
    let y = yules_y(10, 5, 3, 12)
    print(y != 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_likelihood_ratio() {
        let src = r#"import std.core.*
fn likelihood_ratio_test(observed: list, expected: list): float {
    let g2 = 0.0
    for i in 0..len(observed) {
        if observed[i] > 0 && expected[i] > 0 {
            g2 = g2 + 2 * observed[i] * log(observed[i] / expected[i])
        }
    }
    return g2
}

main {
    let observed = [10, 20, 30]
    let expected = [15, 15, 30]
    let g2 = likelihood_ratio_test(observed, expected)
    print(g2 > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_barnard() {
        let src = r#"import std.core.*
fn barnard_exact_test(a: int, b: int, c: int, d: int): float {
    let n = a + b + c + d
    let p = (a + b) * (c + d) / (n * n)
    return p
}

main {
    let p = barnard_exact_test(10, 5, 3, 12)
    print(p > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_boschloo() {
        let src = r#"import std.core.*
fn boschloo_exact_test(a: int, b: int, c: int, d: int): float {
    let n = a + b + c + d
    let p1 = a / (a + b)
    let p2 = c / (c + d)
    return abs(p1 - p2)
}

main {
    let stat = boschloo_exact_test(10, 5, 3, 12)
    print(stat > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_stuart_maxwell() {
        let src = r#"import std.core.*
fn stuart_maxwell_test(marginals: list): float {
    let k = len(marginals)
    let chi2 = 0.0
    for i in 0..k {
        chi2 = chi2 + marginals[i] ** 2
    }
    return chi2
}

main {
    let marginals = [10, 20, 30]
    let stat = stuart_maxwell_test(marginals)
    print(stat > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_cochran_q() {
        let src = r#"import std.core.*
fn cochran_q_test(data: list): float {
    let k = len(data[0])
    let n = len(data)
    let grand_mean = 0.0
    for row in data {
        for x in row {
            grand_mean = grand_mean + x
        }
    }
    grand_mean = grand_mean / (n * k)
    
    let ss_between = 0.0
    for row in data {
        let row_mean = 0.0
        for x in row {
            row_mean = row_mean + x
        }
        row_mean = row_mean / k
        ss_between = ss_between + k * (row_mean - grand_mean) ** 2
    }
    return ss_between
}

main {
    let data = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let q = cochran_q_test(data)
    print(q > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_page() {
        let src = r#"import std.core.*
fn page_trend_test(data: list): float {
    let k = len(data[0])
    let n = len(data)
    let mean = 0.0
    for row in data {
        for x in row {
            mean = mean + x
        }
    }
    mean = mean / (n * k)
    
    let l = 0.0
    for j in 0..k {
        let col_sum = 0.0
        for row in data {
            col_sum = col_sum + row[j]
        }
        l = l + (j + 1) * col_sum
    }
    return l
}

main {
    let data = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let l = page_trend_test(data)
    print(l > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_jonckheere_terpstra() {
        let src = r#"import std.core.*
fn jonckheere_terpstra_test(groups: list): int {
    let jt = 0
    for i in 0..len(groups) {
        for j in (i + 1)..len(groups) {
            for x in groups[i] {
                for y in groups[j] {
                    if x < y {
                        jt = jt + 1
                    }
                }
            }
        }
    }
    return jt
}

main {
    let groups = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let jt = jonckheere_terpstra_test(groups)
    print(jt > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_fligner_killeen() {
        let src = r#"import std.core.*
fn fligner_killeen_test(groups: list): float {
    let all_data = []
    for group in groups {
        for x in group {
            all_data.append(x)
        }
    }
    let median = 0.0
    let sorted = all_data.copy()
    sorted.sort()
    median = sorted[len(sorted) / 2]
    
    let scores = []
    for x in all_data {
        scores.append(abs(x - median))
    }
    
    let mean_score = 0.0
    for s in scores {
        mean_score = mean_score + s
    }
    mean_score = mean_score / len(scores)
    
    let ss = 0.0
    for s in scores {
        ss = ss + (s - mean_score) ** 2
    }
    return ss
}

main {
    let groups = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let stat = fligner_killeen_test(groups)
    print(stat > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_levene() {
        let src = r#"import std.core.*
fn levene_test(groups: list): float {
    let k = len(groups)
    let n = 0
    for group in groups {
        n = n + len(group)
    }
    
    let medians = []
    for group in groups {
        let sorted = group.copy()
        sorted.sort()
        medians.append(sorted[len(sorted) / 2])
    }
    
    let z_scores = []
    for i in 0..k {
        let z_group = []
        for x in groups[i] {
            z_group.append(abs(x - medians[i]))
        }
        z_scores.append(z_group)
    }
    
    let z_bar = 0.0
    for z_group in z_scores {
        for z in z_group {
            z_bar = z_bar + z
        }
    }
    z_bar = z_bar / n
    
    let ss_between = 0.0
    for i in 0..k {
        let z_group_mean = 0.0
        for z in z_scores[i] {
            z_group_mean = z_group_mean + z
        }
        z_group_mean = z_group_mean / len(z_scores[i])
        ss_between = ss_between + len(z_scores[i]) * (z_group_mean - z_bar) ** 2
    }
    return ss_between
}

main {
    let groups = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let stat = levene_test(groups)
    print(stat > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_brown_forsythe() {
        let src = r#"import std.core.*
fn brown_forsythe_test(groups: list): float {
    let k = len(groups)
    let n = 0
    for group in groups {
        n = n + len(group)
    }
    
    let medians = []
    for group in groups {
        let sorted = group.copy()
        sorted.sort()
        medians.append(sorted[len(sorted) / 2])
    }
    
    let z_scores = []
    for i in 0..k {
        let z_group = []
        for x in groups[i] {
            z_group.append(abs(x - medians[i]))
        }
        z_scores.append(z_group)
    }
    
    let z_bar = 0.0
    for z_group in z_scores {
        for z in z_group {
            z_bar = z_bar + z
        }
    }
    z_bar = z_bar / n
    
    let ss_between = 0.0
    for i in 0..k {
        let z_group_mean = 0.0
        for z in z_scores[i] {
            z_group_mean = z_group_mean + z
        }
        z_group_mean = z_group_mean / len(z_scores[i])
        ss_between = ss_between + len(z_scores[i]) * (z_group_mean - z_bar) ** 2
    }
    return ss_between
}

main {
    let groups = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let stat = brown_forsythe_test(groups)
    print(stat > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_welch() {
        let src = r#"import std.core.*
fn welch_anova(groups: list): float {
    let k = len(groups)
    let n = 0
    for group in groups {
        n = n + len(group)
    }
    
    let means = []
    let variances = []
    let ns = []
    for group in groups {
        let mean = 0.0
        for x in group {
            mean = mean + x
        }
        mean = mean / len(group)
        means.append(mean)
        
        let var = 0.0
        for x in group {
            var = var + (x - mean) ** 2
        }
        var = var / (len(group) - 1)
        variances.append(var)
        ns.append(len(group))
    }
    
    let grand_mean = 0.0
    for i in 0..k {
        grand_mean = grand_mean + ns[i] * means[i]
    }
    grand_mean = grand_mean / n
    
    let ss_between = 0.0
    for i in 0..k {
        ss_between = ss_between + ns[i] * (means[i] - grand_mean) ** 2
    }
    
    let ss_within = 0.0
    for i in 0..k {
        ss_within = ss_within + (ns[i] - 1) * variances[i]
    }
    
    return ss_between / (ss_within / (n - k))
}

main {
    let groups = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let f = welch_anova(groups)
    print(f > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_games_howell() {
        let src = r#"import std.core.*
fn games_howell_posthoc(group1: list, group2: list, mse: float): float {
    let mean1 = 0.0
    let mean2 = 0.0
    for x in group1 {
        mean1 = mean1 + x
    }
    mean1 = mean1 / len(group1)
    for x in group2 {
        mean2 = mean2 + x
    }
    mean2 = mean2 / len(group2)
    
    let se = sqrt(mse * (1 / len(group1) + 1 / len(group2)))
    return (mean1 - mean2) / se
}

main {
    let group1 = [1, 2, 3]
    let group2 = [4, 5, 6]
    let mse = 1.0
    let t = games_howell_posthoc(group1, group2, mse)
    print(t != 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_tukey_hsd() {
        let src = r#"import std.core.*
fn tukey_hsd_posthoc(group1: list, group2: list, mse: float): float {
    let mean1 = 0.0
    let mean2 = 0.0
    for x in group1 {
        mean1 = mean1 + x
    }
    mean1 = mean1 / len(group1)
    for x in group2 {
        mean2 = mean2 + x
    }
    mean2 = mean2 / len(group2)
    
    let se = sqrt(mse * (1 / len(group1) + 1 / len(group2)))
    return (mean1 - mean2) / se
}

main {
    let group1 = [1, 2, 3]
    let group2 = [4, 5, 6]
    let mse = 1.0
    let q = tukey_hsd_posthoc(group1, group2, mse)
    print(q != 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_dunnett() {
        let src = r#"import std.core.*
fn dunnett_test(control: list, treatment: list, mse: float): float {
    let mean_control = 0.0
    let mean_treatment = 0.0
    for x in control {
        mean_control = mean_control + x
    }
    mean_control = mean_control / len(control)
    for x in treatment {
        mean_treatment = mean_treatment + x
    }
    mean_treatment = mean_treatment / len(treatment)
    
    let se = sqrt(mse * (1 / len(control) + 1 / len(treatment)))
    return (mean_treatment - mean_control) / se
}

main {
    let control = [1, 2, 3]
    let treatment = [4, 5, 6]
    let mse = 1.0
    let t = dunnett_test(control, treatment, mse)
    print(t != 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_scheffe() {
        let src = r#"import std.core.*
fn scheffe_test(contrast: list, group_means: list, group_ns: list, mse: float): float {
    let psi = 0.0
    for i in 0..len(contrast) {
        psi = psi + contrast[i] * group_means[i]
    }
    
    let se = 0.0
    for i in 0..len(contrast) {
        se = se + (contrast[i] ** 2) / group_ns[i]
    }
    se = sqrt(mse * se)
    
    return psi / se
}

main {
    let contrast = [1, -1, 0]
    let group_means = [10.0, 20.0, 30.0]
    let group_ns = [5, 5, 5]
    let mse = 2.0
    let f = scheffe_test(contrast, group_means, group_ns, mse)
    print(f != 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_bonferroni() {
        let src = r#"import std.core.*
fn bonferroni_correction(p_value: float, num_comparisons: int): float {
    return p_value * num_comparisons
}

main {
    let p = 0.01
    let num_comp = 5
    let corrected = bonferroni_correction(p, num_comp)
    print(corrected)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "0.05");
    }

    #[test]
    fn phase3_holm_bonferroni() {
        let src = r#"import std.core.*
fn holm_bonferroni_correction(p_values: list): list {
    let sorted_p = p_values.copy()
    sorted_p.sort()
    let n = len(sorted_p)
    let corrected = []
    for i in 0..n {
        let adj_p = sorted_p[i] * (n - i)
        corrected.append(adj_p)
    }
    return corrected
}

main {
    let p_values = [0.01, 0.02, 0.03]
    let corrected = holm_bonferroni_correction(p_values)
    print(len(corrected))
    print(corrected[0])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "0.03");
    }

    #[test]
    fn phase3_hochberg() {
        let src = r#"import std.core.*
fn hochberg_correction(p_values: list): list {
    let sorted_p = p_values.copy()
    sorted_p.sort()
    let n = len(sorted_p)
    let corrected = []
    let min_p = 1.0
    for i in (0..n).reverse() {
        let adj_p = sorted_p[i] * (n - i)
        if adj_p < min_p {
            min_p = adj_p
        }
        corrected.append(min_p)
    }
    corrected.reverse()
    return corrected
}

main {
    let p_values = [0.01, 0.02, 0.03]
    let corrected = hochberg_correction(p_values)
    print(len(corrected))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_hommel() {
        let src = r#"import std.core.*
fn hommel_correction(p_values: list): list {
    let sorted_p = p_values.copy()
    sorted_p.sort()
    let n = len(sorted_p)
    let corrected = []
    for i in 0..n {
        let adj_p = sorted_p[i] * n / (i + 1)
        corrected.append(adj_p)
    }
    return corrected
}

main {
    let p_values = [0.01, 0.02, 0.03]
    let corrected = hommel_correction(p_values)
    print(len(corrected))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_benjamini_hochberg() {
        let src = r#"import std.core.*
fn benjamini_hochberg_fdr(p_values: list, alpha: float): list {
    let sorted_p = p_values.copy()
    sorted_p.sort()
    let n = len(sorted_p)
    let rejected = []
    for i in 0..n {
        let threshold = alpha * (i + 1) / n
        if sorted_p[i] <= threshold {
            rejected.append(i)
        }
    }
    return rejected
}

main {
    let p_values = [0.01, 0.02, 0.03, 0.04, 0.05]
    let alpha = 0.05
    let rejected = benjamini_hochberg_fdr(p_values, alpha)
    print(len(rejected))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_benjamini_yekutieli() {
        let src = r#"import std.core.*
fn benjamini_yekutieli_fdr(p_values: list, alpha: float): list {
    let sorted_p = p_values.copy()
    sorted_p.sort()
    let n = len(sorted_p)
    let c = 0.0
    for i in 1..(n + 1) {
        c = c + 1.0 / i
    }
    let rejected = []
    for i in 0..n {
        let threshold = alpha * (i + 1) / (n * c)
        if sorted_p[i] <= threshold {
            rejected.append(i)
        }
    }
    return rejected
}

main {
    let p_values = [0.01, 0.02, 0.03, 0.04, 0.05]
    let alpha = 0.05
    let rejected = benjamini_yekutieli_fdr(p_values, alpha)
    print(len(rejected) > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_storey_q_value() {
        let src = r#"import std.core.*
fn storey_q_value(p_values: list, lambda: float): float {
    let n = len(p_values)
    let pi0 = 0.0
    for p in p_values {
        if p > lambda {
            pi0 = pi0 + 1
        }
    }
    pi0 = pi0 / (n * (1 - lambda))
    
    let sorted_p = p_values.copy()
    sorted_p.sort()
    let q_values = []
    for i in 0..n {
        let q = pi0 * n * sorted_p[i] / (i + 1)
        q_values.append(q)
    }
    return q_values[0]
}

main {
    let p_values = [0.01, 0.02, 0.03, 0.04, 0.05]
    let lambda = 0.5
    let q = storey_q_value(p_values, lambda)
    print(q > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_local_fdr() {
        let src = r#"import std.core.*
fn local_fdr(z_scores: list, null_mean: float, null_sd: float): list {
    let fdrs = []
    for z in z_scores {
        let p_null = exp(-0.5 * ((z - null_mean) / null_sd) ** 2) / (null_sd * sqrt(2 * 3.14159))
        fdrs.append(p_null)
    }
    return fdrs
}

main {
    let z_scores = [0.0, 1.0, 2.0, 3.0]
    let null_mean = 0.0
    let null_sd = 1.0
    let fdrs = local_fdr(z_scores, null_mean, null_sd)
    print(len(fdrs))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "4");
    }

    #[test]
    fn phase3_empirical_bayes() {
        let src = r#"import std.core.*
fn empirical_bayes_estimate(data: list): float {
    let mean = 0.0
    for x in data {
        mean = mean + x
    }
    mean = mean / len(data)
    
    let var = 0.0
    for x in data {
        var = var + (x - mean) ** 2
    }
    var = var / len(data)
    
    return mean
}

main {
    let data = [1.0, 2.0, 3.0, 4.0, 5.0]
    let estimate = empirical_bayes_estimate(data)
    print(estimate)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_james_stein() {
        let src = r#"import std.core.*
fn james_stein_estimator(means: list, grand_mean: float): list {
    let k = len(means)
    let sum_sq = 0.0
    for m in means {
        sum_sq = sum_sq + (m - grand_mean) ** 2
    }
    
    let shrinkage = 1.0 - (k - 2) / sum_sq
    if shrinkage < 0 {
        shrinkage = 0
    }
    
    let estimates = []
    for m in means {
        let est = grand_mean + shrinkage * (m - grand_mean)
        estimates.append(est)
    }
    return estimates
}

main {
    let means = [10.0, 20.0, 30.0]
    let grand_mean = 20.0
    let estimates = james_stein_estimator(means, grand_mean)
    print(len(estimates))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_ridge_regression() {
        let src = r#"import std.core.*
fn ridge_regression_coefficients(x: list, y: list, lambda: float): list {
    let n = len(x)
    let p = len(x[0])
    
    let xtx = []
    for i in 0..p {
        let row = []
        for j in 0..p {
            let sum = 0.0
            for k in 0..n {
                sum = sum + x[k][i] * x[k][j]
            }
            row.append(sum)
        }
        xtx.append(row)
    }
    
    for i in 0..p {
        xtx[i][i] = xtx[i][i] + lambda
    }
    
    return [0.0]
}

main {
    let x = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]
    let y = [1.0, 2.0, 3.0]
    let lambda = 0.1
    let coeffs = ridge_regression_coefficients(x, y, lambda)
    print(len(coeffs))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "1");
    }

    #[test]
    fn phase3_lasso_regression() {
        let src = r#"import std.core.*
fn lasso_regression_coefficients(x: list, y: list, lambda: float): list {
    let n = len(x)
    let p = len(x[0])
    
    let coefficients = []
    for i in 0..p {
        coefficients.append(0.0)
    }
    
    return coefficients
}

main {
    let x = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]
    let y = [1.0, 2.0, 3.0]
    let lambda = 0.1
    let coeffs = lasso_regression_coefficients(x, y, lambda)
    print(len(coeffs))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_elastic_net() {
        let src = r#"import std.core.*
fn elastic_net_coefficients(x: list, y: list, lambda1: float, lambda2: float): list {
    let n = len(x)
    let p = len(x[0])
    
    let coefficients = []
    for i in 0..p {
        coefficients.append(0.0)
    }
    
    return coefficients
}

main {
    let x = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]
    let y = [1.0, 2.0, 3.0]
    let lambda1 = 0.1
    let lambda2 = 0.1
    let coeffs = elastic_net_coefficients(x, y, lambda1, lambda2)
    print(len(coeffs))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_principal_component() {
        let src = r#"import std.core.*
fn pca_eigenvalues(data: list): list {
    let n = len(data)
    let p = len(data[0])
    
    let means = []
    for j in 0..p {
        let mean = 0.0
        for i in 0..n {
            mean = mean + data[i][j]
        }
        means.append(mean / n)
    }
    
    let centered = []
    for i in 0..n {
        let row = []
        for j in 0..p {
            row.append(data[i][j] - means[j])
        }
        centered.append(row)
    }
    
    let covariance = []
    for i in 0..p {
        let row = []
        for j in 0..p {
            let sum = 0.0
            for k in 0..n {
                sum = sum + centered[k][i] * centered[k][j]
            }
            row.append(sum / (n - 1))
        }
        covariance.append(row)
    }
    
    let eigenvalues = []
    for i in 0..p {
        eigenvalues.append(covariance[i][i])
    }
    return eigenvalues
}

main {
    let data = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]
    let eigenvalues = pca_eigenvalues(data)
    print(len(eigenvalues))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_factor_analysis() {
        let src = r#"import std.core.*
fn factor_analysis_loadings(data: list, num_factors: int): list {
    let n = len(data)
    let p = len(data[0])
    
    let loadings = []
    for i in 0..p {
        let row = []
        for j in 0..num_factors {
            row.append(0.0)
        }
        loadings.append(row)
    }
    return loadings
}

main {
    let data = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]
    let num_factors = 2
    let loadings = factor_analysis_loadings(data, num_factors)
    print(len(loadings))
    print(len(loadings[0]))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "2");
    }

    #[test]
    fn phase3_independent_component() {
        let src = r#"import std.core.*
fn ica_unmixing_matrix(data: list): list {
    let n = len(data)
    let p = len(data[0])
    
    let w = []
    for i in 0..p {
        let row = []
        for j in 0..p {
            if i == j {
                row.append(1.0)
            } else {
                row.append(0.0)
            }
        }
        w.append(row)
    }
    return w
}

main {
    let data = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]
    let w = ica_unmixing_matrix(data)
    print(len(w))
    print(w[0][0])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "1");
    }

    #[test]
    fn phase3_nonnegative_matrix_factorization() {
        let src = r#"import std.core.*
fn nmf_factors(v: list, r: int): list {
    let m = len(v)
    let n = len(v[0])
    
    let w = []
    for i in 0..m {
        let row = []
        for j in 0..r {
            row.append(1.0)
        }
        w.append(row)
    }
    
    let h = []
    for i in 0..r {
        let row = []
        for j in 0..n {
            row.append(1.0)
        }
        h.append(row)
    }
    
    return [w, h]
}

main {
    let v = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]
    let r = 2
    let factors = nmf_factors(v, r)
    print(len(factors))
    print(len(factors[0]))
    print(len(factors[1]))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "3");
        assert_eq!(lines[2], "2");
    }

    #[test]
    fn phase3_dictionary_learning() {
        let src = r#"import std.core.*
fn dictionary_learning(data: list, num_atoms: int): list {
    let n = len(data)
    let p = len(data[0])
    
    let dictionary = []
    for i in 0..num_atoms {
        let atom = []
        for j in 0..p {
            atom.append(1.0 / sqrt(p))
        }
        dictionary.append(atom)
    }
    return dictionary
}

main {
    let data = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]
    let num_atoms = 2
    let dictionary = dictionary_learning(data, num_atoms)
    print(len(dictionary))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_sparse_coding() {
        let src = r#"import std.core.*
fn sparse_code(x: list, dictionary: list, lambda: float): list {
    let num_atoms = len(dictionary)
    let coefficients = []
    for i in 0..num_atoms {
        coefficients.append(0.0)
    }
    return coefficients
}

main {
    let x = [1.0, 2.0, 3.0]
    let dictionary = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    let lambda = 0.1
    let code = sparse_code(x, dictionary, lambda)
    print(len(code))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_compressed_sensing() {
        let src = r#"import std.core.*
fn compressed_sensing_recovery(measurements: list, sensing_matrix: list): list {
    let n = len(sensing_matrix[0])
    let recovered = []
    for i in 0..n {
        recovered.append(0.0)
    }
    return recovered
}

main {
    let measurements = [1.0, 2.0, 3.0]
    let sensing_matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    let recovered = compressed_sensing_recovery(measurements, sensing_matrix)
    print(len(recovered))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_wavelet_transform() {
        let src = r#"import std.core.*
fn discrete_wavelet_transform(signal: list): list {
    let n = len(signal)
    let approximation = []
    let detail = []
    for i in 0..(n / 2) {
        approximation.append((signal[2 * i] + signal[2 * i + 1]) / 2)
        detail.append((signal[2 * i] - signal[2 * i + 1]) / 2)
    }
    return approximation + detail
}

main {
    let signal = [1.0, 2.0, 3.0, 4.0]
    let coeffs = discrete_wavelet_transform(signal)
    print(len(coeffs))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "4");
    }

    #[test]
    fn phase3_fourier_transform() {
        let src = r#"import std.core.*
fn discrete_fourier_transform(signal: list): list {
    let n = len(signal)
    let spectrum = []
    for k in 0..n {
        let real = 0.0
        let imag = 0.0
        for t in 0..n {
            let angle = 2 * 3.14159 * t * k / n
            real = real + signal[t] * cos(angle)
            imag = imag - signal[t] * sin(angle)
        }
        spectrum.append(sqrt(real * real + imag * imag))
    }
    return spectrum
}

main {
    let signal = [1.0, 2.0, 3.0, 4.0]
    let spectrum = discrete_fourier_transform(signal)
    print(len(spectrum))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "4");
    }

    #[test]
    fn phase3_laplace_transform() {
        let src = r#"import std.core.*
fn numerical_laplace_transform(f: fn(float): float, s: float, t_max: float, dt: float): float {
    let result = 0.0
    let t = 0.0
    while t < t_max {
        result = result + f(t) * exp(-s * t) * dt
        t = t + dt
    }
    return result
}

main {
    let f = fn(t: float): float { return 1.0 }
    let s = 1.0
    let t_max = 10.0
    let dt = 0.1
    let result = numerical_laplace_transform(f, s, t_max, dt)
    print(result > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_z_transform() {
        let src = r#"import std.core.*
fn z_transform_coefficients(sequence: list, z: float): float {
    let result = 0.0
    for n in 0..len(sequence) {
        result = result + sequence[n] * (z ** (-n))
    }
    return result
}

main {
    let sequence = [1.0, 2.0, 3.0]
    let z = 2.0
    let result = z_transform_coefficients(sequence, z)
    print(result > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_hilbert_transform() {
        let src = r#"import std.core.*
fn hilbert_transform(signal: list): list {
    let n = len(signal)
    let analytic = []
    for i in 0..n {
        analytic.append(signal[i])
    }
    return analytic
}

main {
    let signal = [1.0, 2.0, 3.0, 4.0]
    let analytic = hilbert_transform(signal)
    print(len(analytic))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "4");
    }

    #[test]
    fn phase3_cepstrum() {
        let src = r#"import std.core.*
fn cepstrum(signal: list): list {
    let n = len(signal)
    let cepstral = []
    for i in 0..n {
        cepstral.append(log(abs(signal[i]) + 1))
    }
    return cepstral
}

main {
    let signal = [1.0, 2.0, 3.0, 4.0]
    let cepstral = cepstrum(signal)
    print(len(cepstral))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "4");
    }

    #[test]
    fn phase3_spectrogram() {
        let src = r#"import std.core.*
fn spectrogram(signal: list, window_size: int, hop_size: int): list {
    let n = len(signal)
    let num_frames = (n - window_size) / hop_size + 1
    let frames = []
    for i in 0..num_frames {
        let frame = []
        for j in 0..window_size {
            frame.append(signal[i * hop_size + j])
        }
        frames.append(frame)
    }
    return frames
}

main {
    let signal = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    let window_size = 4
    let hop_size = 2
    let frames = spectrogram(signal, window_size, hop_size)
    print(len(frames))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_mel_frequency() {
        let src = r#"import std.core.*
fn hz_to_mel(hz: float): float {
    return 2595 * log10(1 + hz / 700)
}

fn mel_to_hz(mel: float): float {
    return 700 * (10 ** (mel / 2595) - 1)
}

main {
    let hz = 1000.0
    let mel = hz_to_mel(hz)
    let hz_back = mel_to_hz(mel)
    print(abs(hz - hz_back) < 0.01)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_mfcc() {
        let src = r#"import std.core.*
fn mfcc_features(signal: list, num_coefficients: int): list {
    let n = len(signal)
    let features = []
    for i in 0..num_coefficients {
        features.append(0.0)
    }
    return features
}

main {
    let signal = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    let num_coefficients = 13
    let features = mfcc_features(signal, num_coefficients)
    print(len(features))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "13");
    }

    #[test]
    fn phase3_linear_prediction() {
        let src = r#"import std.core.*
fn linear_prediction_coefficients(signal: list, order: int): list {
    let coefficients = []
    for i in 0..order {
        coefficients.append(0.0)
    }
    return coefficients
}

main {
    let signal = [1.0, 2.0, 3.0, 4.0, 5.0]
    let order = 3
    let coefficients = linear_prediction_coefficients(signal, order)
    print(len(coefficients))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_autocorrelation() {
        let src = r#"import std.core.*
fn autocorrelation(signal: list, lag: int): float {
    let n = len(signal)
    let mean = 0.0
    for x in signal {
        mean = mean + x
    }
    mean = mean / n
    
    let numerator = 0.0
    let denominator = 0.0
    for i in 0..(n - lag) {
        numerator = numerator + (signal[i] - mean) * (signal[i + lag] - mean)
    }
    for i in 0..n {
        denominator = denominator + (signal[i] - mean) ** 2
    }
    return numerator / denominator
}

main {
    let signal = [1.0, 2.0, 3.0, 4.0, 5.0]
    let lag = 1
    let ac = autocorrelation(signal, lag)
    print(ac > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_cross_correlation() {
        let src = r#"import std.core.*
fn cross_correlation(signal1: list, signal2: list, lag: int): float {
    let n = len(signal1)
    let mean1 = 0.0
    let mean2 = 0.0
    for x in signal1 {
        mean1 = mean1 + x
    }
    mean1 = mean1 / n
    for x in signal2 {
        mean2 = mean2 + x
    }
    mean2 = mean2 / n
    
    let numerator = 0.0
    for i in 0..(n - lag) {
        numerator = numerator + (signal1[i] - mean1) * (signal2[i + lag] - mean2)
    }
    return numerator
}

main {
    let signal1 = [1.0, 2.0, 3.0, 4.0, 5.0]
    let signal2 = [2.0, 4.0, 6.0, 8.0, 10.0]
    let lag = 0
    let cc = cross_correlation(signal1, signal2, lag)
    print(cc > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_coherence() {
        let src = r#"import std.core.*
fn coherence(signal1: list, signal2: list): float {
    let n = len(signal1)
    let sum1 = 0.0
    let sum2 = 0.0
    for x in signal1 {
        sum1 = sum1 + x
    }
    for x in signal2 {
        sum2 = sum2 + x
    }
    return (sum1 * sum2) / (n * n)
}

main {
    let signal1 = [1.0, 2.0, 3.0]
    let signal2 = [2.0, 4.0, 6.0]
    let coh = coherence(signal1, signal2)
    print(coh > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_transfer_function() {
        let src = r#"import std.core.*
fn transfer_function(input: list, output: list): float {
    let n = len(input)
    let sum_in = 0.0
    let sum_out = 0.0
    for x in input {
        sum_in = sum_in + x
    }
    for x in output {
        sum_out = sum_out + x
    }
    return sum_out / sum_in
}

main {
    let input = [1.0, 2.0, 3.0]
    let output = [2.0, 4.0, 6.0]
    let tf = transfer_function(input, output)
    print(tf)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_impulse_response() {
        let src = r#"import std.core.*
fn impulse_response(system: fn(float): float, n: int): list {
    let response = []
    for i in 0..n {
        if i == 0 {
            response.append(system(1.0))
        } else {
            response.append(system(0.0))
        }
    }
    return response
}

main {
    let system = fn(x: float): float { return x * 2 }
    let n = 5
    let ir = impulse_response(system, n)
    print(len(ir))
    print(ir[0])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "5");
        assert_eq!(lines[1], "2");
    }

    #[test]
    fn phase3_step_response() {
        let src = r#"import std.core.*
fn step_response(system: fn(float): float, n: int): list {
    let response = []
    for i in 0..n {
        response.append(system(1.0))
    }
    return response
}

main {
    let system = fn(x: float): float { return x * 2 }
    let n = 5
    let sr = step_response(system, n)
    print(len(sr))
    print(sr[0])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "5");
        assert_eq!(lines[1], "2");
    }

    #[test]
    fn phase3_frequency_response() {
        let src = r#"import std.core.*
fn frequency_response(system: fn(float): float, frequencies: list): list {
    let response = []
    for f in frequencies {
        response.append(system(f))
    }
    return response
}

main {
    let system = fn(f: float): float { return 1.0 / (1.0 + f) }
    let frequencies = [0.0, 1.0, 2.0, 3.0]
    let fr = frequency_response(system, frequencies)
    print(len(fr))
    print(fr[0])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "4");
        assert_eq!(lines[1], "1");
    }

    #[test]
    fn phase3_bode_plot() {
        let src = r#"import std.core.*
fn bode_plot(frequencies: list, magnitudes: list): list {
    let db_magnitudes = []
    for m in magnitudes {
        db_magnitudes.append(20 * log10(m))
    }
    return db_magnitudes
}

main {
    let frequencies = [1.0, 10.0, 100.0]
    let magnitudes = [1.0, 0.1, 0.01]
    let db = bode_plot(frequencies, magnitudes)
    print(len(db))
    print(db[0])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "0");
    }

    #[test]
    fn phase3_nyquist_plot() {
        let src = r#"import std.core.*
fn nyquist_plot(real_parts: list, imag_parts: list): list {
    let points = []
    for i in 0..len(real_parts) {
        points.append({"real": real_parts[i], "imag": imag_parts[i]})
    }
    return points
}

main {
    let real_parts = [1.0, 0.5, 0.0]
    let imag_parts = [0.0, 0.5, 1.0]
    let points = nyquist_plot(real_parts, imag_parts)
    print(len(points))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_root_locus() {
        let src = r#"import std.core.*
fn root_locus(numerator: list, denominator: list, gains: list): list {
    let roots = []
    for gain in gains {
        roots.append(gain)
    }
    return roots
}

main {
    let numerator = [1.0]
    let denominator = [1.0, 2.0, 1.0]
    let gains = [0.1, 1.0, 10.0]
    let r = root_locus(numerator, denominator, gains)
    print(len(r))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_pid_controller() {
        let src = r#"import std.core.*
struct PIDController {
    kp: float,
    ki: float,
    kd: float,
    integral: float,
    prev_error: float
}

impl PIDController {
    fn compute(setpoint: float, measurement: float, dt: float): float {
        let error = setpoint - measurement
        integral = integral + error * dt
        let derivative = (error - prev_error) / dt
        prev_error = error
        return kp * error + ki * integral + kd * derivative
    }
}

main {
    let pid = PIDController { kp: 1.0, ki: 0.1, kd: 0.01, integral: 0.0, prev_error: 0.0 }
    let output = pid.compute(10.0, 5.0, 0.1)
    print(output > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_state_space() {
        let src = r#"import std.core.*
struct StateSpaceSystem {
    a: list,
    b: list,
    c: list,
    d: list
}

impl StateSpaceSystem {
    fn simulate(x0: list, u: list, dt: float): list {
        let n = len(x0)
        let x_next = []
        for i in 0..n {
            let sum = 0.0
            for j in 0..n {
                sum = sum + a[i][j] * x0[j]
            }
            for j in 0..len(u) {
                sum = sum + b[i][j] * u[j]
            }
            x_next.append(x0[i] + sum * dt)
        }
        return x_next
    }
}

main {
    let sys = StateSpaceSystem {
        a: [[0.0, 1.0], [-1.0, -1.0]],
        b: [[0.0], [1.0]],
        c: [[1.0, 0.0]],
        d: [[0.0]]
    }
    let x0 = [1.0, 0.0]
    let u = [1.0]
    let x_next = sys.simulate(x0, u, 0.1)
    print(len(x_next))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_kalman_filter() {
        let src = r#"import std.core.*
struct KalmanFilter {
    x: list,
    p: list,
    q: float,
    r: float
}

impl KalmanFilter {
    fn predict() {
        p = p + q
    }
    
    fn update(measurement: float) {
        let k = p / (p + r)
        x[0] = x[0] + k * (measurement - x[0])
        p = (1 - k) * p
    }
}

main {
    let kf = KalmanFilter { x: [0.0], p: [1.0], q: 0.1, r: 0.1 }
    kf.predict()
    kf.update(1.0)
    print(kf.x[0] > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_extended_kalman_filter() {
        let src = r#"import std.core.*
struct ExtendedKalmanFilter {
    x: list,
    p: list
}

impl ExtendedKalmanFilter {
    fn predict(f: fn(list): list, q: float) {
        x = f(x)
        p = p + q
    }
    
    fn update(h: fn(list): float, measurement: float, r: float) {
        let z_pred = h(x)
        let y = measurement - z_pred
        let k = p / (p + r)
        x[0] = x[0] + k * y
        p = (1 - k) * p
    }
}

main {
    let ekf = ExtendedKalmanFilter { x: [0.0], p: [1.0] }
    ekf.predict(fn(x: list): list { return [x[0] + 1.0] }, 0.1)
    ekf.update(fn(x: list): float { return x[0] }, 2.0, 0.1)
    print(ekf.x[0] > 0)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn phase3_unscented_kalman_filter() {
        let src = r#"import std.core.*
struct UnscentedKalmanFilter {
    x: list,
    p: list,
    alpha: float,
    beta: float,
    kappa: float
}

impl UnscentedKalmanFilter {
    fn generate_sigma_points(): list {
        let n = len(x)
        let lambda = alpha * alpha * (n + kappa) - n
        let sigma_points = []
        sigma_points.append(x)
        for i in 0..n {
            let point = x.copy()
            point[i] = point[i] + sqrt((n + lambda) * p[i])
            sigma_points.append(point)
        }
        for i in 0..n {
            let point = x.copy()
            point[i] = point[i] - sqrt((n + lambda) * p[i])
            sigma_points.append(point)
        }
        return sigma_points
    }
}

main {
    let ukf = UnscentedKalmanFilter { x: [0.0], p: [1.0], alpha: 0.1, beta: 2.0, kappa: 0.0 }
    let sigma_points = ukf.generate_sigma_points()
    print(len(sigma_points))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_particle_filter() {
        let src = r#"import std.core.*
struct ParticleFilter {
    particles: list,
    weights: list
}

impl ParticleFilter {
    fn init(num_particles: int, initial_state: list) {
        particles = []
        weights = []
        for i in 0..num_particles {
            particles.append(initial_state.copy())
            weights.append(1.0 / num_particles)
        }
    }
    
    fn predict(state_transition: fn(list): list) {
        for i in 0..len(particles) {
            particles[i] = state_transition(particles[i])
        }
    }
    
    fn update(measurement: float, measurement_model: fn(list): float) {
        let total_weight = 0.0
        for i in 0..len(particles) {
            let likelihood = measurement_model(particles[i])
            weights[i] = weights[i] * likelihood
            total_weight = total_weight + weights[i]
        }
        for i in 0..len(weights) {
            weights[i] = weights[i] / total_weight
        }
    }
}

main {
    let pf = ParticleFilter { particles: [], weights: [] }
    pf.init(10, [0.0])
    print(len(pf.particles))
    print(len(pf.weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "10");
        assert_eq!(lines[1], "10");
    }

    #[test]
    fn phase3_interacting_multiple_model() {
        let src = r#"import std.core.*
struct IMMFilter {
    models: list,
    probabilities: list
}

impl IMMFilter {
    fn init(num_models: int) {
        models = []
        probabilities = []
        for i in 0..num_models {
            models.append({"x": [0.0], "p": [1.0]})
            probabilities.append(1.0 / num_models)
        }
    }
    
    fn interaction(transition_matrix: list) {
        let n = len(models)
        let new_probs = []
        for i in 0..n {
            let sum = 0.0
            for j in 0..n {
                sum = sum + transition_matrix[j][i] * probabilities[j]
            }
            new_probs.append(sum)
        }
        probabilities = new_probs
    }
}

main {
    let imm = IMMFilter { models: [], probabilities: [] }
    imm.init(3)
    print(len(imm.models))
    print(len(imm.probabilities))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "3");
    }

    #[test]
    fn phase3_gaussian_mixture_filter() {
        let src = r#"import std.core.*
struct GaussianMixtureFilter {
    means: list,
    covariances: list,
    weights: list
}

impl GaussianMixtureFilter {
    fn init(num_components: int, dimension: int) {
        means = []
        covariances = []
        weights = []
        for i in 0..num_components {
            let mean = []
            let cov = []
            for j in 0..dimension {
                mean.append(0.0)
                let cov_row = []
                for k in 0..dimension {
                    if j == k {
                        cov_row.append(1.0)
                    } else {
                        cov_row.append(0.0)
                    }
                }
                cov.append(cov_row)
            }
            means.append(mean)
            covariances.append(cov)
            weights.append(1.0 / num_components)
        }
    }
}

main {
    let gmf = GaussianMixtureFilter { means: [], covariances: [], weights: [] }
    gmf.init(3, 2)
    print(len(gmf.means))
    print(len(gmf.weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "3");
        assert_eq!(lines[1], "3");
    }

    #[test]
    fn phase3_multiple_hypothesis_tracking() {
        let src = r#"import std.core.*
struct MHTFilter {
    hypotheses: list
}

impl MHTFilter {
    fn init() {
        hypotheses = []
    }
    
    fn add_hypothesis(state: list, score: float) {
        hypotheses.append({"state": state, "score": score})
    }
    
    fn prune(min_score: float) {
        let new_hypotheses = []
        for h in hypotheses {
            if h["score"] >= min_score {
                new_hypotheses.append(h)
            }
        }
        hypotheses = new_hypotheses
    }
    
    fn get_best(): dict {
        if len(hypotheses) == 0 {
            return {}
        }
        let best = hypotheses[0]
        for h in hypotheses {
            if h["score"] > best["score"] {
                best = h
            }
        }
        return best
    }
}

main {
    let mht = MHTFilter { hypotheses: [] }
    mht.init()
    mht.add_hypothesis([1.0, 2.0], 0.5)
    mht.add_hypothesis([3.0, 4.0], 0.8)
    mht.add_hypothesis([5.0, 6.0], 0.3)
    mht.prune(0.4)
    let best = mht.get_best()
    print(best["score"])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "0.8");
    }

    #[test]
    fn phase3_joint_probabilistic_data_association() {
        let src = r#"import std.core.*
struct JPDAFilter {
    tracks: list,
    measurements: list
}

impl JPDAFilter {
    fn init() {
        tracks = []
        measurements = []
    }
    
    fn add_track(track_id: int, state: list) {
        tracks.append({"id": track_id, "state": state})
    }
    
    fn add_measurement(measurement_id: int, position: list) {
        measurements.append({"id": measurement_id, "position": position})
    }
    
    fn associate(): list {
        let associations = []
        for track in tracks {
            let best_measurement = {}
            let best_distance = 999999.0
            for measurement in measurements {
                let distance = 0.0
                for i in 0..len(track["state"]) {
                    distance = distance + (track["state"][i] - measurement["position"][i]) ** 2
                }
                distance = sqrt(distance)
                if distance < best_distance {
                    best_distance = distance
                    best_measurement = measurement
                }
            }
            if len(best_measurement) > 0 {
                associations.append({"track": track["id"], "measurement": best_measurement["id"]})
            }
        }
        return associations
    }
}

main {
    let jpda = JPDAFilter { tracks: [], measurements: [] }
    jpda.init()
    jpda.add_track(1, [1.0, 2.0])
    jpda.add_track(2, [3.0, 4.0])
    jpda.add_measurement(1, [1.1, 2.1])
    jpda.add_measurement(2, [3.1, 4.1])
    let associations = jpda.associate()
    print(len(associations))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_global_nearest_neighbor() {
        let src = r#"import std.core.*
struct GNNFilter {
    tracks: list,
    measurements: list
}

impl GNNFilter {
    fn init() {
        tracks = []
        measurements = []
    }
    
    fn add_track(track_id: int, state: list) {
        tracks.append({"id": track_id, "state": state})
    }
    
    fn add_measurement(measurement_id: int, position: list) {
        measurements.append({"id": measurement_id, "position": position})
    }
    
    fn associate(): list {
        let associations = []
        let used_measurements = []
        for track in tracks {
            let best_measurement = {}
            let best_distance = 999999.0
            for measurement in measurements {
                let already_used = false
                for used in used_measurements {
                    if used == measurement["id"] {
                        already_used = true
                    }
                }
                if already_used {
                    continue
                }
                let distance = 0.0
                for i in 0..len(track["state"]) {
                    distance = distance + (track["state"][i] - measurement["position"][i]) ** 2
                }
                distance = sqrt(distance)
                if distance < best_distance {
                    best_distance = distance
                    best_measurement = measurement
                }
            }
            if len(best_measurement) > 0 {
                associations.append({"track": track["id"], "measurement": best_measurement["id"]})
                used_measurements.append(best_measurement["id"])
            }
        }
        return associations
    }
}

main {
    let gnn = GNNFilter { tracks: [], measurements: [] }
    gnn.init()
    gnn.add_track(1, [1.0, 2.0])
    gnn.add_track(2, [3.0, 4.0])
    gnn.add_measurement(1, [1.1, 2.1])
    gnn.add_measurement(2, [3.1, 4.1])
    let associations = gnn.associate()
    print(len(associations))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_auction_algorithm() {
        let src = r#"import std.core.*
fn auction_assignment(cost_matrix: list): list {
    let n = len(cost_matrix)
    let assignment = []
    let prices = []
    for i in 0..n {
        assignment.append(-1)
        prices.append(0.0)
    }
    
    let epsilon = 0.1
    let max_iterations = 100
    let iteration = 0
    
    while iteration < max_iterations {
        let all_assigned = true
        for i in 0..n {
            if assignment[i] == -1 {
                all_assigned = false
                let best_j = -1
                let best_value = -999999.0
                for j in 0..n {
                    let value = cost_matrix[i][j] - prices[j]
                    if value > best_value {
                        best_value = value
                        best_j = j
                    }
                }
                if best_j != -1 {
                    assignment[i] = best_j
                    prices[best_j] = prices[best_j] + epsilon
                }
            }
        }
        if all_assigned {
            break
        }
        iteration = iteration + 1
    }
    
    return assignment
}

main {
    let cost_matrix = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]
    let assignment = auction_assignment(cost_matrix)
    print(len(assignment))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_hungarian_algorithm() {
        let src = r#"import std.core.*
fn hungarian_assignment(cost_matrix: list): list {
    let n = len(cost_matrix)
    let assignment = []
    for i in 0..n {
        assignment.append(i)
    }
    return assignment
}

main {
    let cost_matrix = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]
    let assignment = hungarian_assignment(cost_matrix)
    print(len(assignment))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_munkres_algorithm() {
        let src = r#"import std.core.*
fn munkres_assignment(cost_matrix: list): list {
    let n = len(cost_matrix)
    let assignment = []
    for i in 0..n {
        assignment.append(i)
    }
    return assignment
}

main {
    let cost_matrix = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]
    let assignment = munkres_assignment(cost_matrix)
    print(len(assignment))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_branch_and_bound() {
        let src = r#"import std.core.*
fn branch_and_bound_tsp(distance_matrix: list): list {
    let n = len(distance_matrix)
    let tour = []
    for i in 0..n {
        tour.append(i)
    }
    return tour
}

main {
    let distance_matrix = [[0, 10, 15], [10, 0, 20], [15, 20, 0]]
    let tour = branch_and_bound_tsp(distance_matrix)
    print(len(tour))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_cutting_plane() {
        let src = r#"import std.core.*
fn cutting_plane_method(objective: list, constraints: list): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let constraints = [[1.0, 1.0, 1.0]]
    let solution = cutting_plane_method(objective, constraints)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_benders_decomposition() {
        let src = r#"import std.core.*
fn benders_decomposition(master_vars: int, subproblem_vars: int): list {
    let solution = []
    for i in 0..master_vars {
        solution.append(0.0)
    }
    return solution
}

main {
    let master_vars = 3
    let subproblem_vars = 2
    let solution = benders_decomposition(master_vars, subproblem_vars)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_dantzig_wolfe() {
        let src = r#"import std.core.*
fn dantzig_wolfe_decomposition(num_blocks: int, vars_per_block: int): list {
    let solution = []
    for i in 0..(num_blocks * vars_per_block) {
        solution.append(0.0)
    }
    return solution
}

main {
    let num_blocks = 3
    let vars_per_block = 2
    let solution = dantzig_wolfe_decomposition(num_blocks, vars_per_block)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "6");
    }

    #[test]
    fn phase3_lagrangian_relaxation() {
        let src = r#"import std.core.*
fn lagrangian_relaxation(objective: list, constraints: list, lambda: list): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let constraints = [[1.0, 1.0, 1.0]]
    let lambda = [0.5]
    let solution = lagrangian_relaxation(objective, constraints, lambda)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_column_generation() {
        let src = r#"import std.core.*
fn column_generation(master_problem: dict, pricing_problem: dict): list {
    let solution = []
    for i in 0..5 {
        solution.append(0.0)
    }
    return solution
}

main {
    let master_problem = {"objective": [1.0, 2.0], "constraints": [[1.0, 1.0]]}
    let pricing_problem = {"cost": [1.0, 1.0]}
    let solution = column_generation(master_problem, pricing_problem)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_row_generation() {
        let src = r#"import std.core.*
fn row_generation(master_problem: dict, pricing_problem: dict): list {
    let solution = []
    for i in 0..5 {
        solution.append(0.0)
    }
    return solution
}

main {
    let master_problem = {"objective": [1.0, 2.0], "constraints": [[1.0, 1.0]]}
    let pricing_problem = {"cost": [1.0, 1.0]}
    let solution = row_generation(master_problem, pricing_problem)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_goal_programming() {
        let src = r#"import std.core.*
fn goal_programming(objectives: list, goals: list, priorities: list): list {
    let n = len(objectives[0])
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objectives = [[1.0, 2.0], [3.0, 4.0]]
    let goals = [10.0, 20.0]
    let priorities = [1, 2]
    let solution = goal_programming(objectives, goals, priorities)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_fuzzy_linear_programming() {
        let src = r#"import std.core.*
fn fuzzy_linear_programming(objective: list, constraints: list, fuzzy_sets: list): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let constraints = [[1.0, 1.0, 1.0]]
    let fuzzy_sets = [{"type": "triangular", "params": [0.5, 1.0, 1.5]}]
    let solution = fuzzy_linear_programming(objective, constraints, fuzzy_sets)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_stochastic_programming() {
        let src = r#"import std.core.*
fn stochastic_programming(objective: list, scenarios: list, probabilities: list): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let scenarios = [[[1.0, 1.0, 1.0]], [[2.0, 2.0, 2.0]]]
    let probabilities = [0.5, 0.5]
    let solution = stochastic_programming(objective, scenarios, probabilities)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_robust_optimization() {
        let src = r#"import std.core.*
fn robust_optimization(objective: list, uncertainty_set: dict): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let uncertainty_set = {"type": "box", "bounds": [[0.5, 1.5], [1.5, 2.5], [2.5, 3.5]]}
    let solution = robust_optimization(objective, uncertainty_set)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_distributionally_robust() {
        let src = r#"import std.core.*
fn distributionally_robust_optimization(objective: list, ambiguity_set: dict): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let ambiguity_set = {"type": "wasserstein", "radius": 0.1}
    let solution = distributionally_robust_optimization(objective, ambiguity_set)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_chance_constrained() {
        let src = r#"import std.core.*
fn chance_constrained_programming(objective: list, constraints: list, probability_level: float): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let constraints = [[1.0, 1.0, 1.0]]
    let probability_level = 0.95
    let solution = chance_constrained_programming(objective, constraints, probability_level)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_semidefinite_programming() {
        let src = r#"import std.core.*
fn semidefinite_programming(objective: list, constraints: list): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let constraints = [[1.0, 1.0, 1.0]]
    let solution = semidefinite_programming(objective, constraints)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_second_order_cone() {
        let src = r#"import std.core.*
fn second_order_cone_programming(objective: list, constraints: list): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let constraints = [[1.0, 1.0, 1.0]]
    let solution = second_order_cone_programming(objective, constraints)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_geometric_programming() {
        let src = r#"import std.core.*
fn geometric_programming(objective: list, constraints: list): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(1.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let constraints = [[1.0, 1.0, 1.0]]
    let solution = geometric_programming(objective, constraints)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_signomial_programming() {
        let src = r#"import std.core.*
fn signomial_programming(objective: list, constraints: list): list {
    let n = len(objective)
    let solution = []
    for i in 0..n {
        solution.append(1.0)
    }
    return solution
}

main {
    let objective = [1.0, 2.0, 3.0]
    let constraints = [[1.0, 1.0, 1.0]]
    let solution = signomial_programming(objective, constraints)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_complementarity_problem() {
        let src = r#"import std.core.*
fn linear_complementarity_problem(m: list, q: list): list {
    let n = len(q)
    let solution = []
    for i in 0..n {
        solution.append(0.0)
    }
    return solution
}

main {
    let m = [[1.0, 0.0], [0.0, 1.0]]
    let q = [1.0, 2.0]
    let solution = linear_complementarity_problem(m, q)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_variational_inequality() {
        let src = r#"import std.core.*
fn variational_inequality(f: fn(list): list, constraint_set: dict): list {
    let solution = [0.0, 0.0]
    return solution
}

main {
    let f = fn(x: list): list { return [x[0], x[1]] }
    let constraint_set = {"type": "box", "lower": [0.0, 0.0], "upper": [1.0, 1.0]}
    let solution = variational_inequality(f, constraint_set)
    print(len(solution))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_nash_equilibrium() {
        let src = r#"import std.core.*
fn nash_equilibrium(payoff_matrices: list): list {
    let n = len(payoff_matrices)
    let equilibrium = []
    for i in 0..n {
        equilibrium.append(1.0 / n)
    }
    return equilibrium
}

main {
    let payoff_matrices = [[[3, 1], [0, 2]], [[2, 0], [1, 3]]]
    let equilibrium = nash_equilibrium(payoff_matrices)
    print(len(equilibrium))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_correlated_equilibrium() {
        let src = r#"import std.core.*
fn correlated_equilibrium(payoff_matrices: list): list {
    let n = len(payoff_matrices)
    let m = len(payoff_matrices[0])
    let distribution = []
    for i in 0..n {
        let row = []
        for j in 0..m {
            row.append(1.0 / (n * m))
        }
        distribution.append(row)
    }
    return distribution
}

main {
    let payoff_matrices = [[[3, 1], [0, 2]], [[2, 0], [1, 3]]]
    let distribution = correlated_equilibrium(payoff_matrices)
    print(len(distribution))
    print(len(distribution[0]))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "2");
        assert_eq!(lines[1], "2");
    }

    #[test]
    fn phase3_stackelberg_equilibrium() {
        let src = r#"import std.core.*
fn stackelberg_equilibrium(leader_payoff: list, follower_payoff: list): list {
    let n = len(leader_payoff)
    let equilibrium = []
    for i in 0..n {
        equilibrium.append(1.0 / n)
    }
    return equilibrium
}

main {
    let leader_payoff = [[3, 1], [0, 2]]
    let follower_payoff = [[2, 0], [1, 3]]
    let equilibrium = stackelberg_equilibrium(leader_payoff, follower_payoff)
    print(len(equilibrium))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_bayesian_equilibrium() {
        let src = r#"import std.core.*
fn bayesian_nash_equilibrium(type_spaces: list, payoff_functions: list, beliefs: list): list {
    let n = len(type_spaces)
    let equilibrium = []
    for i in 0..n {
        equilibrium.append(1.0 / n)
    }
    return equilibrium
}

main {
    let type_spaces = [[1, 2], [1, 2]]
    let payoff_functions = [[[3, 1], [0, 2]], [[2, 0], [1, 3]]]
    let beliefs = [[0.5, 0.5], [0.5, 0.5]]
    let equilibrium = bayesian_nash_equilibrium(type_spaces, payoff_functions, beliefs)
    print(len(equilibrium))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_evolutionary_stable_strategy() {
        let src = r#"import std.core.*
fn evolutionary_stable_strategy(payoff_matrix: list): list {
    let n = len(payoff_matrix)
    let ess = []
    for i in 0..n {
        ess.append(1.0 / n)
    }
    return ess
}

main {
    let payoff_matrix = [[3, 1], [0, 2]]
    let ess = evolutionary_stable_strategy(payoff_matrix)
    print(len(ess))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_replicator_dynamics() {
        let src = r#"import std.core.*
fn replicator_dynamics(population: list, payoff_matrix: list, dt: float): list {
    let n = len(population)
    let new_population = []
    let fitness = []
    for i in 0..n {
        let f = 0.0
        for j in 0..n {
            f = f + payoff_matrix[i][j] * population[j]
        }
        fitness.append(f)
    }
    let avg_fitness = 0.0
    for i in 0..n {
        avg_fitness = avg_fitness + population[i] * fitness[i]
    }
    for i in 0..n {
        new_population.append(population[i] + dt * population[i] * (fitness[i] - avg_fitness))
    }
    return new_population
}

main {
    let population = [0.5, 0.5]
    let payoff_matrix = [[3, 1], [0, 2]]
    let dt = 0.1
    let new_pop = replicator_dynamics(population, payoff_matrix, dt)
    print(len(new_pop))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_fictitious_play() {
        let src = r#"import std.core.*
fn fictitious_play(payoff_matrix: list, num_iterations: int): list {
    let n = len(payoff_matrix)
    let beliefs = []
    for i in 0..n {
        beliefs.append(1.0 / n)
    }
    return beliefs
}

main {
    let payoff_matrix = [[3, 1], [0, 2]]
    let num_iterations = 100
    let beliefs = fictitious_play(payoff_matrix, num_iterations)
    print(len(beliefs))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_best_response_dynamics() {
        let src = r#"import std.core.*
fn best_response_dynamics(payoff_matrix: list, initial_strategy: list, num_iterations: int): list {
    let n = len(payoff_matrix)
    let strategy = initial_strategy.copy()
    for iter in 0..num_iterations {
        let best_response = 0
        let best_payoff = -999999.0
        for i in 0..n {
            let payoff = 0.0
            for j in 0..n {
                payoff = payoff + payoff_matrix[i][j] * strategy[j]
            }
            if payoff > best_payoff {
                best_payoff = payoff
                best_response = i
            }
        }
        strategy = []
        for i in 0..n {
            if i == best_response {
                strategy.append(1.0)
            } else {
                strategy.append(0.0)
            }
        }
    }
    return strategy
}

main {
    let payoff_matrix = [[3, 1], [0, 2]]
    let initial_strategy = [0.5, 0.5]
    let num_iterations = 10
    let strategy = best_response_dynamics(payoff_matrix, initial_strategy, num_iterations)
    print(len(strategy))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_logit_dynamics() {
        let src = r#"import std.core.*
fn logit_dynamics(payoff_matrix: list, temperature: float, initial_strategy: list): list {
    let n = len(payoff_matrix)
    let strategy = []
    for i in 0..n {
        let payoff = 0.0
        for j in 0..n {
            payoff = payoff + payoff_matrix[i][j] * initial_strategy[j]
        }
        strategy.append(exp(payoff / temperature))
    }
    let sum = 0.0
    for s in strategy {
        sum = sum + s
    }
    for i in 0..n {
        strategy[i] = strategy[i] / sum
    }
    return strategy
}

main {
    let payoff_matrix = [[3, 1], [0, 2]]
    let temperature = 1.0
    let initial_strategy = [0.5, 0.5]
    let strategy = logit_dynamics(payoff_matrix, temperature, initial_strategy)
    print(len(strategy))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_smooth_fictitious_play() {
        let src = r#"import std.core.*
fn smooth_fictitious_play(payoff_matrix: list, learning_rate: float, num_iterations: int): list {
    let n = len(payoff_matrix)
    let strategy = []
    for i in 0..n {
        strategy.append(1.0 / n)
    }
    return strategy
}

main {
    let payoff_matrix = [[3, 1], [0, 2]]
    let learning_rate = 0.1
    let num_iterations = 100
    let strategy = smooth_fictitious_play(payoff_matrix, learning_rate, num_iterations)
    print(len(strategy))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_regret_matching() {
        let src = r#"import std.core.*
fn regret_matching(payoff_matrix: list, num_iterations: int): list {
    let n = len(payoff_matrix)
    let strategy = []
    for i in 0..n {
        strategy.append(1.0 / n)
    }
    return strategy
}

main {
    let payoff_matrix = [[3, 1], [0, 2]]
    let num_iterations = 100
    let strategy = regret_matching(payoff_matrix, num_iterations)
    print(len(strategy))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_hedge_algorithm() {
        let src = r#"import std.core.*
fn hedge_algorithm(experts: list, learning_rate: float, num_rounds: int): list {
    let n = len(experts)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let experts = [0.8, 0.6, 0.9]
    let learning_rate = 0.1
    let num_rounds = 100
    let weights = hedge_algorithm(experts, learning_rate, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_multiplicative_weights() {
        let src = r#"import std.core.*
fn multiplicative_weights_update(weights: list, losses: list, learning_rate: float): list {
    let n = len(weights)
    let new_weights = []
    for i in 0..n {
        new_weights.append(weights[i] * exp(-learning_rate * losses[i]))
    }
    let sum = 0.0
    for w in new_weights {
        sum = sum + w
    }
    for i in 0..n {
        new_weights[i] = new_weights[i] / sum
    }
    return new_weights
}

main {
    let weights = [0.33, 0.33, 0.34]
    let losses = [0.1, 0.2, 0.05]
    let learning_rate = 0.1
    let new_weights = multiplicative_weights_update(weights, losses, learning_rate)
    print(len(new_weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_exponential_weights() {
        let src = r#"import std.core.*
fn exponential_weights_algorithm(decisions: list, losses: list, learning_rate: float): list {
    let n = len(decisions)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let decisions = [1, 2, 3]
    let losses = [0.1, 0.2, 0.05]
    let learning_rate = 0.1
    let weights = exponential_weights_algorithm(decisions, losses, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_follow_the_leader() {
        let src = r#"import std.core.*
fn follow_the_leader(history: list): int {
    let counts = {}
    for action in history {
        counts[action] = counts.get(action, 0) + 1
    }
    let best_action = 0
    let best_count = 0
    for action in counts.keys() {
        if counts[action] > best_count {
            best_count = counts[action]
            best_action = action
        }
    }
    return best_action
}

main {
    let history = [1, 2, 1, 3, 1, 2]
    let action = follow_the_leader(history)
    print(action)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "1");
    }

    #[test]
    fn phase3_follow_the_regularized_leader() {
        let src = r#"import std.core.*
fn follow_the_regularized_leader(history: list, regularizer: float): int {
    let counts = {}
    for action in history {
        counts[action] = counts.get(action, 0) + 1
    }
    let best_action = 0
    let best_score = -999999.0
    for action in counts.keys() {
        let score = counts[action] - regularizer * action
        if score > best_score {
            best_score = score
            best_action = action
        }
    }
    return best_action
}

main {
    let history = [1, 2, 1, 3, 1, 2]
    let regularizer = 0.1
    let action = follow_the_regularized_leader(history, regularizer)
    print(action)
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "1");
    }

    #[test]
    fn phase3_online_gradient_descent() {
        let src = r#"import std.core.*
fn online_gradient_descent(initial_point: list, gradients: list, learning_rate: float): list {
    let point = initial_point.copy()
    for grad in gradients {
        for i in 0..len(point) {
            point[i] = point[i] - learning_rate * grad[i]
        }
    }
    return point
}

main {
    let initial_point = [0.0, 0.0]
    let gradients = [[1.0, 2.0], [2.0, 1.0]]
    let learning_rate = 0.1
    let point = online_gradient_descent(initial_point, gradients, learning_rate)
    print(len(point))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_online_newton_method() {
        let src = r#"import std.core.*
fn online_newton_method(initial_point: list, gradients: list, hessians: list, learning_rate: float): list {
    let point = initial_point.copy()
    for i in 0..len(gradients) {
        for j in 0..len(point) {
            point[j] = point[j] - learning_rate * gradients[i][j]
        }
    }
    return point
}

main {
    let initial_point = [0.0, 0.0]
    let gradients = [[1.0, 2.0], [2.0, 1.0]]
    let hessians = [[[1.0, 0.0], [0.0, 1.0]], [[1.0, 0.0], [0.0, 1.0]]]
    let learning_rate = 0.1
    let point = online_newton_method(initial_point, gradients, hessians, learning_rate)
    print(len(point))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "2");
    }

    #[test]
    fn phase3_exponential_gradient() {
        let src = r#"import std.core.*
fn exponential_gradient(initial_weights: list, gradients: list, learning_rate: float): list {
    let weights = initial_weights.copy()
    for grad in gradients {
        for i in 0..len(weights) {
            weights[i] = weights[i] * exp(learning_rate * grad[i])
        }
        let sum = 0.0
        for w in weights {
            sum = sum + w
        }
        for i in 0..len(weights) {
            weights[i] = weights[i] / sum
        }
    }
    return weights
}

main {
    let initial_weights = [0.33, 0.33, 0.34]
    let gradients = [[0.1, -0.2, 0.05], [-0.1, 0.2, -0.05]]
    let learning_rate = 0.1
    let weights = exponential_gradient(initial_weights, gradients, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_normal_hedge() {
        let src = r#"import std.core.*
fn normal_hedge(initial_weights: list, losses: list, learning_rate: float): list {
    let weights = initial_weights.copy()
    for loss in losses {
        for i in 0..len(weights) {
            weights[i] = weights[i] * exp(-learning_rate * loss[i])
        }
        let sum = 0.0
        for w in weights {
            sum = sum + w
        }
        for i in 0..len(weights) {
            weights[i] = weights[i] / sum
        }
    }
    return weights
}

main {
    let initial_weights = [0.33, 0.33, 0.34]
    let losses = [[0.1, 0.2, 0.05], [0.2, 0.1, 0.15]]
    let learning_rate = 0.1
    let weights = normal_hedge(initial_weights, losses, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_adaptive_hedge() {
        let src = r#"import std.core.*
fn adaptive_hedge(initial_weights: list, losses: list): list {
    let weights = initial_weights.copy()
    for loss in losses {
        let min_loss = 999999.0
        for l in loss {
            if l < min_loss {
                min_loss = l
            }
        }
        for i in 0..len(weights) {
            weights[i] = weights[i] * exp(-(loss[i] - min_loss))
        }
        let sum = 0.0
        for w in weights {
            sum = sum + w
        }
        for i in 0..len(weights) {
            weights[i] = weights[i] / sum
        }
    }
    return weights
}

main {
    let initial_weights = [0.33, 0.33, 0.34]
    let losses = [[0.1, 0.2, 0.05], [0.2, 0.1, 0.15]]
    let weights = adaptive_hedge(initial_weights, losses)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_lazy_hedge() {
        let src = r#"import std.core.*
fn lazy_hedge(initial_weights: list, losses: list): list {
    let weights = initial_weights.copy()
    for loss in losses {
        for i in 0..len(weights) {
            weights[i] = weights[i] * exp(-loss[i])
        }
        let sum = 0.0
        for w in weights {
            sum = sum + w
        }
        for i in 0..len(weights) {
            weights[i] = weights[i] / sum
        }
    }
    return weights
}

main {
    let initial_weights = [0.33, 0.33, 0.34]
    let losses = [[0.1, 0.2, 0.05], [0.2, 0.1, 0.15]]
    let weights = lazy_hedge(initial_weights, losses)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_boosting_by_majority() {
        let src = r#"import std.core.*
fn boosting_by_majority(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = boosting_by_majority(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_brown_boost() {
        let src = r#"import std.core.*
fn brown_boosting(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = brown_boosting(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_total_boost() {
        let src = r#"import std.core.*
fn total_boosting(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = total_boosting(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_lp_boost() {
        let src = r#"import std.core.*
fn lp_boosting(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = lp_boosting(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_smooth_boost() {
        let src = r#"import std.core.*
fn smooth_boost(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = smooth_boost(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_gentle_boost() {
        let src = r#"import std.core.*
fn gentle_boost(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = gentle_boost(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_logit_boost() {
        let src = r#"import std.core.*
fn logit_boost(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = logit_boost(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_madaboost() {
        let src = r#"import std.core.*
fn madaboost(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = madaboost(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_erc() {
        let src = r#"import std.core.*
fn erc_boosting(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = erc_boosting(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_filter_boost() {
        let src = r#"import std.core.*
fn filter_boosting(weak_learners: list, num_rounds: int): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let weights = filter_boosting(weak_learners, num_rounds)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_stochastic_gradient_boost() {
        let src = r#"import std.core.*
fn stochastic_gradient_boost(weak_learners: list, num_rounds: int, subsample_rate: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let subsample_rate = 0.8
    let weights = stochastic_gradient_boost(weak_learners, num_rounds, subsample_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_xgboost_style() {
        let src = r#"import std.core.*
fn xgboost_style(weak_learners: list, num_rounds: int, learning_rate: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let learning_rate = 0.1
    let weights = xgboost_style(weak_learners, num_rounds, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_lightgbm_style() {
        let src = r#"import std.core.*
fn lightgbm_style(weak_learners: list, num_rounds: int, learning_rate: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let learning_rate = 0.1
    let weights = lightgbm_style(weak_learners, num_rounds, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_catboost_style() {
        let src = r#"import std.core.*
fn catboost_style(weak_learners: list, num_rounds: int, learning_rate: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let learning_rate = 0.1
    let weights = catboost_style(weak_learners, num_rounds, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_random_forest_style() {
        let src = r#"import std.core.*
fn random_forest_style(weak_learners: list, num_trees: int, feature_subset: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_trees = 10
    let feature_subset = 0.8
    let weights = random_forest_style(weak_learners, num_trees, feature_subset)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_extra_trees_style() {
        let src = r#"import std.core.*
fn extra_trees_style(weak_learners: list, num_trees: int, feature_subset: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_trees = 10
    let feature_subset = 0.8
    let weights = extra_trees_style(weak_learners, num_trees, feature_subset)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_gradient_boosting_regression() {
        let src = r#"import std.core.*
fn gradient_boosting_regression(weak_learners: list, num_rounds: int, learning_rate: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let learning_rate = 0.1
    let weights = gradient_boosting_regression(weak_learners, num_rounds, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_gradient_boosting_classification() {
        let src = r#"import std.core.*
fn gradient_boosting_classification(weak_learners: list, num_rounds: int, learning_rate: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let learning_rate = 0.1
    let weights = gradient_boosting_classification(weak_learners, num_rounds, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_histogram_gradient_boosting() {
        let src = r#"import std.core.*
fn histogram_gradient_boosting(weak_learners: list, num_rounds: int, learning_rate: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_rounds = 10
    let learning_rate = 0.1
    let weights = histogram_gradient_boosting(weak_learners, num_rounds, learning_rate)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_quantile_regression_forest() {
        let src = r#"import std.core.*
fn quantile_regression_forest(weak_learners: list, num_trees: int, quantile: float): list {
    let n = len(weak_learners)
    let weights = []
    for i in 0..n {
        weights.append(1.0 / n)
    }
    return weights
}

main {
    let weak_learners = [0.6, 0.7, 0.8]
    let num_trees = 10
    let quantile = 0.5
    let weights = quantile_regression_forest(weak_learners, num_trees, quantile)
    print(len(weights))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "3");
    }

    #[test]
    fn phase3_isotonic_regression() {
        let src = r#"import std.core.*
fn isotonic_regression(values: list): list {
    let n = len(values)
    let result = values.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let values = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = isotonic_regression(values)
    print(len(result))
    print(result[2])
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines[0], "5");
        assert_eq!(lines[1], "3");
    }

    #[test]
    fn phase3_pool_adjacent_violators() {
        let src = r#"import std.core.*
fn pool_adjacent_violators(values: list, weights: list): list {
    let n = len(values)
    let result = values.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            let weighted_avg = (result[i - 1] * weights[i - 1] + result[i] * weights[i]) / (weights[i - 1] + weights[i])
            result[i - 1] = weighted_avg
            result[i] = weighted_avg
        }
    }
    return result
}

main {
    let values = [1.0, 3.0, 2.0, 4.0, 3.0]
    let weights = [1.0, 1.0, 1.0, 1.0, 1.0]
    let result = pool_adjacent_violators(values, weights)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_monotone_regression() {
        let src = r#"import std.core.*
fn monotone_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = monotone_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_shape_constrained_regression() {
        let src = r#"import std.core.*
fn shape_constrained_regression(x: list, y: list, constraint: str): list {
    let n = len(x)
    let result = y.copy()
    if constraint == "increasing" {
        for i in 1..n {
            if result[i] < result[i - 1] {
                result[i] = result[i - 1]
            }
        }
    } else if constraint == "decreasing" {
        for i in 1..n {
            if result[i] > result[i - 1] {
                result[i] = result[i - 1]
            }
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = shape_constrained_regression(x, y, "increasing")
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_convex_regression() {
        let src = r#"import std.core.*
fn convex_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 2..n {
        let slope1 = (result[i - 1] - result[i - 2]) / (x[i - 1] - x[i - 2])
        let slope2 = (result[i] - result[i - 1]) / (x[i] - x[i - 1])
        if slope2 < slope1 {
            result[i] = result[i - 1] + slope1 * (x[i] - x[i - 1])
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = convex_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_concave_regression() {
        let src = r#"import std.core.*
fn concave_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 2..n {
        let slope1 = (result[i - 1] - result[i - 2]) / (x[i - 1] - x[i - 2])
        let slope2 = (result[i] - result[i - 1]) / (x[i] - x[i - 1])
        if slope2 > slope1 {
            result[i] = result[i - 1] + slope1 * (x[i] - x[i - 1])
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = concave_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_unimodal_regression() {
        let src = r#"import std.core.*
fn unimodal_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    let mode_idx = 0
    let max_val = result[0]
    for i in 1..n {
        if result[i] > max_val {
            max_val = result[i]
            mode_idx = i
        }
    }
    for i in 0..mode_idx {
        if i > 0 && result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    for i in (mode_idx + 1)..n {
        if result[i] > result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 5.0, 4.0, 2.0]
    let result = unimodal_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_log_concave_regression() {
        let src = r#"import std.core.*
fn log_concave_regression(x: list, y: list): list {
    let n = len(x)
    let log_y = []
    for val in y {
        log_y.append(log(val + 1))
    }
    let result = log_y.copy()
    for i in 2..n {
        let slope1 = (result[i - 1] - result[i - 2]) / (x[i - 1] - x[i - 2])
        let slope2 = (result[i] - result[i - 1]) / (x[i] - x[i - 1])
        if slope2 < slope1 {
            result[i] = result[i - 1] + slope1 * (x[i] - x[i - 1])
        }
    }
    let final_result = []
    for val in result {
        final_result.append(exp(val) - 1)
    }
    return final_result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = log_concave_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_log_convex_regression() {
        let src = r#"import std.core.*
fn log_convex_regression(x: list, y: list): list {
    let n = len(x)
    let log_y = []
    for val in y {
        log_y.append(log(val + 1))
    }
    let result = log_y.copy()
    for i in 2..n {
        let slope1 = (result[i - 1] - result[i - 2]) / (x[i - 1] - x[i - 2])
        let slope2 = (result[i] - result[i - 1]) / (x[i] - x[i - 1])
        if slope2 > slope1 {
            result[i] = result[i - 1] + slope1 * (x[i] - x[i - 1])
        }
    }
    let final_result = []
    for val in result {
        final_result.append(exp(val) - 1)
    }
    return final_result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = log_convex_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_s_shape_regression() {
        let src = r#"import std.core.*
fn s_shape_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    let inflection_idx = n / 2
    for i in 1..inflection_idx {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    for i in (inflection_idx + 1)..n {
        if result[i] > result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = s_shape_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_bateman_regression() {
        let src = r#"import std.core.*
fn bateman_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = bateman_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_mitscherlich_regression() {
        let src = r#"import std.core.*
fn mitscherlich_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = mitscherlich_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_gompertz_regression() {
        let src = r#"import std.core.*
fn gompertz_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = gompertz_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_richards_regression() {
        let src = r#"import std.core.*
fn richards_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = richards_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_weibull_regression() {
        let src = r#"import std.core.*
fn weibull_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = weibull_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_logistic_regression() {
        let src = r#"import std.core.*
fn logistic_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = logistic_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_probit_regression() {
        let src = r#"import std.core.*
fn probit_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = probit_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_cloglog_regression() {
        let src = r#"import std.core.*
fn cloglog_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = cloglog_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_cauchit_regression() {
        let src = r#"import std.core.*
fn cauchit_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = cauchit_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_loglog_regression() {
        let src = r#"import std.core.*
fn loglog_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = loglog_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_complementary_log_log() {
        let src = r#"import std.core.*
fn complementary_log_log_regression(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = complementary_log_log_regression(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_generalized_linear_model() {
        let src = r#"import std.core.*
fn generalized_linear_model(x: list, y: list, family: str): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = generalized_linear_model(x, y, "gaussian")
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_generalized_additive_model() {
        let src = r#"import std.core.*
fn generalized_additive_model(x: list, y: list, smooth_terms: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let smooth_terms = ["s(x1)", "s(x2)"]
    let result = generalized_additive_model(x, y, smooth_terms)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_generalized_estimating_equations() {
        let src = r#"import std.core.*
fn generalized_estimating_equations(x: list, y: list, groups: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let groups = [1, 1, 2, 2, 3]
    let result = generalized_estimating_equations(x, y, groups)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_mixed_effects_model() {
        let src = r#"import std.core.*
fn mixed_effects_model(x: list, y: list, random_effects: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let random_effects = [1, 1, 2, 2, 3]
    let result = mixed_effects_model(x, y, random_effects)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_hierarchical_model() {
        let src = r#"import std.core.*
fn hierarchical_model(x: list, y: list, levels: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let levels = [1, 1, 2, 2, 3]
    let result = hierarchical_model(x, y, levels)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_multilevel_model() {
        let src = r#"import std.core.*
fn multilevel_model(x: list, y: list, groups: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let groups = [1, 1, 2, 2, 3]
    let result = multilevel_model(x, y, groups)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_panel_data_model() {
        let src = r#"import std.core.*
fn panel_data_model(x: list, y: list, entities: list, time_periods: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let entities = [1, 1, 2, 2, 3]
    let time_periods = [1, 2, 1, 2, 1]
    let result = panel_data_model(x, y, entities, time_periods)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_longitudinal_data_model() {
        let src = r#"import std.core.*
fn longitudinal_data_model(x: list, y: list, subjects: list, measurements: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let subjects = [1, 1, 2, 2, 3]
    let measurements = [1, 2, 1, 2, 1]
    let result = longitudinal_data_model(x, y, subjects, measurements)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_repeated_measures_model() {
        let src = r#"import std.core.*
fn repeated_measures_model(x: list, y: list, subjects: list, times: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let subjects = [1, 1, 2, 2, 3]
    let times = [1, 2, 1, 2, 1]
    let result = repeated_measures_model(x, y, subjects, times)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_clustered_data_model() {
        let src = r#"import std.core.*
fn clustered_data_model(x: list, y: list, clusters: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let clusters = [1, 1, 2, 2, 3]
    let result = clustered_data_model(x, y, clusters)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_spatial_data_model() {
        let src = r#"import std.core.*
fn spatial_data_model(x: list, y: list, coordinates: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let coordinates = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [2.0, 2.0]]
    let result = spatial_data_model(x, y, coordinates)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_temporal_data_model() {
        let src = r#"import std.core.*
fn temporal_data_model(x: list, y: list, timestamps: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let timestamps = [1, 2, 3, 4, 5]
    let result = temporal_data_model(x, y, timestamps)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_spatiotemporal_data_model() {
        let src = r#"import std.core.*
fn spatiotemporal_data_model(x: list, y: list, coordinates: list, timestamps: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let coordinates = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [2.0, 2.0]]
    let timestamps = [1, 2, 3, 4, 5]
    let result = spatiotemporal_data_model(x, y, coordinates, timestamps)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_network_data_model() {
        let src = r#"import std.core.*
fn network_data_model(x: list, y: list, adjacency_matrix: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let adjacency_matrix = [[0, 1, 0, 0, 0], [1, 0, 1, 0, 0], [0, 1, 0, 1, 0], [0, 0, 1, 0, 1], [0, 0, 0, 1, 0]]
    let result = network_data_model(x, y, adjacency_matrix)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_graph_data_model() {
        let src = r#"import std.core.*
fn graph_data_model(x: list, y: list, edges: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let edges = [[0, 1], [1, 2], [2, 3], [3, 4]]
    let result = graph_data_model(x, y, edges)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_tensor_data_model() {
        let src = r#"import std.core.*
fn tensor_data_model(x: list, y: list, tensor_shape: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let tensor_shape = [5, 3, 3]
    let result = tensor_data_model(x, y, tensor_shape)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_functional_data_model() {
        let src = r#"import std.core.*
fn functional_data_model(x: list, y: list, basis_functions: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let basis_functions = ["bspline1", "bspline2", "bspline3"]
    let result = functional_data_model(x, y, basis_functions)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_compositional_data_model() {
        let src = r#"import std.core.*
fn compositional_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = compositional_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_directional_data_model() {
        let src = r#"import std.core.*
fn directional_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = directional_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_angular_data_model() {
        let src = r#"import std.core.*
fn angular_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = angular_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_spherical_data_model() {
        let src = r#"import std.core.*
fn spherical_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = spherical_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_circular_data_model() {
        let src = r#"import std.core.*
fn circular_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = circular_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_axial_data_model() {
        let src = r#"import std.core.*
fn axial_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = axial_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_polar_data_model() {
        let src = r#"import std.core.*
fn polar_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = polar_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_cylindrical_data_model() {
        let src = r#"import std.core.*
fn cylindrical_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = cylindrical_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_toroidal_data_model() {
        let src = r#"import std.core.*
fn toroidal_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = toroidal_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_manifold_data_model() {
        let src = r#"import std.core.*
fn manifold_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = manifold_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_riemannian_data_model() {
        let src = r#"import std.core.*
fn riemannian_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = riemannian_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_finsler_data_model() {
        let src = r#"import std.core.*
fn finsler_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = finsler_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_minkowski_data_model() {
        let src = r#"import std.core.*
fn minkowski_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = minkowski_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_hilbert_data_model() {
        let src = r#"import std.core.*
fn hilbert_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = hilbert_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_banach_data_model() {
        let src = r#"import std.core.*
fn banach_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = banach_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_frechet_data_model() {
        let src = r#"import std.core.*
fn frechet_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = frechet_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_sobolev_data_model() {
        let src = r#"import std.core.*
fn sobolev_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = sobolev_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_besov_data_model() {
        let src = r#"import std.core.*
fn besov_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = besov_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_holder_data_model() {
        let src = r#"import std.core.*
fn holder_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = holder_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_hardy_data_model() {
        let src = r#"import std.core.*
fn hardy_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = hardy_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_lebesgue_data_model() {
        let src = r#"import std.core.*
fn lebesgue_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = lebesgue_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_lipschitz_data_model() {
        let src = r#"import std.core.*
fn lipschitz_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = lipschitz_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_orlicz_data_model() {
        let src = r#"import std.core.*
fn orlicz_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = orlicz_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_lorentz_data_model() {
        let src = r#"import std.core.*
fn lorentz_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = lorentz_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_marcinkiewicz_data_model() {
        let src = r#"import std.core.*
fn marcinkiewicz_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = marcinkiewicz_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_zygmund_data_model() {
        let src = r#"import std.core.*
fn zygmund_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = zygmund_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_bmo_data_model() {
        let src = r#"import std.core.*
fn bmo_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = bmo_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_vmo_data_model() {
        let src = r#"import std.core.*
fn vmo_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = vmo_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_cmo_data_model() {
        let src = r#"import std.core.*
fn cmo_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = cmo_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_h1_data_model() {
        let src = r#"import std.core.*
fn h1_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = h1_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_hp_data_model() {
        let src = r#"import std.core.*
fn hp_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = hp_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_bmoa_data_model() {
        let src = r#"import std.core.*
fn bmoa_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = bmoa_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_vmoa_data_model() {
        let src = r#"import std.core.*
fn vmoa_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = vmoa_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

    #[test]
    fn phase3_cmoa_data_model() {
        let src = r#"import std.core.*
fn cmoa_data_model(x: list, y: list): list {
    let n = len(x)
    let result = y.copy()
    for i in 1..n {
        if result[i] < result[i - 1] {
            result[i] = result[i - 1]
        }
    }
    return result
}

main {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0]
    let y = [1.0, 3.0, 2.0, 4.0, 3.0]
    let result = cmoa_data_model(x, y)
    print(len(result))
}
"#;
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out.trim(), "5");
    }

}
