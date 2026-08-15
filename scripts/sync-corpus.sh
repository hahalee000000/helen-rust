#!/usr/bin/env bash
# sync-corpus.sh — M14 Task 14.3: pull the latest `.helen` corpus from the
# Python reference (`~/helen/`) into this repo, re-capturing goldens so new
# test programs are picked up automatically.
#
# Usage:
#   bash scripts/sync-corpus.sh            # sync + recapture + report
#   bash scripts/sync-corpus.sh --dry-run  # show what would change
#
# Env:
#   HELEN_SRC — path to the Python reference repo (default: $HOME/helen)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

HELEN_SRC="${HELEN_SRC:-$HOME/helen}"
DRY_RUN=0
[[ "${1:-}" == "--dry-run" ]] && DRY_RUN=1

if [[ ! -d "$HELEN_SRC/tests" ]]; then
  echo "(!) HELEN_SRC=$HELEN_SRC has no tests/ dir" >&2
  exit 1
fi

echo "── syncing corpus from $HELEN_SRC ──"

# 1. Extract inline `run_helen(src)` test programs (Tier A).
#    extract_corpus.py lives in this repo and knows the pytest shapes.
if [[ -f tests/conformance/extract_corpus.py ]]; then
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "  [dry] python3 tests/conformance/extract_corpus.py $HELEN_SRC/tests/interpreter --out tests/programs/pytest --suite interpreter"
  else
    python3 tests/conformance/extract_corpus.py "$HELEN_SRC/tests/interpreter" \
      --out tests/programs/pytest --suite interpreter || \
      echo "(!) interpreter suite: none extractable (see extract_corpus.py)"
  fi
fi

# 2. Copy any new authored `.helen` programs from the reference corpus.
NEW=0
for f in "$HELEN_SRC"/tests/programs/*.helen; do
  [[ -f "$f" ]] || continue
  name="$(basename "$f")"
  if [[ ! -f "tests/programs/authored/$name" ]]; then
    if [[ "$DRY_RUN" -eq 1 ]]; then
      echo "  [dry] new authored program: $name"
    else
      cp "$f" "tests/programs/authored/$name"
      echo "  + added authored/$name"
    fi
    NEW=$((NEW+1))
  fi
done
[[ "$DRY_RUN" -eq 1 ]] && echo "  (dry-run) $NEW new programs detected" && exit 0

# 3. Recapture goldens for every corpus suite (authored, interpreter, display).
echo "── recapturing goldens ──"
for suite in authored interpreter display; do
  dir="tests/programs/$suite"
  [[ -d "$dir" ]] || continue
  python3 tests/conformance/capture_golden.py "$dir" \
    --out tests/conformance/golden --suite "$suite"
done

# 4. Re-run the error-diff sweep (exit-code + E-code parity).
if [[ -f scripts/gen-error-diff.py ]]; then
  python3 scripts/gen-error-diff.py --all >/dev/null 2>&1 || true
fi

echo "✅ corpus synced ($NEW new)"
