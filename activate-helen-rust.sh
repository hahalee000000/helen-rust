#!/usr/bin/env bash
# Convenience script to activate the helen-rust uv environment
# Usage: source ~/helen-rust/activate-helen-rust.sh
source ~/helen-rust-env/bin/activate
echo "✓ helen-rust env activated — helen $(helen --version 2>/dev/null || echo 'not found')"
