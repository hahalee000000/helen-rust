#!/bin/bash
# v1.0 架构完整性验证脚本
# 用法: bash tests/verify_v4.3_integrity.sh

set -e

cd "$(dirname "$0")/.."

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

errors=0
warnings=0

pass() { echo -e "  ${GREEN}✓${NC} $1"; }
fail() { echo -e "  ${RED}✗${NC} $1"; errors=$((errors + 1)); }
warn() { echo -e "  ${YELLOW}⚠${NC} $1"; warnings=$((warnings + 1)); }

echo "===================================================="
echo " HelenAgent v1.0 架构完整性验证"
echo "===================================================="
echo ""

# ── 1. 核心 .helen 文件编译检查 ──
echo "1. 核心 .helen 文件编译检查"
for f in chat_session_actor.helen \
         task_manager.helen output.helen contracts/contracts.helen; do
    if [ ! -f "$f" ]; then
        fail "缺失文件: $f"
    elif result=$(helen check "$f" 2>&1); then
        if echo "$result" | grep -q "OK"; then
            pass "$f"
        else
            fail "$f 编译失败: $result"
        fi
    else
        fail "$f 编译失败"
    fi
done
echo ""

# ── 2. 已删除文件确认 ──
echo "2. 已删除文件确认"
for f in helen_programmer.helen skill_worker.helen \
         contractor.helen test_builder.helen implementer.helen \
         quality_gate.helen skill_evaluator.helen specialist_common.helen; do
    if [ -f "$f" ]; then
        fail "$f 应该已删除但仍存在"
    else
        pass "$f 已删除"
    fi
done
echo ""

# ── 3. Skills 文件检查 ──
echo "3. Skills 文件检查"
for skill_dir in \
    "architecture/helen-contractor-design" \
    "testing/helen-test-patterns" \
    "testing/helen-tdd-methodology" \
    "code-quality/helen-quality-rubrics" \
    "code-quality/helen-code-integrity"; do
    skill_file=".helen/skills/$skill_dir/SKILL.md"
    if [ ! -f "$skill_file" ]; then
        fail "缺失 skill 文件: $skill_file"
    else
        # 检查 YAML frontmatter
        if head -1 "$skill_file" | grep -q "^---"; then
            if grep -q "^name:" "$skill_file"; then
                name=$(grep "^name:" "$skill_file" | head -1 | sed 's/name: *//')
                pass "$skill_file (name: $name)"
            else
                warn "$skill_file 缺少 name 字段"
            fi
        else
            fail "$skill_file 缺少 YAML frontmatter"
        fi
    fi
done
echo ""

# ── 4. CHAT_TOOLS vs functions {} 一致性 ──
echo "4. CHAT_TOOLS vs chat_session_actor.helen functions {} 一致性检查"
# 提取 CHAT_TOOLS 中列出的工具名
tool_names=$(grep -A 30 "const CHAT_TOOLS" contracts/contracts.helen | \
             grep '"' | sed 's/.*"\([^"]*\)".*/\1/' | \
             grep -v "^exec_async$")  # exec_async 是内置工具

# 检查每个工具在 chat_session_actor.helen 中是否有对应函数
for tool in $tool_names; do
    # 内置工具不需要在 chat_session_actor.helen 中声明
    case "$tool" in
        read_file|write_file|patch_file|shell_exec|web_search|web_fetch|\
        calculate|path_exists|mkdir_p|list_dir|load_skill|exec_async|\
        glob_files|update_memory|update_user_preference)
            continue
            ;;
    esac
    if grep -q "fn $tool" chat_session_actor.helen; then
        pass "$tool 函数已声明"
    else
        fail "$tool 在 CHAT_TOOLS 中但 chat_session_actor.helen 未声明函数"
    fi
done
echo ""

