"""Python Import Hook — import `.helen` modules from Python (M11).

Port of `helen/python_bridge/import_hook.py` backed by `helen_rust._core`.
The Rust runtime does not yet consume session IDs for transcripts; the
resolution priority (override → `HELEN_SESSION_ID` → `.helen/current_session_id`
memento) is preserved for API parity.

Import-hook edge cases (reference parity):
- missing `.helen` file → the finder declines, Python raises `ModuleNotFoundError`
- parse/semantic/load errors → `RuntimeError("Failed to parse/load/...")`
- requested-but-missing agent → Python's `from m import X` machinery raises
  `ImportError` (and plain attribute access raises `AttributeError`)
"""

import importlib.abc
import importlib.util
import json
import os
import sys
from pathlib import Path

from . import _core
from .agent_wrapper import HelenAgentWrapper
from .function_wrapper import HelenFunctionWrapper

# Process-level explicit session_id (highest priority; None = unset).
_session_id_override = None


def set_session_id(session_id):
    """Explicitly set the session_id used by bridge-created interpreters."""
    global _session_id_override
    _session_id_override = session_id


def get_session_id():
    """Resolve the session_id by priority (override → env → memento)."""
    return _detect_session_id()


def _detect_session_id():
    # 1. Explicit set_session_id().
    if _session_id_override is not None:
        return _session_id_override
    # 2. Environment variable.
    env_sid = os.environ.get("HELEN_SESSION_ID")
    if env_sid:
        return env_sid
    # 3. Memento file (relative to cwd); supports the JSON object form
    #    {"main": "...", "child": "..."} and the plain-string form.
    memento = Path.cwd() / ".helen" / "current_session_id"
    if memento.exists():
        raw = memento.read_text(encoding="utf-8").strip()
        if raw:
            if raw.startswith("{"):
                try:
                    data = json.loads(raw)
                    sid = data.get("main", "")
                    if sid:
                        return sid
                except (json.JSONDecodeError, AttributeError):
                    pass
            else:
                return raw
    # 4. Default: None (create a new session).
    return None


class HelenMetaPathFinder(importlib.abc.MetaPathFinder):
    """Meta-path finder that locates `<name>.helen` beside Python modules."""

    def find_spec(self, fullname, path, target=None):
        module_name = fullname.split(".")[-1]
        search_paths = path if path else sys.path
        for search_path in search_paths:
            try:
                sp = Path(search_path)
                helen_file = sp / f"{module_name}.helen"
                if helen_file.exists():
                    loader = HelenLoader(str(helen_file))
                    return importlib.util.spec_from_loader(
                        fullname, loader, origin=str(helen_file)
                    )
            except Exception:
                continue
        return None


class HelenLoader(importlib.abc.Loader):
    """Loads a `.helen` file, exposing its agents and functions as module
    attributes (agents as classes, functions as wrappers)."""

    def __init__(self, file_path):
        self.file_path = file_path

    def create_module(self, spec):
        return None  # default module creation

    def exec_module(self, module):
        helen_file = str(Path(self.file_path).resolve())
        declarations = _core.describe_file(helen_file)

        for kind, name, description in declarations:
            if kind == "agent":

                def make_init(name, file):
                    def __init__(self):
                        HelenAgentWrapper.__init__(self, name, file)

                    return __init__

                agent_class = type(
                    name,
                    (HelenAgentWrapper,),
                    {
                        "__init__": make_init(name, helen_file),
                        "__module__": module.__name__,
                        "__doc__": description or f"Helen agent: {name}",
                    },
                )
                setattr(module, name, agent_class)
            else:  # function
                wrapper = HelenFunctionWrapper(name, helen_file)
                setattr(module, name, wrapper)

        module.__file__ = helen_file
        module.__loader__ = self
        module.__helen_file__ = helen_file


def install_import_hook():
    """Install the Helen meta-path finder (idempotent)."""
    for finder in sys.meta_path:
        if isinstance(finder, HelenMetaPathFinder):
            return
    sys.meta_path.insert(0, HelenMetaPathFinder())


def uninstall_import_hook():
    """Remove the Helen meta-path finder."""
    sys.meta_path[:] = [
        finder
        for finder in sys.meta_path
        if not isinstance(finder, HelenMetaPathFinder)
    ]
