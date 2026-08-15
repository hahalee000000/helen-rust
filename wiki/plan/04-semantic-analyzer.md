# M2 — Semantic Analyzer: Types, Symbols, Analysis

**Status: COMPLETE** (commit `4e6a6f6`)

**Objective:** Port `semantic/{types,symbols,analyzer,type_utils}.py`. Exit criterion: identical E-codes for the corpus. **Achieved: 47/47 corpus files + 15/15 semantic fixtures byte-identical to `reference.py --semantic-only`; 32 unit tests; clippy 0; lex/parse diff 47/47.**

## Files

```
crates/helen-semantic/src/types.rs       (420 LOC)
crates/helen-semantic/src/symbols.rs     (287 LOC)
crates/helen-semantic/src/type_utils.rs  (146 LOC)
crates/helen-semantic/src/analyzer.rs    (2303 LOC, 28 E-codes)
crates/helen-semantic/src/diagnostics.rs (ErrorReporter port)
crates/helen-semantic/src/stdlib.rs      + stdlib_data.json (generated from Python)
crates/helen-rust/src/main.rs            (--semantic-only mode)
tests/conformance/fixtures/semantic/     (15 fixtures)
scripts/diff-semantic.sh + diff-semantic-fixtures.sh
```

## Task 2.1: Type system (14 types) — DONE

`Type` enum with `assignable_to` / `type_from_typenode` ported from `types.py` + `type_utils.py`. Map key types unrestricted (D5). Note: Python's abstract `Number` is a marker type.

## Task 2.2: Symbol tables — DONE

`SymbolTable` with scope stack, `enter_scope`/`exit_scope`, `define`/`resolve`/`undefine`/`global_undefine`. Faithful duplicate semantics: `define` returns the existing symbol on duplicate (Python parity) — this is what makes pass-1 `DUPLICATE_SYMBOL`/`DUPLICATE_AGENT_NAME` fire.

## Task 2.3: Analyzer — DONE (verified differentially)

All visitor rules ported. Two latent bugs found & fixed during porting:
1. **Impl/protocol method dispatch** — Rust called `accept_into` directly, skipping `_in_function` tracking; Python dispatches `method.accept(self)` → `visit_function_decl`. Fixed: impl/protocol methods now route through `visit_function_decl`.
2. **Duplicate-symbol registration** — `_define_user_symbol` discarded the "already defined" return so pass-1 `_register_function_signature` never emitted E0333/E0335.

## C1 correction (verified against reference, NOT the plan note)

The plan's "11 native exceptions" is **outdated**. Current `analyzer.py` whitelist is **15 entries**:
- 9 Helen-native: `AnyError, LLMError, TimeoutError, ModelError, AgentError, ToolError, RuntimeError, AssertionError, AggregateError`
- 6 Python names accepted: `ValueError, TypeError, KeyError, IndexError, FileNotFoundError, PermissionError`
- **Rejected:** `PromptTooLongError`, `LLMOutputContractError` (not in the analyzer's frozenset, despite being interpreter exceptions)

Also verified: `return` inside `main {}` is **legal** (v1.12 Issue #26); the only reachable E0340 trigger is a top-level `return` (which also emits E0355 in real-file contexts; `<test>`/`<unknown>`/`<repl>` filenames skip top-level checks, matching Python line 440).

## Phase B (found during M2): AST type-annotation fidelity

M1's parse-diff was **blind to annotations** (the printer omits them), masking a latent AST collapse: `int?`/`A|B` became synthetic `TypeRef` names `optional<int>`/`union<A|B>`. Python keeps `OptionalTypeNode`/`UnionTypeNode` as first-class expression nodes. Fixed in `ast.rs`:
```rust
pub enum TypeRefKind { Simple, Optional, Union }
```
Parser emits `TypeRef { name, kind: Optional, span }` for `T?` and `kind: Union` for `A|B`. **47/47 parse-diff still passes** — the change is structure-preserving. This unblocks M3 type inference, which needs real type structure.

## Task 2.4: Error-code parity table

Deferred to M3 (all 28 E-codes already present in `helen-core`'s `ErrorCode` enum; message-level parity table is a separate deliverable).

## Definition of Done — M2

- [x] `--semantic-only` differential passes on corpus: **47/47**
- [x] Semantic fixtures (catch/throw whitelist C1/C2, duplicates, const-assign, context errors, store methods): **15/15**
- [x] Unit tests: **32** (types/symbols/type_utils/analyzer)
- [x] Full gate: fmt clean, clippy 0, `cargo test` 80, lex 47/47, parse 47/47
- [ ] E-code parity table (deferred to M3)

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

**Map key types (D5):** `Map(K, V)` — the key type is not restricted to `String`. Verified: map literals accept arbitrary hashable keys (`{"a": 1, 2: "two"}`), so `K` may be `Int | Float | Str | Bool | Null` (unhashable types like list/dict → semantic error, mirroring Python's `unhashable` behavior).

## Task 2.2: Symbol tables

Port `symbols.py`: `Symbol { name, kind, type, mutable, scope_id }`, scope stack (`begin_scope`/`end_scope`), `declare`/`resolve` with shadowing rules matching Python.

## Task 2.3: Analyzer (47+ visitor rules, E-codes)

**Step 1 — tests:** Port `tests/semantic/` (207 tests). Each test asserts an E-code. Use a helper that parses → analyzes → returns codes:

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
4. **Operator type constraints** (table in `wiki/compiler/types.md`): `+ - * / %`, comparisons, `!`, unary `-`. (No `//`, `**`, or bitwise operators exist in the language — do not model them.)
5. **Function arity/type checks** for annotated params.
6. **Predefined-exception whitelist (verified)** — the whitelist is **exactly the 11 Helen-native names**: `AnyError, LLMError, TimeoutError, ModelError, PromptTooLongError, AgentError, LLMOutputContractError, ToolError, RuntimeError, AssertionError, AggregateError`. Python exception names (`TypeError`, `ValueError`, `KeyError`, `IndexError`, `ZeroDivisionError`, …) are **not** valid — `catch ValueError …` / `throw ValueError(…)` → error. **Source:** `interpreter/exceptions.py` `_PREDEFINED_EXCEPTIONS`. Also: `catch X` without a bound variable → E0301 `Expected error variable name` (the grammar requires `catch X err`).
7. **Protocol/impl conformance**, pipe-operator type flow, `spawn` agent-return-type check, `llm act` result typing (Any). (No `for await` — the feature does not exist.)

**Step 3 — verify:** `cargo test -p helen-semantic`; differential: parse+analyze corpus, compare sorted E-code lists against `reference.py --semantic-only` (in-process Python driver, M0.4).

## Task 2.4: Error-code parity table

Generate `tests/conformance/e-codes.csv` (code, python_message_pattern, rust_message_pattern) from Python tests; keep patterns lenient (span positions excluded).

## Definition of Done — M2

- [ ] All Tier-C `tests/semantic` cases reproduce identical E-codes.
- [ ] `--semantic-only` differential passes on corpus.
- [ ] E-code parity table committed with zero unmatched codes.
