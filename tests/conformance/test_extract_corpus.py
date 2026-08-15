"""Contract tests for the Tier-A corpus extractor (M0 Task 0.4 / decision 2).

The extractor turns inline Helen source strings inside pytest files into a
runnable `.helen` corpus with a provenance manifest. Literal string sources
are extracted; f-string / variable sources are recorded as skipped.
"""

import json
import subprocess
import sys
from pathlib import Path

CONF = Path(__file__).parent
EXTRACT = CONF / "extract_corpus.py"
FIXTURE_SUITE = CONF / "fixtures" / "synthetic_suite"


def run_extract(out: Path, suite: Path = FIXTURE_SUITE, name: str = "demo"):
    p = subprocess.run(
        [sys.executable, str(EXTRACT), str(suite), "--out", str(out), "--suite", name],
        capture_output=True,
        text=True,
    )
    assert p.returncode == 0, f"extract_corpus.py failed:\n{p.stdout}\n{p.stderr}"
    return out


def test_extracts_literal_sources(tmp_path):
    out = run_extract(tmp_path / "out")
    demo = out / "demo"
    files = sorted(p.name for p in demo.glob("*.helen"))
    assert "test_add.helen" in files
    assert "test_variable_sourced.helen" in files

    content = (demo / "test_add.helen").read_text()
    assert content.startswith("import std.core.*")
    assert "print(1 + 2)" in content


def test_does_not_extract_fstring_sources(tmp_path):
    out = run_extract(tmp_path / "out")
    assert not (out / "demo" / "test_dynamic_greeting.helen").exists()


def test_manifest_has_provenance(tmp_path):
    out = run_extract(tmp_path / "out")
    manifest = json.loads((out / "manifest.json").read_text())
    assert manifest["suite"] == "demo"

    extracted = {e["test_function"] for e in manifest["extracted"]}
    assert {"test_add", "test_variable_sourced"} <= extracted

    skipped = {e["test_function"] for e in manifest["skipped"]}
    assert "test_dynamic_greeting" in skipped
    # every extracted entry carries provenance fields
    for e in manifest["extracted"]:
        assert e["source_file"] and e["line"] and e["output"] and e["source_hash"]
