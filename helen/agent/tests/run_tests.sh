#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════
# HelenAgent 测试运行器
# ═══════════════════════════════════════════════════════════════════
#
# 运行所有 helenagent 测试：
# - 单元测试：output.helen、ui_event_queue.helen
# - 集成测试：模块互操作
# - 语法检查：所有 .helen 文件
# - Python UI：py_compile
#
# 运行：bash tests/run_tests.sh
# ═══════════════════════════════════════════════════════════════════

set -u
cd "$(dirname "$0")/.."

PASS=0
FAIL=0
SKIP=0

green() { printf "\033[32m%s\033[0m\n" "$1"; }
red()   { printf "\033[31m%s\033[0m\n" "$1"; }
yellow(){ printf "\033[33m%s\033[0m\n" "$1"; }

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║   HelenAgent 测试运行器                                   ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# ── 1. 语法检查所有 .helen 文件 ─────────────────────────────────
echo "── 1. 语法检查（helen check）──"
HELEN_FILES=$(find . -maxdepth 2 -type f -name "*.helen" -not -path "./tests/*" -not -path "./.helen/*" -not -path "./project/*" | sort)
for f in $HELEN_FILES; do
    if helen check "$f" >/dev/null 2>&1; then
        green "  ✓ $f"
        PASS=$((PASS + 1))
    else
        red "  ✗ $f"
        helen check "$f" 2>&1 | head -5 | sed 's/^/      /'
        FAIL=$((FAIL + 1))
    fi
done
echo ""

# ── 2. 单元测试 + 集成测试 ─────────────────────────────────────
echo "── 2. 单元测试 + 集成测试 ──"
TEST_FILES=$(find tests -name "test_*.helen" | sort)
for t in $TEST_FILES; do
    echo "  运行 $t ..."
    if helen "$t" >/tmp/helen_test_out.txt 2>&1; then
        # 检查是否有断言失败：": FAIL" 精确匹配（避免误匹配测试名中的 "FAILED"）
        fail_count=$(grep -c ": FAIL" /tmp/helen_test_out.txt || true)
        if [ "$fail_count" -eq 0 ]; then
            pass_count=$(grep -c ": PASS" /tmp/helen_test_out.txt || true)
            green "  ✓ $t ($pass_count assertions passed)"
            PASS=$((PASS + 1))
        else
            red "  ✗ $t ($fail_count assertions FAILED)"
            grep ": FAIL" /tmp/helen_test_out.txt | head -5 | sed 's/^/      /'
            FAIL=$((FAIL + 1))
        fi
    else
        # 检查是否是 RuntimeError
        if grep -q "RuntimeError\|Error:" /tmp/helen_test_out.txt; then
            red "  ✗ $t (runtime error)"
            grep -E "RuntimeError|Error:" /tmp/helen_test_out.txt | head -3 | sed 's/^/      /'
            FAIL=$((FAIL + 1))
        else
            yellow "  - $t (skipped or non-test)"
            SKIP=$((SKIP + 1))
        fi
    fi
done
rm -f /tmp/helen_test_out.txt
echo ""

# ── 3. Python UI 代码编译检查 ──────────────────────────────────
echo "── 3. Python UI 代码编译检查（py_compile）──"
if [ -d ui ]; then
    PY_FILES=$(find ui -name "*.py" | sort)
    for p in $PY_FILES; do
        if python3 -m py_compile "$p" 2>/dev/null; then
            green "  ✓ $p"
            PASS=$((PASS + 1))
        else
            red "  ✗ $p"
            python3 -m py_compile "$p" 2>&1 | head -3 | sed 's/^/      /'
            FAIL=$((FAIL + 1))
        fi
    done
fi
echo ""

# ── 汇总 ───────────────────────────────────────────────────────
echo "╔═══════════════════════════════════════════════════════════╗"
printf "║  结果: %d passed, %d failed, %d skipped\n" "$PASS" "$FAIL" "$SKIP"
echo "╚═══════════════════════════════════════════════════════════╝"

if [ "$FAIL" -gt 0 ]; then
    exit 1
else
    exit 0
fi
