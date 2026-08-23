#!/bin/bash
# Start HR Agent (Helen Rust) using local development build
# Usage: ./scripts/start-hr-agent.sh [port]

set -e

# Configuration
PORT=${1:-8001}
HELEN_BIN="$HOME/helen-rust/target/release/helen"
LOG_FILE="$HOME/helen-rust/logs/hr-agent.log"
PID_FILE="$HOME/helen-rust/logs/hr-agent.pid"

# Ensure log directory exists
mkdir -p "$(dirname "$LOG_FILE")"

# Check if binary exists
if [ ! -f "$HELEN_BIN" ]; then
    echo "Error: Helen binary not found at $HELEN_BIN"
    echo "Run 'cargo build --release' first"
    exit 1
fi

# Check if already running
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE")
    if ps -p "$OLD_PID" > /dev/null 2>&1; then
        echo "HR Agent already running (PID: $OLD_PID)"
        echo "Stop it first with: kill $OLD_PID"
        exit 1
    else
        # Stale PID file
        rm -f "$PID_FILE"
    fi
fi

# Check if port is in use
if ss -tlnp | grep -q ":$PORT "; then
    echo "Warning: Port $PORT is already in use"
    echo "Kill existing process with: fuser -k $PORT/tcp"
    exit 1
fi

# Start the agent
echo "Starting HR Agent on port $PORT..."
echo "Binary: $HELEN_BIN"
echo "Log: $LOG_FILE"

nohup "$HELEN_BIN" agent --port "$PORT" > "$LOG_FILE" 2>&1 &
AGENT_PID=$!

# Save PID
echo "$AGENT_PID" > "$PID_FILE"

# Wait a moment and check if it's running
sleep 2
if ps -p "$AGENT_PID" > /dev/null 2>&1; then
    echo "✅ HR Agent started successfully"
    echo "   PID: $AGENT_PID"
    echo "   Port: $PORT"
    echo "   URL: http://127.0.0.1:$PORT"
    echo ""
    echo "View logs: tail -f $LOG_FILE"
    echo "Stop agent: kill $AGENT_PID"
else
    echo "❌ Failed to start HR Agent"
    echo "Check logs: cat $LOG_FILE"
    rm -f "$PID_FILE"
    exit 1
fi
