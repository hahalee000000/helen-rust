"""装饰器 (M11) — `@helen_agent` / `@helen_module`。

Port of `helen/python_bridge/decorators.py` backed by the Rust bridge.
"""

from functools import wraps


def helen_agent(helen_file, agent_name=None):
    """装饰器：将 Python 函数包装为 Helen agent 调用。

    Example:
        @helen_agent("translator.helen", "TranslatorAgent")
        def translate(text: str, target: str) -> str:
            pass

        result = translate("Hello", "French")
    """

    def decorator(func):
        name = agent_name or func.__name__
        agent_wrapper = None

        @wraps(func)
        def wrapper(*args, **kwargs):
            nonlocal agent_wrapper
            if agent_wrapper is None:
                from .agent_wrapper import HelenAgentWrapper

                agent_wrapper = HelenAgentWrapper(name, helen_file)
            return agent_wrapper(*args, **kwargs)

        wrapper.__helen_file__ = helen_file
        wrapper.__helen_agent__ = name
        return wrapper

    return decorator


def helen_module(helen_file):
    """装饰器：将整个模块包装为 Helen agents 集合。

    Example:
        @helen_module("agents.helen")
        class Agents:
            pass

        agents = Agents()
        result = agents.TranslatorAgent("Hello", "French")
    """

    def decorator(cls):
        _agents_cache = {}

        def __init__(self):
            from .agent_wrapper import generate_python_classes

            agents = generate_python_classes(helen_file)
            _agents_cache.update(agents)

        def __getattr__(self, name):
            if name in _agents_cache:
                return _agents_cache[name]()
            raise AttributeError(f"Module has no agent '{name}'")

        cls.__init__ = __init__
        cls.__getattr__ = __getattr__
        cls.__helen_file__ = helen_file
        return cls

    return decorator
