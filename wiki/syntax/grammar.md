<!-- helen-rust edition: Pratt parser in crates/helen-parser (M1) — 47/47 parse differential. -->

# Grammar

> Module M2 | `helen/core/parser.py` | Tests: `tests/parser/`

---

## Overview

The Helen Parser uses **Pratt Parsing** (10-level precedence table) + recursive descent to convert a token stream into an AST.

---

## Full EBNF Grammar

### Programs and Blocks

```ebnf
program       → declaration* main_block?
declaration   → decorator? (agent_decl | fn_decl | import_stmt | shared_store_decl
                | const_decl | shared_let_decl | alias_decl | protocol_decl | impl_decl)
decorator     → "@" IDENTIFIER
main_block    → "main" "{" statement* "}"
```

> **v1.30 top-level restriction (E0355 TOP_LEVEL_STATEMENT)**: Only **declarations**
> (`fn`, `agent`, `const`, `import`, `alias`, `shared let`, `shared store`, `protocol`,
> `impl`) and at most one `main {}` block are allowed at the module level.
> Module-level `let` is **forbidden** — all executable code (variable bindings,
> function calls, control flow) MUST be inside `main {}` or inside a `fn`/`agent main {}`.

### Agent Declarations

```ebnf
agent_decl    → "agent" IDENTIFIER "{" agent_body "}"
agent_body    → agent_setting* prompt_def? functions_block?
agent_setting → "description" string
              | "model" string
              | "tools" "[" string ("," string)* "]"
              | "sub-agents" "{" agent_param* "}"
              | "memory" string
              | "temperature" NUMBER
              | "max-turns" NUMBER
agent_param   → IDENTIFIER ":" type?
prompt_def    → "prompt" string
functions_block → "functions" "{" (var_decl | fn_decl)* "}"
var_decl      → ("let" | "const" | "shared" "let") IDENTIFIER ("=" expression)?
```

**v1.10 shared let** (updated in v1.30):
- `shared let` declares cross-agent visible mutable variables
- Module-level `let` is **forbidden** (E0355 TOP_LEVEL_STATEMENT); use `const`, `shared let`, or `shared store` instead
- Module-level `const` is auto-visible (read-only sharing)

**v1.12 isolation annotations**:
- `@open` / `@strict` / `@sandbox` decorate agent declarations
- `@open`: Can access module `let`
- `@strict`: Deep-copies shared let on access
- `@sandbox`: Disables external tools, prohibits shared let

### Shared Store (v1.12)

```ebnf
shared_store_decl → "shared" "store" IDENTIFIER "{" store_body "}"
store_body        → (store_field | store_method)*
store_field       → var_decl
store_method      → "fn" IDENTIFIER "(" fn_params? ")" fn_body
```

**Semantics**:
- `shared store`: Controlled shared mutable state (reference type shared across agents)
- Reuses the `SharedStore` class at runtime (RLock thread-safe)
- `_` prefixed fields/methods are private — not directly accessible from agent code
- **v1.18**: `channel X { fields }` declaration syntax has been removed; channels are now created via `Channel()` constructor or `spawn`

### Function Declarations

```ebnf
fn_decl       → "fn" IDENTIFIER "(" fn_params? ")" fn_body
fn_params     → fn_param ("," fn_param)*
fn_param      → IDENTIFIER (":" type)?
fn_body       → "{" statement* "}"
```

### Imports

```ebnf
import_stmt   → "import" string ("as" IDENTIFIER)?
```

### Statements

```ebnf
statement     → var_decl
              | expr_stmt
              | if_stmt
              | for_stmt
              | while_stmt
              | match_stmt
              | try_stmt
              | throw_stmt
              | llm_stmt
              | call_stmt
              | return_stmt
              | break_stmt
              | continue_stmt

var_decl      → ("let" | "const" | "shared" "let") IDENTIFIER ("=" expression)?
expr_stmt     → expression
```

**v1.10 shared let**: Available in top-level declarations for sharing mutable state across agents.
**v1.30 top-level restriction**: Module-level `var_decl` only allows `const` and `shared let` — bare `let` at the module level is now E0355.

### Control Flow

```ebnf
if_stmt       → "if" "(" expression ")" "{" statement* "}" ("else" ("if" expression "{" statement* "}")?)?
for_stmt      → "for" IDENTIFIER "in" expression "{" statement* "}"
while_stmt    → "while" "(" expression ")" "{" statement* "}"
break_stmt    → "break"
continue_stmt → "continue"
return_stmt   → "return" expression?
```

### Pattern Matching

