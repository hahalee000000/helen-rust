#!/usr/bin/env python3
"""gen-error-diff.py — M13 Task 13.5 error-parity sweep.

Runs the candidate Rust binary and the Python reference over every corpus
program and produces `tests/conformance/error-diff.csv`:

    file,rust_exit,py_exit,rust_codes,py_codes,match

- exit codes must match (0 success / 2 semantic / 3 runtime / 1 lex-parse)
- error classes / E-codes must match (cosmetic span text is normalized)

Usage:
    python3 scripts/gen-error-diff.py [--suite authored|interpreter|agent|display|--all]

Exit 0 when every row matches, 1 otherwise.
"""

from __future__ import annotations

import csv
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CAND = Path(os.environ.get("HELEN_CANDIDATE", ROOT / "target" / "release" / "helen"))
REF = ROOT / "tests" / "conformance" / "reference.py"
OUT = ROOT / "tests" / "conformance" / "error-diff.csv"
SPAN = re.compile(r" at \S+:\d+:\d+-\d+|\x1b\[[0-9;]*m")

CORPORA = {
    "authored": ROOT / "tests" / "programs" / "authored",
    "interpreter": ROOT / "tests" / "programs" / "pytest" / "interpreter",
    "agent": ROOT / "tests" / "programs" / "pytest" / "agent",
    "display": ROOT / "tests" / "programs" / "display",
    "stdlib": ROOT / "tests" / "programs" / "stdlib",
}


def pick_suites(argv: list[str]) -> list[str]:
    if not argv or "--all" in argv:
        return sorted(CORPORA)
    return argv


def norm(s: str) -> str:
    s = SPAN.sub("", s)
    return "|".join(sorted(set(line.strip() for line in s.splitlines() if line.strip())))


def run_candidate(f: Path, mock: bool) -> dict:
    args = [str(CAND), "--run"]
    if mock:
        args.append("--mock-llm")
    args.append(str(f))
    p = subprocess.run(args, capture_output=True, text=True, cwd=ROOT, timeout=60)
    try:
        return json.loads(p.stdout)
    except json.JSONDecodeError:
        return {"exit_code": p.returncode, "stderr": p.stderr or p.stdout}


def run_reference(f: Path, mock: bool) -> dict:
    args = [sys.executable, str(REF)]
    if mock:
        args.append("--mock-llm")
    args.append(str(f))
    p = subprocess.run(args, capture_output=True, text=True, timeout=60)
    try:
        return json.loads(p.stdout)
    except json.JSONDecodeError:
        return {"exit_code": p.returncode, "stderr": p.stderr or p.stdout}


def error_codes(stderr: str) -> list[str]:
    return re.findall(r"\bE\d{4}\b", stderr)


def main() -> int:
    suites = pick_suites(sys.argv[1:])
    rows, total, bad = [], 0, 0
    for suite in suites:
        d = CORPORA[suite]
        if not d.is_dir():
            print(f"[{suite}] missing {d}", file=sys.stderr)
            continue
        for f in sorted(d.glob("*.helen")):
            src = f.read_text(encoding="utf-8")
            mock = "llm act" in src or "llm if" in src
            r = run_candidate(f, mock)
            p = run_reference(f, mock)
            total += 1
            rc, py = r.get("exit_code", "?"), p.get("exit_code", "?")
            rcodes = error_codes(r.get("stderr", ""))
            pcodes = error_codes(p.get("stderr", ""))
            ok = str(rc) == str(py) and rcodes == pcodes
            if not ok:
                bad += 1
            rows.append({
                "file": str(f.relative_to(ROOT)),
                "rust_exit": rc,
                "py_exit": py,
                "rust_codes": " ".join(rcodes),
                "py_codes": " ".join(pcodes),
                "match": str(ok),
            })
    with open(OUT, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)
    print(f"error-diff: {total - bad}/{total} match -> {OUT}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
