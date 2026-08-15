<!-- helen-rust edition: crates/helen-runtime skills.rs (M6) — 3-layer lookup; bundled skills at crates/helen-runtime/skills/ (17 skills incl. differential-porting). -->

# Skill System

> Helen Skill System — Providing AI Agents with expertise and workflows

---

## Overview

Helen's skill system uses a **three-tier search architecture** to ensure language-bundled knowledge, user-custom skills, and project-specific skills are all correctly utilized.

```
┌─────────────────────────────────────────┐
│  🥇 Project   <project>/.helen/skills/  │  ← Highest priority
├─────────────────────────────────────────┤
│  🥈 User      ~/.helen/skills/          │  ← User global skills
├─────────────────────────────────────────┤
│  🥉 Built-in  ~/helen/skills/           │  ← Distributed with language
├─────────────────────────────────────────┤
│  4️⃣ Hermes    ~/.hermes/skills/         │  ← Compatibility fallback
├─────────────────────────────────────────┤
│  5️⃣ Hermes    ~/.hermes/hermes-agent/   │  ← Compatibility fallback
│              skills/                     │
└─────────────────────────────────────────┘
```

**Higher priority overrides lower**: If a skill with the same name exists in multiple directories, the higher-priority version is used.

---

## Three-Tier Search Details

### 🥇 Project-Level Skills

Located in the project root's `.helen/skills/`, used for project-specific knowledge:

```
my-project/
├── .helen/
│   └── skills/
│       └── my-api/
│           └── SKILL.md       # Project API documentation
├── main.helen
└── agents/
```

**Use cases**:
- Project API documentation
- Business rule descriptions
- Team coding standards
- Deployment guides

**Advantages**:
- Can be git-committed, shared across the team
- Does not pollute user global skills
- Highest priority, ensuring project knowledge is used first

### 🥈 User-Level Skills

Located in `~/.helen/skills/`, used for user's global skills:

```bash
~/.helen/
├── config.yaml
└── skills/
    └── my-workflow/
        └── SKILL.md
```

**Use cases**:
- Personal commonly-used workflows
- Cross-project custom skills

### 🥉 Built-in Skills

Located in `<helen-install>/skills/`, distributed with Helen:

```
~/helen/skills/
├── README.md
├── LICENSE-THIRD-PARTY.md
├── software-development/
│   ├── helen-language-development/   # Helen language patterns and pitfalls
│   ├── helen-syntax/                 # Syntax reference
│   ├── helen-stdlib/                 # Standard library guide
│   ├── helen-security/               # Security best practices
│   ├── helen-agent-patterns/         # Agent design patterns
│   ├── code-quality/                 # 7-dimension quality assessment
│   ├── debugging/                    # Debugging methodology
│   ├── test-driven-development/      # TDD workflow
│   └── ...
└── devops/
    ├── hellen-consistency-checker/   # Design consistency checker
    └── github/                       # GitHub workflow
```

**Currently 13 built-in skills**:

| Skill | Category | Description |
|------|------|------|
| `helen-language-development` | Helen-specific | Language patterns, pitfalls, best practices |
| `helen-syntax` | Helen-specific | Syntax reference (keywords, types, expressions) |
| `helen-stdlib` | Helen-specific | 193 standard library functions guide |
| `helen-agent-patterns` | Helen-specific | Agent design patterns |
| `code-quality` | Development | 7-dimension code quality assessment |
| `debugging` | Development | Systematic debugging methodology |
| `test-driven-development` | Development | TDD RED-GREEN-REFACTOR |
| `writing-plans` | Development | Implementation plan writing |
| `plan` | Development | Plan mode (write-only, no execution) |
| `subagent-driven-development` | Development | Subagent-driven development |
| `hellen-consistency-checker` | DevOps | Design document consistency checking |
| `github` | DevOps | GitHub workflow |

---

## Two-Phase Disclosure Mechanism

Helen uses **two-phase skill disclosure** to balance knowledge coverage and token consumption:

### Tier 1 — Skill Index (Lightweight)

All skills' names, descriptions, and tags are collected into an index and injected into the LLM's system prompt:

