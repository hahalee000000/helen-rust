# Conventional Commits Quick Reference

Format: `type(scope): description`

## Types

| Type | When to use | Example |
|------|------------|---------|
| `feat` | New feature | `feat(auth): add OAuth2 login flow` |
| `fix` | Bug fix | `fix(api): handle null response` |
| `refactor` | Restructuring, no behavior change | `refactor(db): extract query builder` |
| `docs` | Documentation only | `docs: update API examples` |
| `test` | Adding/updating tests | `test(auth): add integration tests` |
| `ci` | CI/CD configuration | `ci: add Python 3.12 to matrix` |
| `chore` | Maintenance, deps, tooling | `chore: upgrade pytest to 8.x` |
| `perf` | Performance improvement | `perf(search): add index on email` |
| `style` | Formatting, whitespace | `style: run black formatter` |
| `build` | Build system or external deps | `build: switch to hatch` |
| `revert` | Reverts a previous commit | `revert: revert "feat(auth): ..."` |

## Breaking Changes

Add `!` after type or `BREAKING CHANGE:` in footer:
```
feat(api)!: change auth to bearer tokens

BREAKING CHANGE: API now requires Bearer token instead of API key header.
```

## Multi-line Body

Wrap at 72 characters:
```
feat(auth): add JWT-based user authentication

- Add login/register endpoints with input validation
- Add User model with argon2 password hashing
- Add auth middleware for protected routes

Closes #42
```

## Linking Issues

```
Closes #42    ← closes when merged
Fixes #42     ← same effect
Refs #42      ← references without closing
```
