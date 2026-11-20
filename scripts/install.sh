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
#   ./scripts/install.sh --skip-frontend # skip frontend build (agent UI won't work)
#
# Requires: curl (or wget) + tar for the download path.
#           node + npm for the frontend build (default for --from-build).

set -euo pipefail

REPO="hahalee000000/helen-rust"
PREFIX="${PREFIX:-$HOME/.cargo/bin}"
WITH_BRIDGE=0
FROM_BUILD=0
SKIP_FRONTEND=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix) PREFIX="$2"; shift 2 ;;
    --with-bridge) WITH_BRIDGE=1; shift ;;
    --from-build) FROM_BUILD=1; shift ;;
    --skip-frontend) SKIP_FRONTEND=1; shift ;;
    -h|--help)
      echo "Usage: $0 [--prefix DIR] [--with-bridge] [--from-build] [--skip-frontend]"
      exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
done

echo "── helen-rust installer ──"
mkdir -p "$PREFIX"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ── Frontend build (for --from-build path) ──────────────────────────────────
build_frontend() {
  if [[ "$SKIP_FRONTEND" -eq 1 ]]; then
    echo "⚠  skipping frontend build (--skip-frontend)"
    echo "   agent UI will show blank page unless frontend is built separately"
    return 0
  fi

  local frontend_dir="$ROOT/crates/helen-agent/frontend"
  if [[ ! -d "$frontend_dir" ]]; then
    echo "⚠  frontend directory not found: $frontend_dir"
    echo "   skipping frontend build"
    return 0
  fi

  # Check if frontend is already built (has main JS bundle)
  if ls "$frontend_dir/dist/assets/index-"*.js >/dev/null 2>&1; then
    echo "✓ frontend already built (skipping rebuild)"
    return 0
  fi

  echo "── building frontend ──"
  if ! command -v npm >/dev/null 2>&1; then
    echo "(!) npm not found — cannot build frontend" >&2
    echo "    install Node.js or use --skip-frontend" >&2
    exit 1
  fi

  cd "$frontend_dir"
  echo "   npm install..."
  npm install --silent
  echo "   npm run build..."
  # Use vite build directly to avoid TypeScript errors in test files
  npx vite build
  cd "$ROOT"
  echo "✓ frontend built"
}

if [[ "$FROM_BUILD" -eq 1 ]]; then
  # Build frontend first (rust-embed needs the assets at compile time)
  build_frontend

  echo "── building binary (cargo build --release) ──"
  cd "$ROOT"
  cargo build --release
  cd - >/dev/null

  echo "── installing from local build (target/release/helen) ──"
  if [[ ! -x "$ROOT/target/release/helen" ]]; then
    echo "(!) target/release/helen not found after build" >&2
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
  echo "   (release binary includes pre-built frontend)"
fi

"$PREFIX/helen" --version

if [[ "$WITH_BRIDGE" -eq 1 ]]; then
  echo "── installing Python bridge wheel ──"
  python3 -m pip install --upgrade helen-rust
  echo "✓ bridge installed (import helen_rust)"
fi

echo "✅ helen-rust installed. Add '$PREFIX' to your PATH if needed."
