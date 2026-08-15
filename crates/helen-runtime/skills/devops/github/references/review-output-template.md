# Review Output Template

Use this as the structure for PR review summary comments.

## For PR Summary Comment

```markdown
## Code Review Summary

**Verdict: [Approved ✅ | Changes Requested 🔴 | Reviewed 💬]** ([N] issues, [N] suggestions)

**PR:** #[number] — [title]
**Author:** @[username]
**Files changed:** [N] (+[additions] -[deletions])

### 🔴 Critical
- **file.py:line** — [description]. Suggestion: [fix].

### ⚠️ Warnings
- **file.py:line** — [description].

### 💡 Suggestions
- **file.py:line** — [description].

### ✅ Looks Good
- [aspect that was done well]

---
*Reviewed by Hermes Agent*
```

## Severity Guide

| Level | Icon | When to use | Blocks merge? |
|-------|------|-------------|---------------|
| Critical | 🔴 | Security vulnerabilities, data loss, crashes | Yes |
| Warning | ⚠️ | Bugs in non-critical paths, missing error handling | Usually yes |
| Suggestion | 💡 | Style, refactoring ideas, perf hints | No |
| Looks Good | ✅ | Clean patterns, good coverage | N/A |

## Verdict Decision

- **Approved ✅** — Zero critical/warning items
- **Changes Requested 🔴** — Any critical or warning item exists
- **Reviewed 💬** — Observations only (draft PRs, uncertain findings)

## For Inline Comments

Prefix with severity icon:
- `🔴 **Critical:** ...`
- `⚠️ **Warning:** ...`
- `💡 **Suggestion:** ...`
- `✅ **Nice:** ...`
