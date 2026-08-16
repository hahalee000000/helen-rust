#!/usr/bin/env python3
"""Helen Web UI - Cross-platform launcher.

This script starts both the FastAPI backend and Vite frontend,
handling process management and cleanup on both Windows and Unix.

Usage:
    python start_webui.py

Environment variables:
    HELEN_WEBUI_CWD: Working directory for the Web UI (session boundary).
                     If not set, uses the current directory.
"""

import atexit
import os
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

IS_WINDOWS = sys.platform == "win32"

# Global process references for cleanup
_backend_proc: subprocess.Popen | None = None
_frontend_proc: subprocess.Popen | None = None


def terminate_process_tree(proc: subprocess.Popen | None) -> None:
    """Terminate a process and all its children."""
    if proc is None or proc.poll() is not None:
        return
    try:
        if IS_WINDOWS:
            subprocess.run(
                ["taskkill", "/F", "/T", "/PID", str(proc.pid)],
                capture_output=True,
                timeout=10,
            )
        else:
            os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
    except (subprocess.TimeoutExpired, ProcessLookupError, OSError):
        pass


def _wait_for_proc(proc: subprocess.Popen | None, timeout: float = 0.5) -> None:
    """Wait up to `timeout` seconds for a process to die.

    If it doesn't die within the timeout, escalate to SIGKILL and wait
    briefly more. We MUST wait because dying children (notably uvicorn)
    emit ANSI escape sequences (e.g. \033[32m for green "INFO") that
    clobber the terminal reset below if we reset before they're dead.
    """
    if proc is None or proc.poll() is not None:
        return
    try:
        proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            if not IS_WINDOWS:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            proc.wait(timeout=1.0)
        except Exception:
            pass


def _restore_terminal() -> None:
    """Reset terminal state so the shell prompt is usable.

    Vite/npm/uvicorn manipulate terminal state while running (cursor,
    colors). If any of them are killed, the terminal may be left with
    the cursor hidden or text attributes changed. Reset here so the
    shell prompt is visible immediately, without the user needing to
    press Enter.

    MUST be called AFTER all children have actually exited (see
    _wait_for_proc). Otherwise, their shutdown output (which includes
    ANSI escapes — uvicorn prints \033[32m for green "INFO" etc.)
    clobbers our reset and the user is back to pressing Enter.
    """
    if not sys.stdout.isatty():
        return
    try:
        # \033[?25h — show cursor (DEC private mode 25)
        # \033[0m   — reset all text attributes (SGR 0)
        # \r\n      — move cursor to start of a fresh line
        sys.stdout.write("\033[?25h\033[0m\r\n")
        sys.stdout.flush()
    except Exception:
        pass


# Flag to ensure cleanup only runs once (signal handler + atexit both call it).
_cleanup_done = False


def cleanup_all() -> None:
    """Clean up all child processes and restore terminal state.

    Idempotent: the second and subsequent calls are no-ops. Both the
    SIGTERM signal handler and the atexit hook call this, so without
    the guard we'd reset the terminal twice.
    """
    global _cleanup_done
    if _cleanup_done:
        return
    _cleanup_done = True

    # 1. Ask children to terminate.
    terminate_process_tree(_backend_proc)
    terminate_process_tree(_frontend_proc)
    # 2. Wait for them to actually exit. Uvicorn's shutdown output
    #    includes ANSI color escapes (e.g. \033[32m for green "INFO");
    #    if we reset the terminal before they die, that output clobbers
    #    the reset and the user has to press Enter again.
    _wait_for_proc(_frontend_proc)
    _wait_for_proc(_backend_proc)
    # 3. Reset terminal state now that no child is writing to it.
    _restore_terminal()


def find_helen_python() -> str:
    """Find the Python executable where Helen is installed.

    Tries (in order):
    1. HELEN_VENV environment variable (set by user or start_webui.py)
    2. ~/.venv (shared venv used by start_webui.py)
    3. Current sys.executable (fallback)
    """
    import sys
    from pathlib import Path

    # Check HELEN_VENV env var
    helen_venv = os.environ.get("HELEN_VENV")
    if helen_venv:
        venv_path = Path(helen_venv)
        if IS_WINDOWS:
            python = venv_path / "Scripts" / "python.exe"
        else:
            python = venv_path / "bin" / "python"
        if python.exists():
            return str(python)

    # Check default shared venv (~/.venv)
    home_venv = Path.home() / ".venv"
    if home_venv.exists():
        if IS_WINDOWS:
            python = home_venv / "Scripts" / "python.exe"
        else:
            python = home_venv / "bin" / "python"
        if python.exists():
            return str(python)

    # Fallback to current Python
    return sys.executable


