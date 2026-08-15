#!/usr/bin/env python3
"""M4 Task 4.1 — extract the stdlib builtin catalog.

Parses `helen/stdlib/__init__.py` registration tables and emits
`crates/helen-interpreter/src/builtins_catalog.rs` (a Rust static array)
plus `stdlib_catalog.json` (name, module, description, signature, category).

The Rust catalog is the source of truth for:
  * the `std.core` builtin registry (runtime dispatch),
  * `helen docgen` parity (descriptions/signatures),
  * the interpreter's module export tables.

Regenerate when the Python stdlib changes:  python3 scripts/extract_builtins.py
"""
from __future__ import annotations

import json
import os
import sys

HELEN_SRC = os.environ.get("HELEN_SRC", os.path.expanduser("~/helen"))
OUT_JSON = os.path.join(
    os.path.dirname(__file__), "..", "crates", "helen-interpreter", "stdlib_catalog.json"
)
OUT_RS = os.path.join(
    os.path.dirname(__file__), "..", "crates", "helen-interpreter", "src", "builtins_catalog.rs"
)


def main() -> int:
    sys.path.insert(0, HELEN_SRC)
    from helen.stdlib import stdlib  # type: ignore

    builtins = []
    for name in sorted(stdlib._canonical_names):
        f = stdlib._builtins[name]
        builtins.append(
            {
                "name": f.name,
                "description": f.description,
                "signature": f.signature,
                "category": f.category,
            }
        )
    aliases = {a: stdlib._aliases[a] for a in sorted(stdlib._aliases)}

    catalog = {"count": len(builtins), "builtins": builtins, "aliases": aliases}
    with open(OUT_JSON, "w", encoding="utf-8") as fh:
        json.dump(catalog, fh, ensure_ascii=False, indent=1)
        fh.write("\n")

    # --- Rust catalog -------------------------------------------------------
    lines = []
    lines.append("//! Auto-generated stdlib catalog (M4 Task 4.1).")
    lines.append("//! Source: Python `helen/stdlib/__init__.py` registration tables.")
    lines.append("//! Regenerate: `python3 scripts/extract_builtins.py`.")
    lines.append("")
    lines.append("/// A catalog entry for docgen / registry metadata.")
    lines.append("pub struct BuiltinCatalogEntry {")
    lines.append("    pub name: &'static str,")
    lines.append("    pub description: &'static str,")
    lines.append("    pub signature: &'static str,")
    lines.append("    pub category: &'static str,")
    lines.append("}")
    lines.append("")
    lines.append("/// All 378 canonical builtins, sorted by name.")
    lines.append("pub static BUILTIN_CATALOG: &[BuiltinCatalogEntry] = &[")
    for b in builtins:
        lines.append(
            f"    BuiltinCatalogEntry {{\n"
            f"        name: {json.dumps(b['name'])},\n"
            f"        description: {json.dumps(b['description'])},\n"
            f"        signature: {json.dumps(b['signature'])},\n"
            f"        category: {json.dumps(b['category'])},\n"
            f"    }},"
        )
    lines.append("];")
    lines.append("")
    lines.append("/// All 351 localized aliases: alias -> canonical name.")
    lines.append("pub static BUILTIN_ALIASES: &[(&str, &str)] = &[")
    for alias, canon in aliases.items():
        lines.append(
            f"    ({json.dumps(alias)}, {json.dumps(canon)}),"
        )
    lines.append("];")
    lines.append("")

    os.makedirs(os.path.dirname(OUT_RS), exist_ok=True)
    with open(OUT_RS, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))

    print(f"catalog: {len(builtins)} builtins, {len(aliases)} aliases")
    print(f"  json -> {os.path.relpath(OUT_JSON, os.getcwd())}")
    print(f"  rust -> {os.path.relpath(OUT_RS, os.getcwd())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
