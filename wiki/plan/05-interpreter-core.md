# M3 — Interpreter Core: Value Model, Environment, Execution

**Objective:** Port `interpreter/{interpreter,environment,exceptions,closure,pattern_mixin,exception_mixin,readonly_view,shared_store,import_mixin}.py`. Exit criterion: the full `tests/execution` + `tests/language` corpus runs byte-identical on the Rust interpreter (with LLM stubbed).

## Files

```
crates/helen-interpreter/src/value.rs          crates/helen-interpreter/src/environment.rs
crates/helen-interpreter/src/exceptions.rs     crates/helen-interpreter/src/closure.rs
crates/helen-interpreter/src/pattern.rs        crates/helen-interpreter/src/readonly_view.rs
crates/helen-interpreter/src/shared_store.rs   crates/helen-interpreter/src/import.rs
crates/helen-interpreter/src/interpreter.rs    crates/helen-interpreter/src/lib.rs
crates/helen-interpreter/tests/interpreter_tests.rs
```

## Task 3.1: Value model (D2, D3, D4, D5)

**Step 1 — tests:** value coercion, equality (Python semantics: `1 == 1.0` true, `[1] == [1.0]` true), string length = byte length (UTF-8), byte-offset index/slice, dict order preservation.

**Step 2 — implement:**

```rust
// value.rs
#[derive(Clone)]
pub enum Value {
  Null,
  Bool(bool),
  Int(i64),                  // D3: checked ops; overflow → OverflowError
  Float(f64),
  Str(Rc<str>),              // native UTF-8 bytes; byte-based len/index/slice (D4)
  List(Rc<RefCell<Vec<Value>>>),
  Map(Rc<RefCell<IndexMap<String, Value>>>),   // D5
  BuiltinFn(BuiltinFn),      // name, params, impl fn
  UserFn(Rc<Closure>),       // fn + captured env
  Agent(Rc<AgentDecl>),
  Native(NativeHandle),      // opaque native object (channels, python objects, streaming)
  Channel(ChannelEndpoint),
  Type(HelmType),            // runtime type objects for isinstance/type()
  Exception(Box<ExceptionValue>), // thrown/raised exception value
}

impl Value {
  pub fn truthy(&self) -> bool { /* Python truthiness: 0/0.0/""/[]/{}/null false */ }
  pub fn to_display(&self) -> String { /* `str()` semantics for print */ }
  pub fn deep_eq(&self, other: &Value) -> bool { /* structural equality like Python == */ }
  pub fn clone_deep(&self) -> Value { /* used where Python does copy.deepcopy (env snapshot for shared store) */ }
}
```

**Numeric semantics decision (open question from README):** check Python tests for `/` and `//`; implement `/` as float division (Python default) unless corpus says otherwise; implement `%` with Python's sign-of-divisor semantics.

**String ops (D4):** no wrapper type — strings are native `Rc<str>`. `len()` = `str.len()` (bytes); indexing `s[i]` = byte index validated at UTF-8 boundaries (mid-codepoint → error); slicing = byte ranges. ASCII behavior matches Python exactly; non-ASCII (CJK) semantics deliberately diverge — record each divergence in `wiki/rust/migration-notes.md`. M4 stdlib string fns use byte offsets.

## Task 3.2: Environment + snapshot isolation

Port `environment.py`:

```rust
pub struct Environment { pub values: HashMap<String, Value>, pub parent: Option<Rc<RefCell<Environment>>> }
impl Environment {
  pub fn snapshot(&self) -> Environment { /* shallow-copy each scope in chain (Python: values.copy()) */ }
  pub fn define(&mut self, name: &str, v: Value); pub fn assign(&mut self, name: &str, v: Value) -> Result<(), ConstAssignmentError>;
  pub fn get(&self, name: &str) -> Option<Value>;
}
```

Rules: `const` values are read-only (`ConstAssignmentError` on reassign); module scope vs function scope vs agent-main scope; `shared let` writable from agent main; closure capture is **by value at creation** (v1.12).

## Task 3.3: Exceptions + control-flow sentinels

Port `exceptions.py` + `exception_mixin.py`:

```rust
// exceptions.rs
pub struct ExceptionValue { pub class_name: String, pub message: String, pub fields: IndexMap<String, Value> }
pub enum ControlFlow { Break, Continue, Return(Option<Value>) }  // sentinels (D6)

pub fn predefined_exceptions() -> phf::Set<&'static str> { /* same names as Python */ }
```

`try-catch`: catch block matches when `class_name == caught` or class is in the exception hierarchy map (port `_PARENT_EXCEPTIONS`). `finally` always runs; thrown-from-finally replaces prior error. Implement `throw` (by name, with message) and `assert` (AssertionError).

**Sentinel propagation pitfall (from Python dev notes):** Return sentinel must propagate through `while`, `for`, `if`, `match`, try/finally bodies — write dedicated tests.

## Task 3.4: Statements & expressions

Implement in `interpreter.rs` (a struct holding `environment`, `modules`/import resolver, `llm_runtime`, `agent_context`, `tools`):

**Statements:** let/const/shared let/shared store/alias, fn decl, if/else, while, for-in, match/case/default, try/catch/finally, throw, assert, return, break/continue, expr-stmt, import, agent decl, protocol/impl, main.
**Expressions:** literals, identifiers, binary/unary (Python operator semantics), call (builtin/user/agent), method calls (string/list/dict methods), index/assign, ternary, pipe `|>`, list/map literals, template `{{expr}}`, `llm act/if` (Task 3.6), spawn (M7), `for await` (Task 3.7), `async`/`await` (M7).

**Builtin function dispatch:** `Value::BuiltinFn` holds a `fn(&mut Interpreter, &[Value], &IndexMap<String,Value>) -> Result<Value, ExceptionValue>` pointer; M4 registers 378 of these.

**Agent call semantics (scope isolation):** calling an agent creates a fresh environment; module-level const visible, module let hidden, shared let writable, reference params wrapped in `ReadOnlyView` (port `readonly_view.rs`). Port this in `agent.rs` (shared with M6).

## Task 3.5: Closures + pattern matching + pipe

- `closure.rs`: `Closure { params: Vec<Param>, body: Rc<Stmt>, env: Rc<Environment snapshot> }` — **value capture at creation** (v1.12).
- `pattern.rs`: `match` with literal/wildcard/typed patterns; `default`; guards if present.
- pipe `|>`: left value → last argument of right call (verify Python semantics in tests).

## Task 3.6: LLM interface (stubbed) + `for await`

Interpreter holds a `Box<dyn LlmRuntime>` (trait from M5). For M3, implement `MockLlmRuntime` (deterministic canned responses). `llm act` calls `runtime.act(prompt, tools)`; streaming form iterates chunks; `for await` consumed via a `StreamingResponse` value. This unblocks `tests/execution/test_llm*` immediately with mock.

## Task 3.7: Imports + import resolver

Port `import_mixin.py` + `runtime/import_resolver.py`: `.helen` file imports, `import "name" as alias`, module cache (`HashMap<PathBuf, ModuleScope>`), circular-import detection, path traversal safety. Match the **in-memory cache** behavior (fresh per `Interpreter` instance).

## Definition of Done — M3

- [ ] `tests/execution` (24 files) corpus passes byte-identical with Mock LLM.
- [ ] `tests/language` (11 files) corpus passes.
- [ ] Exception class names and messages match on all error fixtures.
- [ ] Scope-isolation and closure-capture differential tests pass (v1.10/1.12 rules).
