---
title: 更新模块：自动检查、下载、SHA256校验与安装
type: module
source_files:
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/commands/updater.rs
tags: [更新, 校验和, 退避重试, 镜像源, Tauri命令]
---

## Overview
本模块负责 Wxxy-CampusLogin 应用的自动更新全流程，涵盖版本检查、多源降级、安装包下载、SHA256 完整性校验及安装触发。核心设计原则包括：严格的 SHA256 校验（默认拒绝未校验安装）、失败退避重试机制、以及审计驱动的并发安全控制（A6, A13）。

## Key Components

### `tauri-app/src-tauri/src/update/updater.rs`

#### 常量与全局状态
- `GITHUB_REPO`: `"ikliml666/Wxxy-CampusLogin"` (L10)
- `AUTO_CHECK_INTERVAL_SECS`: `86400` (24小时) (L11)
- `BACKOFF_RETRY_SECS`: `[300, 900, 3600]` (5min, 15min, 1h) (L14)
- `STARTUP_CHECK_DELAY_SECS`: `5` (L17)
- `LAST_CHECK_ERROR`: `Mutex<Option<String>>` (L20)，记录最近一次检查失败原因，供前端展示。
- `LAST_CHECK_TIME_MS`: `AtomicU64` (L24)，记录最近一次检查完成时间。
- `VERSION_FILE_URL`: GitHub Raw URL (L27)
- `VERSION_MIRRORS`: 三个代理镜像源列表 (L30-L34)
- `UPDATE_TEMP_DIR`: `"campus-login-update"` (L165)
- `UPDATE_STALE_SECS`: `86400` (24小时) (L168)
- `UPDATE_ASSET_EXTS`: `["exe", "msi", "sha256"]` (L171)

#### 数据结构
- `ReleaseAsset` (L37): 发布资产信息 (`name`, `url`, `size`)。
- `UpdateInfo` (L42): 更新检查结果。
    - `has_update`: bool
    - `latest_version`: String
    - `release_notes`: String
    - `assets`: Vec<ReleaseAsset>
    - `sha256_checksum`: Option<String> (JSON 数组字符串，包含所有校验源 URL)
    - `last_check_error`: Option<String> (上次失败原因快照)
    - `last_check_time`: Option<u64>
- `DownloadProgress` (L58): 下载进度 (`downloaded`, `total`, `speed`, `percent`)。

#### 核心函数
- `compare_versions(current: &str, latest: &str) -> bool` (L63-L80):
    - 语义化版本比较。去除 'v' 前缀，按 '.' 分割提取数字段。
    - 支持不等长比较（短版本高位补 0），解决 hotfix 版本（如 2.3.0.1 vs 2.3.0）误判问题。
- `build_short_timeout_http_client() -> Result<&'static reqwest::Client>` (L88-L100):
    - 构建超时 10s 的 HTTP 客户端，模块级单例缓存。
- `decide_checksum_missing(all_client_errors: bool, had_transport_error: bool, allow_skip: bool) -> Result<(), String>` (L108-L124):
    - **决策逻辑**：
        - 若全为 4xx 且无传输错误：`allow_skip=true` 则放行（记录 warn），否则报错。
        - 若有 5xx 或传输错误：一律拒绝。
- `extract_checksum(url: &str, text: &str) -> Option<String>` (L129-L160):
    - 从 GitHub API JSON 或 .sha256 文本中提取 64 位小写 SHA256。
    - 支持 shasum 风格 (`<hash> <file>`) 和 BSD 风格 (`SHA256 (file) = <hash>`)。
- `verify_download_sha256(file_path: &str, checksum_urls: &[String], allow_skip_missing: bool) -> Result<bool, String>` (L165-L236):
    - **流程**：
        1. 遍历 `checksum_urls` 获取期望哈希。
        2. 若所有源失败，调用 `decide_checksum_missing` 决定是否跳过。
        3. 若获得期望哈希，分块计算本地文件 SHA256（64KB buffer，`spawn_blocking`）。
        4. 比对并返回结果。
- `schedule_update_cleanup(app_handle: &AppHandle)` (L244-L268):
    - **审计 A13**：24小时后清理临时目录。若检测到下载进行中则跳过。
- `cleanup_stale_update_dir()` (L276-L308):
    - 启动期兜底清理：删除临时目录中超过 24h 的残留 exe/msi/sha256 文件。
- `backoff_delay_secs(fail_count: u32) -> u64` (L320-L327):
    - 计算连续失败后的等待时间。fail_count 0→24h, 1→5min, 2→15min, 3→1h, ≥4→24h。
- `check_and_record(mirror_first: bool) -> Result<UpdateInfo, String>` (L335-L346):
    - 执行检查并更新全局状态 `LAST_CHECK_ERROR` 和 `LAST_CHECK_TIME_MS`。
- `start_update_check_loop(app_handle: &AppHandle)` (L348-L387):
    - 启动后台循环：启动延迟 5s，随后每 24h 检查一次。支持取消令牌中断和失败退避。
- `do_update_check(...)` (L389-L418):
    - 单次检查执行：调用 `check_and_record`，发送通知（Windows Toast 或普通通知）。
- `fetch_latest_release(mirror_first: bool) -> Result<(bool, String, String, Option<String>)>` (L420-L446):
    - 按配置排序源（GitHub 官方或镜像优先），依次尝试 `version.json`。
