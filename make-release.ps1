<#
.SYNOPSIS
    一键构建并汇总 GitHub Release 资产到本地 release/ 文件夹。

.DESCRIPTION
    包装 tauri-app/build.ps1（编译 + 产物收集），并补齐发布资产三件事：
      1. 安装包统一改名为 updater 硬编码要求的 Wxxy-CampusLogin_<版本>_x64-setup.exe
         （updater 按 "{github_exe_url}.sha256" 拼校验地址，文件名必须与上传资产一致）
      2. 生成 shasum 兼容的 .sha256 校验文件（"<hash>  <文件名>"，两空格无换行，
         与 build.ps1 第 5/5 步同款格式）
      3. 把 APK（android/build-apk.ps1 产物，如已构建）与 RELEASE_NOTES_v<版本>.md
         一并收进同一文件夹——该文件夹内容即可原样上传 GitHub Release

    用法：
      pwsh -File make-release.ps1                # 完整编译 + 汇总
      pwsh -File make-release.ps1 -SkipBuild     # 跳过编译，直接从已有产物汇总
      pwsh -File make-release.ps1 -SkipFrontend  # 跳过前端构建（透传 build.ps1）

    编译参数（-SkipFrontend / -TargetDir）原样透传 tauri-app/build.ps1。
#>
param(
    [switch]$SkipBuild,
    [switch]$SkipFrontend,
    [string]$TargetDir = ""
)

$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$releaseDir = Join-Path $root "release"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  CampusLogin make-release" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# [1/4] 版本号（权威源 tauri.conf.json，与 Cargo.toml 同值）
Write-Host "[1/4] Reading version from tauri.conf.json..." -ForegroundColor Yellow
$tauriConf = Get-Content (Join-Path $root "tauri-app\src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$version = $tauriConf.version
if (-not $version) { Write-Host "ERROR: tauri.conf.json 缺少 version 字段" -ForegroundColor Red; exit 1 }
Write-Host "  version: $version" -ForegroundColor Green

# [2/4] 编译（复用 tauri-app/build.ps1；同进程调用，其设置的 CARGO_TARGET_DIR 对本脚本可见）
if (-not $SkipBuild) {
    Write-Host ""
    Write-Host "[2/4] Building via tauri-app/build.ps1..." -ForegroundColor Yellow
    $buildParams = @{}
    if ($SkipFrontend) { $buildParams.SkipFrontend = $true }
    if ($TargetDir -ne "") { $buildParams.TargetDir = $TargetDir }
    & (Join-Path $root "tauri-app\build.ps1") @buildParams
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: build.ps1 失败（exit $LASTEXITCODE）" -ForegroundColor Red
        exit $LASTEXITCODE
    }
} else {
    Write-Host ""
    Write-Host "[2/4] Skipping build (-SkipBuild)" -ForegroundColor Yellow
}

# [3/4] 收集 Windows 安装包（改名）+ 生成 .sha256
Write-Host ""
Write-Host "[3/4] Collecting Windows installer..." -ForegroundColor Yellow

# 定位 bundle 目录：优先本进程的 CARGO_TARGET_DIR（刚编译过），否则按 build.ps1 的默认顺序
$targetCandidates = @()
if ($env:CARGO_TARGET_DIR) { $targetCandidates += $env:CARGO_TARGET_DIR }
$targetCandidates += "C:\cl-build\target"
$targetCandidates += (Join-Path $root "tauri-app\src-tauri\target")
$bundleDir = $null
foreach ($t in $targetCandidates) {
    $p = Join-Path $t "release\bundle"
    if (Test-Path $p) { $bundleDir = $p; break }
}
if (-not $bundleDir) {
    Write-Host "ERROR: 未找到 bundle 目录，请先编译（去掉 -SkipBuild）或检查 CARGO_TARGET_DIR" -ForegroundColor Red
    exit 1
}
Write-Host "  bundle dir: $bundleDir" -ForegroundColor Gray

# NSIS 安装包：产物名可能为 productName（如"校园网登录助手_..."）或已符合规范，取最新一个
$setupExe = Get-ChildItem -Path $bundleDir -Recurse -File -Filter "*-setup.exe" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setupExe) {
    Write-Host "ERROR: bundle 下未找到 *-setup.exe" -ForegroundColor Red
    exit 1
}

$assetName = "Wxxy-CampusLogin_${version}_x64-setup.exe"
New-Item -ItemType Directory -Path $releaseDir -Force | Out-Null
Copy-Item $setupExe.FullName -Destination (Join-Path $releaseDir $assetName) -Force
Write-Host "  + $assetName  (from $($setupExe.Name))" -ForegroundColor Green

# .sha256：应用内更新强制校验，文件名 = 资产名 + ".sha256"（updater 按 URL 拼接请求）
$hash = (Get-FileHash -Path (Join-Path $releaseDir $assetName) -Algorithm SHA256).Hash.ToLower()
Set-Content -Path (Join-Path $releaseDir "$assetName.sha256") -Value "$hash  $assetName" -NoNewline
Write-Host "  + $assetName.sha256" -ForegroundColor Green

# [4/4] 收集 APK 与 Release Notes
Write-Host ""
Write-Host "[4/4] Collecting APK / release notes..." -ForegroundColor Yellow

$apkBuildDir = Join-Path $root "android\gen\android\app\build\outputs\apk"
$apk = Get-ChildItem -Path $apkBuildDir -Recurse -File -Filter "Wxxy-CampusLogin_$version.apk" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $apk) {
    # 版本精确名没找到时，退到任意已构建 APK 并提示核对版本
    $apk = Get-ChildItem -Path $apkBuildDir -Recurse -File -Filter "Wxxy-CampusLogin_*.apk" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($apk) {
        Write-Host "  ! 未找到 Wxxy-CampusLogin_$version.apk，取最新已构建 APK：$($apk.Name)（请核对版本是否匹配）" -ForegroundColor Yellow
    }
}
if ($apk) {
    Copy-Item $apk.FullName -Destination (Join-Path $releaseDir $apk.Name) -Force
    Write-Host "  + $($apk.Name)" -ForegroundColor Green
} else {
    Write-Host "  - 未找到已构建 APK（如需发布请先跑 pwsh android/build-apk.ps1 后重跑本脚本 -SkipBuild）" -ForegroundColor Gray
}

$notesName = "RELEASE_NOTES_v$version.md"
$notesPath = Join-Path $root $notesName
if (Test-Path $notesPath) {
    Copy-Item $notesPath -Destination (Join-Path $releaseDir $notesName) -Force
    Write-Host "  + $notesName" -ForegroundColor Green
} else {
    Write-Host "  - 无 $notesName（可选：发布前从 CHANGELOG 整理一份放项目根，会自动收集）" -ForegroundColor Gray
}

# 汇总
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Release assets ready: $releaseDir" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Get-ChildItem $releaseDir -File | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  $($_.Name) ($size MB)" -ForegroundColor White
}
$ghAssets = (Get-ChildItem $releaseDir -File | Where-Object { $_.Name -notlike "RELEASE_NOTES_*" }).Name
Write-Host ""
Write-Host "发布（创建 Release 后上传资产，正文可用 $notesName）：" -ForegroundColor White
Write-Host "  gh release create v$version --title `"Wxxy-CampusLogin v$version`" --notes-file `"$releaseDir\$notesName`" release/$assetName `"$releaseDir\$assetName.sha256`"" -ForegroundColor DarkGray
Write-Host "  gh release upload v$version --clobber $($ghAssets | ForEach-Object { "release/$_" })" -ForegroundColor DarkGray
