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

Write-Host "`nDone! Launch the Linux server in WSL with:" -ForegroundColor Green
Write-Host "  wsl.exe bash /mnt/c/Users/etaip/.gemini/antigravity/scratch/vampire_noir_rpg/run_wsl_server.sh" -ForegroundColor Yellow
