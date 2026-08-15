# M3 — Interpreter Core: Value Model, Environment, Execution

**Objective:** Port `interpreter/{interpreter,environment,exceptions,closure,pattern_mixin,exception_mixin,readonly_view,shared_store,import_mixin}.py`. Exit criterion: the Tier-A extracted corpus from `tests/interpreter` + `tests/execution`-equivalent Rust tests run byte-identical on the Rust interpreter (LLM stubbed).

## Files

```
crates/helen-interpreter/src/value.rs          crates/helen-interpreter/src/environment.rs
crates/helen-interpreter/src/exceptions.rs     crates/helen-interpreter/src/closure.rs
crates/helen-interpreter/src/pattern.rs        crates/helen-interpreter/src/readonly_view.rs
crates/helen-interpreter/src/shared_store.rs   crates/helen-interpreter/src/import.rs
crates/helen-interpreter/src/interpreter.rs    crates/helen-interpreter/src/lib.rs
crates/helen-interpreter/tests/interpreter_tests.rs
```

## Task 3.1: Value model (D2–D5) + display parity (D11)

**Step 1 — tests:** value coercion; equality with Python semantics (`1 == 1.0` true, `[1] == [1.0]` true); string `len()` = byte length (UTF-8), byte-offset index/slice with boundary validation; **string iteration raises** (only lists iterate — verified Helen behavior); dict insertion-order + non-string keys.

**Step 2 — implement:**

```rust
// value.rs
#[derive(Clone)]
pub enum Value {
  Null,
  Bool(bool),
  Int(num_bigint::BigInt),       // D3: arbitrary precision from day one
  Float(f64),
  Str(Rc<str>),                  // native UTF-8 bytes; byte-based len/index/slice (D4)
  List(Rc<RefCell<Vec<Value>>>),
  Map(Rc<RefCell<IndexMap<Value, Value>>>),   // D5: arbitrary hashable keys
  BuiltinFn(BuiltinFn),          // name, params, impl fn
  UserFn(Rc<Closure>),           // fn + captured env
  Agent(Rc<AgentDecl>),
  Native(NativeHandle),          // opaque native object (channels, python objects)
  Channel(ChannelEndpoint),
  Type(HelmType),                // runtime type objects (type() returns strings — see below)
  Exception(Box<ExceptionValue>), // thrown/raised exception value
}

impl Value {
  pub fn truthy(&self) -> bool { /* Python truthiness: 0/0.0/""/[]/{}/null false */ }
  pub fn to_display(&self, top_level: bool) -> String { /* D11, see below */ }
  pub fn deep_eq(&self, other: &Value) -> bool { /* structural equality like Python == */ }
  pub fn clone_deep(&self) -> Value { /* used where Python does copy.deepcopy (env snapshot for shared store) */ }
}
```

**Map keys (D5):** `Value` implements structural `Hash`/`Eq` (int 1 == float 1.0 must hash equally, mirroring Python `{1:…}` vs `{1.0:…}` collision behavior). Verified: `{"a": 1, 2: "two"}` legal, `m[2]` works.

**Numeric semantics (verified, resolved):**
- Operator set: `+ - * / % == != < <= > >= && || ! |> .. -> =`. **No `//`, `**`, bitwise `| & ^ ~ << >>`.**
- `/` always returns `Float` (`7/2` = 3.5). `%` uses Python sign-of-divisor semantics (`-7 % 3` = 2, `7 % -3` = -2). `int()` truncates toward zero.
- All integer ops via `num-bigint` — no overflow path.

**String ops (D4):** no wrapper type — strings are native `Rc<str>`. `len()` = `str.len()` (bytes); indexing `s[i]` = byte index validated at UTF-8 boundaries (mid-codepoint → error); slicing = byte ranges. **`for c in "abc"` → runtime error** (`Cannot iterate over str/dict`) — only lists iterate. `range(n)` returns a list. ASCII behavior matches Python exactly; non-ASCII (CJK) semantics deliberately diverge — record each divergence in `wiki/rust/migration-notes.md`.

**`type()` / `isinstance`:** `type(x)` returns **strings** (`"int","str","list","dict","NoneType","bool"`); `isinstance(x, "int")` takes a string type name. Keep the `Type` enum for the semantic analyzer only; the runtime type() builtin returns `Value::Str`.

**Task 3.1b — print/str/repr display parity (D11, dedicated):**
`to_display(value, top_level: bool)` encodes the verified asymmetric rules:
- **Top-level `print(x)`** uses Python `str()` semantics: `print(true)` → `true`, `print(false)` → `false`, `print(null)` → `None`, `print(3.5)` → `3.5`.
- **Nested (inside containers)** uses Python `repr()` of elements: `print([1, 'a', true, null])` → `[1, 'a', True, None]` (bools → `True`/`False`, null → `None`, strings single-quoted).
- **Float formatting = Python `repr`**: `1e+20`, `1.5e-05`, `3.0`, `3.5` — port `repr`-style shortest-roundtrip with Python's exponent rules (thresholds `1e-4`/`1e16`; note Python prints `1e+20`, not `1e20`).
- Also used by error messages, `{{expr}}` template interpolation, and `std.debug` output.

