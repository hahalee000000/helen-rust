#!/usr/bin/env bash
# diff-lex.sh — differential token-stream comparison (M1 exit criterion).
#
# Runs the Python reference lexer and the Rust candidate lexer over the
# same source and compares every token: type, lexeme, line/col/end positions,
# and literal value (floats compared numerically).
#
# Usage: diff-lex.sh <file.helen>
#   VERDICT: MATCH      exit 0
#   VERDICT: MISMATCH   exit 1  (first differing token printed)
#   VERDICT: SKIP       exit 2  (candidate not built)
#
# Environment:
#   HELEN_SRC        path to Python interpreter source (default ~/helen)
#   HELEN_CANDIDATE  path to the Rust `helen` binary (default target/release/helen)
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REF="$ROOT/tests/conformance/reference.py"
CAND="${HELEN_CANDIDATE:-$ROOT/target/release/helen}"
HELEN_SRC="${HELEN_SRC:-$HOME/helen}"

file="${1:-}"
if [[ -z "$file" || ! -f "$file" ]]; then
  echo "usage: diff-lex.sh <file.helen>" >&2
  exit 2
fi

ref_json="$(HELEN_SRC="$HELEN_SRC" python3 "$REF" --lex "$file")"
ref_rc=$?
if [[ $ref_rc -ne 0 ]]; then
  echo "diff-lex.sh: reference.py --lex failed (rc=$ref_rc)" >&2
  exit 2
fi

if [[ ! -x "$CAND" ]]; then
  echo "diff-lex.sh: candidate not built ($CAND) — run: cargo build --release -p helen-rust"
  echo "VERDICT: SKIP (candidate not built)"
  exit 2
fi

cand_json="$("$CAND" --lex "$file")"
cand_rc=$?
if [[ $cand_rc -ne 0 ]]; then
  echo "diff-lex.sh: candidate failed (rc=$cand_rc)" >&2
  exit 2
fi

python3 - "$ref_json" "$cand_json" <<'PY'
import json
import sys


def float_val(lit):
    try:
        return float(lit["value"])
    except (KeyError, ValueError):
        return None


def literal_eq(a, b):
    if a["kind"] != b["kind"]:
        return False
    if a["kind"] == "float":
        fa, fb = float_val(a), float_val(b)
        if fa is None or fb is None:
            return False
        return fa == fb
    return a.get("value") == b.get("value")


def main():
    ref = json.loads(sys.argv[1])
    cand = json.loads(sys.argv[2])
    n = max(len(ref), len(cand))
    for i in range(n):
        if i >= len(ref):
            print(f"MISMATCH at token {i}: reference ran out (candidate extra: {cand[i]})")
            sys.exit(1)
        if i >= len(cand):
            print(f"MISMATCH at token {i}: candidate ran out (reference extra: {ref[i]})")
            sys.exit(1)
        a, b = ref[i], cand[i]
        if a["type"] != b["type"]:
            print(f"MISMATCH at token {i}: type ref={a['type']} cand={b['type']} lexeme={a['lexeme']!r}")
            sys.exit(1)
        if a["lexeme"] != b["lexeme"]:
            print(f"MISMATCH at token {i}: lexeme ref={a['lexeme']!r} cand={b['lexeme']!r}")
            sys.exit(1)
        for k in ("line", "col", "end_line", "end_col"):
            if a[k] != b[k]:
                print(f"MISMATCH at token {i}: {k} ref={a[k]} cand={b[k]}")
                sys.exit(1)
        if not literal_eq(a["literal"], b["literal"]):
            print(f"MISMATCH at token {i}: literal ref={a['literal']} cand={b['literal']}")
            sys.exit(1)
    print(f"VERDICT: MATCH ({len(ref)} tokens)")
    sys.exit(0)


main()
PY
