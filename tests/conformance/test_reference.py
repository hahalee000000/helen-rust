"""Contract tests for the Python reference driver (M0 Task 0.4).

These tests pin the `reference.py` contract: JSON output, exit-code mapping
(0/1/2/3), Helen-native error classes, deterministic mock-LLM behavior, and
CLI-mode parity.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

CONF_DIR = Path(__file__).parent
REF = CONF_DIR / "reference.py"
HELEN_SRC = Path(os.environ.get("HELEN_SRC", str(Path.home() / "helen")))


@pytest.fixture
def env():
    e = dict(os.environ)
    e["HELEN_SRC"] = str(HELEN_SRC)
    return e


def run_ref(program: str, env: dict, *args: str) -> dict:
    """Feed a program on stdin and decode reference.py's JSON contract."""
    p = subprocess.run(
        [sys.executable, str(REF), *args, "-"],
        input=program,
        capture_output=True,
        text=True,
        env=env,
        cwd=str(CONF_DIR),
    )
    assert p.returncode == 0, f"reference.py crashed (rc={p.returncode}):\n{p.stderr}"
    return json.loads(p.stdout)


def run_ref_file(file: Path, env: dict, *args: str) -> dict:
    p = subprocess.run(
        [sys.executable, str(REF), *args, str(file)],
        capture_output=True,
        text=True,
        env=env,
        cwd=str(CONF_DIR),
    )
    assert p.returncode == 0, f"reference.py crashed (rc={p.returncode}):\n{p.stderr}"
    return json.loads(p.stdout)


HELLO = 'import std.core.*\nmain { print("hello") }\n'


def test_hello_world_exit_0(env):
    res = run_ref(HELLO, env)
    assert res["exit_code"] == 0
    assert res["stdout"] == "hello\n"


def test_semantic_error_exit_2(env):
    res = run_ref('import std.core.*\nmain { print(undeclared_var) }\n', env)
    assert res["exit_code"] == 2
    assert res["error_classes"] == []


def test_runtime_error_exit_3_and_class(env):
    res = run_ref('import std.core.*\nmain { throw RuntimeError("boom") }\n', env)
    assert res["exit_code"] == 3
    assert res["error_classes"] == ["RuntimeError"]


def test_lex_error_exit_1(env):
    res = run_ref('import std.core.*\nmain { let s = "unterminated }\n', env)
    assert res["exit_code"] == 1


def test_stderr_is_normalized(env):
    res = run_ref('import std.core.*\nmain { print(undeclared_var) }\n', env)
    assert " at " not in res["stderr"] or "/" not in res["stderr"].split(" at ")[1].split(":")[0]


def test_mock_llm_deterministic(env):
    program = (
        "import std.core.*\n"
        'agent EchoAgent(msg: str) {\n'
        '    description "echo"\n'
        '    prompt "Echo: {{msg}}"\n'
        "    main { return llm act }\n"
        "}\n"
        'main { print(EchoAgent("hi")) }\n'
    )
    res = run_ref(program, env, "--mock-llm")
    assert res["exit_code"] == 0
    assert res["stdout"] == "MOCK_REPLY\n"


def test_cli_mode_matches_inprocess(tmp_path, env):
    f = tmp_path / "prog.helen"
    f.write_text('import std.core.*\nmain { print(6 * 7) }\n')
    inp = run_ref_file(f, env)
    cli = run_ref_file(f, env, "--mode", "cli")
    assert inp["stdout"] == cli["stdout"] == "42\n"
    assert inp["exit_code"] == cli["exit_code"] == 0
