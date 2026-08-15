#!/usr/bin/env bash
# check-parity.sh — M14 Task 14.3: full M13 conformance sweep against a
# release build. Runs every parity gate and reports a single pass/fail.
#
# Usage:
#   bash scripts/check-parity.sh            # full sweep (release build)
#   bash scripts/check-parity.sh --quick    # skip benchmarks + coverage
#   bash scripts/check-parity.sh --fast     # tests only (debug build)
#
# Exit code 0 = all gates green; 1 = any gate failed.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-full}"
FAILED=0
PASSED=0

step() {
  local name="$1"; shift
  echo ""
  echo "═══ $name ═══"
  if "$@" >/tmp/parity-${name// /_}.log 2>&1; then
    echo "  ✓ $name"
    PASSED=$((PASSED+1))
  else
    echo "  ✗ $name (see /tmp/parity-${name// /_}.log)"
    FAILED=$((FAILED+1))
  fi
}

echo "── helen-rust parity sweep ($MODE) ──"

# 0. Workspace tests (Rust unit + integration + Tier C + fuzz).
step "cargo-test-workspace" timeout 900 cargo test --workspace

# 1. Release build (D1/D5 artifact). Built WITH python-ffi so the FFI corpus
#    tests (import "math" etc.) match their goldens; without it those 4
#    interpreter programs fail with "Python module imports are n..." and the
#    Tier A + error-parity gates would regress.
step "cargo-build-release" timeout 600 cargo build --release --features python-ffi

# 2. Tier A: differential run vs Python reference (--run --mock-llm).
if [[ -f scripts/diff-tier-a.sh ]]; then
  step "tier-a-differential" timeout 600 bash scripts/diff-tier-a.sh
fi

# 3. Tier B: subprocess suites vs the Rust binary.
if [[ -f scripts/diff-tier-b.sh ]]; then
  step "tier-b-subprocess" timeout 600 bash scripts/diff-tier-b.sh
fi

# 4. Display corpus byte-identical check.
step "display-corpus" python3 tests/conformance/check_golden.py tests/programs/display --suite display

# 5. Error parity sweep (E-code + exit codes).
if [[ -f scripts/gen-error-diff.py ]]; then
  step "error-parity" python3 scripts/gen-error-diff.py --all
fi

# 6. FFI examples (D3).
step "ffi-examples" bash -c '
  for f in examples/python_bridge/*.helen; do
    ./target/release/helen --run "$f" >/dev/null || exit 1
  done'

# 7. Python bridge DoD suite (D4) — requires the wheel or maturin develop.
step "bridge-dod" bash -c '
  cd crates/helen-python-bridge && python3 -m pytest tests/test_bridge_python.py -q'

# 8. Benchmarks (D6) — skip in fast modes.
if [[ "$MODE" != "--fast" ]]; then
  step "benchmarks" timeout 600 bash scripts/bench.sh --runs 3
fi

# 9. Coverage gate (D6/D8) — skip in fast modes.
if [[ "$MODE" == "full" ]] && command -v cargo-llvm-cov >/dev/null 2>&1; then
  step "coverage" timeout 900 cargo llvm-cov --no-clean --workspace \
    --exclude helen-lsp --exclude helen-ffi --exclude helen-python-bridge
fi

echo ""
echo "════════════════════════════════════════"
echo "parity sweep: $PASSED passed, $FAILED failed"
exit "$FAILED"
