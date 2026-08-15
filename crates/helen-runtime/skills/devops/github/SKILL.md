---
name: github
description: "Complete GitHub workflow: auth, repos, PRs, code review, issues, releases, CI/CD, and Actions. Covers both `gh` CLI and git+curl fallbacks."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [GitHub, Git, Pull-Requests, Issues, Code-Review, CI/CD, Releases, Authentication]
---

# GitHub — Complete Workflow

This is the umbrella skill for all GitHub operations. It covers authentication, repository management, pull request workflows, code review, issues, releases, secrets, and GitHub Actions.

## Subsections

Each subsection below covers a distinct area of GitHub workflow. For detailed reference material, templates, and scripts, see the `references/`, `templates/`, and `scripts/` directories.

---

## 1. Authentication Setup

Before any GitHub operation, ensure authentication is configured. Two paths:

### Detection Flow

```bash
# Check what's available
git --version
gh --version 2>/dev/null || echo "gh not installed"
gh auth status 2>/dev/null || echo "gh not authenticated"
```

**Decision tree:**
1. If `gh auth status` shows authenticated → use `gh` for everything
2. If `gh` installed but not authenticated → use `gh auth login`
3. If `gh` not installed → use git-only method (HTTPS token or SSH key)

### Git-Only (No gh, No sudo)

**HTTPS with Personal Access Token:**
1. Create token at https://github.com/settings/tokens (scopes: `repo`, `workflow`, `read:org`)
2. Configure: `git config --global credential.helper store`
3. Test: `git ls-remote https://github.com/<user>/<repo>.git` (enter credentials once)

**SSH Key:**
1. Generate: `ssh-keygen -t ed25519 -C "email@example.com" -f ~/.ssh/id_ed25519 -N ""`
2. Add public key at https://github.com/settings/keys
3. Test: `ssh -T git@github.com`
4. Optional: `git config --global url."git@github.com:".insteadOf "https://github.com/"`

### gh CLI Auth

```bash
# Interactive (desktop with browser)
gh auth login

# Headless/token-based
echo "<TOKEN>" | gh auth login --with-token
gh auth setup-git
```

### Auth Detection Helper

Use this pattern at the start of any GitHub workflow:

```bash
if command -v gh &>/dev/null && gh auth status &>/dev/null; then
  AUTH_METHOD="gh"
elif [ -n "$GITHUB_TOKEN" ]; then
  AUTH_METHOD="curl"
elif grep -q "github.com" ~/.git-credentials 2>/dev/null; then
  export GITHUB_TOKEN=$(grep "github.com" ~/.git-credentials | head -1 | sed 's|https://[^:]*:\([^@]*\)@.*|\1|')
  AUTH_METHOD="curl"
else
  AUTH_METHOD="none"
fi
```

See `references/auth-troubleshooting.md` for common issues (stale credentials, SSH over HTTPS port, multiple accounts).

---

## 2. Repository Management

### Clone

```bash
git clone https://github.com/owner/repo.git
gh repo clone owner/repo            # shorthand
git clone --depth 1 URL             # shallow (faster)
```

### Create

```bash
gh repo create my-project --public --clone
gh repo create my-project --private --description "desc" --license MIT --clone
gh repo create org/project --public --clone    # under org
```

With curl: `POST https://api.github.com/user/repos` with JSON body `{"name": "...", "private": false}`.

### Fork

```bash
gh repo fork owner/repo --clone
# Then: git remote add upstream https://github.com/owner/repo.git
# Sync: git fetch upstream && git checkout main && git merge upstream/main && git push
```

### Repo Info & Settings

```bash
gh repo view owner/repo
gh repo edit --description "..." --visibility public --enable-auto-merge
gh repo edit --add-topic "python,automation"
```

### Releases

```bash
gh release create v1.0.0 --title "v1.0.0" --generate-notes
gh release create v2.0.0-rc1 --draft --prerelease --generate-notes
gh release list
gh release download v1.0.0
```

### Gists

```bash
gh gist create script.py --public --desc "Useful script"
gh gist list
```

See `references/repo-management-api.md` for curl equivalents and branch protection setup.

---

## 3. Pull Request Workflow

### Branch & Commit

```bash
git checkout main && git pull origin main
git checkout -b feat/description
# ... make changes ...
git add . && git commit -m "feat: add feature X"
git push -u origin HEAD
```

Branch naming: `feat/`, `fix/`, `refactor/`, `docs/`, `ci/`, `chore/`

**⚠️ Pitfall: "Commit to git" means push to remote.** When the user asks you to commit changes, always complete the full workflow: `git add` → `git commit` → `git push`. Do not stop at local commit — the user expects the changes to be available on the remote repository. If you only do a local commit, the user will ask "why can't I pull?" and you'll look like you didn't finish the job.

