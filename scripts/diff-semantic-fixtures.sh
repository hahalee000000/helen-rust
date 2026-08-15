#!/bin/bash
# M2 semantic fixture battery: exercise many analyzer paths differentially.
# Usage: bash scripts/diff-semantic-fixtures.sh
set -u
HELEN=./target/debug/helen
REF="python3 tests/conformance/reference.py"
FIXDIR=tests/conformance/fixtures/semantic
PASS=0; FAIL=0; FAILED=""
for f in "$FIXDIR"/*.helen; do
  r=$($HELEN --semantic-only "$f" 2>&1)
  p=$($REF --semantic-only "$f" 2>&1)
  if [ "$r" == "$p" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1)); FAILED="$FAILED $f"
    echo "MISMATCH $f"
    echo "  rust:   $r"
    echo "  python: $p"
  fi
done
echo "semantic-fixtures PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ]
