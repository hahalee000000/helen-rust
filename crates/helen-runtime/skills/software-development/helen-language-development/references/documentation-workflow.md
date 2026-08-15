# Documentation & Wiki Workflow

> Patterns for keeping Helen documentation in sync across wiki and git repo

## Overview

Helen documentation exists in two locations that must stay synchronized:

1. **Wiki** (`~/wiki/helen/`) — Standalone documentation site, NOT git-tracked
2. **Git Repo** (`~/helen/docs/`) — Version-controlled with source code

## Directory Structure

### Wiki Structure
```
~/wiki/helen/
├── index.md                    # Wiki index/navigation
├── tutorial/
│   ├── 01-getting-started.md
│   ├── ...
│   ├── 10-stdlib.md           # User-facing stdlib tutorial
│   └── 11-building-agents.md
├── toolchain/
│   ├── stdlib.md              # Technical stdlib reference
│   ├── cli.md
│   └── ...
└── ...
```

### Git Repo Structure
```
~/helen/docs/
├── tutorial.md                # Complete tutorial (all sections)
├── stdlib_design.md           # Stdlib design document
├── stdlib_implementation_report.md
├── stdlib_p1_implementation_report.md
└── stdlib_p2_p3_implementation_report.md
```

## Update Workflow

### After Implementing New Stdlib Modules

1. **Update Wiki Tutorial** (`~/wiki/helen/tutorial/10-stdlib.md`):
   ```markdown
   # 教程 10: 标准库参考
   
   > 200 个内置函数，覆盖 AI 应用开发的所有核心需求
   
   ## 概览
   
   | 类别 | 函数数 | 功能 |
   |------|--------|------|
   | **String** | 37 | 字符串处理、正则、文本分析 |
   | **Data** | 25 | JSON、HTML、CSV、Markdown、YAML、TOML、XML |
   | **File** | 18 | 文件读写、目录操作、临时文件、文件搜索 |
   ...
   ```

2. **Update Wiki Technical Docs** (`~/wiki/helen/toolchain/stdlib.md`):
   ```markdown
   # 标准库 (Stdlib)
   
   > 模块 M15 | `helen/stdlib/__init__.py` | **185 builtins** | 测试: `tests/stdlib/`
   
   ## 函数分类统计
   
   | 类别 | 函数数 | 模块文件 |
   |------|--------|----------|
   | **Core** | 11 | `__init__.py` |
   | **String** | 36 | `string.py` |
   ...
   ```

4. **Update Wiki Index** (`~/wiki/helen/index.md`):
   ```markdown
   - [[toolchain/stdlib|标准库]] — 185 builtins (core/string/data/collection/network/time/math/file/system/crypto/io)
   ```

5. **Commit and Push**:
   ```bash
   cd ~/helen
   git add -A
   git commit -m "docs: update stdlib reference (185 functions)"
   git push origin master
   ```

## Key Differences

### Wiki Tutorial (User-Facing)
- **Purpose**: Teach users how to use stdlib
- **Style**: Examples, explanations, exercises
- **Format**: Organized by category with code snippets
- **Audience**: Helen programmers

### Wiki Technical Docs (Developer-Facing)
- **Purpose**: Reference for developers extending stdlib
- **Style**: Function signatures, implementation details
- **Format**: Tables with signatures and descriptions
- **Audience**: Contributors, maintainers

### Git Repo Tutorial (Version-Controlled)
- **Purpose**: Single source of truth for tutorial content
- **Style**: Same as wiki tutorial
- **Format**: One large file with all sections
- **Audience**: Anyone reading from git repo

## Synchronization Strategy

### What to Sync
- ✅ Function lists and counts
- ✅ Code examples
- ✅ Category organization
- ✅ Usage patterns

### What NOT to Sync
- ❌ Wiki-specific navigation/links
- ❌ Git-specific metadata
- ❌ Implementation reports (git-only)

### Sync Direction
```
Wiki Tutorial ←→ Git Repo Tutorial
     ↓
Wiki Technical Docs (separate, technical focus)
```

## Common Pitfalls

### 1. Wiki is Not Git-Tracked
**Problem**: Editing wiki files doesn't create git commits
**Solution**: Always sync wiki changes to git repo manually

### 2. Tutorial.md is One Large File
**Problem**: Git repo has all tutorials in one file (`tutorial.md`)
**Solution**: Use `patch` to replace specific sections, not entire file

### 3. Function Count Mismatch
**Problem**: Different files show different function counts
**Solution**: Update all locations in same session:
- Wiki tutorial header
- Wiki technical docs header
- Wiki index entry
- Git repo tutorial section

### 4. Missing Category Updates
**Problem**: Adding functions to existing category without updating counts
**Solution**: Update category table in all three locations:
```markdown
| **String** | 36 | 字符串处理、正则、文本分析 |
```

### 5. Table of Contents Drift
**Problem**: TOC entry doesn't match actual section title
**Solution**: Update TOC when changing section titles:
```markdown
| [10](#教程-10-标准库参考) | 200 个内置函数... |
```

## Example: Adding 10 New Functions

### Before
```
Total: 190 functions
String: 26 functions
```

### After
```
Total: 200 functions
String: 36 functions
```

### Files to Update
1. `~/wiki/helen/tutorial/10-stdlib.md`:
   - Header: `> 200 个内置函数`
   - String section: Add 10 new functions with examples
   - Category table: Update String count to 36

2. `~/wiki/helen/toolchain/stdlib.md`:
   - Header: `**200 builtins**`
   - String table: Add 10 new rows
   - Category stats: Update String count

3. `~/wiki/helen/index.md`:
   - Entry: `185 builtins (core/string/...)`

## Verification Checklist

After updating documentation:

- [ ] Wiki tutorial header shows correct total count
- [ ] Wiki tutorial category table shows correct per-category counts
- [ ] Wiki technical docs header shows correct total count
- [ ] Wiki technical docs function tables are complete
- [ ] Wiki index entry shows correct total count
- [ ] Git repo committed and pushed
- [ ] All code examples are syntactically correct
- [ ] All function signatures match implementation

## Automation Opportunities

### Future: Count Validation
```python
# Validate function counts across all docs
def validate_counts():
    wiki_tutorial = count_functions("~/wiki/helen/tutorial/10-stdlib.md")
    wiki_tech = count_functions("~/wiki/helen/toolchain/stdlib.md")
    actual = count_registered_functions()
    
    assert wiki_tutorial == wiki_tech == actual
```

## Summary

Documentation workflow for Helen requires:
1. **Dual updates**: Wiki + git repo must stay in sync
2. **Multiple locations**: Tutorial, technical docs, index all need updates
3. **Consistent counts**: Function counts must match everywhere
4. **Manual sync**: Wiki is not git-tracked, requires explicit sync
5. **Verification**: Check all locations after updates

Following this workflow ensures documentation accuracy and consistency across all Helen documentation sources.