def start_backend(backend_dir: Path, env: dict) -> subprocess.Popen:
    """Start the FastAPI backend server."""
    global _backend_proc

    user_cwd = env.get("HELEN_WEBUI_CWD", str(Path.cwd()))
    helen_python = find_helen_python()

    backend_env = env.copy()

    # Add backend dir to PYTHONPATH so `import app.xxx` resolves
    existing_pythonpath = backend_env.get("PYTHONPATH", "")
    backend_env["PYTHONPATH"] = (
        str(backend_dir) + (os.pathsep + existing_pythonpath if existing_pythonpath else "")
    )
    backend_env["HELEN_WEBUI_CWD"] = user_cwd
    # Pass backend_dir so the inline Python can fix sys.path[0]
    # (prevents namespace-package shadowing when launched from e.g. ~/helen-rust/)
    backend_env["_HELEN_BACKEND_DIR"] = str(backend_dir)

    # Set up .env file
    env_file = backend_dir / ".env"
    if env_file.exists():
        backend_env["ENV_FILE"] = str(env_file)
    elif (backend_dir / ".env.example").exists():
        shutil.copy(backend_dir / ".env.example", env_file)
        backend_env["ENV_FILE"] = str(env_file)

    print("🔧 Starting backend...")
    print(f"   Python: {helen_python}")
    print("   Backend:  http://localhost:8000")
    print("   API Docs: http://localhost:8000/docs")

    # Use inline Python to avoid separate launcher file
    # The key is to chdir via env var, not string interpolation (avoids path escaping issues)
    # Read HOST from env (default 127.0.0.1), warn if binding to 0.0.0.0
    #
    # IMPORTANT: Fix sys.path[0] BEFORE os.chdir or any imports.
    # python -c sets sys.path[0] = '' (cwd at startup). If the user launched
    # start_webui.py from a directory containing a `helen/` namespace package
    # (e.g. ~/helen-rust/helen/ with no __init__.py), it shadows the real
    # Python helen package on sys.path. Replacing sys.path[0] with backend_dir
    # ensures the correct package resolution.
    backend_code = (
        "import os, sys\n"
        "sys.path[0] = os.environ.get('_HELEN_BACKEND_DIR', '')\n"
        "cwd = os.environ.get('HELEN_WEBUI_CWD')\n"
        "if cwd:\n"
        "    try:\n"
        "        os.chdir(cwd)\n"
        "    except OSError:\n"
        "        pass\n"
        "import uvicorn\n"
        "from app.config import settings\n"
        "host = settings.HOST\n"
        "if host == '0.0.0.0':\n"
        "    print('⚠️  WARNING: Binding to 0.0.0.0 exposes WebUI to LAN.', file=sys.stderr)\n"
        "    print('   For local-only access, set HOST=127.0.0.1 in .env', file=sys.stderr)\n"
        "uvicorn.run('app.main:app', host=host, port=settings.PORT, reload=False)\n"
    )

    creationflags = subprocess.CREATE_NEW_PROCESS_GROUP if IS_WINDOWS else 0
    preexec_fn = None if IS_WINDOWS else os.setsid

    _backend_proc = subprocess.Popen(
        [helen_python, "-c", backend_code],
        cwd=str(backend_dir),
        env=backend_env,
        creationflags=creationflags,
        preexec_fn=preexec_fn,
        # Don't capture stderr so errors are visible
    )

    return _backend_proc


