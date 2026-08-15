#!/bin/bash
# M2 semantic differential sweep: Rust vs Python on corpus files.
#
# Compares `helen --semantic-only` against `reference.py --semantic-only`
# (both emit {"exit_code":N,"e_codes":[...]} with byte-identical JSON).
#
# Usage: bash scripts/diff-semantic.sh [corpus_dir]
set -u
HELEN=./target/debug/helen
REF="python3 tests/conformance/reference.py"
CORPUS="${1:-tests/programs}"
PASS=0; FAIL=0; FAILED=""
for f in $(find "$CORPUS" -name "*.helen" | sort); do
  r=$($HELEN --semantic-only "$f" 2>&1)
  p=$($REF --semantic-only "$f" 2>&1)
  if [ "$r" == "$p" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1)); FAILED="$FAILED $f"
  fi
done
echo "semantic-diff PASS=$PASS FAIL=$FAIL (corpus: $CORPUS)"
for f in $FAILED; do echo "FAILED: $f"; done
[ "$FAIL" -eq 0 ]
