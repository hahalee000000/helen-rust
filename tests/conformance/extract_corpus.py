#!/usr/bin/env python3
"""extract_corpus.py — Tier-A corpus extractor (M0 Task 0.4 / decision 2).

Parses pytest files with the `ast` module, finds Helen source strings passed
to the suite's in-process helpers (`run_helen`, `run_helen_code`,
`run_helen_with_session`, `write_text`), re-applies the stdlib import block
the helpers prepend (v1.39 removed global builtins), and writes a runnable
`.helen` file per extracted source plus a provenance manifest.

Extractable sources: literal string arguments AND simple local variables that
resolve to a literal string via an `x = "..."` assignment in the same
enclosing function (the canonical `src = <triple-quoted> ; run_helen(src)`
pattern). f-string / other dynamic sources are recorded in the manifest
under `skipped`.

Usage:
    extract_corpus.py <pytest_suite_dir> --out <out_dir> --suite <name>

Output:
    <out_dir>/<suite>/<test_function>_<n>.helen
    <out_dir>/manifest.json
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import re
import sys
from pathlib import Path

HELPERS = ("run_helen", "run_helen_code", "run_helen_with_session", "write_text")
STDLIB_BLOCK = (
    "import std.core.*\n"
    "import std.str.*\n"
    "import std.list.*\n"
    "import std.dict.*\n"
    "import std.math.*\n"
    "import std.debug.*\n"
)


def _scope_for(tree: ast.Module, lineno: int) -> ast.AST:
    """Return the innermost function (or module) containing `lineno`."""
    best = tree
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.lineno <= lineno <= getattr(node, "end_lineno", node.lineno):
                if best is tree or node.lineno >= best.lineno:
                    best = node
    return best


def _local_string_consts(scope: ast.AST) -> dict[str, str]:
    """Map local variable name -> str constant for direct assignments."""
    consts: dict[str, str] = {}
    body = scope.body if hasattr(scope, "body") else []
    for stmt in body:
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Name)
            and isinstance(stmt.value, ast.Constant)
            and isinstance(stmt.value.value, str)
        ):
            consts[stmt.targets[0].id] = stmt.value.value
    return consts


def _resolve_literal(arg: ast.expr, consts: dict[str, str]) -> str | None:
    """Resolve a call arg to a Helen source string, or None if not literal."""
    if isinstance(arg, ast.Constant) and isinstance(arg.value, str):
        value = arg.value
    elif isinstance(arg, ast.Name) and arg.id in consts:
        value = consts[arg.id]
    else:
        return None

    stripped = value.strip()
    if not stripped or ("main" not in value and "\n" not in value):
        return None
    return value


def _ensure_stdlib_block(source: str) -> str:
    """Re-apply the stdlib import block the test helper prepends.

    Prepends only the modules the extracted source does not already import
    (e.g. a source with `import std.context.*` still needs `std.core.*` for
    `print`/`str`). Duplicate imports are avoided.
    """
    imported = set(re.findall(r"import std\.(\w+)", source))
    missing = [m for m in ("core", "str", "list", "dict", "math", "debug") if m not in imported]
    if not missing:
        return source
    block = "".join(f"import std.{m}.*\n" for m in missing)
    return block + source


def extract_file(path: Path, suite: str, out_dir: Path,
                 extracted: list[dict], skipped: list[dict]) -> int:
    """Extract all extractable sources from one test file. Returns count."""
    try:
        tree = ast.parse(path.read_text(encoding="utf-8"))
    except SyntaxError:
        skipped.append({
            "source_file": path.name,
            "test_function": "<module>",
            "line": 0,
            "reason": "unparseable python",
        })
        return 0

    count = 0
    candidates: list[tuple[str, int, str]] = []  # (func_name, line, source)
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        if not (isinstance(node.func, ast.Name) and node.func.id in HELPERS):
            continue
        if not node.args:
            continue

        line = getattr(node, "lineno", 0)
        scope = _scope_for(tree, line)
        func_name = scope.name if hasattr(scope, "name") else "<module>"
        consts = _local_string_consts(scope)
        source = _resolve_literal(node.args[0], consts)

        if source is None:
            skipped.append({
                "source_file": path.name,
                "test_function": func_name,
                "line": line,
                "reason": f"non-literal arg ({type(node.args[0]).__name__})",
            })
            continue
        candidates.append((func_name, line, source))

    # Assign output names: `{func}.helen` when unique, `{func}_{i}.helen` otherwise.
    func_counts: dict[str, int] = {}
    for func_name, _, _ in candidates:
        func_counts[func_name] = func_counts.get(func_name, 0) + 1

    per_func: dict[str, int] = {}
    for func_name, line, source in candidates:
        count += 1
        per_func[func_name] = per_func.get(func_name, 0) + 1
        suffix = "" if func_counts[func_name] == 1 else f"_{per_func[func_name]}"
        out_name = f"{func_name}{suffix}.helen"
        out_path = out_dir / suite / out_name
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(_ensure_stdlib_block(source), encoding="utf-8")

        extracted.append({
            "source_file": path.name,
            "test_function": func_name,
            "line": line,
            "output": f"{suite}/{out_name}",
            "source_hash": hashlib.sha256(source.encode("utf-8")).hexdigest()[:16],
        })
    return count


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="extract_corpus.py")
    parser.add_argument("suite_dir", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--suite", required=True)
    args = parser.parse_args(argv)

    test_files = sorted(args.suite_dir.rglob("test_*.py"))
    if not test_files:
        print(f"extract_corpus.py: no test_*.py under {args.suite_dir}", file=sys.stderr)
        return 2

    extracted: list[dict] = []
    skipped: list[dict] = []
    total = 0
    for tf in test_files:
        total += extract_file(tf, args.suite, args.out, extracted, skipped)

    manifest = {
        "suite": args.suite,
        "source_dir": str(args.suite_dir),
        "generated_by": "extract_corpus.py (helen-rust M0)",
        "extracted": extracted,
        "skipped": skipped,
    }
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8"
    )

    print(f"extracted {total} sources from {len(test_files)} files -> {args.out}/{args.suite}")
    print(f"skipped {len(skipped)} non-literal sources (see manifest.json)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
