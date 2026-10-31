$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot

Write-Host "==> Compiling Crimson Ledger to WebAssembly (wasm32-unknown-unknown)..." -ForegroundColor Cyan
cargo build --release --target wasm32-unknown-unknown --bin vampire_noir_rpg

$WasmBindgen = Join-Path $ProjectRoot "wasm-bindgen-0.2.130-x86_64-pc-windows-msvc\wasm-bindgen.exe"
if (-not (Test-Path $WasmBindgen)) {
    $WasmBindgen = "wasm-bindgen"
}

Write-Host "==> Generating browser JS/WASM bindings in web/pkg/..." -ForegroundColor Cyan
& $WasmBindgen `
    --out-dir (Join-Path $ProjectRoot "web\pkg") `
    --target web `
    --no-typescript `
    (Join-Path $ProjectRoot "target\wasm32-unknown-unknown\release\vampire_noir_rpg.wasm")

Write-Host "==> Building local HTTP + WebSocket Co-op Server..." -ForegroundColor Cyan
cargo build --release --bin web_server

Write-Host "`nDone! Run .\target\release\web_server.exe and open http://127.0.0.1:8080 in your browser." -ForegroundColor Green
