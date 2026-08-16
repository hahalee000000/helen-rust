#!/bin/bash

# Helen Web UI - 停止所有服务
# 杀掉占用 8000 和 5173 端口的所有进程

echo "🛑 Stopping Helen Web UI services..."

# 需要清理的端口
PORTS=(8000 5173 5174)
KILLED=0

for port in "${PORTS[@]}"; do
    # 查找占用端口的进程
    pids=$(lsof -ti :$port 2>/dev/null || true)

    if [ -n "$pids" ]; then
        for pid in $pids; do
            # 获取进程信息
            cmd=$(ps -p $pid -o comm= 2>/dev/null || echo "unknown")
            echo "🔄 Stopping process on port $port (PID: $pid, $cmd)..."

            # 先尝试优雅终止
            kill $pid 2>/dev/null || true
        done

        # 等待进程退出
        sleep 2

        # 如果还在运行，强制 kill
        for pid in $pids; do
            if kill -0 $pid 2>/dev/null; then
                echo "⚠️  Force killing PID $pid..."
                kill -9 $pid 2>/dev/null || true
            fi
        done

        KILLED=$((KILLED + 1))
    fi
done

# 同时杀掉可能残留的 uvicorn 和 vite 进程
pkill -f "uvicorn app.main:app" 2>/dev/null && echo "🔄 Killed uvicorn processes" || true
pkill -f "vite" 2>/dev/null && echo "🔄 Killed vite processes" || true

sleep 1

# 验证端口已释放
echo ""
echo "📋 Port status:"
for port in "${PORTS[@]}"; do
    if lsof -ti :$port >/dev/null 2>&1; then
        echo "   ❌ Port $port still in use"
    else
        echo "   ✅ Port $port is free"
    fi
done

if [ $KILLED -eq 0 ]; then
    echo ""
    echo "ℹ️  No services were running"
else
    echo ""
    echo "✅ All services stopped"
fi