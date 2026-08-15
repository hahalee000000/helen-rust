#!/usr/bin/env bash
# bench.sh — M13 Task 13.6: comparative benchmarks, Python reference vs Rust candidate.
#
# Runs each program in benchmarks/programs/ through BOTH implementations
# (median of N runs) and emits a comparison table. Fails (exit 1) if the
# Rust implementation is > 2x SLOWER than the Python reference on any case.
#
# Usage:
#   bash scripts/bench.sh            # all benchmarks, 5 runs each
#   bash scripts/bench.sh --runs 3   # median of 3
#   bash scripts/bench.sh --ci       # CI mode: 3 runs, compact table
#
# Environment:
#   HELEN_SRC   path to Python reference (default ~/helen)
#   HELEN_PY    python helen CLI binary (default $HELEN_SRC/../helenenv/bin/helen,
#               falls back to `helen` on PATH)
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="$ROOT/benchmarks/programs"
RUST="${HELEN_CANDIDATE:-$ROOT/target/release/helen}"
HELEN_SRC="${HELEN_SRC:-$HOME/helen}"

RUNS=5
CI_MODE=0
for arg in "$@"; do
    case "$arg" in
        --runs) RUNS="${2:-5}"; shift 2 ;;
        --ci) CI_MODE=1; RUNS=3 ;;
    esac
done

# Locate the Python helen CLI.
if [[ -n "${HELEN_PY:-}" ]]; then
    PY_HELEN="$HELEN_PY"
elif [[ -x "$HELEN_SRC/../helenenv/bin/helen" ]]; then
    PY_HELEN="$HELEN_SRC/../helenenv/bin/helen"
else
    PY_HELEN="$(command -v helen || true)"
fi
if [[ -z "${PY_HELEN:-}" || ! -x "${PY_HELEN:-}" ]]; then
    echo "bench.sh: cannot find Python helen CLI (set HELEN_PY)" >&2
    exit 2
fi
if [[ ! -x "$RUST" ]]; then
    echo "bench.sh: Rust binary not found at $RUST (build first)" >&2
    exit 2
fi

median_ms() {
    # $1 = command to time; prints median of RUNS wall-clock ms to stdout.
    local cmd="$1" t
    local times=()
    for _ in $(seq 1 "$RUNS"); do
        t=$(/usr/bin/time -f "%e" bash -c "$cmd" 2>&1 >/dev/null)
        times+=("$t")
    done
    # sort numerically, take middle
    local sorted
    sorted=$(printf '%s\n' "${times[@]}" | sort -n)
    echo "$sorted" | sed -n "$(( (RUNS + 1) / 2 ))p"
}

run_py()  { ( cd "$HELEN_SRC" && "$PY_HELEN" "$1" >/dev/null 2>&1; ) }
run_rust(){ ( cd "$ROOT" && "$RUST" --run "$1" >/dev/null 2>&1; ) }
# Make the helpers visible to the bash -c subshells used by median_ms.
export -f run_py run_rust
export HELEN_SRC PY_HELEN ROOT RUST

printf "%-22s %10s %10s %8s  %s\n" "benchmark" "python(ms)" "rust(ms)" "ratio" "status"
printf "%s\n" "------------------------------------------------------------------------"

FAIL=0
for f in "$BENCH_DIR"/*.helen; do
    name=$(basename "$f" .helen)
    py_ms=$(median_ms "run_py '$f'")
    rs_ms=$(median_ms "run_rust '$f'")
    ratio=$(awk -v p="$py_ms" -v r="$rs_ms" 'BEGIN { if (p+0<=0) print "n/a"; else printf "%.2f", r/p }')
    status="ok"
    if [[ "$ratio" != "n/a" ]]; then
        if awk -v r="$ratio" 'BEGIN { exit !(r > 2.0) }'; then
            status="REGRESSION>2x"
            FAIL=1
        fi
    fi
    printf "%-22s %10s %10s %8s  %s\n" "$name" "$py_ms" "$rs_ms" "$ratio" "$status"
done

echo ""
if [[ "$FAIL" -eq 0 ]]; then
    echo "bench.sh: OK — no Rust regression > 2x vs Python reference."
else
    echo "bench.sh: FAIL — regression(s) > 2x detected." >&2
fi
exit "$FAIL"
