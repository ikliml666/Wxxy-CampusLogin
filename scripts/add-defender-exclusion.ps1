# ============================================================
# Windows Defender 排除项脚本 - Wxxy-CampusLogin 项目
# ============================================================
# 用途：将 node_modules 和 Vite 预构建缓存目录加入 Defender 实时扫描排除
# 原因：Vite 预构建 2000+ 模块时，Defender 实时扫描会显著拖慢构建
# 使用：右键脚本 → "使用 PowerShell 运行(管理员)"
# ============================================================

$ErrorActionPreference = 'Stop'

# 检查管理员权限
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "[X] 需要管理员权限才能修改 Defender 排除项" -ForegroundColor Red
    Write-Host "    请右键此脚本 → '使用 PowerShell 运行(管理员)'" -ForegroundColor Yellow
    pause
    exit 1
}

# 项目路径
$projectRoot = "C:\Users\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\frontend"
$nodeModules = Join-Path $projectRoot "node_modules"
$viteCache   = Join-Path $nodeModules ".vite"
$distFolder  = Join-Path $projectRoot "dist"

# 检查路径是否存在
if (-not (Test-Path $nodeModules)) {
    Write-Host "[X] 路径不存在：$nodeModules" -ForegroundColor Red
    Write-Host "    请先在该目录运行一次 npm install" -ForegroundColor Yellow
    pause
    exit 1
}

# 获取当前排除项
$current = Get-MpPreference | Select-Object -ExpandProperty ExclusionPath
Write-Host "`n=== 当前 Defender 排除项 ===" -ForegroundColor Cyan
if ($current) { $current | ForEach-Object { Write-Host "  - $_" } } else { Write-Host "  (无)" -ForegroundColor Gray }

# 待添加的路径
$toAdd = @($nodeModules)
if (Test-Path $viteCache)   { $toAdd += $viteCache }
if (Test-Path $distFolder)  { $toAdd += $distFolder }

# 实际添加（去重）
$added = @()
$skipped = @()
foreach ($path in $toAdd) {
    if ($current -contains $path) {
        $skipped += $path
    } else {
        try {
            Add-MpPreference -ExclusionPath $path -ErrorAction Stop
            $added += $path
        } catch {
            Write-Host "[X] 添加失败：$path → $_" -ForegroundColor Red
        }
    }
}

# 输出结果
Write-Host "`n=== 添加结果 ===" -ForegroundColor Cyan
if ($added.Count -gt 0) {
    Write-Host "[OK] 新增排除项 ($($added.Count)):" -ForegroundColor Green
    $added | ForEach-Object { Write-Host "  + $_" }
}
if ($skipped.Count -gt 0) {
    Write-Host "[=] 已存在 ($($skipped.Count)):" -ForegroundColor Yellow
    $skipped | ForEach-Object { Write-Host "  = $_" }
}

# 验证
Write-Host "`n=== 验证当前排除项 ===" -ForegroundColor Cyan
(Get-MpPreference | Select-Object -ExpandProperty ExclusionPath) | ForEach-Object { Write-Host "  - $_" }

Write-Host "`n[完成] 现在 vite build 应该回到 3-5 秒" -ForegroundColor Green
Write-Host "       如果以后不再需要这些排除项，可以运行：" -ForegroundColor Gray
Write-Host "       Remove-MpPreference -ExclusionPath '$nodeModules'" -ForegroundColor Gray
pause