### Create PR

```bash
gh pr create --title "feat: description" --body "## Summary\n..." --label "enhancement"
gh pr create --draft    # draft PR
```

With curl: `POST https://api.github.com/repos/{owner}/{repo}/pulls` with `{"title": "...", "head": "branch", "base": "main", "body": "..."}`.

### Monitor CI

```bash
gh pr checks                    # one-shot
gh pr checks --watch            # poll until done
```

### Auto-Fix CI Loop

1. `gh run list --branch <branch> --limit 5` → identify failures
2. `gh run view <RUN_ID> --log-failed` → read logs
3. Fix code, commit, push
4. Re-check until green (max 3 attempts, then ask user)

### Merge

```bash
gh pr merge --squash --delete-branch
gh pr merge --auto --squash --delete-branch    # auto-merge when checks pass
```

See `references/ci-troubleshooting.md` and `references/conventional-commits.md` for details.
See `templates/pr-body-feature.md` and `templates/pr-body-bugfix.md` for PR body templates.

---

## 4. Code Review

### Reviewing Local Changes (Pre-Push)

```bash
git diff main...HEAD --stat          # scope
git diff main...HEAD                 # full diff
git diff main...HEAD | grep -n "print(\|console\.log\|TODO\|FIXME\|debugger"  # leftovers
git diff main...HEAD | grep -in "password\|secret\|api_key\|token.*="         # secrets
```

### Reviewing a PR on GitHub

```bash
gh pr view 123
gh pr diff 123
gh pr diff 123 --name-only
gh pr checkout 123                   # check out locally for full review
```

### Leave Comments

```bash
gh pr comment 123 --body "Overall looks good..."
gh pr review 123 --approve --body "LGTM!"
gh pr review 123 --request-changes --body "See inline comments."
```

### Review Checklist

- **Correctness**: Edge cases, error paths, does it do what it claims?
- **Security**: No hardcoded secrets, input validation, no injection
- **Quality**: Clear naming, DRY, single responsibility
- **Testing**: New paths tested, happy + error cases
- **Performance**: No N+1, appropriate caching
- **Documentation**: Public APIs documented, "why" comments

See `references/review-output-template.md` for structured review output format.

---

## 5. Issues Management

### View & Search

```bash
gh issue list
gh issue list --state open --label "bug"
gh issue list --assignee @me
gh issue list --search "authentication error" --state all
gh issue view 42
```

### Create

```bash
gh issue create --title "Bug title" --body "..." --label "bug,backend" --assignee "user"
```

### Manage

```bash
gh issue edit 42 --add-label "priority:high"
gh issue edit 42 --add-assignee username
gh issue comment 42 --body "Investigated — root cause is X"
gh issue close 42
gh issue close 42 --reason "not planned"
gh issue reopen 42
```

### Triage Workflow

1. List untriaged: `gh issue list --label "needs-triage" --state open`
2. Read and categorize each
3. Apply labels and priority
4. Assign if owner is clear
5. Comment with triage notes

### Bulk Operations

```bash
gh issue list --label "wontfix" --json number --jq '.[].number' | \
  xargs -I {} gh issue close {} --reason "not planned"
```

See `templates/bug-report.md` and `templates/feature-request.md` for issue templates.

---

## 6. GitHub Actions & Secrets

### Workflows

```bash
gh workflow list
gh run list --limit 10
gh run view <RUN_ID>
gh run view <RUN_ID> --log-failed
gh run rerun <RUN_ID>
gh run rerun <RUN_ID> --failed
gh workflow run ci.yml --ref main
```

### Secrets

```bash
gh secret set API_KEY --body "value"
gh secret set SSH_KEY < ~/.ssh/id_rsa
gh secret list
gh secret delete API_KEY
```

Note: Setting secrets via curl requires encrypting with the repo's public key (PyNaCl). Use `gh` when possible — it's dramatically simpler.

---

## Quick Reference

| Action | gh command | curl endpoint |
|--------|-----------|---------------|
| Auth check | `gh auth status` | — |
| Clone | `gh repo clone o/r` | `git clone` |
| Create repo | `gh repo create name` | `POST /user/repos` |
| Create PR | `gh pr create` | `POST /repos/o/r/pulls` |
| Check CI | `gh pr checks` | `GET /commits/sha/status` |
| Review PR | `gh pr review N --approve` | `POST /pulls/N/reviews` |
| List issues | `gh issue list` | `GET /repos/o/r/issues` |
| Create issue | `gh issue create` | `POST /repos/o/r/issues` |
| Create release | `gh release create v1` | `POST /repos/o/r/releases` |
| Set secret | `gh secret set KEY` | `PUT /repos/o/r/actions/secrets/KEY` |
| Rerun CI | `gh run rerun ID` | `POST /runs/ID/rerun` |
