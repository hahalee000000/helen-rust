# Helen Rust Skills

> **helen-rust** bundled skills — professional knowledge and workflows for AI
> agents using the Rust implementation of Helen.

These skills mirror the reference implementation's built-in skills
(`~/helen/helen/skills/`) and are distributed with the Rust runtime. They are
found by the Rust skills loader (`crates/helen-runtime/src/skills.rs`) via the
bundled `skills/` directory next to the runtime crate.

## Directory Structure

```
crates/helen-runtime/skills/          ← Bundled skills (distributed with helen-rust)
├── README.md                          ← This file
├── LICENSE-THIRD-PARTY.md             ← Third-party license notices
├── software-development/
│   ├── differential-porting/          ← ⭐ helen-rust specific (from M1–M12)
│   ├── helen-language-development/
│   ├── helen-syntax/
│   ├── helen-stdlib/
│   ├── helen-agent-patterns/
│   ├── helen-agent-collaboration/
│   ├── helen-testing/
│   ├── helen-quality/
│   ├── code-quality/
│   ├── debugging/
│   ├── test-driven-development/
│   ├── planning/
│   └── subagent-driven-development/
└── devops/
    └── github/
```

## Skill Search Priority

The Rust runtime searches for skills in the following order (higher priority
overrides lower):

| Priority | Location | Description |
|----------|----------|-------------|
| 1 (highest) | `<project>/.helen/skills/` | **Project-level** — closest ancestor of cwd |
| 2 | `~/.helen/skills/` | **User-level** |
| 3 | bundled `skills/` (this dir) | **Built-in** — distributed with helen-rust |

## How Skills Work

Two-tier skill disclosure:

1. **Tier 1 — Skill Index**: lightweight metadata (name + description) injected
   into the `<available_skills>` section of the system prompt.
2. **Tier 2 — Full Content**: agents call `load_skill` to read the complete
   `SKILL.md`.

## Rust Implementation Notes

- **`differential-porting`** — the flagship helen-rust skill: byte-faithful
  porting methodology, Rust porting pitfalls (UFCS recursion, `Arc<dyn Trait>`
  &self, Send/Sync, pyo3 FFI), parser/AST fidelity traps, and the per-milestone
  gate checklist. Distilled from M1–M12 of this project.
- Language-level skills (helen-syntax, helen-stdlib, helen-testing, agent
  patterns, etc.) describe the *same* Helen language the Rust port implements;
  each carries a `helen-rust edition` note where the implementation differs.
- Methodology skills (debugging, TDD, code-quality, planning,
  subagent-driven-development, github) are implementation-agnostic and apply
  unchanged.

## Creating Skills

Create project-level skills in `.helen/skills/` (never in this bundled
directory — they would be lost on update). Format: `name/SKILL.md` with YAML
frontmatter (`name`, `description`, `version`, `author`, `tags`). See the
reference wiki `guide/` or `helen-language-development` skill for examples.

## Attribution

Skills are derived from the Helen reference implementation and [Hermes Agent]
(https://github.com/NousResearch/hermes-agent) by Nous Research, used under
the MIT license. See `LICENSE-THIRD-PARTY.md` for details.
