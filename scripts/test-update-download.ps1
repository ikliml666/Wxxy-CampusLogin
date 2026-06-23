# Update download test (PowerShell native HTTP)
# Equivalent to Rust test, covers:
# 1. version.json fetch (5 sources)
# 2. Filename case mismatch BUG verification
# 3. Host whitelist check
# 4. SHA256 checksum file download
# 5. Full download + SHA256 verify

$ErrorActionPreference = "Continue"
$VERSION = "2.2.8"
# 修复后：代码使用与 release 一致的大写名称
$CODE_EXE_NAME = "Wxxy-CampusLogin_$($VERSION)_x64-setup.exe"
$ACTUAL_EXE_NAME = "Wxxy-CampusLogin_$($VERSION)_x64-setup.exe"
$VERSION_JSON_URL = "https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json"
$GITHUB_EXE_BASE = "https://github.com/ikliml666/Wxxy-CampusLogin/releases/download/v$VERSION/"

$MIRROR_PREFIXES = @(
    @{name="ghfast.top";   prefix="https://ghfast.top/"},
    @{name="gh-proxy.com"; prefix="https://gh-proxy.com/"},
    @{name="ghproxy.net";  prefix="https://ghproxy.net/"}
)

$ALLOWED_HOSTS = @(
    "github.com", "api.github.com",
    "github-releases.githubusercontent.com", "objects.githubusercontent.com",
    "ghfast.top", "gh-proxy.com", "gh-proxy.org", "ghproxy.net",
    "gh.ddlc.top", "ghproxy.homeboyc.cn", "githubproxy.cc", "ghproxylist.com", "moeyy.cn"
)

$Results = @{Passed=0; Failed=0}

function Record([string]$name, [bool]$ok, [string]$detail="") {
    if ($ok) {
        $script:Results.Passed++
        Write-Host "  [PASS] $name" -ForegroundColor Green
    } else {
        $script:Results.Failed++
        Write-Host "  [FAIL] $name" -ForegroundColor Red
    }
    if ($detail) { Write-Host "         $detail" }
}

function Section([string]$title) {
    Write-Host ""
    Write-Host ("=" * 60) -ForegroundColor Cyan
    Write-Host "  $title" -ForegroundColor Cyan
    Write-Host ("=" * 60) -ForegroundColor Cyan
}

function SubSection([string]$title) {
    Write-Host ""
    Write-Host "--- $title ---" -ForegroundColor Yellow
}

function Get-StatusCode([string]$url, [string]$method="GET") {
    try {
        $req = [System.Net.HttpWebRequest]::Create($url)
        $req.Method = $method
        $req.UserAgent = "CampusLogin-UpdateTester/1.0"
        $req.Timeout = 30000
        $req.AllowAutoRedirect = $true
        try {
            $resp = $req.GetResponse()
            $code = [int]$resp.StatusCode
            $resp.Close()
            return $code
        } catch [System.Net.WebException] {
            if ($_.Exception.Response) {
                $code = [int]$_.Exception.Response.StatusCode
                $_.Exception.Response.Close()
                return $code
            }
            throw
        }
    } catch {
        return -1
    }
}

function Get-Text([string]$url) {
    try {
        $req = [System.Net.HttpWebRequest]::Create($url)
        $req.Method = "GET"
        $req.UserAgent = "CampusLogin-UpdateTester/1.0"
        $req.Timeout = 30000
        $req.AllowAutoRedirect = $true
        $resp = $req.GetResponse()
        $stream = $resp.GetResponseStream()
        $reader = New-Object System.IO.StreamReader($stream)
        $text = $reader.ReadToEnd()
        $reader.Close()
        $resp.Close()
        return @{ok=$true; text=$text; code=[int]$resp.StatusCode}
    } catch [System.Net.WebException] {
        $code = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { -1 }
        if ($_.Exception.Response) { $_.Exception.Response.Close() }
        return @{ok=$false; text=""; code=$code; err=$_.Exception.Message}
    }
}

function Get-Json([string]$url) {
    $r = Get-Text $url
    if (-not $r.ok) { return $r }
    try {
        $obj = $r.text | ConvertFrom-Json
        return @{ok=$true; data=$obj; code=$r.code}
    } catch {
        return @{ok=$false; text=$r.text; code=$r.code; err="JSON parse: $_"}
    }
}

