#!/usr/bin/env bash
# Publish all helen-rust crates to crates.io in dependency order.
#
# Prerequisites:
#   1. cargo login <your-api-token>   (get token from https://crates.io/me)
#   2. git working tree must be clean (commit + push first)
#   3. All crates must have the same version in Cargo.toml
#
# Usage:
#   ./scripts/publish-crates.sh            # dry-run (default)
#   ./scripts/publish-crates.sh --publish  # actually publish
#   ./scripts/publish-crates.sh --verify   # verify all crates already published

set -euo pipefail

CRATES=(
  helen-core
  helen-stdlib
  helen-parser
  helen-semantic
  helen-runtime
  helen-interpreter
  helen-lsp
  helen-ffi
  helen-rust
)

MODE="${1:---dry-run}"

case "$MODE" in
  --publish)
    echo "🚀 Publishing to crates.io..."
    for crate in "${CRATES[@]}"; do
      echo ""
      echo "=== Publishing $crate ==="
      cargo publish -p "$crate" --registry crates-io
      echo "✅ $crate published"
      # Wait for crates.io index to update before publishing dependents
      sleep 15
    done
    echo ""
    echo "🎉 All crates published!"
    ;;
  --verify)
    echo "🔍 Verifying all crates exist on crates.io..."
    for crate in "${CRATES[@]}"; do
      if cargo search "$crate" --limit 1 2>/dev/null | grep -q "^${crate} "; then
        version=$(cargo search "$crate" --limit 1 | head -1 | sed 's/.*= "//;s/".*//')
        echo "  ✅ $crate = $version"
      else
        echo "  ❌ $crate NOT FOUND on crates.io"
      fi
    done
    ;;
  --dry-run|*)
    echo "🧪 Dry-run: verifying all crates can be published..."
    for crate in "${CRATES[@]}"; do
      echo ""
      echo "=== Dry-run: $crate ==="
      cargo publish --dry-run -p "$crate" --registry crates-io 2>&1 | tail -3
    done
    echo ""
    echo "Note: Dependents will show 'no matching package' until leaf crates"
    echo "are actually published. This is expected in dry-run mode."
    echo "Run with --publish to actually publish."
    ;;
esac
