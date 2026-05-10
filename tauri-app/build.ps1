param(
    [switch]$SkipFrontend,
    [switch]$SkipCopy,
    [string]$TargetDir = ""
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  CampusLogin Tauri v2.0.0 Build" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$scriptDir = $PSScriptRoot
$tauriAppDir = $scriptDir
$srcTauriDir = Join-Path $tauriAppDir "src-tauri"
$frontendDir = Join-Path $tauriAppDir "frontend"
$campusLoginDir = Split-Path -Parent $tauriAppDir
$releaseOutputDir = Join-Path $campusLoginDir "tauri-release"

Write-Host "Project layout:" -ForegroundColor White
Write-Host "  CampusLogin/           (root)" -ForegroundColor Gray
Write-Host "    archive/             (old Electron source)" -ForegroundColor Gray
Write-Host "    installer-build/     (Electron installer)" -ForegroundColor Gray
Write-Host "    release-build/       (Electron portable)" -ForegroundColor Gray
Write-Host "    renderer-app/        (Electron frontend)" -ForegroundColor Gray
Write-Host "    tauri-app/           (Tauri source)" -ForegroundColor Green
Write-Host "    tauri-release/       (Tauri output)" -ForegroundColor Green
Write-Host ""

Write-Host "[1/5] Checking environment..." -ForegroundColor Yellow

$rustcPath = Get-Command rustc -ErrorAction SilentlyContinue
if (-not $rustcPath) {
    Write-Host "ERROR: rustc not found. Install Rust: https://rustup.rs" -ForegroundColor Red
    exit 1
}
$rustcVersion = & rustc --version 2>&1
Write-Host "  rustc: $rustcVersion" -ForegroundColor Green

$nodePath = Get-Command node -ErrorAction SilentlyContinue
if (-not $nodePath) {
    Write-Host "ERROR: node not found. Install Node.js" -ForegroundColor Red
    exit 1
}
$nodeVersion = & node --version 2>&1
Write-Host "  node: $nodeVersion" -ForegroundColor Green

$cargoPath = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargoPath) {
    Write-Host "ERROR: cargo not found." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "[2/5] Installing dependencies..." -ForegroundColor Yellow

if (-not $SkipFrontend) {
    if (-not (Test-Path (Join-Path $frontendDir "node_modules"))) {
        Write-Host "  Installing frontend dependencies..." -ForegroundColor Gray
        Push-Location $frontendDir
        npm install
        Pop-Location
    } else {
        Write-Host "  Frontend node_modules exists, skipping." -ForegroundColor Green
    }

    if (-not (Test-Path (Join-Path $tauriAppDir "node_modules"))) {
        Write-Host "  Installing root dependencies..." -ForegroundColor Gray
        Push-Location $tauriAppDir
        npm install
        Pop-Location
    } else {
        Write-Host "  Root node_modules exists, skipping." -ForegroundColor Green
    }
} else {
    Write-Host "  Skipping frontend (--SkipFrontend)" -ForegroundColor Gray
}

Write-Host ""
Write-Host "[3/5] Building frontend..." -ForegroundColor Yellow

if (-not $SkipFrontend) {
    Push-Location $frontendDir
    npm run build
    Pop-Location
    Write-Host "  Frontend built successfully." -ForegroundColor Green
} else {
    if (Test-Path (Join-Path $frontendDir "dist\index.html")) {
        Write-Host "  Using existing frontend build." -ForegroundColor Green
    } else {
        Write-Host "  ERROR: No frontend build found. Remove --SkipFrontend or build manually." -ForegroundColor Red
        exit 1
    }
}

Write-Host ""
Write-Host "[4/5] Building Tauri application (release)..." -ForegroundColor Yellow

$cpuCount = [System.Environment]::ProcessorCount
$buildJobs = [Math]::Max(1, [Math]::Min($cpuCount, 8))
$env:CARGO_BUILD_JOBS = $buildJobs.ToString()
Write-Host "  Using $buildJobs build jobs (detected $cpuCount CPUs)" -ForegroundColor Gray

if ($TargetDir -ne "") {
    $env:CARGO_TARGET_DIR = $TargetDir
    Write-Host "  Using custom target dir: $TargetDir" -ForegroundColor Gray
} else {
    $shortTargetDir = "C:\cl-build\target"
    if (-not (Test-Path "C:\cl-build")) {
        New-Item -ItemType Directory -Path "C:\cl-build" -Force | Out-Null
    }
    $env:CARGO_TARGET_DIR = $shortTargetDir
    Write-Host "  Using short target dir to avoid path length issues: $shortTargetDir" -ForegroundColor Gray
}

Push-Location $tauriAppDir
npx tauri build
$buildResult = $LASTEXITCODE
Pop-Location

if ($buildResult -ne 0) {
    Write-Host "  Build failed with exit code $buildResult" -ForegroundColor Red
    Write-Host "  If path length error, try: .\build.ps1 -TargetDir 'C:\short\target'" -ForegroundColor Yellow
    exit $buildResult
}

Write-Host "  Tauri build succeeded." -ForegroundColor Green

if ($SkipCopy) {
    Write-Host ""
    Write-Host "Build complete (skip copy)." -ForegroundColor Cyan
    exit 0
}

Write-Host ""
Write-Host "[5/5] Copying release artifacts to tauri-release/..." -ForegroundColor Yellow

$actualTargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $srcTauriDir "target" }
$bundleDir = Join-Path $actualTargetDir "release\bundle"

if (-not (Test-Path $bundleDir)) {
    Write-Host "ERROR: Bundle directory not found at $bundleDir" -ForegroundColor Red
    exit 1
}

if (Test-Path $releaseOutputDir) {
    Remove-Item -Recurse -Force $releaseOutputDir
}
New-Item -ItemType Directory -Path $releaseOutputDir -Force | Out-Null

Get-ChildItem -Path $bundleDir -Recurse -File | ForEach-Object {
    $relativePath = $_.FullName.Substring($bundleDir.Length + 1)
    $destPath = Join-Path $releaseOutputDir $relativePath
    $destDir = Split-Path -Parent $destPath
    if (-not (Test-Path $destDir)) {
        New-Item -ItemType Directory -Path $destDir -Force | Out-Null
    }
    Copy-Item $_.FullName -Destination $destPath -Force
    Write-Host "  + $relativePath" -ForegroundColor Green
}

$exePath = Join-Path $actualTargetDir "release\campus-login.exe"
if (Test-Path $exePath) {
    Copy-Item $exePath -Destination $releaseOutputDir -Force
    Write-Host "  + campus-login.exe" -ForegroundColor Green
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Build complete!" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Output directory: $releaseOutputDir" -ForegroundColor White
Write-Host ""
Write-Host "Artifacts:" -ForegroundColor White
Get-ChildItem -Path $releaseOutputDir -Recurse -File | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  $($_.Name) ($size MB)" -ForegroundColor White
}
Write-Host ""
Write-Host "Directory structure (separated from Electron):" -ForegroundColor White
Write-Host "  CampusLogin/" -ForegroundColor Gray
Write-Host "    installer-build/   <- Electron NSIS installer" -ForegroundColor DarkGray
Write-Host "    release-build/     <- Electron portable build" -ForegroundColor DarkGray
Write-Host "    tauri-release/     <- Tauri release build (NEW)" -ForegroundColor Green
