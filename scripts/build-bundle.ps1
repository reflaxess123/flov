# One-shot release builder. Produces an NSIS installer .exe under
# target/release/bundle/nsis/ that ships:
#   - flov_app.exe (Tauri main app)
#   - flov-whisper-cpu.exe        (always — CPU fallback)
#   - flov-whisper-vulkan.exe     (always — works on most modern GPUs)
#   - flov-whisper-cuda.exe       (default — skip with -NoCuda)
#   - cublas64_*.dll, cublasLt64_*.dll  (with CUDA, ~300 MB extra)
#
# The Whisper model is NOT bundled — it's ~1.6 GB and the user picks
# which one in Settings → Models, which downloads on demand.
#
# Usage:
#   .\scripts\build-bundle.ps1                  # CPU + Vulkan + CUDA (full)
#   .\scripts\build-bundle.ps1 -NoCuda          # skip CUDA (smaller installer)
#   .\scripts\build-bundle.ps1 -SkipSidecars    # skip rebuilding sidecars
#
# Output: full path to the produced installer is printed at the end.

param(
    [switch]$NoCuda,
    [switch]$SkipSidecars
)
$IncludeCuda = -not $NoCuda

$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."
$cratesDir = Join-Path $root "crates"
$targetDir = Join-Path $root "target"
$binDir = Join-Path $root "src-tauri\binaries"
$runtimeDir = Join-Path $binDir "runtime"

# Tauri's externalBin convention: file must be named `<name>-<triple>.exe`,
# and gets renamed to `<name>.exe` at install time.
$triple = "x86_64-pc-windows-msvc"

function Build-Sidecar($name) {
    $crate = Join-Path $cratesDir "flov-whisper-$name"
    if (-not (Test-Path $crate)) {
        throw "missing crate: $crate"
    }
    Write-Host ">> building flov-whisper-$name (release)" -ForegroundColor Cyan
    cargo build --release --manifest-path "$crate\Cargo.toml" --target-dir $targetDir
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed for flov-whisper-$name" }
}

function Stage-Sidecar($name) {
    $src = Join-Path $targetDir "release\flov-whisper-$name.exe"
    if (-not (Test-Path $src)) { throw "expected sidecar not found: $src" }
    $dst = Join-Path $binDir "flov-whisper-$name-$triple.exe"
    Copy-Item $src $dst -Force
    Write-Host "   staged $dst" -ForegroundColor DarkGray
}

function Stage-CudaDlls {
    $cudaBin = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.2\bin\x64"
    if (-not (Test-Path $cudaBin)) {
        Write-Warning "CUDA bin dir not found at $cudaBin — cublas DLLs not staged"
        return
    }
    foreach ($dll in @("cublas64_13.dll", "cublasLt64_13.dll")) {
        $src = Join-Path $cudaBin $dll
        if (Test-Path $src) {
            Copy-Item $src (Join-Path $runtimeDir $dll) -Force
            Write-Host "   staged runtime\$dll" -ForegroundColor DarkGray
        } else {
            Write-Warning "missing: $src"
        }
    }
}

# Visual C++ Redistributable (2015-2022, x64). Required by ALL sidecars —
# they import MSVCP140.dll / VCRUNTIME140.dll. Modern Win11 usually has it
# (any C++ app installs it), but freshly imaged boxes do not, so we ship
# it and run silently in the NSIS post-install hook.
#
# Permanent Microsoft URL — always serves the latest patched build.
function Stage-VCRedist {
    $dst = Join-Path $runtimeDir "vc_redist.x64.exe"
    $url = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
    Write-Host ">> downloading vc_redist.x64.exe (latest)" -ForegroundColor Cyan
    try {
        Invoke-WebRequest -Uri $url -OutFile $dst -UseBasicParsing -ErrorAction Stop
        $sz = [math]::Round((Get-Item $dst).Length / 1MB, 1)
        Write-Host "   staged runtime\vc_redist.x64.exe ($sz MB)" -ForegroundColor DarkGray
    } catch {
        throw "vc_redist download failed: $_"
    }
}

# ── 1. Build sidecars ────────────────────────────────────────────────
if (-not $SkipSidecars) {
    Build-Sidecar "cpu"
    Build-Sidecar "vulkan"
    if ($IncludeCuda) { Build-Sidecar "cuda" }
}

# ── 2. Stage with Tauri's expected naming ────────────────────────────
if (-not (Test-Path $binDir)) { New-Item -ItemType Directory -Path $binDir | Out-Null }
if (-not (Test-Path $runtimeDir)) { New-Item -ItemType Directory -Path $runtimeDir | Out-Null }

# Clean previous staging so an aborted CUDA build doesn't smuggle stale
# sidecars into the bundle.
Get-ChildItem $binDir -Filter "*-$triple.exe" -ErrorAction SilentlyContinue | Remove-Item -Force
Get-ChildItem $runtimeDir -File -Exclude ".gitkeep" -ErrorAction SilentlyContinue | Remove-Item -Force

Stage-Sidecar "cpu"
Stage-Sidecar "vulkan"
if ($IncludeCuda) {
    Stage-Sidecar "cuda"
    Stage-CudaDlls
}
Stage-VCRedist

# Patch tauri.conf.json's externalBin only if CUDA is requested — Tauri
# fails the bundle if it lists a binary that isn't on disk. We use a
# scratch override file (`tauri.bundle.conf.json`) merged via -c to keep
# the source config clean.
$cfgOverride = Join-Path $root "src-tauri\tauri.bundle.conf.json"
if ($IncludeCuda) {
    $patch = @{
        bundle = @{
            externalBin = @(
                "binaries/flov-whisper-cpu",
                "binaries/flov-whisper-vulkan",
                "binaries/flov-whisper-cuda"
            )
        }
    }
    ($patch | ConvertTo-Json -Depth 10) | Out-File $cfgOverride -Encoding utf8 -Force
} elseif (Test-Path $cfgOverride) {
    Remove-Item $cfgOverride -Force
}

# ── 3. Build the bundle ──────────────────────────────────────────────
Write-Host ">> tauri build (NSIS installer)" -ForegroundColor Cyan
$tauri = Join-Path $root "ui\node_modules\.bin\tauri.cmd"
$args = @("build", "--bundles", "nsis")
if ($IncludeCuda) {
    $args += @("-c", $cfgOverride)
}

# Tauri CLI must run with the repo root as cwd so it picks up
# src-tauri/tauri.conf.json regardless of where this script was invoked.
Push-Location $root
try {
    & $tauri @args
    if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }
} finally {
    Pop-Location
}

# ── 4. Report installer location ─────────────────────────────────────
$nsisDir = Join-Path $targetDir "release\bundle\nsis"
$installer = Get-ChildItem $nsisDir -Filter "*.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($installer) {
    Write-Host "`ndone." -ForegroundColor Green
    Write-Host "Installer: $($installer.FullName)" -ForegroundColor Green
    Write-Host "Size: $([math]::Round($installer.Length / 1MB, 1)) MB" -ForegroundColor Green
} else {
    Write-Warning "Tauri reported success but no installer found in $nsisDir"
}
