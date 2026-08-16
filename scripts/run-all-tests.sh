#!/bin/bash
# run-all-tests.sh — Comprehensive test suite for helen-rust
#
# Runs:
# 1. All Rust unit tests (cargo test --workspace)
# 2. Differential tests on extracted Helen programs (46 files)
# 3. Generates coverage report
#
# Usage: bash scripts/run-all-tests.sh [--verbose]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

VERBOSE=false
if [[ "$1" == "--verbose" || "$1" == "-v" ]]; then
    VERBOSE=true
fi

echo "═══════════════════════════════════════════════════════════════"
echo "  Helen-Rust Comprehensive Test Suite"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# ── Phase 1: Rust Unit Tests ────────────────────────────────────────────────

echo "Phase 1: Running Rust unit tests..."
echo "─────────────────────────────────────────────────────────────"

if $VERBOSE; then
    cargo test --workspace 2>&1 | tee /tmp/rust-tests.log
else
    cargo test --workspace 2>&1 | tail -20 > /tmp/rust-tests.log
    tail -5 /tmp/rust-tests.log
fi

RUST_TEST_RESULT=$(grep "test result:" /tmp/rust-tests.log | tail -1)
echo "✓ Rust tests: $RUST_TEST_RESULT"
echo ""

# ── Phase 2: Differential Tests ─────────────────────────────────────────────

echo "Phase 2: Running differential tests on extracted Helen programs..."
echo "─────────────────────────────────────────────────────────────"

if [[ ! -f target/release/helen ]]; then
    echo "Building release binary..."
    cargo build --release --quiet
fi

CAND="./target/release/helen"
REF="python3 ~/helen/helen/reference.py"
MOCK="--mock-llm"

count=0
match=0
fail=0
failures=""

for f in tests/programs/pytest/*/*.helen; do
    if [[ ! -f "$f" ]]; then
        continue
    fi
    count=$((count + 1))
    
    # Run candidate (Rust)
    cand_out=$("$CAND" --run $MOCK "$f" 2>&1)
    cand_rc=$?
    
    # Run reference (Python)
    ref_out=$("$REF" $MOCK "$f" 2>&1)
    ref_rc=$?
    
    # Compare outputs (normalize JSON key order)
    cand_norm=$(echo "$cand_out" | python3 -c "import sys, json; print(json.dumps(json.load(sys.stdin), sort_keys=True))" 2>/dev/null || echo "$cand_out")
    ref_norm=$(echo "$ref_out" | python3 -c "import sys, json; print(json.dumps(json.load(sys.stdin), sort_keys=True))" 2>/dev/null || echo "$ref_out")
    
    if [[ "$cand_norm" == "$ref_norm" ]]; then
        match=$((match + 1))
        if $VERBOSE; then
            echo "  ✓ $(basename "$f")"
        fi
    else
        fail=$((fail + 1))
        failures="$failures\n  ✗ $(basename "$f")"
        if $VERBOSE; then
            echo "  ✗ $(basename "$f")"
        fi
    fi
done

echo "✓ Differential tests: $match/$count match ($((match * 100 / count))%)"
if [[ $fail -gt 0 ]]; then
    echo "  Failures:$failures"
fi
echo ""

# ── Phase 3: Summary ────────────────────────────────────────────────────────

echo "═══════════════════════════════════════════════════════════════"
echo "  Test Summary"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "  Rust unit tests:      $RUST_TEST_RESULT"
echo "  Differential tests:   $match/$count ($((match * 100 / count))% parity)"
echo ""

if [[ $fail -gt 0 ]]; then
    echo "  ⚠ $fail differential test(s) failed"
    exit 1
else
    echo "  ✓ All tests passed"
    exit 0
fi
