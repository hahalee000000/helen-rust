#!/usr/bin/env python3
"""reference.py — Python reference driver for the helen-rust differential harness.

Runs a Helen program through the *reference* implementation (the Python
interpreter in ``HELEN_SRC``, default ``~/helen``) and emits a normalized
result as a single JSON object on stdout:

    {"stdout": str, "stderr": str, "exit_code": int, "error_classes": [str]}

exit_code follows the Python CLI mapping (verified against v1.44.0):

    0 = success
    1 = lex / parse / IO error
    2 = semantic error
    3 = runtime error (uncaught exception)

``error_classes`` lists Helen-native exception class names of uncaught
runtime errors (the 11 predefined names).

Usage:
    reference.py <file.helen> [--mock-llm] [--mode inprocess|cli]
    reference.py - [--mock-llm] [--mode inprocess]      # read source from stdin

Environment:
    HELEN_SRC   path to the Python interpreter source (default ~/helen)
"""

from __future__ import annotations

import argparse
import io
import json
import os
import re
import subprocess
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Location normalization — goldens must be portable across machines.
# The Python interpreter formats errors as
#   "E0354 at /abs/path/file.helen:2:17-27: message"
# We strip the " at <path>:<line>:<col>-<col>" suffix.
# ---------------------------------------------------------------------------
_SPAN_RE = re.compile(r" at \S+:\d+:\d+-\d+")


def _normalize_stderr(s: str) -> str:
    return _SPAN_RE.sub("", s)


# ---------------------------------------------------------------------------
# Reference resolution
# ---------------------------------------------------------------------------
def _helen_src() -> Path:
    src = os.environ.get("HELEN_SRC") or str(Path.home() / "helen")
    path = Path(src).expanduser().resolve()
    if not (path / "helen").is_dir():
        sys.stderr.write(
            f"reference.py: HELEN_SRC={path} has no 'helen' package\n"
        )
        sys.exit(2)
    return path


def _import_reference(helen_src: Path):
    """Import the reference interpreter modules from HELEN_SRC."""
    sys.path.insert(0, str(helen_src))
    from helen.core.lexer import Scanner  # noqa: F401
    from helen.core.parser import Parser  # noqa: F401
    from helen.core.errors import ErrorReporter  # noqa: F401
    from helen.semantic.analyzer import SemanticAnalyzer  # noqa: F401
    from helen.interpreter.interpreter import Interpreter  # noqa: F401
    from helen.runtime.import_resolver import ImportResolver  # noqa: F401
    from helen.runtime.llm_runtime import MockLLMRuntime  # noqa: F401

    return Scanner, Parser, ErrorReporter, SemanticAnalyzer, Interpreter, ImportResolver, MockLLMRuntime


# ---------------------------------------------------------------------------
# in-process mode
# ---------------------------------------------------------------------------
def run_inprocess(
    source: str,
    file: str,
    mock_llm: bool,
    helen_src: Path,
) -> dict:
    (Scanner, Parser, ErrorReporter, SemanticAnalyzer,
     Interpreter, ImportResolver, MockLLMRuntime) = _import_reference(helen_src)

    errors = ErrorReporter()

    # Lex (CLI wraps scan_all in try/except -> exit 1)
    try:
        scanner = Scanner(source=source, file=file)
        tokens = scanner.scan_all()
    except Exception as e:  # noqa: BLE001 — mirror CLI behavior
        return {
            "stdout": "",
            "stderr": _normalize_stderr(f"E300: {e}"),
            "exit_code": 1,
            "error_classes": [],
        }

    # Parse
    parser = Parser(tokens, errors=errors)
    program = parser.parse()
    if errors.has_errors:
        return {
            "stdout": "",
            "stderr": _normalize_stderr("\n".join(str(e) for e in errors.errors)),
            "exit_code": 1,
            "error_classes": [],
        }

    # Analyze
    analyzer = SemanticAnalyzer(errors)
    analyzer.analyze(program)
    if errors.has_errors:
        return {
            "stdout": "",
            "stderr": _normalize_stderr("\n".join(str(e) for e in errors.errors)),
            "exit_code": 2,
            "error_classes": [],
        }

    # Interpret
    llm_runtime = None
    if mock_llm:
        llm_runtime = MockLLMRuntime(act_return="MOCK_REPLY", route_return="__mock__")
    interp = Interpreter(
        errors=errors,
        llm_runtime=llm_runtime,
        transcript_store_enabled=False,  # mirror CLI batch mode
    )
    if file != "<stdin>":
        interp.import_resolver = ImportResolver(base_dir=str(Path(file).parent))

    old_stdout, old_stderr = sys.stdout, sys.stderr
    sys.stdout, sys.stderr = io.StringIO(), io.StringIO()
    try:
        try:
            interp.interpret(program)
            out, err = sys.stdout.getvalue(), sys.stderr.getvalue()
            code = 2 if errors.has_errors else 0
            classes: list[str] = []
        except Exception as e:  # noqa: BLE001 — mirror CLI exit-3 path
            out, err = sys.stdout.getvalue(), sys.stderr.getvalue()
            code = 3
            classes = [type(e).__name__]
            err += f"RuntimeError: {e}\n"
    finally:
        sys.stdout, sys.stderr = old_stdout, old_stderr

    return {
        "stdout": out,
        "stderr": _normalize_stderr(err),
        "exit_code": code,
        "error_classes": classes,
    }


# ---------------------------------------------------------------------------
# CLI mode
# ---------------------------------------------------------------------------
def run_cli(file: str, helen_src: Path) -> dict:
    env = dict(os.environ)
    env["HELEN_SRC"] = str(helen_src)
    env.setdefault("HELEN_API_KEY", "test-dummy-key-for-ci")
    p = subprocess.run(
        [sys.executable, "-m", "helen.cli", file],
        capture_output=True,
        text=True,
        cwd=str(helen_src),
        env=env,
        timeout=60,
    )
    return {
        "stdout": p.stdout,
        "stderr": _normalize_stderr(p.stderr),
        "exit_code": p.returncode,
        "error_classes": [],
    }


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="reference.py")
    parser.add_argument("file", help=".helen source file, or '-' for stdin")
    parser.add_argument("--mock-llm", action="store_true", help="inject MockLLMRuntime")
    parser.add_argument(
        "--mode", choices=["inprocess", "cli"], default="inprocess",
        help="inprocess = interpreter in this process (default); cli = python -m helen.cli",
    )
    args = parser.parse_args(argv)

    helen_src = _helen_src()

    if args.mode == "cli":
        if args.file == "-":
            sys.stderr.write("reference.py: --mode cli requires a file path (not stdin)\n")
            return 2
        result = run_cli(args.file, helen_src)
    else:
        if args.file == "-":
            source = sys.stdin.read()
            file = "<stdin>"
        else:
            source = Path(args.file).read_text(encoding="utf-8")
            file = args.file
        result = run_inprocess(source, file, args.mock_llm, helen_src)

    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
