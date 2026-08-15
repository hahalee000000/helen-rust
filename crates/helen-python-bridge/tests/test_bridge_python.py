"""M11 Python Bridge DoD suite (Tier B — runs against the installed wheel).

Requires `maturin develop` (or `pip install helen-rust`) first:
    cd crates/helen-python-bridge
    maturin develop --release

Then run pytest from the crate directory:
    python -m pytest tests/test_bridge_python.py -v
"""

import asyncio
import sys
from pathlib import Path

import pytest

TESTS_DIR = Path(__file__).resolve().parent


@pytest.fixture(scope="module", autouse=True)
def _ensure_import_path():
    sys.path.insert(0, str(TESTS_DIR))
    yield


def test_import_hook_exposes_agent_class():
    import helen_rust  # noqa: F401  (auto-installs the hook)
    from translator import SumAgent

    agent = SumAgent()
    assert agent(10, 20) == 30
    assert agent(a=15, b=25) == 40
    assert agent(30, b=40) == 70
    # repr shows the agent identity.
    assert "<HelenAgent 'SumAgent'" in repr(agent)


def test_agent_type_errors():
    import helen_rust  # noqa: F401
    from translator import SumAgent

    agent = SumAgent()
    # Missing required argument.
    with pytest.raises(TypeError, match="SumAgent\\(\\) missing required argument: 'b'"):
        agent(10)
    # Too many positional arguments.
    with pytest.raises(TypeError, match=r"takes 2 positional arguments but 3 were given"):
        agent(1, 2, 3)
    # Unknown keyword argument.
    with pytest.raises(TypeError, match=r"unexpected keyword argument 'c'"):
        agent(a=1, c=2)


def test_agent_default_parameter():
    import helen_rust  # noqa: F401
    from translator import GreetAgent

    agent = GreetAgent()
    assert agent("World") == "Hello, World!"  # default greeting
    assert agent("World", greeting="Hi") == "Hi, World!"
    assert agent(name="Bob") == "Hello, Bob!"


def test_async_call():
    import helen_rust  # noqa: F401
    from translator import SumAgent

    agent = SumAgent()

    async def main():
        return await agent.async_call(5, 7)

    assert asyncio.run(main()) == 12


def test_import_hook_function():
    import helen_rust  # noqa: F401
    from translator import add

    assert add(2, 3) == 5
    assert add(2, b=4) == 6
    with pytest.raises(TypeError, match=r"got multiple values for argument 'a'"):
        add(2, a=9)
    with pytest.raises(TypeError, match=r"unexpected keyword argument 'x'"):
        add(x=1)


def test_module_not_found():
    import helen_rust  # noqa: F401

    with pytest.raises(ModuleNotFoundError):
        import definitely_missing_module_xyz  # noqa: F401


def test_missing_agent_attribute_error():
    import helen_rust  # noqa: F401
    from translator import SumAgent

    with pytest.raises(AttributeError):
        SumAgent.no_such_attribute


def test_parse_check():
    import helen_rust

    # Valid program with stdlib import -> no codes.
    assert helen_rust.parse_check("import std.core.*\nmain {\n    print(1)\n}\n") == []
    # Undefined variable -> at least one E-code.
    codes = helen_rust.parse_check("main {\n    print(y)\n}\n")
    assert codes, "expected E-codes for undefined variable"
    # Parse failure raises RuntimeError.
    with pytest.raises(RuntimeError, match="Failed to parse"):
        helen_rust.parse_check("let = =")


def test_eval_helen():
    import helen_rust

    assert helen_rust.eval_helen("x * 2", {"x": 21}) == 42
    assert helen_rust.eval_helen('"hello" + " world"', {}) == "hello world"


def test_generate_python_classes():
    import helen_rust
    from helen_rust import generate_python_classes

    classes = generate_python_classes(str(TESTS_DIR / "translator.helen"))
    assert "SumAgent" in classes
    assert "GreetAgent" in classes
    agent = classes["SumAgent"]()
    assert agent(1, 2) == 3


def test_decorator():
    from helen_rust import helen_agent

    @helen_agent(str(TESTS_DIR / "translator.helen"), "SumAgent")
    def my_sum(a, b):
        pass

    assert my_sum(3, 4) == 7
    assert my_sum.__helen_agent__ == "SumAgent"


def test_load_agent_direct():
    import helen_rust

    native = helen_rust.load_agent(str(TESTS_DIR / "translator.helen"), "SumAgent")
    assert native(4, 5) == 9
    assert native.name == "SumAgent"


def test_dod_translator_agent():
    """M11 Definition of Done: `from translator import TranslatorAgent;
    agent("Hello","French")` works (sync + keyword args).

    The reference TranslatorAgent's main block runs `llm act "..."`; the
    Rust bridge interpreter uses its default MockLlmRuntime, so the call
    returns the canned mock response without any network access.
    """
    import helen_rust  # noqa: F401  (auto-installs the hook)
    from translator import TranslatorAgent

    agent = TranslatorAgent()
    # The bridge's fresh interpreter uses the default MockLlmRuntime
    # (Python parity: `act_return is None` -> LLMResponse(text="")), so the
    # DoD check is that the call flows through the full pipeline (param
    # binding -> main -> llm act) and returns a string.
    result = agent("Hello", "French")
    assert isinstance(result, str), f"expected str, got {result!r}"
    # Keyword form works too.
    result2 = agent(text="Hello", target="French")
    assert isinstance(result2, str)
