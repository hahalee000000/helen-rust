#!/usr/bin/env bash
# diff-tier-a.sh — Tier A: run candidate over a corpus and compare with goldens.
#
# Usage:
#   diff-tier-a.sh <suite>          # e.g. authored, interpreter, agent, stdlib
#   diff-tier-a.sh --all            # all suites with goldens
#
# Golden format (jsonl): {"file", "stdout", "stderr", "exit_code", "error_classes"}
# Comparison: stdout byte-identical, exit_code equal, error_classes equal.
# stderr is normalized (spans stripped) and compared too when non-empty.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CAND="${HELEN_CANDIDATE:-$ROOT/target/release/helen}"
GOLDEN_DIR="$ROOT/tests/conformance/golden"

python3 - "$@" <<'EOF'
import json
import os
import subprocess
import sys

ROOT = "/home/rxx/helen-rust"
CAND = os.environ.get("HELEN_CANDIDATE", os.path.join(ROOT, "target", "release", "helen"))
GOLDEN_DIR = os.path.join(ROOT, "tests", "conformance", "golden")

def pick_suites(argv):
    if not argv or "--all" in argv:
        return sorted(p.stem for p in __import__("pathlib").Path(GOLDEN_DIR).glob("*.jsonl"))
    return argv

def norm_stderr(s: str) -> str:
    # strip span info "(file:line:col)" — same as reference._normalize_stderr
    import re
    return re.sub(r"\([^)]*\.helen:\d+:\d+\)", "(LOC)", s)

def run_candidate(file: str, mock: bool) -> dict:
    # Run from ROOT with a path relative to ROOT so error messages embed the
    # same relative path the goldens captured. --mock-llm mirrors
    # reference.py's mock (act_return="MOCK_REPLY", route_return="__mock__").
    args = [CAND, "--run"]
    if mock:
        args.append("--mock-llm")
    args.append(os.path.relpath(file, ROOT))
    p = subprocess.run(args, capture_output=True, text=True, timeout=60, cwd=ROOT)
    try:
        return json.loads(p.stdout)
    except json.JSONDecodeError:
        return {"stdout": "", "stderr": p.stderr or p.stdout, "exit_code": p.returncode, "error_classes": []}

def main():
    suites = pick_suites(sys.argv[1:])
    total = matched = mismatched = errored = 0
    by_suite = {}
    for suite in suites:
        golden_path = os.path.join(GOLDEN_DIR, f"{suite}.jsonl")
        if not os.path.isfile(golden_path):
            print(f"[{suite}] no golden, skip")
            continue
        goldens = {}
        with open(golden_path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                g = json.loads(line)
                goldens[g["file"]] = g
        corpus_dir = os.path.join(ROOT, "tests", "programs", "pytest", suite)
        if not os.path.isdir(corpus_dir):
            corpus_dir = os.path.join(ROOT, "tests", "programs", suite)
        files = sorted(p for p in __import__("pathlib").Path(corpus_dir).rglob("*.helen") if p.name in goldens)
        suite_total = suite_match = 0
        for f in files:
            suite_total += 1
            g = goldens[f.name]
            mock = "llm act" in f.read_text(encoding="utf-8") or "llm if" in f.read_text(encoding="utf-8")
            c = run_candidate(str(f), mock)
            ok = (c["stdout"] == g["stdout"]
                  and c["exit_code"] == g["exit_code"]
                  and c["error_classes"] == g["error_classes"]
                  and (not g["stderr"] or norm_stderr(c["stderr"]) == norm_stderr(g["stderr"])))
            if ok:
                suite_match += 1
            else:
                print(f"  MISMATCH {suite}/{f.name}")
                print(f"    golden: exit={g['exit_code']} classes={g['error_classes']} stdout={g['stdout'][:80]!r} stderr={g['stderr'][:80]!r}")
                print(f"    cand:   exit={c['exit_code']} classes={c['error_classes']} stdout={c['stdout'][:80]!r} stderr={c['stderr'][:80]!r}")
        print(f"[{suite}] {suite_match}/{suite_total} match")
        total += suite_total
        matched += suite_match
        by_suite[suite] = (suite_match, suite_total)
    print(f"\nTOTAL: {matched}/{total} match")
    return 0 if total == matched else 1

if __name__ == "__main__":
    sys.exit(main())
EOF
