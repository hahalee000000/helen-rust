#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════
# HelenAgent 性能基准测试 - 进程级测量
# ═══════════════════════════════════════════════════════════════════
#
# 测量：
# - 各模块 helen check 启动时间（解释器冷启动 + 语法检查）
# - 全量 check 总耗时
# - 进程峰值内存
#
# 运行：bash tests/benchmark.sh
# ═══════════════════════════════════════════════════════════════════

set -e
cd "$(dirname "$0")/.."

MODULES=(
    chat_session_actor.helen
    chat_tui.helen
    task_manager.helen
    output.helen
    context.helen
    memory_utils.helen
    system_reminders.helen
    ui_bridge.helen
    ui_event_queue.helen
    commands.helen
)

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║   HelenAgent 进程级基准测试                               ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# ── 1. 各模块 helen check 耗时 ──────────────────────────────────
echo "── 1. 各模块 helen check 耗时（冷启动 + 语法检查）──"
printf "%-28s %10s %10s\n" "模块" "耗时(ms)" "峰值内存(KB)"
printf "%-28s %10s %10s\n" "----" "--------" "------------"
total_ms=0
for mod in "${MODULES[@]}"; do
    # 用 /usr/bin/time 测量耗时和内存（输出到 stderr）
    # 跑 3 次取中位数，减少抖动
    times=()
    for run in 1 2 3; do
        start=$(date +%s.%N)
        helen check "$mod" >/dev/null 2>&1 || true
        end=$(date +%s.%N)
        ms=$(awk "BEGIN{print ($end - $start) * 1000}")
        times+=("$ms")
    done
    # 取中位数（3 个值排序取第 2 个）
    sorted=($(printf '%s\n' "${times[@]}" | sort -n))
    median=${sorted[1]}
    total_ms=$(awk "BEGIN{print $total_ms + $median}")
    printf "%-28s %10.1f\n" "$mod" "$median"
done
printf "%-28s %10.1f\n" "总计" "$total_ms"
echo ""

# ── 2. 进程峰值内存（单次 check）────────────────────────────────
echo "── 2. 进程峰值内存（helen check chat_session_actor.helen）──"
if command -v /usr/bin/time >/dev/null 2>&1; then
    /usr/bin/time -v helen check chat_session_actor.helen >/dev/null 2>"tests/.bench_mem.txt" || true
    peak=$(grep "Maximum resident set size" tests/.bench_mem.txt | awk '{print $NF}')
    echo "chat_session_actor.helen 峰值 RSS: ${peak} KB"
    rm -f tests/.bench_mem.txt
else
    echo "（/usr/bin/time 不可用，跳过内存测量）"
fi
echo ""

# ── 3. in-process 基准（调用 benchmark.helen）──────────────────
echo "── 3. in-process 基准（tests/benchmark.helen）──"
if [ -f tests/benchmark.helen ]; then
    timeout 150 helen tests/benchmark.helen 2>&1 | grep -E "ops/s|ms |入队|跨模块" || true
else
    echo "（tests/benchmark.helen 不存在）"
fi
echo ""

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║   基准测试完成                                            ║"
echo "╚═══════════════════════════════════════════════════════════╝"