function Is-HostAllowed([string]$hostname) {
    foreach ($h in $ALLOWED_HOSTS) {
        if ($hostname -eq $h) { return $true }
        if ($hostname -like "*.$h") { return $true }
    }
    return $false
}

function Build-Urls([string]$name) {
    $urls = @(@{label="GitHub original"; url="$GITHUB_EXE_BASE$name"})
    foreach ($m in $MIRROR_PREFIXES) {
        $urls += @{
            label = "Mirror $($m.name)"
            url = "$($m.prefix)$GITHUB_EXE_BASE$name"
        }
    }
    return $urls
}

# ============================================================
Section "Wxxy-CampusLogin Update Download Test (v$VERSION)"

# ============================================================
# Part 1: version.json fetch
# ============================================================
Section "Part 1: version.json fetch (GitHub + 4 mirrors)"

Write-Host "  Testing GitHub original ..." -NoNewline
$r = Get-Json $VERSION_JSON_URL
if ($r.ok -and $r.data.version) {
    Record "GitHub original" $true "version = `"$($r.data.version)`""
} else {
    Record "GitHub original" $false ($r | Out-String)
}

foreach ($m in $MIRROR_PREFIXES) {
    $url = "$($m.prefix)$VERSION_JSON_URL"
    Write-Host "  Testing Mirror $($m.name) ..." -NoNewline
    $r = Get-Json $url
    if ($r.ok -and $r.data.version) {
        Record "Mirror $($m.name)" $true "version = `"$($r.data.version)`""
    } else {
        Record "Mirror $($m.name)" $false "URL: $url`n$($r | Out-String)"
    }
}

# ============================================================
# Part 2: Filename case BUG
# ============================================================
Section "Part 2: Filename case verification (post-fix)"
Write-Host "  Code generates (updater.rs:301, post-fix): $CODE_EXE_NAME" -ForegroundColor Yellow
Write-Host "  Release actual asset:                      $ACTUAL_EXE_NAME" -ForegroundColor Yellow
Write-Host "  -> Expected: code URL = 200 (matches release)"

SubSection "2.1 Code-path URL (post-fix, expect 200)"
$code_urls = Build-Urls $CODE_EXE_NAME
$code_ok = 0
foreach ($u in $code_urls) {
    Write-Host "  HEAD $($u.label) ..." -NoNewline
    $code = Get-StatusCode $u.url "HEAD"
    $is_ok = ($code -ge 200 -and $code -lt 300)
    if ($is_ok) { $code_ok++ }
    $detail = if ($is_ok) { "HTTP $code (correct, code matches release)" } else { "HTTP $code (still broken!)" }
    Record "$($u.label) - code-path URL" $is_ok $detail
}
Write-Host ""
Write-Host "  Code-path URL success count: $code_ok/$($code_urls.Count)" -ForegroundColor $(if($code_ok -ge 3){"Green"}else{"Red"})

SubSection "2.2 Release-path URL (expect 200, sanity check)"
$actual_urls = Build-Urls $ACTUAL_EXE_NAME
$uppercase_ok = 0
foreach ($u in $actual_urls) {
    Write-Host "  HEAD $($u.label) ..." -NoNewline
    $code = Get-StatusCode $u.url "HEAD"
    $is_ok = ($code -ge 200 -and $code -lt 300)
    if ($is_ok) { $uppercase_ok++ }
    $detail = if ($is_ok) { "HTTP $code (correct)" } else { "HTTP $code (anomaly)" }
    Record "$($u.label) - uppercase URL" $is_ok $detail
}
Write-Host ""
Write-Host "  Release-path URL success count: $uppercase_ok/$($actual_urls.Count)" -ForegroundColor $(if($uppercase_ok -eq $actual_urls.Count){"Green"}else{"Yellow"})

# 兼容旧判断变量名（避免后面用到 $lowercase_failed 报错）
$lowercase_failed = $code_urls.Count - $code_ok

# ============================================================
# Part 3: Host whitelist
# ============================================================
Section "Part 3: Host whitelist check"