Corpus: `tests/programs/display/` (authored in M0, asserted byte-identical in M13).

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

pub const PREDEFINED_EXCEPTIONS: [&str; 11] = [
  "AnyError", "LLMError", "TimeoutError", "ModelError", "PromptTooLongError",
  "AgentError", "LLMOutputContractError", "ToolError", "RuntimeError",
  "AssertionError", "AggregateError",
];
```

- **`catch` matches by exact class name** against `PREDEFINED_EXCEPTIONS` — no hierarchy map. Python exception names (`TypeError`, `ValueError`, `KeyError`, `IndexError`, `ZeroDivisionError`) are **invalid** throw/catch types → compile error.
- **`catch X err` requires a bound variable** (bare `catch X` → E0301 `Expected error variable name`); the message is `err.message`, fields via `err.<field>`.
- `try`/`catch`/`finally`: `finally` always runs; thrown-from-finally replaces prior error. Implement `throw Name("msg")` and `assert` (`AssertionError`).
- Stdlib functions that wrap Python errors raise `RuntimeError` (e.g. `int("abc")` → `RuntimeError: Python ValueError: invalid literal ...`) — never a Python-named class.

**Sentinel propagation pitfall (from Python dev notes):** Return sentinel must propagate through `while`, `for`, `if`, `match`, try/finally bodies — write dedicated tests.

## Task 3.4: Statements & expressions

Implement in `interpreter.rs` (a struct holding `environment`, `modules`/import resolver, `llm_runtime`, `agent_context`, `tools`):

**Statements:** let/const/shared let/shared store/alias, fn decl, if/else, while, for-in (**lists only**), match/case/default, try/catch/finally, throw, assert, return, break/continue, expr-stmt, import, agent decl, protocol/impl, main.
**Expressions:** literals, identifiers, binary/unary (operator set per §3.1), call (builtin/user/agent), method calls (string/list/dict methods), index/assign, ternary, pipe `|>`, list/map literals, template `{{expr}}`, `llm act/if` (Task 3.6), spawn (M7). **No `async`/`await`/`for await` in the language** — verified absent.

**Builtin function dispatch:** `Value::BuiltinFn` holds a `fn(&mut Interpreter, &[Value], &IndexMap<String,Value>) -> Result<Value, ExceptionValue>` pointer; M4 registers 378 of these.

**Agent call semantics (scope isolation):** calling an agent creates a fresh environment; module-level const visible, module let hidden, shared let writable, reference params wrapped in `ReadOnlyView` (port `readonly_view.rs`). Port this in `agent.rs` (shared with M6).

## Task 3.5: Closures + pattern matching + pipe

- `closure.rs`: `Closure { params: Vec<Param>, body: Rc<Stmt>, env: Rc<Environment snapshot> }` — **value capture at creation** (v1.12).
- `pattern.rs`: `match` with literal/wildcard/typed patterns; `default`; guards if present.
- pipe `|>`: left value → last argument of right call (verify Python semantics in tests).

## Task 3.6: LLM interface (stubbed) + streaming callbacks

Interpreter holds a `Box<dyn LlmRuntime>` (trait from M5). For M3, implement `MockLlmRuntime` (deterministic canned responses — strings identical to Python's `MockLLMRuntime`).

- `llm act "prompt"` → `runtime.act(prompt, tools)` (sync).
- **Streaming is via callbacks** (there is no `for await`): `llm act "..." { on_chunk(chunk) {} on_complete(result) {} }` — port `visit_llm_act_streaming` semantics from `llm_mixin.py`: chunks delivered incrementally to `on_chunk`, final result to `on_complete`.
- `llm if "desc" { ... }` → `runtime.route(...)`.

This unblocks the Tier-A `tests/interpreter` corpus (LLM tests use `MockLLMRuntime` in-process, mirrored by `MockLlmRuntime`).

## Task 3.7: Imports + import resolver

Port `import_mixin.py` + `runtime/import_resolver.py`: `.helen` file imports, `import "name" as alias`, module cache (`HashMap<PathBuf, ModuleScope>`), circular-import detection, path traversal safety. Match the **in-memory cache** behavior (fresh per `Interpreter` instance).

Note the verified import rules: **builtins are not globals** — `import std.core.*` (or `import std.core as C`, `import std.core.{len, str}`) is required; bare `print` → E0332 `undeclared variable 'print'`.

## Definition of Done — M3

- [ ] Tier-A extracted `tests/interpreter` corpus passes byte-identical with Mock LLM.
- [ ] Tier-C ported `tests/execution` test cases pass (AST-constructed suite reimplemented in Rust).
- [ ] Exception class names and messages match on all error fixtures (11 native names; E0301 for bare `catch`).
- [ ] Display-parity corpus (`tests/programs/display/`) passes byte-identical (D11).
- [ ] Scope-isolation and closure-capture differential tests pass (v1.10/1.12 rules).
