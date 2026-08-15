#!/usr/bin/env python3
"""check_golden.py — verify a corpus directory's current output matches the
captured goldens (byte-identical stdout, matching exit codes / error classes).

M14 Task 14.3: used by scripts/check-parity.sh as the display-parity gate.

Usage:
    python3 tests/conformance/check_golden.py tests/programs/display --suite display
"""

import argparse
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# __file__ is tests/conformance/check_golden.py → ROOT = repo root.
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BIN = os.path.join(ROOT, "target", "release", "helen")
GOLDEN = os.path.join(ROOT, "tests", "conformance", "golden")


def load_golden(suite: str) -> dict:
    path = os.path.join(GOLDEN, f"{suite}.jsonl")
    if not os.path.exists(path):
        return {}
    rows = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            rows[row["file"]] = row
    return rows


def run_file(path: str) -> dict:
    """Run a program with the release binary; return the result dict."""
    proc = subprocess.run(
        [BIN, "--run", path],
        capture_output=True,
        text=True,
        timeout=60,
    )
    # --run emits a JSON result envelope on stdout.
    try:
        data = json.loads(proc.stdout.strip().splitlines()[-1])
        return data
    except (json.JSONDecodeError, IndexError):
        return {
            "stdout": proc.stdout,
            "stderr": proc.stderr,
            "exit_code": proc.returncode,
            "error_classes": [],
        }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("corpus_dir")
    ap.add_argument("--suite", required=True)
    args = ap.parse_args()

    if not os.path.exists(BIN):
        print(f"(!) build the release binary first: cargo build --release ({BIN})")
        return 1

    goldens = load_golden(args.suite)
    if not goldens:
        print(f"(!) no goldens for suite {args.suite} in {GOLDEN}")
        return 1

    files = sorted(
        os.path.join(args.corpus_dir, n) for n in goldens
        if os.path.exists(os.path.join(args.corpus_dir, n))
    )
    if not files:
        print(f"(!) no corpus files matched in {args.corpus_dir}")
        return 1

    failed = 0
    for path in files:
        name = os.path.basename(path)
        want = goldens.get(name)
        if want is None:
            print(f"  MISSING-GOLDEN  {name}")
            failed += 1
            continue
        got = run_file(path)
        if got["stdout"] == want["stdout"] and got["exit_code"] == want["exit_code"]:
            if got["exit_code"] != 0 and got["error_classes"] != want["error_classes"]:
                print(f"  FAIL  {name}: error_classes {got['error_classes']} != {want['error_classes']}")
                failed += 1
                continue
            print(f"  ok    {name}")
        else:
            print(f"  FAIL  {name}: stdout/exit mismatch")
            print(f"    want: exit={want['exit_code']} stdout={want['stdout']!r}")
            print(f"    got:  exit={got['exit_code']} stdout={got['stdout']!r}")
            failed += 1

    total = len(files)
    print(f"\n{total - failed}/{total} byte-identical")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
