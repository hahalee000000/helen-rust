#!/bin/bash
# Stop HR Agent (Helen Rust)
# Usage: ./scripts/stop-hr-agent.sh

PID_FILE="$HOME/helen-rust/logs/hr-agent.pid"

if [ ! -f "$PID_FILE" ]; then
    echo "No PID file found. Agent may not be running."
    echo "Try: pkill -f 'helen agent'"
    exit 0
fi

PID=$(cat "$PID_FILE")

if ps -p "$PID" > /dev/null 2>&1; then
    echo "Stopping HR Agent (PID: $PID)..."
    kill "$PID"
    sleep 1
    
    # Check if still running
    if ps -p "$PID" > /dev/null 2>&1; then
        echo "Force killing..."
        kill -9 "$PID"
    fi
    
    rm -f "$PID_FILE"
    echo "✅ HR Agent stopped"
else
    echo "Process $PID not running. Cleaning up PID file."
    rm -f "$PID_FILE"
fi