- `fetch_version_from_url(url: &str)` (L448-L482):
    - 解析 `version.json`，提取 version, notes, asset。
- `check_update_inner(mirror_first: bool)` (L485-L533):
    - 内部检查逻辑：
        1. 获取版本信息。
        2. **404 探测**：HEAD 请求检测 Release 资产是否存在，避免 version.json 先于 Release 发布导致的误报。
        3. 构造 SHA256 校验源列表（GitHub API digest → 原始 .sha256 → 镜像）。
        4. 返回 `UpdateInfo`。

### `tauri-app/src-tauri/src/commands/updater.rs`

#### 全局状态
- `DOWNLOAD_CANCEL_TOKEN`: `Mutex<Option<Arc<CancellationToken>>>` (L10)，当前下载任务的取消令牌。
- `CancelSlotGuard`: RAII 结构体，Drop 时清空取消令牌槽（审计 A6）。

#### Tauri 命令
- `check_update(app_handle, state)` (L24):
    - 手动检查更新入口。调用 `updater::check_and_record`。
- `download_update(app_handle, url, state)` (L32-L168):
    - **审计 A6**：
        1. 互斥检查：`is_downloading` TaskGuard。
        2. 注册取消令牌。
        3. 域名白名单校验。
        4. 文件名清洗（防止路径穿越）。
        5. 大小限制 500MB。
        6. 异步流式下载（`tokio::fs`），每 200ms 或结束时推送进度。
        7. 支持取消：检测到取消则删除半成品。
- `cancel_download_update()` (L170-L180):
    - 触发取消令牌，幂等返回。
- `install_update(app_handle, file_path, checksum_url)` (L182-L265):
    - **安全校验**：
        1. 路径必须在 `update_temp_dir()` 下。
        2. SHA256 校验：解析 `checksum_url`（支持 JSON 数组），调用 `verify_download_sha256`。
        3. 若校验失败或未提供，删除文件并报错。
    - **安装执行**：
        - `.exe`: 使用 `open::that`。
        - `.msi`: Windows 使用 `msiexec /i` (带 UAC 标志)，非 Windows 报错。
        - 安装成功后调度清理任务。
- `get_mirror_urls(github_url)` (L267-L296):
    - 生成前端可用的镜像 URL 列表。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      Frontend (Tauri)                        │
└───────────────┬───────────────────────────────┬──────────────┘
                │ check_update                  │ download_update
                ▼                               ▼
┌──────────────────────────────┐ ┌────────────────────────────────┐
│  commands/updater.rs         │ │  update/updater.rs             │
│                              │ │                                │
│  check_update(cmd)           │ │  start_update_check_loop()     │
│      │                       │ │      │                         │
│      ▼                       │ │      ▼                         │
│  check_and_record()          │ │  do_update_check()             │
│      │                       │ │      │                         │
│      ▼                       │ │      ▼                         │
│  fetch_latest_release()      │ │  check_and_record()            │
│      │                       │ │      │                         │
│      ▼                       │ │      ▼                         │
│  fetch_version_from_url()    │ │  check_update_inner()          │
│                              │ │      │                         │
│  download_update(cmd)        │ │      ▼                         │
│      │                       │ │  fetch_latest_release()        │
│      ▼                       │ │      │                         │
│  verify_download_sha256()    │ │      ▼                         │
│      │                       │ │  install_update(cmd)           │
│      ▼                       │ │      │                         │
│  schedule_update_cleanup()   │ │  verify_download_sha256()      │
└──────────────────────────────┘ │      │                         │
                                 │      ▼                         │
                                 │  open::that / msiexec           │
                                 └────────────────────────────────┘
```

## Data Flow

1. **自动检查**: `start_update_check_loop` 启动后延迟 5s，进入 24h 周期循环。每次循环调用 `do_update_check`，进而调用 `check_and_record`。
2. **版本获取**: `check_update_inner` 调用 `fetch_latest_release`，按配置顺序尝试 GitHub 官方或镜像源的 `version.json`。
3. **资产探测**: 若版本有新变化，执行 HEAD 请求确认 Release 资产存在，防止时序问题导致的 404。
4. **校验源构建**: 组合 GitHub API Digest、原始 .sha256 及镜像源 URL。
5. **手动检查**: 用户触发 `check_update` 命令，复用 `check_and_record` 逻辑，确保前后端状态一致。
6. **下载**: `download_update` 命令进行互斥检查、域名白名单校验，然后流式下载至临时目录，实时推送进度。
7. **安装与校验**: `install_update` 命令首先验证文件路径在临时目录内，然后调用 `verify_download_sha256`。校验通过后才启动安装程序，并调度 24h 后的清理任务。

## Connections
[[release-asset-integrity]]、[[version-json-push-before-release-404]]

## Known Issues
- 无。旧版文章中提到的 `decide_checksum_missing` 语义偏宽和无法区分"未校验但放行"的问题已在当前代码中解决：
    - `decide_checksum_missing` 现在严格区分 4xx（文件不存在/未上传）和传输/5xx 错误。
    - 只有当 `skipSha256WhenMissing` 显式开启且全为 4xx 时才放行，否则报错。
    - 错误文案准确反映原因（"校验和源全部不可用（4xx）" vs "所有校验和源均失败（含 5xx/网络错误）"）。