def start_frontend(frontend_dir: Path, env: dict) -> subprocess.Popen:
    """Start the Vite dev server."""
    global _frontend_proc

    # Verify node_modules are intact
    node_modules = frontend_dir / "node_modules"
    if IS_WINDOWS:
        vite_bin = node_modules / ".bin" / "vite.cmd"
    else:
        vite_bin = node_modules / ".bin" / "vite"

    if not vite_bin.exists():
        print("⚠️  Frontend node_modules incomplete, reinstalling...")
        try:
            subprocess.run(
                ["npm", "install"],
                cwd=str(frontend_dir),
                shell=IS_WINDOWS,
                check=True,
            )
        except (subprocess.CalledProcessError, OSError) as e:
            print(f"❌ npm install failed: {e}")

    # Read token from user's cwd and pass to frontend via env var
    frontend_env = env.copy()
    user_cwd = env.get("HELEN_WEBUI_CWD", str(Path.cwd()))
    token_file = Path(user_cwd) / ".helen" / "webui_token"
    if token_file.exists():
        try:
            token = token_file.read_text().strip()
            if token:
                frontend_env["HELEN_WEBUI_TOKEN"] = token
                print(f"🔑 Token loaded for frontend auto-injection")
        except Exception:
            pass  # Ignore read errors

    print("🎨 Starting frontend...")
    print("   Frontend: http://localhost:5173")

    creationflags = subprocess.CREATE_NEW_PROCESS_GROUP if IS_WINDOWS else 0
    preexec_fn = None if IS_WINDOWS else os.setsid

    _frontend_proc = subprocess.Popen(
        ["npm", "run", "dev"],
        cwd=str(frontend_dir),
        env=frontend_env,
        shell=IS_WINDOWS,  # npm is a .cmd on Windows
        creationflags=creationflags,
        preexec_fn=preexec_fn,
    )

    return _frontend_proc


def check_ports() -> list[int]:
    """Check if ports 8000 and 5173 are in use. Returns list of occupied ports."""
    # This is a best-effort check; actual binding will fail if port is taken
    return []


def main() -> int:
    """Main entry point."""
    # Determine directories
    script_dir = Path(__file__).parent
    backend_dir = script_dir / "backend"
    frontend_dir = script_dir / "frontend"

    # Validate directories exist
    if not backend_dir.exists():
        print(f"❌ Backend directory not found: {backend_dir}")
        return 1
    if not frontend_dir.exists():
        print(f"❌ Frontend directory not found: {frontend_dir}")
        return 1

    # Set up environment
    env = os.environ.copy()
    if "HELEN_WEBUI_CWD" not in env:
        env["HELEN_WEBUI_CWD"] = str(Path.cwd())
    # Default to hiding debug output unless explicitly enabled
    if "HELEN_DEBUG" not in env:
        env["HELEN_DEBUG"] = "0"

    print("=" * 60)
    print("🚀 Helen Web UI")
    print("=" * 60)
    print()
    print(f"📂 Working directory: {env['HELEN_WEBUI_CWD']}")
    print()

    # Register cleanup
    atexit.register(cleanup_all)

    # Set up signal handlers (Unix only)
    if not IS_WINDOWS:
        def forward_signal(signum, frame):
            cleanup_all()
            sys.exit(0)

        signal.signal(signal.SIGTERM, forward_signal)
        signal.signal(signal.SIGINT, forward_signal)

    try:
        # Start backend
        start_backend(backend_dir, env)

        # Wait for backend to bind port
        time.sleep(3)

        # Start frontend
        start_frontend(frontend_dir, env)

        print()
        print("✅ Helen Web UI is running!")
        print("   Press Ctrl+C to stop")
        print()

        # Main loop: wait for either process to exit
        while True:
            backend_rc = _backend_proc.poll() if _backend_proc else None
            frontend_rc = _frontend_proc.poll() if _frontend_proc else None

            if backend_rc is not None and frontend_rc is not None:
                # Both exited
                return backend_rc or frontend_rc

            if backend_rc is not None:
                # Only warn if exit code is non-zero (unexpected)
                if backend_rc != 0:
                    print("⚠️  Backend exited unexpectedly (code: {})".format(backend_rc))
                terminate_process_tree(_frontend_proc)
                return backend_rc

            if frontend_rc is not None:
                # Only warn if exit code is non-zero (unexpected)
                # Exit code 0 or negative (killed by signal) is normal user exit
                if frontend_rc > 0:
                    print("⚠️  Frontend exited unexpectedly (code: {})".format(frontend_rc))
                terminate_process_tree(_backend_proc)
                return frontend_rc

            time.sleep(0.5)

    except KeyboardInterrupt:
        print("\n🛑 Stopping services...")
        cleanup_all()
        print("✅ All services stopped")
        return 0
    except Exception as e:
        print(f"❌ Error: {e}")
        cleanup_all()
        return 1


if __name__ == "__main__":
    sys.exit(main())
