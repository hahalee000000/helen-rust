# CI Troubleshooting Quick Reference

## Reading CI Logs

```bash
gh run view <RUN_ID> --log-failed
```

## Common Failure Patterns

### Test Failures
**Signatures:** `FAILED tests/test_foo.py::test_bar - AssertionError`
**Fix:** Update assertion, add missing dependency, fix flaky test

### Lint / Formatting
**Signatures:** `E302 expected 2 blank lines`, `would reformat src/utils.py`
**Fix:** Run `black .`, `ruff check --fix .`, `isort .`

### Type Check (mypy/pyright)
**Signatures:** `error: Argument 1 has incompatible type "str"; expected "int"`
**Fix:** Add type cast, fix signature, or `# type: ignore` as last resort

### Build / Compilation
**Signatures:** `ModuleNotFoundError`, `Could not find a version`
**Fix:** Add missing dep, pin compatible version, update lockfile

### Permission / Auth
**Signatures:** `Resource not accessible by integration`, `403 Forbidden`
**Fix:** Add `permissions:` to workflow YAML, verify secrets

### Timeout
**Signatures:** `The operation was canceled`, `exceeded maximum execution time`
**Fix:** Add `timeout-minutes: 10`, fix perf issue, split into parallel jobs

### Docker / Container
**Signatures:** `COPY failed: file not found in build context`
**Fix:** Fix path in COPY/ADD, update base image tag

## Auto-Fix Decision Tree

```
CI Failed
├── Test failure → update test or fix logic / add dependency
├── Lint failure → run formatter, fix style
├── Type error → fix types
├── Build failure → add dep / update pins
├── Permission error → update workflow permissions (needs user)
└── Timeout → investigate perf (may need user input)
```
