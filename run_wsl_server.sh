#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-8080}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Prefer native Linux Rust server compiled inside WSL if rustc is installed
if [ -x "$HOME/.cargo/bin/rustc" ]; then
    RUSTC_BIN="$HOME/.cargo/bin/rustc"
elif command -v rustc >/dev/null 2>&1; then
    RUSTC_BIN="$(command -v rustc)"
else
    RUSTC_BIN=""
fi

if [ -n "$RUSTC_BIN" ]; then
    BIN_PATH="/tmp/crimson_ledger_web_server"
    echo "[WSL Linux] Compiling native Linux Rust server -> $BIN_PATH ..."
    "$RUSTC_BIN" -O --edition 2021 "$SCRIPT_DIR/src/bin/web_server.rs" -o "$BIN_PATH"
    exec "$BIN_PATH" "$PORT" "$SCRIPT_DIR"
else
    echo "[WSL Linux] Launching Linux HTTP + WebSocket Co-op Relay Server (Python 3)..."
    exec python3 -u "$SCRIPT_DIR/scripts/wsl_server.py" "$PORT" "$SCRIPT_DIR"
fi
