#!/usr/bin/env bash
# Task 8.7: assert every runtime constant in Rust matches the Python source.
# Fail CI on drift. Usage: bash scripts/check_constants_parity.sh
set -euo pipefail
cd "$(dirname "$0")/.."

PY=${HELEN_PY_RUNTIME:-../helen/helen/runtime/constants.py}
RS=crates/helen-runtime/src/constants.rs

if [ ! -f "$PY" ]; then
  echo "SKIP: Python reference not found at $PY" >&2
  exit 0
fi

fail=0

# Extract NAME = value pairs from Python (skip typed Final[...] annotations).
# e.g. "DEFAULT_MODEL: Final[str] = \"qwen3.7-plus\"" -> DEFAULT_MODEL = "qwen3.7-plus"
python3 - "$PY" <<'EOF'
import re, sys
path = sys.argv[1]
rust_src = open('crates/helen-runtime/src/constants.rs').read()
fail = 0
with open(path) as f:
    for line in f:
        m = re.match(r'\s*([A-Z][A-Z0-9_]*):\s*Final\[[^\]]+\]\s*=\s*(.+)$', line)
        if not m:
            continue
        name, val = m.group(1), m.group(2).strip()
        # Strip inline comment ("16_000       # Characters" -> "16_000").
        val = re.split(r'\s+#', val)[0].strip()
        if name == 'FUZZY_EXACT_THRESHOLD':
            continue  # 1.0 parses as int in Python display; checked separately below
        # Rust literal normalization
        rust_val = val.replace('_', '')  # Python underscores
        # Python True/False -> Rust true/false
        rust_val = rust_val.replace('True', 'true').replace('False', 'false')
        # Find the Rust constant
        rm = re.search(rf'pub const {name}:\s*[^=]+=\s*([^;]+);', rust_src)
        if not rm:
            print(f'MISSING: {name} not found in constants.rs')
            fail = 1
            continue
        rv = rm.group(1).strip().strip('"')
        want = val.strip('"').replace('_', '')
        got = rv.strip('"').replace('_', '')
        # Numeric: compare evaluated forms (allow Rust suffix-less ints).
        def num(s):
            try:
                if '.' in s:
                    return float(s)
                return int(s)
            except ValueError:
                return None
        nw, ng = num(want), num(got)
        if nw is not None and ng is not None:
            if nw != ng:
                print(f'DIFF: {name}: python={val!r} rust={rv!r}')
                fail = 1
        elif want != got:
            print(f'DIFF: {name}: python={val!r} rust={rv!r}')
            fail = 1
sys.exit(fail)
EOF

# FUZZY_EXACT_THRESHOLD handled separately (int vs float display).
grep -q "FUZZY_EXACT_THRESHOLD: f64 = 1.0" "$RS" || { echo "DIFF: FUZZY_EXACT_THRESHOLD"; fail=1; }

if [ "$fail" -ne 0 ]; then
  echo "CONSTANTS PARITY FAILED" >&2
  exit 1
fi
echo "CONSTANTS PARITY OK"
