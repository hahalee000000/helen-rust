"""Helen Function wrapper (M11) — Python wrapper around the native PyFunction.

Port of `helen/python_bridge/function_wrapper.py` backed by
`helen_rust._core.load_function`.
"""

from pathlib import Path

from . import _core


class HelenFunctionWrapper:
    """Helen 函数的 Python 包装器（基于 Rust 原生 PyFunction）。"""

    def __init__(self, func_name, helen_file, interpreter=None):
        self.func_name = func_name
        self.helen_file = str(Path(helen_file).resolve())
        self._native = _core.load_function(self.helen_file, func_name)

    def __call__(self, *args, **kwargs):
        """Call the Helen function (positional + keyword arguments)."""
        return self._native(*args, **kwargs)

    async def async_call(self, *args, **kwargs):
        """Call the function on an executor thread."""
        import asyncio

        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(None, self._native, *args, **kwargs)

    def __repr__(self):
        return repr(self._native)

    def __str__(self):
        return str(self._native)




def load_helen_functions(helen_file, interpreter=None):
    """从 Helen 文件加载所有函数 {func_name: HelenFunctionWrapper}（M11）。"""
    helen_file = str(Path(helen_file).resolve())
    functions = {}
    for kind, name, _description in _core.describe_file(helen_file):
        if kind == "function":
            functions[name] = HelenFunctionWrapper(name, helen_file)
    return functions
