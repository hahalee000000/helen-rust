#!/usr/bin/env bash
# Stub candidate for diff.sh tests — mimics the future Rust `helen --conformance`
# JSON contract for the hello-world program.
set -euo pipefail
echo '{"stdout": "hello, world\n", "stderr": "", "exit_code": 0, "error_classes": []}'