```
<available_skills>
Before replying, scan skills below. If relevant,
use load_skill tool to load full content.

  devops:
    - github: Complete GitHub workflow (tags: GitHub, Git, Pull-Requests, Issues, Code-Review)
  software-development:
    - helen-syntax: Helen syntax reference (tags: helen, syntax, reference, language)
    - helen-stdlib: 193 standard library functions (tags: helen, stdlib, builtins, reference)
  ...
</available_skills>
```

**Features**:
- Contains name + description + category + **tags** (lightweight)
- Tags field helps LLM quickly locate relevant skills via keywords, improving hit rate
- Helps LLM decide which skill to load

### Tier 2 — Full Load (On-Demand)

When the LLM needs detailed content of a skill, it calls the `load_skill` tool:

```python
# LLM calls via function calling
load_skill(name="helen-stdlib")
# Returns full SKILL.md content

# Can also list reference documents at the same time
load_skill(name="helen-language-development", include_references=True)
# Returns SKILL.md content + references/ directory file list
```

**Features**:
- On-demand loading, saves tokens
- `include_references=true` can also fetch the reference document list

### Tier 3 — Reference Documents (Deep Dive)

Skills may contain a `references/` directory for in-depth reference documents. Accessible via:

```python
# Approach 1: List all reference documents (name, path, size, preview)
list_skill_references(name="helen-language-development")

# Approach 2: Include reference list with load_skill
load_skill(name="helen-language-development", include_references=True)

# Approach 3: Load specific reference document with read_file
read_file(path=".../helen-language-development/references/parser-disambiguation.md")
```

**`list_skill_references` return format**:
```json
{
  "name": "helen-language-development",
  "skill_path": ".../SKILL.md",
  "references": [
    {"name": "parser-disambiguation.md", "path": "...", "size": 3200, "preview": "# Parser disambiguation..."},
    ...
  ],
  "total": 17
}
```

**Features**:
- Reference documents are not automatically loaded; LLM consults them on demand
- Each reference file includes a 3-line preview to help LLM decide whether to load it

---

## Skill Format

Each skill is a directory containing `SKILL.md`:

```
my-skill/
├── SKILL.md          # Required: Skill main file
├── references/       # Optional: Reference documents
├── templates/        # Optional: Template files
├── scripts/          # Optional: Helper scripts
└── assets/           # Optional: Static resources
```

### SKILL.md Format

```markdown
---
name: my-skill
description: "Skill description"
version: 1.0.0
author: Author name
license: MIT
tags: [helen, tutorial]
---

# Skill Title

## Overview
Skill content...

## Examples
Code examples...
```

---

## Creating Custom Skills

### 1. Project-Level Skill

```bash
cd my-project/
mkdir -p .helen/skills/my-api/
cat > .helen/skills/my-api/SKILL.md << 'EOF'
---
name: my-api
description: "My Project API Documentation"
version: 1.0.0
---

# My API

## Endpoints
- GET /api/users
- POST /api/users
EOF
```

### 2. User-Level Skill

```bash
mkdir -p ~/.helen/skills/my-workflow/
# Create SKILL.md...
```

---

## Implementation Details

Skill search is implemented by the `get_skill_dirs()` function in `helen/runtime/config.py`:

```python
def get_skill_dirs() -> list[Path]:
    """Return skill directory list, sorted by priority."""
    dirs = []

    # 1. Project-level (walk up from current directory for .helen/skills/)
    cwd = Path.cwd()
    for parent in [cwd] + list(cwd.parents):
        project_skills = parent / ".helen" / "skills"
        if project_skills.exists():
            dirs.append(project_skills)
            break

    # 2. User-level
    helen_skills = Path.home() / ".helen" / "skills"
    if helen_skills.exists():
        dirs.append(helen_skills)

    # 3. Built-in (distributed with Helen)
    helen_package = Path(__file__).parent.parent.parent
    builtin_skills = helen_package / "skills"
    if builtin_skills.exists():
        dirs.append(builtin_skills)

    # 4-5. Hermes fallback
    hermes_skills = Path.home() / ".hermes" / "skills"
    if hermes_skills.exists():
        dirs.append(hermes_skills)

    return dirs
```

---

## Statistics

| Tier | Skill Count | Description |
|------|--------|------|
| Built-in | 13 | Distributed with Helen |
| Hermes fallback | 63 | Hermes Agent compatibility |
| Hermes Agent | 73 | Hermes core skills |
| **Total** | **149** | All available skills |
