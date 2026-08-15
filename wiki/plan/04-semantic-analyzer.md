# M2 — Semantic Analyzer: Types, Symbols, Analysis

**Objective:** Port `semantic/{types,symbols,analyzer,type_utils}.py` (2,600+ lines). Exit criterion: the same E-codes are produced for the full semantic test corpus.

## Files

```
crates/helen-semantic/src/types.rs      crates/helen-semantic/src/symbols.rs
crates/helen-semantic/src/analyzer.rs   crates/helen-semantic/src/lib.rs
crates/helen-semantic/tests/semantic_tests.rs
```

## Task 2.1: Type system (14 types)

**Step 1 — tests:** Port `tests/semantic/test_types.py` assignability tables as Rust tests.

**Step 2 — implement:**

```rust
// types.rs
#[derive(Clone, Debug, PartialEq)]
pub enum Type { Any, Bool, Int, Float, Str, Null, Number, // Number is abstract in Python; keep as marker
  Optional(Box<Type>), List(Box<Type>), Map(Box<Type>, Box<Type>),   // key, value
  Union(Vec<Type>), Literal(Value), Agent(String) }

impl Type {
  pub fn assignable_to(&self, target: &Type) -> bool { /* rules from wiki/compiler/types.md */ }
  pub fn from_type_annotation(&self, ann: &Annotation) -> Type { /* type_from_typenode() */ }
}
```

Rules to encode exactly: Any ← anything; `T?` ← T or null; `A|B` ← A or B; list element match; map key/value match; literal assignability; Agent name match.

## Task 2.2: Symbol tables

Port `symbols.py`: `Symbol { name, kind, type, mutable, scope_id }`, scope stack (`begin_scope`/`end_scope`), `declare`/`resolve` with shadowing rules matching Python.

## Task 2.3: Analyzer (47+ visitor rules, E-codes)

**Step 1 — tests:** Port `tests/semantic/` (11 files). Each test asserts an E-code. Use a helper that parses → analyzes → returns codes:

```rust
fn codes(src: &str) -> Vec<String> { /* parse, analyze, collect E-codes */ }
```

**Step 2 — implement** `analyzer.rs` with a `SemanticAnalyzer` struct (`env`, `module_level_lets: HashSet`, `errors`). Cover in priority order:

1. **Top-level rules** — E0355 `TOP_LEVEL_STATEMENT`: module-level `let` forbidden; only declarations + one `main {}`.
2. **Undefined/duplicate/const-reassign** name errors.
3. **Agent scope isolation (v1.10/1.12):**
   - module `let` invisible in `agent main {}` → error;
   - module `const` auto-visible (read-only) → OK;
   - `shared let` must be a **value type** → else `SemanticError`;
   - closure initializers in agent scope checked against visibility.
4. **Operator type constraints** (table in `wiki/compiler/types.md`): `+ - * / %`, comparisons, `!`, unary `-`.
5. **Function arity/type checks** for annotated params.
6. **Predefined-exception whitelist** — `catch X` where X not in `_PREDEFINED_EXCEPTIONS` → error; keep the same frozenset (RuntimeError, TypeError, ValueError, KeyError, IndexError, ZeroDivisionError, … + Helen-native: HelenRuntimeError, AgentError, LLMError, AggregateError, ParseError, …). **Source:** `interpreter/exceptions.py`.
7. **Protocol/impl conformance**, pipe-operator type flow, `for await` streaming types, `spawn` agent-return-type check, `llm act` result typing (Any).

**Step 3 — verify:** `cargo test -p helen-semantic`; differential: parse+analyze corpus, compare sorted E-code lists against `python -c "…analyze…"` reference script (add to harness as `--semantic-only`).

## Task 2.4: Error-code parity table

Generate `tests/conformance/e-codes.csv` (code, python_message_pattern, rust_message_pattern) from Python tests; keep patterns lenient (span positions excluded).

## Definition of Done — M2

- [ ] All `tests/semantic/*` cases reproduce identical E-codes.
- [ ] `--semantic-only` differential passes on corpus.
- [ ] E-code parity table committed with zero unmatched codes.
