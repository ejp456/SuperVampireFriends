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
    --remove-name-section `
    --remove-producers-section `
    (Join-Path $ProjectRoot "target\wasm32-unknown-unknown\release\vampire_noir_rpg.wasm")

Write-Host "==> Pre-compressing WASM & JS with Gzip for fast HTTP delivery..." -ForegroundColor Cyan
foreach ($rel in @("web\pkg\vampire_noir_rpg_bg.wasm", "web\pkg\vampire_noir_rpg.js")) {
    $srcPath = Join-Path $ProjectRoot $rel
    $gzPath = "$srcPath.gz"
    $inStream = [System.IO.File]::OpenRead($srcPath)
    try {
        $outStream = [System.IO.File]::Create($gzPath)
        try {
            $gzip = New-Object System.IO.Compression.GZipStream($outStream, [System.IO.Compression.CompressionLevel]::Optimal)
            try {
                $inStream.CopyTo($gzip)
            } finally {
                $gzip.Dispose()
            }
        } finally {
            $outStream.Dispose()
        }
    } finally {
        $inStream.Dispose()
    }
}

Write-Host "`nDone! Launch the Linux server in WSL with:" -ForegroundColor Green
Write-Host "  wsl.exe bash /mnt/c/Users/etaip/.gemini/antigravity/scratch/vampire_noir_rpg/run_wsl_server.sh" -ForegroundColor Yellow
