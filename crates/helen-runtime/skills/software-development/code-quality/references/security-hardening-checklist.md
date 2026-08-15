# Security Hardening Checklist for Python Runtimes

Concrete checklist from auditing Helen language runtime (2026-06-19).

## Critical Issues (P0)

### Shell Execution
- [ ] `shell=False` is the DEFAULT for all subprocess calls
- [ ] Commands are split with `shlex.split()` when shell=False
- [ ] Dangerous command patterns are blocked (rm -rf /, fork bombs, chmod 777 /)
- [ ] Timeout is enforced on all subprocess calls

### File System Access
- [ ] `validate_path()` called before ALL file read/write/patch operations
- [ ] Uses `os.path.realpath()` (not just `abspath`) to resolve symlinks
- [ ] Blocked paths: /proc, /sys, /dev, /etc/shadow, /etc/passwd
- [ ] Base directory containment enforced (no path traversal with ../)
- [ ] No "absolute paths are allowed" bypass

### Network Access (SSRF Prevention)
- [ ] `validate_url()` called before ALL HTTP requests
- [ ] Scheme whitelist: only http/https
- [ ] Hostname resolved via `socket.getaddrinfo()` and checked against private IPs
- [ ] Blocked: localhost, 127.0.0.1, 0.0.0.0, ::1, 10.x, 172.16-31.x, 192.168.x, 169.254.x
- [ ] Download size limit enforced with running counter

### Process Management
- [ ] PID validation: block PID 0, 1, and current process
- [ ] Signal whitelist: only SIGTERM, SIGINT, SIGHUP, SIGUSR1, SIGUSR2
- [ ] No arbitrary signal sending

### Environment Variables
- [ ] `env_list` returns masked values for sensitive keys
- [ ] Masked patterns: PASSWORD, SECRET, TOKEN, API_KEY, PRIVATE_KEY, CREDENTIAL, AUTH, SESSION
- [ ] Masking format: first 2 + "****" + last 2 chars (or "****" if ≤4 chars)

### Code Evaluation
- [ ] `eval()` uses AST whitelist (only allow BinOp, UnaryOp, Constant, safe Names)
- [ ] `__builtins__` set to `{}` in eval namespace
- [ ] Only math functions exposed (sqrt, sin, cos, etc.)

## Medium Issues (P1)

### Error Handling
- [ ] No bare `except:` clauses (use `except Exception:` at minimum)
- [ ] Security errors return structured error messages, not stack traces
- [ ] Specific exception types (SecurityError) for security violations

### Input Validation
- [ ] All user-facing functions validate inputs before processing
- [ ] XML parsing uses defusedxml or disables external entities
- [ ] Regex patterns have timeout to prevent ReDoS

### Resource Limits
- [ ] File read size limit (e.g., 16KB for tool output)
- [ ] HTTP response size limit (e.g., 8MB)
- [ ] Download size limit (e.g., 100MB)
- [ ] Command timeout (e.g., 30s default, 300s max)

## Testing

### Security Test Categories
- Path traversal: `../../etc/passwd`, symlink escape
- SSRF: `http://localhost/`, `http://127.0.0.1/`, `http://169.254.169.254/` (AWS metadata)
- Command injection: `rm -rf /`, `:(){:|:&};:`, `chmod -R 777 /`
- Environment leak: verify PASSWORD/SECRET/TOKEN are masked
- PID safety: verify PID 0, 1, self are blocked

## Implementation Pattern

```python
# security.py — central module
class SecurityError(Exception):
    """Raised when a security constraint is violated."""
    pass

def validate_path(path: str, *, base_dir: str | None = None) -> str:
    resolved = os.path.realpath(os.path.abspath(path))
    # Check blocked paths
    for blocked in _BLOCKED_PATHS:
        if resolved.startswith(blocked + os.sep):
            raise SecurityError(f"Access denied: {path}")
    # Check base_dir containment
    if base_dir:
        abs_base = os.path.realpath(os.path.abspath(base_dir))
        if not resolved.startswith(abs_base + os.sep):
            raise SecurityError(f"Path traversal: {path}")
    return resolved

def validate_url(url: str) -> str:
    parsed = urlparse(url)
    if parsed.scheme not in {"http", "https"}:
        raise SecurityError(f"Scheme not allowed: {parsed.scheme}")
    # Resolve and check for private IPs
    for family, _, _, _, sockaddr in socket.getaddrinfo(parsed.hostname, None):
        ip = ipaddress.ip_address(sockaddr[0])
        if any(ip in net for net in _BLOCKED_NETWORKS):
            raise SecurityError(f"Private IP blocked: {url}")
    return url
```

## Integration Points

Every tool/function that accepts external input must call the appropriate validator:
- `read_file(path)` → `validate_path(path)`
- `write_file(path, content)` → `validate_path(path)`
- `shell_exec(cmd)` → `validate_command(cmd)` + `shlex.split(cmd)`
- `web_fetch(url)` → `validate_url(url)`
- `http_request(method, url, ...)` → `validate_url(url)`
- `kill(pid, sig)` → `validate_pid(pid)` + `validate_kill_signal(sig)`
- `env_list()` → `safe_env_list()`
