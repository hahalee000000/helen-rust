# Parser Pattern: Optional Expressions

When a keyword can be followed by an optional expression (e.g., `llm act` with or without a prompt), the parser needs to determine whether an expression follows or the statement has ended.

## The Problem

Given `let result = llm act`, the parser must decide:
1. Is there an expression after `act`? (e.g., `llm act "prompt"`)
2. Or has the statement ended? (e.g., `llm act` followed by newline + `return`)

Naively calling `self._expression()` fails because it tries to consume the next token as an expression start, producing errors like "Expected expression, got RETURN".

## The Solution: Three-Layer Detection

Use three complementary signals to detect bare form:

### Layer 1: Statement Terminators
Check if the next token is a statement terminator or statement-starting keyword:

```python
bare_form_tokens = (
    # Statement terminators
    TokenType.RIGHT_BRACE, TokenType.SEMICOLON, TokenType.EOF,
    # Statement-starting keywords (next statement begins)
    TokenType.RETURN, TokenType.LET, TokenType.CONST,
    TokenType.IF, TokenType.FOR, TokenType.WHILE,
    TokenType.BREAK, TokenType.CONTINUE, TokenType.MATCH,
    TokenType.TRY, TokenType.THROW,
    TokenType.LLM, TokenType.CALL, TokenType.ASYNC,
)

if self._check(*bare_form_tokens):
    prompt_expr = None  # bare form
```

### Layer 2: Newline Boundary Detection (Critical!)

**Problem**: Statement keywords alone miss cases like:
```helen
import std.core.*
let result = llm act
print(result)    // IDENTIFIER on next line — not a keyword!
```

Here `print` is an `IDENTIFIER`, not in `bare_form_tokens`, so the keyword check fails. The parser tries to consume `print(result)` as the expression argument to `llm act`.

**Solution**: Use token **line numbers** to detect newline boundaries:

```python
act_token = self._previous()  # after consuming ACT

if self._check(*bare_form_tokens):
    prompt_expr = None
elif self._current().line > act_token.line:
    prompt_expr = None  # newline = statement boundary
else:
    prompt_expr = self._expression()
```

**Why this works**: Each token has `line` and `col` attributes. If the next token is on a different line than `act`, treat it as a new statement.

**Key insight**: Helen's lexer doesn't emit newline tokens, but tokens carry line numbers. This lets the parser detect "logical newlines" without explicit newline tokens.

### Layer 3: Expression Parsing

If neither terminator nor newline is detected, parse the expression normally:
```python
else:
    prompt_expr = self._expression()
```

## Key Token Categories

### Statement Terminators
- `RIGHT_BRACE` — end of block: `main { llm act }`
- `SEMICOLON` — explicit statement end: `llm act;`
- `EOF` — end of file

### Statement-Starting Keywords
These indicate a NEW statement is beginning, so the current one must have ended:
- `RETURN`, `LET`, `CONST` — declarations/returns
- `IF`, `FOR`, `WHILE`, `MATCH` — control flow
- `BREAK`, `CONTINUE` — loop control
- `TRY`, `THROW` — exception handling
- `LLM`, `CALL`, `ASYNC` — Helen-specific keywords

### NOT Included
- `IDENTIFIER` — could be start of expression (`llm act text`)
- `STRING`, `NUMBER` — literal expressions
- `LEFT_PAREN` — grouping or call
- Operators — handled by expression parser precedence

## Pitfalls

### Missing Keywords Cause Parse Errors

If you only check `RIGHT_BRACE`, `SEMICOLON`, `EOF`, then code like:
```helen
main {
    let result = llm act
    return result    // ← ERROR: tries to parse "return" as expression
}
```

The parser sees `return` after `llm act`, doesn't recognize it as a terminator, tries to parse it as an expression, and fails with "Expected expression, got RETURN".

**Fix**: Include ALL statement-starting keywords in the check.

### Expression vs Statement Ambiguity

Some tokens can start both expressions and statements:
- `IDENTIFIER` — could be variable reference (expression) or start of new statement
- `LEFT_PAREN` — could be grouping (expression) or... actually always expression

For `IDENTIFIER`, the expression parser handles it correctly (parses as variable reference). The ambiguity is only with keywords that CANNOT start expressions.

### Newlines: Invisible but Detectable via Line Numbers

Helen's lexer does not emit newline tokens (they're whitespace). However, each token carries `line` and `col` attributes, so the parser can detect "logical newlines" by comparing line numbers:

```python
if self._current().line > previous_token.line:
    # Next token is on a different line — treat as statement boundary
```

This is essential for cases like:
```helen
import std.core.*
let result = llm act
print(result)    // print is IDENTIFIER, not a keyword — keyword check fails
```

Without newline detection, `print(result)` would be consumed as the expression argument to `llm act`, causing `result` to be undefined.

**General principle**: When making expressions optional, always check line boundaries in addition to keywords. Keywords cover explicit statement starters; line numbers cover everything else that starts on a new line.

## Alternative: Backtracking

For more complex cases, save position, try parsing expression, backtrack on failure:

```python
saved_pos = self._pos
try:
    expr = self._expression()
except ParseError:
    self._pos = saved_pos
    expr = None
```

**Downside**: Requires exception-based control flow, slower, harder to debug.

**When to use**: When the lookahead set is too large or dynamic (e.g., depends on context).

## Testing

Test all three forms:
```helen
// 1. With expression
let a = llm act "prompt"

// 2. Bare form (newline + next statement)
let b = llm act
return b

// 3. Bare form (before closing brace)
main {
    llm act
}
```

Verify:
- Parse succeeds without errors
- AST has `prompt=None` for bare forms
- Interpreter handles `prompt=None` correctly (uses agent prompt as fallback)
