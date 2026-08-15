#!/usr/bin/env bash
# install.sh — M14 Task 14.1 standalone installer for helen-rust.
#
# Downloads the latest release binary from GitHub Releases (or installs from
# a local build), and optionally installs the Python bridge wheel.
#
# Usage:
#   ./scripts/install.sh                 # install binary to ~/.cargo/bin
#   ./scripts/install.sh --prefix DIR    # install binary to DIR
#   ./scripts/install.sh --with-bridge   # also pip-install the bridge wheel
#   ./scripts/install.sh --from-build    # install from local target/release
#
# Requires: curl (or wget) + tar for the download path.

set -euo pipefail

REPO="hahalee000000/helen-rust"
PREFIX="${PREFIX:-$HOME/.cargo/bin}"
WITH_BRIDGE=0
FROM_BUILD=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix) PREFIX="$2"; shift 2 ;;
    --with-bridge) WITH_BRIDGE=1; shift ;;
    --from-build) FROM_BUILD=1; shift ;;
    -h|--help)
      echo "Usage: $0 [--prefix DIR] [--with-bridge] [--from-build]"
      exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
done

echo "── helen-rust installer ──"
mkdir -p "$PREFIX"

if [[ "$FROM_BUILD" -eq 1 ]]; then
  echo "── installing from local build (target/release/helen) ──"
  ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  if [[ ! -x "$ROOT/target/release/helen" ]]; then
    echo "(!) target/release/helen not found — run: cargo build --release" >&2
    exit 1
  fi
  install -m 0755 "$ROOT/target/release/helen" "$PREFIX/helen"
  echo "✓ installed: $PREFIX/helen"
else
  echo "── downloading latest release binary ──"
  OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
  ARCH="$(uname -m)"
  case "$ARCH" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) echo "(!) unsupported arch: $ARCH" >&2; exit 1 ;;
  esac
  URL="https://github.com/$REPO/releases/latest/download/helen-${OS}-${ARCH}.tar.gz"
  TMP="$(mktemp -d)"
  echo "   fetching $URL"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$URL" -o "$TMP/helen.tar.gz"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$URL" -O "$TMP/helen.tar.gz"
  else
    echo "(!) need curl or wget" >&2; exit 1
  fi
  tar -xzf "$TMP/helen.tar.gz" -C "$TMP"
  install -m 0755 "$TMP/helen" "$PREFIX/helen"
  rm -rf "$TMP"
  echo "✓ installed: $PREFIX/helen"
fi

"$PREFIX/helen" --version

if [[ "$WITH_BRIDGE" -eq 1 ]]; then
  echo "── installing Python bridge wheel ──"
  python3 -m pip install --upgrade helen-rust
  echo "✓ bridge installed (import helen_rust)"
fi

echo "✅ helen-rust installed. Add '$PREFIX' to your PATH if needed."
