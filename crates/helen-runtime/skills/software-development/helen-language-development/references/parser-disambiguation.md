# Parser Disambiguation Patterns

Techniques for handling ambiguous syntax where the same keyword can start both a statement and an expression.

## Lookahead + Backtracking Pattern

When a keyword (e.g., `llm act`) can be followed by either:
- A statement form: `llm act target(args) "desc"`
- An expression form: `llm act <expr>`

Use **lookahead with backtracking** to disambiguate.

### Implementation Pattern

```python
def _llm_act_stmt(self) -> StatementNode:
    start = self._previous()  # LLM token
    self._consume(TokenType.ACT, "Expected 'act' after 'llm'.")
    
    # Step 1: Quick check — if not IDENTIFIER, must be expression form
    if not self._check(TokenType.IDENTIFIER):
        prompt_expr = self._expression()
        return ExprStmtNode(expression=LlmActExprNode(prompt=prompt_expr, ...))
    
    # Step 2: IDENTIFIER found — need to distinguish:
    # - Statement: llm act target(args) "desc"
    # - Expression: llm act variable_name
    
    # Save position for backtracking
    saved_pos = self._pos
    ident_tok = self._advance()  # consume IDENTIFIER
    
    # Step 3: Look at what follows the IDENTIFIER
    if self._check(TokenType.LEFT_PAREN) or self._check(TokenType.STRING):
        # Statement form: IDENTIFIER followed by ( or STRING
        # Continue parsing as statement...
        args: dict[str, ExpressionNode] = {}
        if self._check(TokenType.LEFT_PAREN):
            self._advance()
            # ... parse arguments ...
            self._consume(TokenType.RIGHT_PAREN, "Expected ')' after arguments.")
        desc_tok = self._consume(TokenType.STRING, "Expected description...")
        return LlmActStmtNode(target=ident_tok.lexeme, arguments=args, ...)
    else:
        # Expression form: IDENTIFIER is just a variable
        # Backtrack to before IDENTIFIER and parse as expression
        self._pos = saved_pos  # ← CRITICAL: restore position
        prompt_expr = self._expression()
        return ExprStmtNode(expression=LlmActExprNode(prompt=prompt_expr, ...))
```

### Key Principles

1. **Save `self._pos` before lookahead** — not `self._current` (which is a method, not a field)
2. **Restore `self._pos` to backtrack** — this "un-consumes" tokens
3. **Check tokens that uniquely identify each form** — for `llm act`:
   - Statement: IDENTIFIER followed by `(` or STRING
   - Expression: anything else (variable, expression, etc.)
4. **Wrap expression form in `ExprStmtNode`** when used as statement — maintains AST consistency

### Why This Works

The parser maintains a token stream with a position index (`self._pos`). By saving and restoring this index, we can "peek ahead" without permanently consuming tokens. This is simpler than full LL(k) lookahead and works well for local ambiguities.

### Common Pitfalls

#### ❌ Wrong: Saving `self._current`
```python
current_pos = self._current  # ← self._current is a METHOD, not a field
# ... later ...
self._current = current_pos  # ← TypeError: can't assign to method
```

#### ✅ Right: Save `self._pos`
```python
saved_pos = self._pos  # ← integer index
# ... later ...
self._pos = saved_pos  # ← restore position
```

#### ❌ Wrong: Not backtracking
```python
ident_tok = self._advance()  # consume IDENTIFIER
if not self._check(TokenType.LEFT_PAREN):
    # Forgot to backtrack!
    prompt_expr = self._expression()  # ← starts AFTER IDENTIFIER, misses it
```

#### ✅ Right: Backtrack before parsing expression
```python
saved_pos = self._pos
ident_tok = self._advance()
if not self._check(TokenType.LEFT_PAREN):
    self._pos = saved_pos  # ← backtrack to before IDENTIFIER
    prompt_expr = self._expression()  # ← now includes IDENTIFIER
```

### When to Use This Pattern

Use lookahead + backtracking when:
- Same keyword starts multiple syntactic forms
- Forms can be distinguished by looking 1-2 tokens ahead
- The ambiguity is local (not spanning large constructs)

**Examples in Helen:**
- `llm act` — statement vs expression form
- `return` — with or without value (though this uses simpler `_check` logic)

### Alternatives

#### 1. Pratt Parser Precedence
For expression-level ambiguity (e.g., `a + b * c`), use precedence levels instead of backtracking.

#### 2. Separate Keywords
If ambiguity is confusing, consider separate keywords:
- `llm act` (statement) vs `llm call` (expression)
- More verbose but clearer

#### 3. Context Flags
Track parser context (e.g., `self._in_expression`) and dispatch accordingly. More complex but handles nested ambiguities.

### Testing the Pattern

Always test both forms:

```python
def test_statement_form():
    source = 'llm act Translator(text="hello") "translate"'
    # ... parse ...
    assert isinstance(stmt, LlmActStmtNode)

def test_expression_form_string():
    source = 'llm act "translate hello"'
    # ... parse ...
    assert isinstance(stmt, ExprStmtNode)
    assert isinstance(stmt.expression, LlmActExprNode)

def test_expression_form_variable():
    source = 'let msg = "hello"\nllm act msg'
    # ... parse ...
    assert isinstance(stmt, ExprStmtNode)
    assert isinstance(stmt.expression, LlmActExprNode)

def test_expression_form_concatenation():
    source = 'llm act "translate " + text'
    # ... parse ...
    assert isinstance(stmt, ExprStmtNode)
```

### Related Patterns

- **Pratt Parsing**: For operator precedence (see `helen/core/parser.py` `_expression()` method)
- **Recursive Descent**: For structured constructs (if/while/for/function)
- **Panic Mode Recovery**: For error handling (see `_synchronize()` method)

## Summary

Lookahead + backtracking is a pragmatic solution for local syntactic ambiguities. It's simpler than full LL(k) parsing and works well when forms can be distinguished by 1-2 token lookahead. The key is saving/restoring `self._pos` correctly and testing both forms thoroughly.
