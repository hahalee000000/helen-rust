#!/usr/bin/env bash
# diff-tier-b.sh — Tier B: run subprocess-based pytest suites against the
# Rust binary by prepending it to PATH (drop-in `helen` swap, D10).
#
# Usage:
#   diff-tier-b.sh <suite...>     # e.g. language agent cli ffi
#
# The Python suites invoke `helen <args>` via subprocess; with the Rust
# binary first on PATH they exercise the candidate directly. A test failure
# is a parity bug (Python side is the oracle).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELEN_SRC="${HELEN_SRC:-$HOME/helen}"
CAND="${HELEN_CANDIDATE:-$ROOT/target/release/helen}"
BIN_DIR="$(mktemp -d)"
trap 'rm -rf "$BIN_DIR"' EXIT

if [[ ! -x "$CAND" ]]; then
  echo "candidate not built: $CAND" >&2
  exit 2
fi

# Drop-in `helen` symlink (also expose `helen.cli`-style invocation via PATH).
ln -sf "$CAND" "$BIN_DIR/helen"

suites=("$@")
if [[ ${#suites[@]} -eq 0 ]]; then
  suites=(language agent cli)
fi

cd "$HELEN_SRC"
export PATH="$BIN_DIR:$PATH"
export HELEN_API_KEY="test-dummy-key-for-ci"
export HELEN_BINARY="$CAND"
# The reference test suite assumes debug output is enabled; an ambient
# HELEN_DEBUG=0 in the caller's environment would flip `_debug()` to "" and
# fail py-side tests that are actually about the *candidate* binary.
export HELEN_DEBUG=1

failed=0
for suite in "${suites[@]}"; do
  if [[ ! -d "tests/$suite" ]]; then
    echo "[$suite] no such suite dir" >&2
    failed=1
    continue
  fi
  echo "── Tier B: pytest tests/$suite (candidate: $CAND) ──"
  python3 -m pytest "tests/$suite" -x -q \
    --no-header -p no:cacheprovider \
    --ignore=tests/$suite/__pycache__ 2>&1 | tail -25
  rc=${PIPESTATUS[0]}
  echo "[$suite] pytest rc=$rc"
  if [[ $rc -ne 0 ]]; then failed=1; fi
done
exit $failed