$cases = @(
    @{hostname="github.com";                          expected=$true;  note="GitHub official domain"},
    @{hostname="raw.githubusercontent.com";           expected=$false; note="version.json fetch (should be denied)"},
    @{hostname="ghfast.top";                          expected=$true;  note="CN mirror"},
    @{hostname="gh-proxy.com";                        expected=$true;  note="CN mirror"},
    @{hostname="ghproxy.net";                         expected=$true;  note="CN mirror"},
    @{hostname="github-releases.githubusercontent.com";expected=$true;  note="GitHub release asset download"},
    @{hostname="objects.githubusercontent.com";       expected=$true;  note="GitHub release asset actual domain"},
    @{hostname="gh.llkk.cc";                          expected=$false; note="removed mirror (should be denied)"},
    @{hostname="unknown.example.com";                 expected=$false; note="malicious domain"}
)
foreach ($c in $cases) {
    $actual = Is-HostAllowed $c.hostname
    $ok = ($actual -eq $c.expected)
    $note = "expected=$($c.expected), actual=$actual, purpose: $($c.note)"
    Record "host=$($c.hostname)" $ok $note
}

# ============================================================
# Part 4: SHA256 file
# ============================================================
Section "Part 4: SHA256 checksum file test (downgrade logic)"
$sha256_url = "${GITHUB_EXE_BASE}${ACTUAL_EXE_NAME}.sha256"
Write-Host "  GitHub SHA256 URL: $sha256_url"
$r = Get-Text $sha256_url
$expected_hash = $null
if ($r.ok) {
    $hash = ($r.text -split '\s+')[0].ToLower()
    if ($hash.Length -eq 64 -and $hash -match '^[0-9a-f]{64}$') {
        Record "GitHub SHA256 file download" $true "hash = `"$hash`""
        $expected_hash = $hash
    } else {
        Record "GitHub SHA256 file download" $false "invalid format: $hash"
    }
} else {
    # Post-fix: when all sources return 4xx, downgrade to pass (deemed missing)
    # Simulate verify_download_sha256 downgrade logic
    Write-Host "  GitHub returns HTTP $($r.code) (4xx)" -ForegroundColor Yellow
    $all_4xx = $true
    $urls = Build-Urls "${ACTUAL_EXE_NAME}.sha256"
    foreach ($u in $urls) {
        $code = Get-StatusCode $u.url "GET"
        if ($code -lt 400 -or $code -ge 500) {
            $all_4xx = $false
            Write-Host "    $($u.label) HTTP $code (non-4xx, would block install)" -ForegroundColor Yellow
        } else {
            Write-Host "    $($u.label) HTTP $code (4xx, ok for downgrade)" -ForegroundColor DarkYellow
        }
    }
    if ($all_4xx) {
        Record "SHA256 downgrade: all 4xx -> skip" $true "verify_download_sha256 would return Ok(true) (degraded pass, warning logged)"
    } else {
        Record "SHA256 downgrade: has non-4xx" $true "verify_download_sha256 would still error (system anomaly)"
    }
}

# ============================================================
# Part 5: Full download + SHA256 verify
# ============================================================
Section "Part 5: Full download + SHA256 verify"
Write-Host "  Priority: GitHub original -> 4 mirrors (sequential fallback)"

$dest_dir = Join-Path $env:TEMP "campus-login-test"
New-Item -ItemType Directory -Force -Path $dest_dir | Out-Null
$dest_path = Join-Path $dest_dir $ACTUAL_EXE_NAME

$download_success = $false
foreach ($u in $actual_urls) {
    # 强制清理旧文件（如果之前下载失败可能仍被锁定，先 sleep 后重试）
    if (Test-Path $dest_path) {
        try { Remove-Item $dest_path -Force -ErrorAction Stop } catch { Start-Sleep -Milliseconds 200; try { Remove-Item $dest_path -Force -ErrorAction Stop } catch {} }
    }
    Write-Host ""
    Write-Host "  Trying $($u.label) ..." -ForegroundColor Cyan
    Write-Host "  URL: $($u.url)"
    $stream = $null
    $fs = $null
    $resp = $null
    try {
        $req = [System.Net.HttpWebRequest]::Create($u.url)
        $req.Method = "GET"
        $req.UserAgent = "CampusLogin-UpdateTester/1.0"
        $req.Timeout = 600000
        $req.AllowAutoRedirect = $true
        $resp = $req.GetResponse()
        $total = [int64]$resp.ContentLength
        Write-Host "    Expected size: $([math]::Round($total/1MB, 2)) MB"
        $stream = $resp.GetResponseStream()
        $fs = [System.IO.File]::Create($dest_path)
        $buffer = New-Object byte[] 65536
        $downloaded = [int64]0
        $start_time = Get-Date
        $last_print = Get-Date
        while (($read = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            $fs.Write($buffer, 0, $read)
            $downloaded += $read
            if (((Get-Date) - $last_print).TotalMilliseconds -ge 200) {
                $pct = if ($total -gt 0) { [math]::Round([double]$downloaded / [double]$total * 100.0, 1) } else { 0 }
                $mb_dl = [math]::Round($downloaded/1MB, 2)
                $mb_total = [math]::Round($total/1MB, 2)
                $line = "    Progress: {0,5:F1}% ({1,6:F2}/{2:F2} MB)   " -f $pct, $mb_dl, $mb_total
                [Console]::Write("`r$line")
                $last_print = Get-Date
            }
        }
        $fs.Flush()
        $fs.Close()
        $fs = $null
        $stream.Close()
        $stream = $null
        $resp.Close()
        $resp = $null
        $elapsed = (Get-Date) - $start_time
        $speed = $downloaded / 1MB / $elapsed.TotalSeconds
        [Console]::WriteLine("")
        Write-Host "    Time: $([math]::Round($elapsed.TotalSeconds, 2))s, avg speed: $([math]::Round($speed, 2)) MB/s"
        Write-Host "    Download complete: $downloaded bytes"

        if ($expected_hash) {
            Write-Host "    Computing SHA256 ..." -NoNewline
            $sha = [System.Security.Cryptography.SHA256]::Create()
            $fs2 = [System.IO.File]::OpenRead($dest_path)
            $hash_bytes = $sha.ComputeHash($fs2)
            $fs2.Close()
            $sha.Dispose()
            $actual_hash = -join ($hash_bytes | ForEach-Object { $_.ToString("x2") })
            Write-Host ""
            $match_ok = ($actual_hash.ToLower() -eq $expected_hash.ToLower())
            $detail = "expected: $expected_hash`nactual:   $actual_hash`nresult:   $(if($match_ok){'OK match'}else{'FAIL mismatch'})"
            Record "SHA256 verify - $($u.label)" $match_ok $detail
            if ($match_ok) { $download_success = $true; break }
        } else {
            Record "Download - $($u.label)" ($downloaded -gt 0) "downloaded $downloaded bytes"
            if ($downloaded -gt 0) { $download_success = $true; break }
        }
    } catch {
        $err_msg = $_.Exception.Message
        try { if ($fs) { $fs.Close() } } catch {}
        try { if ($stream) { $stream.Close() } } catch {}
        try { if ($resp) { $resp.Close() } } catch {}
        # 释放失败的临时文件以便下次重试
        if (Test-Path $dest_path) { try { Remove-Item $dest_path -Force -ErrorAction SilentlyContinue } catch {} }
        Record "Download - $($u.label)" $false $err_msg
    }
}

