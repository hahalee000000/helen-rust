"""Synthetic pytest suite used to test the Tier-A corpus extractor.

Mimics the canonical in-process `run_helen` helper pattern used throughout
the real `tests/` tree (see tests/interpreter/test_shared_store_calling_
interpreter.py).
"""

from helen.core.errors import ErrorReporter
from helen.core.lexer import Scanner
from helen.core.parser import Parser
from helen.interpreter.interpreter import Interpreter


def run_helen(source: str) -> tuple:
    source = "import std.core.*\nimport std.str.*\n" + source
    errors = ErrorReporter()
    scanner = Scanner(source=source, file="<test>")
    tokens = scanner.scan_all()
    parser = Parser(tokens, errors)
    program = parser.parse()
    return program, errors


def test_add():
    src = """main {
    print(1 + 2)
}
"""
    program, errors = run_helen(src)
    assert program is not None
    assert not errors.has_errors


def test_variable_sourced():
    src = "main { print(42) }"
    program, errors = run_helen(src)
    assert program is not None


def test_dynamic_greeting():
    name = "world"
    src = f'main {{ print("hi {name}") }}'
    program, errors = run_helen(src)
    assert program is not None


def test_no_source_arg():
    # helper call that does not carry an extractable source string
    run_helen("")
