#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/crimson-ledger-target}"

mkdir -p "$PROJECT_ROOT/web/pkg" "$CARGO_TARGET_DIR"

echo "==> Compiling Crimson Ledger to WebAssembly (native Linux toolchain)..."
(
  cd "$PROJECT_ROOT"
  cargo build --release --target wasm32-unknown-unknown --bin vampire_noir_rpg
)

echo "==> Generating browser JS/WASM bindings in web/pkg/..."
wasm-bindgen \
  --out-dir "$PROJECT_ROOT/web/pkg" \
  --target web \
  --no-typescript \
  --remove-name-section \
  --remove-producers-section \
  "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/vampire_noir_rpg.wasm"

echo "==> Pre-compressing WASM & JS with Gzip..."
gzip -9 -k -f "$PROJECT_ROOT/web/pkg/vampire_noir_rpg_bg.wasm" "$PROJECT_ROOT/web/pkg/vampire_noir_rpg.js"

echo "Done! Launch the local Linux server with: ./run_wsl_server.sh"