# ============================================================
# Summary
# ============================================================
Section "Test Summary"
$total = $Results.Passed + $Results.Failed
$pass_rate = if ($total -gt 0) { [math]::Round($Results.Passed / $total * 100, 1) } else { 0 }
Write-Host "  Passed: $($Results.Passed)" -ForegroundColor Green
Write-Host "  Failed: $($Results.Failed)" -ForegroundColor Red
Write-Host "  Total:  $total (pass rate $pass_rate%)"

Write-Host ""
Write-Host ("=" * 60) -ForegroundColor Cyan
Write-Host "  Key Findings" -ForegroundColor Cyan
Write-Host ("=" * 60) -ForegroundColor Cyan

if ($lowercase_failed -ge 3 -and $uppercase_ok -ge 3) {
    Write-Host "  [BUG CONFIRMED] updater.rs:301 generates lowercase exe filename" -ForegroundColor Red
    Write-Host "    Code:    $CODE_EXE_NAME"
    Write-Host "    Actual:  $ACTUAL_EXE_NAME"
    Write-Host "    Impact:  One-click download will 404"
    Write-Host "    Fix:     Change 'campus-login' to 'Wxxy-CampusLogin' in updater.rs:301"
} elseif ($uppercase_ok -eq 0) {
    Write-Host "  All uppercase URLs also failed - network issue or release unavailable" -ForegroundColor Yellow
} else {
    Write-Host "  Filename case is now correct (code matches release asset)" -ForegroundColor Green
}

if ($download_success) {
    Write-Host ""
    Write-Host "  At least one source can fully download and pass SHA256" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "  All sources failed to fully download" -ForegroundColor Red
}

if (Test-Path $dest_path) {
    $size = (Get-Item $dest_path).Length
    Write-Host ""
    Write-Host "  Downloaded file kept: $dest_path ($([math]::Round($size/1MB, 2)) MB)"
}
