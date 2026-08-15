# Attribution: github

- **Original source**: [Hermes Agent](https://github.com/NousResearch/hermes-agent) by Nous Research
- **Original path**: `~/.hermes/skills/devops/github/`
- **License**: MIT
- **Original author**: Hermes Agent
- **Adapted for**: Helen programming language (https://github.com/hahalee000000/helen)

## Changes from original

- Updated `scripts/gh-env.sh`:
  - Added `~/.helen/.env` as primary config source (checked before `~/.hermes/.env`)
  - Updated usage comment to show both Helen and Hermes paths
  - Changed header from "Hermes Agent skills" to "Helen/Hermes Agent skills"
- Removed Hermes-specific frontmatter metadata (`metadata.hermes` block)

## Original copyright

```
MIT License
Copyright (c) 2025 Nous Research
```
