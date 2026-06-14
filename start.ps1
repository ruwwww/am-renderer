# start.ps1 — One-click launcher for the am-renderer web editor
# Starts the Rust preview-service backend and Vite dev server simultaneously.
# Usage: .\start.ps1          (debug build, fast startup)
#        .\start.ps1 -Release  (release build, full optimisations)

param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"
$RootDir = $PSScriptRoot

# ── Colour helpers ──────────────────────────────────────────────────────────
function Write-Header($msg) {
    Write-Host "`n  $msg" -ForegroundColor Cyan
    Write-Host ("  " + ("─" * ($msg.Length))) -ForegroundColor DarkGray
}

function Write-Step($msg) {
    Write-Host "  → $msg" -ForegroundColor Gray
}

function Write-Ok($msg) {
    Write-Host "  ✓ $msg" -ForegroundColor Green
}

function Write-Warn($msg) {
    Write-Host "  ⚠ $msg" -ForegroundColor Yellow
}

# ── Pre-flight checks ───────────────────────────────────────────────────────
Write-Header "am-renderer Editor Launcher"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "  ✗ 'cargo' not found. Install Rust from https://rustup.rs/" -ForegroundColor Red
    exit 1
}

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    Write-Host "  ✗ 'npm' not found. Install Node.js from https://nodejs.org/" -ForegroundColor Red
    exit 1
}

# ── Install frontend deps if needed ─────────────────────────────────────────
$NodeModulesPath = Join-Path $RootDir "packages\web-editor\node_modules"
if (-not (Test-Path $NodeModulesPath)) {
    Write-Step "Installing frontend dependencies (first run)..."
    Push-Location (Join-Path $RootDir "packages\web-editor")
    npm install --silent
    Pop-Location
    Write-Ok "Frontend dependencies installed."
} else {
    Write-Ok "Frontend dependencies already present."
}

# ── Build mode ──────────────────────────────────────────────────────────────
if ($Release) {
    $CargoCmd = "cargo run --release -p preview-service"
    Write-Step "Starting backend in RELEASE mode (optimised, slower first compile)..."
} else {
    $CargoCmd = "cargo run -p preview-service"
    Write-Step "Starting backend in DEBUG mode (faster compile)..."
}

Write-Step "Frontend will be available at http://localhost:3000"
Write-Step "Backend API available at  http://localhost:8080"
Write-Host ""

# ── Launch processes ─────────────────────────────────────────────────────────
# Backend — Rust preview-service
$BackendJob = Start-Job -ScriptBlock {
    param($dir, $cmd)
    Set-Location $dir
    Invoke-Expression $cmd
} -ArgumentList $RootDir, $CargoCmd

# Give the backend a few seconds head-start before launching the frontend
Start-Sleep -Seconds 2

# Frontend — Vite dev server
$FrontendJob = Start-Job -ScriptBlock {
    param($dir)
    Set-Location (Join-Path $dir "packages\web-editor")
    npm run dev
} -ArgumentList $RootDir

Write-Ok "Both processes started. Press Ctrl+C to stop."
Write-Host ""

# ── Stream output from both jobs ─────────────────────────────────────────────
try {
    while ($true) {
        # Print backend output
        $backendOutput = Receive-Job -Job $BackendJob
        if ($backendOutput) {
            $backendOutput | ForEach-Object { Write-Host "  [backend] $_" -ForegroundColor Blue }
        }

        # Print frontend output
        $frontendOutput = Receive-Job -Job $FrontendJob
        if ($frontendOutput) {
            $frontendOutput | ForEach-Object { Write-Host "  [frontend] $_" -ForegroundColor Green }
        }

        # Check if either process died unexpectedly
        if ($BackendJob.State -eq "Failed") {
            Write-Warn "Backend process exited unexpectedly."
            break
        }
        if ($FrontendJob.State -eq "Failed") {
            Write-Warn "Frontend process exited unexpectedly."
            break
        }

        Start-Sleep -Milliseconds 300
    }
} finally {
    Write-Host "`n  Stopping all processes..." -ForegroundColor DarkGray
    Stop-Job -Job $BackendJob, $FrontendJob -ErrorAction SilentlyContinue
    Remove-Job -Job $BackendJob, $FrontendJob -Force -ErrorAction SilentlyContinue
    Write-Ok "All processes stopped."
}
