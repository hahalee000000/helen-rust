#!/usr/bin/env bash
# extract-corpus.sh — rerun Tier-A extraction + golden capture (M0 Task 0.6).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "── extracting pytest sources (Tier A) ──"
python3 tests/conformance/extract_corpus.py tests/interpreter \
  --out tests/programs/pytest --suite interpreter || echo "(!) interpreter suite: none extractable"

echo "── capturing authored goldens ──"
python3 tests/conformance/capture_golden.py tests/programs/authored \
  --out tests/conformance/golden --suite authored

echo "✅ extract-corpus.sh complete"
