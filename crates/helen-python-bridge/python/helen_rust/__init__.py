"""Helen Rust Python Bridge — import and call Helen agents from Python (M11).

Port of `helen/python_bridge/__init__.py` backed by the compiled
`helen_rust._core` extension instead of the pure-Python runtime.

Usage:
    import helen_rust                      # auto-installs the import hook
    from translator import SumAgent        # .helen module via meta-path finder
    agent = SumAgent()
    result = agent(10, 20)                 # sync
    result = await agent.async_call(10, 20)  # async (executor thread)
"""

from ._core import (
    PyAgent,
    PyFunction,
    describe_file,
    eval_helen,
    load_agent,
    load_function,
    parse_check,
)

from .import_hook import (
    get_session_id,
    install_import_hook,
    set_session_id,
    uninstall_import_hook,
)
from .agent_wrapper import HelenAgentWrapper, generate_python_classes
from .function_wrapper import HelenFunctionWrapper, load_helen_functions
from .decorators import helen_agent, helen_module

__version__ = "0.1.0"

__all__ = [
    "install_import_hook",
    "uninstall_import_hook",
    "set_session_id",
    "get_session_id",
    "load_agent",
    "load_function",
    "parse_check",
    "eval_helen",
    "describe_file",
    "PyAgent",
    "PyFunction",
    "HelenAgentWrapper",
    "generate_python_classes",
    "HelenFunctionWrapper",
    "load_helen_functions",
    "helen_agent",
    "helen_module",
]

# Auto-install the import hook (reference `helen.python_bridge` behavior).
install_import_hook()
