"""Contract tests for the one-file differential runner (M0 Task 0.4).

The candidate binary does not exist yet at M0, so these tests exercise the
three diff.sh verdict paths with stub candidates:
  - candidate missing  -> reference output shown, VERDICT: SKIP, exit 2
  - candidate matches  -> VERDICT: MATCH, exit 0
  - candidate mismatches -> VERDICT: MISMATCH, exit 1
"""

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
DIFF = ROOT / "scripts" / "diff.sh"
HELLO = ROOT / "tests" / "programs" / "authored" / "hello.helen"
STUB = ROOT / "tests" / "conformance" / "fixtures" / "stub_candidate.sh"


def run_diff(file: Path, candidate: str) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env["HELEN_CANDIDATE"] = candidate
    return subprocess.run(
        ["bash", str(DIFF), str(file)],
        capture_output=True,
        text=True,
        env=env,
    )


def test_diff_without_candidate_skips(tmp_path):
    missing = tmp_path / "nonexistent-candidate"
    p = run_diff(HELLO, str(missing))
    assert p.returncode == 2
    assert "REFERENCE" in p.stdout
    assert "VERDICT: SKIP" in p.stdout


def test_diff_with_matching_candidate_verdict_match():
    p = run_diff(HELLO, str(STUB))
    assert p.returncode == 0
    assert "REFERENCE" in p.stdout
    assert "CANDIDATE" in p.stdout
    assert "VERDICT: MATCH" in p.stdout


def test_diff_with_mismatching_candidate_verdict_mismatch(tmp_path):
    bad = tmp_path / "bad_candidate.sh"
    bad.write_text(
        '#!/usr/bin/env bash\necho \'{"stdout": "WRONG\\n", "stderr": "", '
        '"exit_code": 0, "error_classes": []}\'\n'
    )
    bad.chmod(0o755)
    p = run_diff(HELLO, str(bad))
    assert p.returncode == 1
    assert "VERDICT: MISMATCH" in p.stdout
