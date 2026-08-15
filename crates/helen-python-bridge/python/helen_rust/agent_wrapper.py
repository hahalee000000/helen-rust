"""Helen Agent wrapper (M11) — Python wrapper around the native PyAgent.

Port of `helen/python_bridge/agent_wrapper.py`. The heavy lifting (lex →
parse → semantic → execute) happens in the Rust `helen_rust._core.load_agent`;
this class adds `async_call` and mirrors the reference API:
`HelenAgentWrapper(agent_name, helen_file)`.

The `interpreter` parameter is accepted for signature parity; the Rust bridge
uses a fresh Interpreter per call (plan 13.1 documented behavior).
"""

from pathlib import Path

from . import _core


class HelenAgentWrapper:
    """Helen Agent 的 Python 包装器（基于 Rust 原生 PyAgent）。"""

    def __init__(self, agent_name, helen_file, interpreter=None):
        self.agent_name = agent_name
        self.helen_file = str(Path(helen_file).resolve())
        self._native = _core.load_agent(self.helen_file, agent_name)

    def __call__(self, *args, **kwargs):
        """Call the agent (positional + keyword arguments)."""
        return self._native(*args, **kwargs)

    async def async_call(self, *args, **kwargs):
        """Call the agent on an executor thread (reference parity:
        `loop.run_in_executor(None, self, *args, **kwargs)`)."""
        import asyncio

        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(None, self._native, *args, **kwargs)

    def __repr__(self):
        return repr(self._native)

    def __str__(self):
        return str(self._native)




def generate_python_classes(helen_file, interpreter=None):
    """从 Helen 文件生成 {agent_name: agent_class} 字典（M11）。

    Port of the reference `generate_python_classes` backed by
    `_core.describe_file` (no separate parse pass).
    """
    helen_file = str(Path(helen_file).resolve())
    agents = {}
    for kind, name, description in _core.describe_file(helen_file):
        if kind != "agent":
            continue

        def make_init(name, file):
            def __init__(self):
                HelenAgentWrapper.__init__(self, name, file)

            return __init__

        agent_class = type(
            name,
            (HelenAgentWrapper,),
            {
                "__init__": make_init(name, helen_file),
                "__module__": __name__,
                "__doc__": description or f"Helen agent: {name}",
            },
        )
        agents[name] = agent_class
    return agents
