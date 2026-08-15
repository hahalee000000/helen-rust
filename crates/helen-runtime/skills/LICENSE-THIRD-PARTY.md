# Third-Party Skills License Notice

The skills in this directory are derived from [Hermes Agent](https://github.com/NousResearch/hermes-agent)
by Nous Research, used under the **MIT License**.

## MIT License

```
MIT License

Copyright (c) 2025 Nous Research

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Skills Included

The following skills were copied from Hermes Agent and adapted for Helen:

| Skill | Original Author | License |
|-------|----------------|---------|
| `software-development/helen-language-development` | Hermes Agent | MIT |
| `software-development/code-quality` | Hermes Agent | MIT |
| `software-development/debugging` | Hermes Agent | MIT |
| `software-development/test-driven-development` | Hermes Agent (adapted from obra/superpowers) | MIT |
| `software-development/writing-plans` | Hermes Agent (adapted from obra/superpowers) | MIT |
| `software-development/plan` | Hermes Agent (writing-craft adapted from obra/superpowers) | MIT |
| `software-development/subagent-driven-development` | Hermes Agent (adapted from obra/superpowers) | MIT |
| `devops/github` | Hermes Agent | MIT |

## Adaptations Made

When integrating these skills into Helen, the following adaptations were made:

1. **plan**: Changed `.hermes/plans/` → `.helen/plans/` for Helen-native path convention
2. **subagent-driven-development**: Added note about Hermes vs Helen standalone context for `hermes_tools` API
3. **github/scripts/gh-env.sh**: Added `~/.helen/.env` as primary config source, with `~/.hermes/.env` as fallback
4. **All skills**: Added `ATTRIBUTION.md` to each skill directory

## Original Source

Original skills are available at:
- Repository: https://github.com/NousResearch/hermes-agent
- Skills directory: `~/.hermes/skills/`

Each skill's `ATTRIBUTION.md` file contains the specific attribution details.
