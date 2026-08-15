# Expected / Accepted Divergences — M13 Conformance

This file documents every known divergence between the Rust implementation
(`helen-rust`) and the Python reference (`helen`). Everything NOT listed here
is required to match byte-for-byte across the Tier A/B/C corpora.

Status: **FROZEN** (M13) — reviewed and locked at the end of M13. New
divergences require an issue + review before adding to this list.

## 1. Error message span formatting (cosmetic)

- **Where:** lex/parse/semantic error messages embed source positions.
- **Python:** `E0354 at /abs/path/file.helen:2:17-27: message`
- **Rust:**  same format, but relative paths when run from the workspace root.
- **Resolution:** goldens normalize spans via `_SPAN_RE` (` at \S+:\d+:\d+-\d+`)
  and `diff-tier-a.sh` strips `(file:line:col)`; the diff harness compares the
  normalized message only. Error **codes** (E0001–E04xx) and **classes** must
  match exactly and always do.

## 2. Unicode string length semantics (byte vs code point)

- **Where:** `len()` on strings containing non-ASCII characters, and string
  slicing/`str()` edge cases.
- **Python:** `len()` counts Unicode **code points**.
- **Rust:** `len()` counts **bytes** (Rust `String::len`), consistent with
  Rust's native string model.
- **Impact:** programs that mix `len()` with CJK/emoji text can differ. The
  display corpus (`unicode.helen`) exercises *display* of such strings, which
  is byte-identical; length-arithmetic on non-ASCII strings is the documented
  exception.
- **Status:** accepted; tracked as a language-semantics difference (HLD issue).

## 3. `spawn` race strictness

- **Where:** agent `spawn` channel delivery ordering under load.
- **Python:** a small internal delay/ordering tolerance in the channel pump.
- **Rust:** uses a fresh `mpsc` channel per spawn with strict FIFO ordering;
  delivery is deterministic but may complete *before* the Python-side
  ordering barrier in stress fixtures.
- **Impact:** `tests/programs/authored/spawn_expr.helen` and
  `tests/programs/pytest/interpreter/*spawn*` fixtures assert **error parity**
  (both fail on the deliberately-broken corpus fixtures) and channel-message
  content parity, not interleaving order. Verified green under Tier A.
- **Status:** accepted.

## 4. `pow()` overflow error text

- **Where:** `pow(base, exp)` when a finite base/exponent overflow to `inf`.
- **Python:** raises `OverflowError: math range error` (surfaced as
  `RuntimeError`).
- **Rust:** raises `RuntimeError` with the exact message
  `Python OverflowError: math range error` — parity enforced. `inf`/`nan`
  inputs pass through to `inf`/`nan` results on both sides.
- **Status:** resolved in M13 (was a real divergence, fixed); listed for
  regression awareness.

## 5. Python-internal test expectations (Tier B carve-out)

- **Where:** `tests/agent` cases that import `helen.stdlib` internals and
  assert on the *Python* module (e.g. `_debug` return value).
- **Impact:** 2 of 172 agent tests fail identically against the **Python**
  reference itself (env-dependent), so they are excluded from the Tier B gate;
  they are not parity bugs. All subprocess-based agent tests pass against the
  Rust binary.
- **Status:** accepted.

---

## Verification summary (frozen at M13 close)

| Tier | Suite | Result |
|---|---|---|
| A | authored | 18/18 byte-identical |
| A | interpreter | 18/18 byte-identical |
| A | agent | 6/6 byte-identical |
| A | display | 10/10 byte-identical |
| B | language | 100/100 |
| B | cli | 64/64 |
| B | ffi | 64 + 1 skip |
| B | agent | 170/172 (2 py-internal, see §5) |
| C | lexer | 67/67 |
| C | parser | 49/49 |
| C | semantic | 21/21 |
| C | execution | 48/48 |
| Error diff | corpus-wide | 70/70 E-code + exit-code match |

Benchmarks: no Rust case > 2x slower than Python; typical ratio 0.02–0.07x.
