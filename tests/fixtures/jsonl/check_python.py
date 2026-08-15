#!/usr/bin/env python3
"""M14 D8 — Python-side check of Rust-written transcript JSONL.

Reads `rust_written.jsonl` (written by the Rust `JsonlBackend`) with the
Python reference's `JSONLBackend` and asserts the messages round-trip.

Run from the repo root:
    HELEN_SRC=$HOME/helen python3 tests/fixtures/jsonl/check_python.py
"""

import os
import sys

HELEN_SRC = os.environ.get("HELEN_SRC", os.path.expanduser("~/helen"))
sys.path.insert(0, HELEN_SRC)

from helen.runtime.transcript_store import JSONLBackend  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.path.join(HERE, "rust_written.jsonl")


def main() -> int:
    backend = JSONLBackend(PATH)
    items = backend.load_all()
    roles = [getattr(it, "role", None) for it in items]
    contents = [getattr(it, "content", None) for it in items]
    assert roles == ["user", "assistant"], f"roles mismatch: {roles}"
    assert contents == ["rust wrote this", "and rust replied"], (
        f"content mismatch: {contents}"
    )
    print(f"OK: Python read {len(items)} Rust-written messages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
