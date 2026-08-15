#!/usr/bin/env bash
# diff.sh — one-file differential runner (M0 Task 0.4)
#
# Runs the reference (Python) interpreter on <file.helen>, then — if a
# candidate binary exists — runs the candidate with the same three-tuple
# contract and compares stdout / exit_code / error_classes.
#
# Verdicts:
#   VERDICT: MATCH      exit 0
#   VERDICT: MISMATCH   exit 1  (details listed)
#   VERDICT: SKIP       exit 2  (candidate not built)
#
# Candidate binary: $HELEN_CANDIDATE (default <repo>/target/release/helen).
# Reference source: $HELEN_SRC (default ~/helen).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REF="$ROOT/tests/conformance/reference.py"
CAND="${HELEN_CANDIDATE:-$ROOT/target/release/helen}"
HELEN_SRC="${HELEN_SRC:-$HOME/helen}"

file="${1:-}"
if [[ -z "$file" ]]; then
  echo "usage: diff.sh <file.helen>" >&2
  exit 2
fi
if [[ ! -f "$file" ]]; then
  echo "diff.sh: file not found: $file" >&2
  exit 2
fi

# LLM-dependent programs need the deterministic mock on both sides.
MOCK=""
if grep -qE 'llm[[:space:]]+(act|if)' "$file"; then
  MOCK="--mock-llm"
fi

ref_json="$(HELEN_SRC="$HELEN_SRC" python3 "$REF" $MOCK "$file")"
ref_rc=$?
if [[ $ref_rc -ne 0 ]]; then
  echo "diff.sh: reference.py failed (rc=$ref_rc)" >&2
  exit 2
fi

echo "===== $file ====="
echo "--- REFERENCE (python) ---"
echo "$ref_json" | python3 -m json.tool

if [[ ! -x "$CAND" ]]; then
  echo "--- CANDIDATE: not built ($CAND) — skipping comparison"
  echo "VERDICT: SKIP (candidate not built)"
  exit 2
fi

cand_json="$("$CAND" --conformance $MOCK "$file")"
cand_rc=$?
if [[ $cand_rc -ne 0 ]]; then
  echo "diff.sh: candidate failed (rc=$cand_rc)" >&2
  echo "$cand_json" >&2
  exit 2
fi

echo "--- CANDIDATE (rust) ---"
echo "$cand_json" | python3 -m json.tool

python3 - "$ref_json" "$cand_json" <<'PY'
import json
import sys

ref = json.loads(sys.argv[1])
cand = json.loads(sys.argv[2])

diffs = []
if ref["stdout"] != cand["stdout"]:
    diffs.append("stdout")
if ref["exit_code"] != cand["exit_code"]:
    diffs.append("exit_code")
if ref["error_classes"] != cand["error_classes"]:
    diffs.append("error_classes")

if diffs:
    print(f"VERDICT: MISMATCH ({', '.join(diffs)})")
    sys.exit(1)
print("VERDICT: MATCH")
PY
