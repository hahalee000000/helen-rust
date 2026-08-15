#!/usr/bin/env python3
"""capture_golden.py — snapshot reference results over a corpus (M0 Task 0.4).

Runs `reference.py` over every `.helen` file in a corpus directory and writes
`golden/<suite>.jsonl` — one JSON object per program:

    {"file": "...", "stdout": "...", "stderr": "...",
     "exit_code": 0, "error_classes": []}

Goldens are committed and refreshed deliberately (scripts/extract-corpus.sh).

Usage:
    capture_golden.py <corpus_dir> --out <golden_dir> --suite <name>
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

def collect_programs(corpus_dir: Path) -> list[Path]:
    return sorted(p for p in corpus_dir.rglob("*.helen"))


def needs_mock_llm(program: Path) -> bool:
    text = program.read_text(encoding="utf-8")
    return "llm act" in text or "llm if" in text


def run_reference(program: Path, mock: bool) -> dict:
    import io
    import sys as _sys

    script = Path(__file__).parent / "reference.py"
    args = [_sys.executable, str(script)]
    if mock:
        args.append("--mock-llm")
    args.append(str(program))
    env = dict(os.environ)
    p = subprocess.run(args, capture_output=True, text=True, env=env)
    if p.returncode != 0:
        raise RuntimeError(f"reference.py failed on {program}: {p.stderr}")
    return json.loads(p.stdout)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="capture_golden.py")
    parser.add_argument("corpus_dir", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--suite", required=True)
    args = parser.parse_args(argv)

    programs = collect_programs(args.corpus_dir)
    if not programs:
        print(f"capture_golden.py: no .helen files under {args.corpus_dir}", file=sys.stderr)
        return 2

    args.out.mkdir(parents=True, exist_ok=True)
    out_file = args.out / f"{args.suite}.jsonl"
    with out_file.open("w", encoding="utf-8") as fh:
        for program in programs:
            result = run_reference(program, needs_mock_llm(program))
            record = {
                "file": str(program.relative_to(args.corpus_dir)),
                "stdout": result["stdout"],
                "stderr": result["stderr"],
                "exit_code": result["exit_code"],
                "error_classes": result["error_classes"],
            }
            fh.write(json.dumps(record, ensure_ascii=False) + "\n")

    print(f"captured {len(programs)} programs -> {out_file}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