```ebnf
match_stmt    → "match" expression "{" case+ default? "}"
case          → "case" pattern guard? "{" statement* "}"
pattern       → expression | range_pattern | wildcard_pattern | variable_pattern | type_pattern
range_pattern → expression ".." expression
wildcard_pattern → "_"
variable_pattern → IDENTIFIER
type_pattern  → "is" IDENTIFIER IDENTIFIER?
guard         → "if" expression
default       → "default" "{" statement* "}"
```

**v1.8 pattern matching enhancements**:
- **Wildcard pattern**: `case _ { }` matches any value (can serve as default branch)
- **Variable binding**: `case x { }` binds the matched value to a variable
- **Type pattern**: `case is Type { }` checks the value's type
- **Type pattern with binding**: `case is Type name { }` checks type and binds to a variable

### Exception Handling

```ebnf
try_stmt      → "try" "{" statement* "}" (catch_clause+ catch_all? | catch_all) finally_block?
catch_clause  → "catch" type IDENTIFIER "{" statement* "}"
catch_all     → "catch" "{" statement* "}"
finally_block → "finally" "{" statement* "}"
throw_stmt    → "throw" type ("(" expression ")")? ";"?
```

### LLM Statements

```ebnf
llm_stmt      → llm_act | llm_if

llm_act       → "llm" "act" act_target? act_args? string?
                 ("on_chunk" expression)?
                 ("on_complete" expression)?
act_target    → IDENTIFIER | expression
act_args      → "(" named_arg ("," named_arg)* ")"
named_arg     → IDENTIFIER "=" expression

llm_if        → "llm" "if" expression "{" llm_branch+ "}"
llm_branch    → "branch" string "{" statement* "}"
              | "default" "{" statement* "}"
```

**v1.14 change**: `llm stream` has been removed; `llm act` supports streaming via optional `on_chunk`/`on_complete` callbacks. Without callbacks it executes synchronously (`act()`); with callbacks it executes as a stream (`act_stream()`).

### Calls

```ebnf
call_stmt     → "call" IDENTIFIER "(" call_args? ")"
call_args     → expression ("," expression)*
```

### Expressions (Pratt 11-level precedence)

```ebnf
expression    → assignment

assignment    → IDENTIFIER "=" assignment | pipe
pipe          → pipe "|>" equality | equality
equality      → comparison ("==" | "!=") comparison
comparison    → term (">" | ">=" | "<" | "<=") term
term          → factor ("+" | "-") factor
factor        → unary ("*" | "/" | "%") unary
unary         → ("!" | "-") unary | call
call          → primary ("(" args ")")* ("[" expression "]")* ("." IDENTIFIER)*
primary       → NUMBER | STRING | "true" | "false" | "null"
              | IDENTIFIER | "(" expression ")"
              | list_literal | map_literal
              | spawn_expr
list_literal  → "[" (expression ("," expression)*)? "]"
map_literal   → "{" (map_entry ("," map_entry)*)? "}"
map_entry     → expression ":" expression

spawn_expr → "spawn" IDENTIFIER "(" args? ")"
```

**v1.8 pipe operator**:
- `value |> fn` is equivalent to `fn(value)`
- Left-associative, low precedence (level 2)
- Supports chained calls: `value |> fn1 |> fn2`

---

## Pratt Parsing Precedence Table

| Precedence | Operator | Associativity | Example |
|---|---|---|---|
| 1 | `=` | Right | `x = y = 0` |
| 2 | `\|>` | Left | `value \|> fn1 \|> fn2` |
| 3 | `\|\|` | Left | `a \|\| b \|\| c` |
| 4 | `&&` | Left | `a && b && c` |
| 5 | `==` `!=` | Left | `a == b != c` |
| 6 | `>` `>=` `<` `<=` | Left | `a > b >= c` |
| 7 | `+` `-` | Left | `a + b - c` |
| 8 | `*` `/` `%` | Left | `a * b / c` |
| 9 | `!` `-` (unary) | Right | `!-x` |
| 10 | `()` `[]` `.` | Left | `f(a)[0].x` |
| 11 | `spawn` | Prefix | `spawn Agent(...)` |

---

## `llm` Context Keyword Disambiguation

`llm` is both a keyword and a potential identifier. The parser disambiguates via **peek logic**:

```python
# When peek sees "llm", check the next token
if peek() == "act":     → parse_llm_act()
elif peek() == "if":    → parse_llm_if()
else:                   → Treat as identifier
```

---

## `spawn` Prefix Handling

`spawn` is a unary prefix expression followed by an agent call:

```python
if peek() == "spawn":
    consume(SPAWN)
    call = parse_call()
    return SpawnExprNode(call=call, span=...)
```

---

## Panic Mode Error Recovery

When the parser encounters an unexpected token, it enters panic mode and synchronizes to a statement boundary:

```python
def _synchronize(self):
    self.advance()
    while not self.is_at_end():
        if self.previous().type == SEMICOLON:
            return
        if self.peek().type in (AGENT, FN, LET, CONST, IF, FOR, WHILE, RETURN, IMPORT, LLM):
            return
        self.advance()
```

Synchronization points: semicolons `;` and statement-starting keywords.

---

## Test Coverage

- ✅ Agent declarations and parameters
- ✅ Function declarations and calls
- ✅ Control flow (if/for/while/match)
- ✅ Exception handling (try/catch/finally/throw)
- ✅ LLM statements (act/if)
- ✅ Concurrent calls (spawn)
- ✅ Expression precedence
- ✅ Panic mode recovery
- ✅ Type annotation parsing

### v1.10 Syntax Updates

#### 1. Subscript/Field Assignment (v1.10)

The left-hand side of assignment statements now supports index access and field access:

```helen
// Array index assignment
main {
    let arr = [1, 2, 3]
    arr[0] = 10  // ✅ Legal

    // Object field assignment
    let obj = { name: "Alice", age: 30 }
    obj.name = "Bob"  // ✅ Legal
    obj["age"] = 31   // ✅ Also legal
}
```

**EBNF update**:
```ebnf
assignment → (call | IDENTIFIER) "=" assignment | pipe
```

Where `call` includes index access (`[i]`) and field access (`.field`).

#### 2. Short-Circuit Evaluation (v1.10)

`&&` and `||` operators now support short-circuit evaluation:

```helen
main {
    // && short-circuit
    let x = false && expensiveCall()  // expensiveCall() is not executed
    let y = true && expensiveCall()   // expensiveCall() is executed

    // || short-circuit
    let a = true || expensiveCall()   // expensiveCall() is not executed
    let b = false || expensiveCall()  // expensiveCall() is executed
}
```

**Precedence table**:
- `||` precedence 3 (left-associative)
- `&&` precedence 4 (left-associative)
- `&&` has higher precedence than `||`

#### 3. Return Type Annotation Syntax (v1.10)

Only the `:` syntax is supported; `->` syntax has been removed:

```helen
// ✅ Correct syntax
fn add(a: int, b: int): int {
  return a + b
}

// ❌ Removed
// fn add(a: int, b: int) -> int { ... }
```

**EBNF update**:
```ebnf
fn_decl → "fn" IDENTIFIER "(" fn_params? ")" (":" type)? fn_body
```

### v1.12 Syntax Updates

#### 1. Isolation Level Annotations (v1.12)

Agent declarations can be prefixed with `@open`/`@strict`/`@sandbox` isolation annotations:

```ebnf
declaration → decorator? (agent_decl | ...)
decorator   → "@" IDENTIFIER
```

#### 2. Shared Store Declarations (v1.12)

```ebnf
shared_store_decl → "shared" "store" IDENTIFIER "{" store_body "}"
store_body        → (store_field | store_method)*
```

### v1.13 Syntax Updates

#### 1. Channel Declarations (v1.13, **removed in v1.18**)

```ebnf
// Removed:
// channel_decl → "channel" IDENTIFIER "{" store_body "}"
```

As of v1.18, `channel X { fields }` declaration syntax has been removed. Channels are now created via `Channel()` constructor or `spawn`.

### v1.14 Syntax Updates

#### 1. `llm stream` Removed, `llm act` Gains Callbacks

```ebnf
llm_act → "llm" "act" act_target? act_args? string?
           ("on_chunk" expression)?
           ("on_complete" expression)?
```

`llm stream` has been removed (`STREAM` TokenType removed), `LlmStreamStmtNode` removed. Streaming functionality is integrated into `llm act` via `on_chunk`/`on_complete` callbacks.

### v1.18 Syntax Updates

#### 1. spawn Concurrency Primitive

```ebnf
spawn_expr → "spawn" IDENTIFIER "(" args? ")"
```

`spawn` is a unary prefix expression that returns a `Channel` type.

#### 2. Removed async/await/detach/channel Declarations

- `async_call_stmt` removed
- `async_call_expr` removed
- `detach_stmt` removed
- `channel_decl` removed
- `for_await_stmt` removed
- Keywords `async`/`await`/`detach`/`channel` (declaration syntax) + Chinese `异步`/`等待`/`分离`/`通道` (declaration syntax) all removed
