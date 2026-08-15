# HLD Compliance — helen-rust

> Conformance of the Rust reimplementation with the Helen Language Design
> (HLD) spec, as enforced by the differential test harness against the Python
> reference implementation.

---

## Conformance gates

| Layer | Gate | Result |
|---|---|---|
| Lex | token-stream differential (key-sorted JSON) | ✅ 36/36 |
| Parse | `--parse` JSON AST diff | ✅ 47/47 corpus |
| Semantic | exit-code parity (2) + E-code order | ✅ 21/21 (Tier C) |
| Run | `{stdout, stderr, exit_code, error_classes}` | ✅ Tier A 42/42, corpus |
| Display | byte-identical output corpus | ✅ 10/10 |
| Error codes | `error-diff.csv` | ✅ 70/70 |
| Tier B (subprocess) | language / agent / cli+ffi | ✅ 100/100 / 170/172* / 128+1skip |
| Stdlib | module/function/alias registry parity | ✅ 378 builtins |
| Docgen | `helen doc` vs reference | ✅ md byte-identical; json order-insensitive |

\* 2 agent failures are pre-existing Python-side reference bugs, not port
divergences.

## HLD quirks deliberately mirrored

- Unreachable break conditions in reference loops — matched exactly.
- Per-module constant drift (e.g. `sleep` in `std.time` not `std.core`).
- Plain-string vs wrapper formats in tool results.
- `spawn` param injection order (channel endpoint last).
- Broken corpus fixtures (`spawn_expr.helen`, `shared_store.helen`) verify
  error parity only.

## Known intentional deviations (D4 etc.)

See `wiki/rust/migration-notes.md` — byte-based strings (D4), string
iteration unsupported, spawn race strictness, custom-provider Python
dependency, context/compression quirks.

## Open issues

See `wiki/plan/STATUS.md` — coverage gate (68.82% vs targets), stdlib
surface driver, publish credentials, `export_transcript` wiring.
