# GitHub REST API Cheatsheet

Base URL: `https://api.github.com`
All requests need: `-H "Authorization: token $GITHUB_TOKEN"`

## Repositories

| Action | Method | Endpoint |
|--------|--------|----------|
| Get repo | GET | `/repos/{owner}/{repo}` |
| Create (user) | POST | `/user/repos` |
| Create (org) | POST | `/orgs/{org}/repos` |
| Update | PATCH | `/repos/{owner}/{repo}` |
| Fork | POST | `/repos/{owner}/{repo}/forks` |
| Template | POST | `/repos/{owner}/{template}/generate` |
| Topics | GET/PUT | `/repos/{owner}/{repo}/topics` |

## Pull Requests

| Action | Method | Endpoint |
|--------|--------|----------|
| List | GET | `/repos/{o}/{r}/pulls?state=open` |
| Create | POST | `/repos/{o}/{r}/pulls` |
| Get | GET | `/repos/{o}/{r}/pulls/{n}` |
| Files | GET | `/repos/{o}/{r}/pulls/{n}/files` |
| Merge | PUT | `/repos/{o}/{r}/pulls/{n}/merge` |
| Review | POST | `/repos/{o}/{r}/pulls/{n}/reviews` |
| Inline comment | POST | `/repos/{o}/{r}/pulls/{n}/comments` |

Merge body: `{"merge_method": "squash"}` — methods: `"merge"`, `"squash"`, `"rebase"`
Review events: `"APPROVE"`, `"REQUEST_CHANGES"`, `"COMMENT"`

## Issues

| Action | Method | Endpoint |
|--------|--------|----------|
| List | GET | `/repos/{o}/{r}/issues?state=open` |
| Create | POST | `/repos/{o}/{r}/issues` |
| Comment | POST | `/repos/{o}/{r}/issues/{n}/comments` |
| Labels | POST | `/repos/{o}/{r}/issues/{n}/labels` |
| Assign | POST | `/repos/{o}/{r}/issues/{n}/assignees` |
| Search | GET | `/search/issues?q=...+repo:{o}/{r}` |

Note: Issues API also returns PRs. Filter with `'pull_request' not in item`.

## CI / Actions

| Action | Method | Endpoint |
|--------|--------|----------|
| Workflows | GET | `/repos/{o}/{r}/actions/workflows` |
| Runs | GET | `/repos/{o}/{r}/actions/runs` |
| Run logs | GET | `/repos/{o}/{r}/actions/runs/{id}/logs` |
| Rerun | POST | `/repos/{o}/{r}/actions/runs/{id}/rerun` |
| Rerun failed | POST | `/repos/{o}/{r}/actions/runs/{id}/rerun-failed-jobs` |
| Dispatch | POST | `/repos/{o}/{r}/actions/workflows/{id}/dispatches` |
| Commit status | GET | `/repos/{o}/{r}/commits/{sha}/status` |

## Releases

| Action | Method | Endpoint |
|--------|--------|----------|
| List | GET | `/repos/{o}/{r}/releases` |
| Create | POST | `/repos/{o}/{r}/releases` |
| Upload asset | POST | `https://uploads.github.com/repos/{o}/{r}/releases/{id}/assets?name={file}` |

## Secrets

| Action | Method | Endpoint |
|--------|--------|----------|
| List | GET | `/repos/{o}/{r}/actions/secrets` |
| Public key | GET | `/repos/{o}/{r}/actions/secrets/public-key` |
| Set | PUT | `/repos/{o}/{r}/actions/secrets/{name}` |

## Pagination & Rate Limits

- `?per_page=100` (max), `?page=2` for next page
- Authenticated: 5,000 req/hour
- Check: `GET /rate_limit`
