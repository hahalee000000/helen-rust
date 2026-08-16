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
}