# ── 5. 废弃引用检查 ──
echo "5. 废弃引用检查"
for term in "HelenProgrammer(" "SimpleImplementer(" "PROGRAMMER_TOOLS" "SIMPLE_TOOLS" \
            "Contractor(" "TestBuilder(" "Implementer(" "QualityGate(" "SkillEvalAgent("; do
    # 排除注释中的引用
    refs=$(grep -rn "$term" --include="*.helen" . 2>/dev/null | \
           grep -v "已删除\|deleted\|删除\|// " || true)
    if [ -n "$refs" ]; then
        fail "发现废弃引用 '$term':"
        echo "$refs" | sed 's/^/    /'
    else
        pass "无废弃引用 '$term'"
    fi
done
echo ""

# ── 6. .helen/MEMORY.md 统一上下文检查 ──
echo "6. .helen/MEMORY.md 统一上下文检查"
if [ ! -f ".helen/MEMORY.md" ]; then
    fail ".helen/MEMORY.md 不存在"
else
    for section in "HelenAgent Unified Context" "Project Boundaries" \
                   "Architecture" "Helen Language Reference"; do
        if grep -q "$section" .helen/MEMORY.md; then
            pass "包含章节: $section"
        else
            warn "缺失章节: $section"
        fi
    done
fi
echo ""

# ── 7. TODO/FIXME 检查（代码中）──
echo "7. 代码中 TODO/FIXME 检查"
todos=$(grep -rn "TODO\|FIXME\|HACK\|XXX" --include="*.helen" \
        chat_session_actor.helen contracts/contracts.helen 2>/dev/null | \
        grep -v '^[^:]*:.*".*TODO\|^[^:]*:.*".*FIXME\|^[^:]*:.*prompt' | \
        grep -v '^\s*//' || true)
if [ -n "$todos" ]; then
    warn "发现潜在 TODO/FIXME（可能是 prompt 内的正常引用）:"
    echo "$todos" | sed 's/^/    /'
else
    pass "代码中无 TODO/FIXME"
fi
echo ""

# ── 8. Skills 防御规则检查 ──
echo "8. Skills 防御规则检查"
# 层 1: Implementer self-check in helen-tdd-methodology skill
if grep -q "返回前自检\|返回前自检规则" .helen/skills/testing/helen-tdd-methodology/SKILL.md 2>/dev/null; then
    pass "层 1 — TDD 自检规则存在"
else
    warn "层 1 — helen-tdd-methodology 缺少自检规则"
fi
# 层 2: TestBuilder coverage in helen-test-patterns skill
if grep -q "覆盖率分析\|dead.code\|死代码" .helen/skills/testing/helen-test-patterns/SKILL.md 2>/dev/null; then
    pass "层 2 — 测试覆盖率规则存在"
else
    warn "层 2 — helen-test-patterns 缺少覆盖率规则"
fi
# 层 3: Code integrity skill exists
if [ -f ".helen/skills/code-quality/helen-code-integrity/SKILL.md" ]; then
    pass "层 3 — helen-code-integrity skill 文件存在"
else
    fail "层 3 — helen-code-integrity skill 文件不存在"
fi
echo ""

# ── 9. Hooks 机制检查 ──
echo "9. Hooks 机制检查"
for hook_fn in "save_code_file" "patch_code_file" "pre_exit_check"; do
    if grep -q "fn $hook_fn" chat_session_actor.helen; then
        pass "$hook_fn 函数存在"
    else
        fail "$hook_fn 函数缺失"
    fi
    if grep -q "\"$hook_fn\"" contracts/contracts.helen; then
        pass "$hook_fn 在 CHAT_TOOLS 中"
    else
        fail "$hook_fn 不在 CHAT_TOOLS 中"
    fi
done
echo ""

# ── 汇总 ──
echo "===================================================="
if [ $errors -eq 0 ]; then
    echo -e " ${GREEN}✓ 验证通过${NC} ($warnings warnings)"
else
    echo -e " ${RED}✗ 验证失败${NC}: $errors errors, $warnings warnings"
fi
echo "===================================================="

exit $errors
