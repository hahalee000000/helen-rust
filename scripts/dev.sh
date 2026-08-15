#!/usr/bin/env bash
# dev.sh — one-command development loop (M0 Task 0.6).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "── cargo fmt ──"
cargo fmt --all -- --check

echo "── cargo clippy ──"
cargo clippy --workspace --all-targets -- -D warnings

echo "── cargo test ──"
cargo test --workspace

echo "── python conformance tests ──"
python3 -m pytest tests/conformance/ -q

echo "── conformance over authored corpus ──"
for f in tests/programs/authored/*.helen; do
  "$ROOT/scripts/diff.sh" "$f" || true
done

echo "✅ dev.sh complete"
