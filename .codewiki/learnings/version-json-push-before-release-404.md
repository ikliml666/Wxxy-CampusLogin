---
title: 更新检查与下载流程（updater）
type: module
source_files:
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/commands/updater.rs
tags: [更新, 版本, SHA256, 镜像源, 下载]
---

## Overview

本模块负责 Tauri 应用的自动更新检查、下载安装与安装执行。核心机制包括：基于 `version.json` 的版本比对、多源降级（GitHub 官方源 + 国内镜像）、SHA256 完整性校验、带取消功能的断点续传式下载，以及 Windows MSI/EXE 安装器调用。

针对旧版“推送 `version.json` 后 Release 资产未就绪导致全员 404”的问题，当前代码在 `check_update_inner` 中增加了 **Asset 存在性探测** 步骤：若 `version.json` 声明有更新但 GitHub Release 资产返回 404，则本轮静默跳过提示，避免用户收到无效更新通知。

## Key Components

### `tauri-app/src-tauri/src/update/updater.rs`

#### 常量与全局状态
- `GITHUB_REPO`: `1` — 仓库名 `"ikliml666/Wxxy-CampusLogin"`
- `AUTO_CHECK_INTERVAL_SECS`: `2` — 自动检查周期 86400s (24h)
- `BACKOFF_RETRY_SECS`: `3` — 失败退避序列 `[300, 900, 3600]` (5min → 15min → 1h)
- `STARTUP_CHECK_DELAY_SECS`: `5` — 启动后延迟 5s 首次检查
- `VERSION_FILE_URL`: `15` — 官方 `version.json` 地址
- `VERSION_MIRRORS`: `18` — 三个国内镜像源列表
- `LAST_CHECK_ERROR`: `12` — `Mutex<Option<String>>`，记录最近一次检查失败原因，供前端 `lastCheckError` 字段展示
- `LAST_CHECK_TIME_MS`: `16` — `AtomicU64`，记录最近一次检查完成时间（epoch ms）
- `UPDATE_TEMP_DIR`: `173` — 临时目录名 `"campus-login-update"`
- `UPDATE_STALE_SECS`: `175` — 残留清理阈值 24h
- `UPDATE_ASSET_EXTS`: `177` — 需清理的文件扩展名 `["exe", "msi", "sha256"]`

#### Structs / Models
- `ReleaseAsset` (`23`): 发布资产信息
  - `name: String`
  - `url: String`
  - `size: u64`
- `UpdateInfo` (`31`): 更新检查结果（序列化使用 camelCase）
  - `has_update: bool`
  - `latest_version: String`
  - `release_notes: String`
  - `assets: Vec<ReleaseAsset>`
  - `sha256_checksum: Option<String>` (JSON 数组字符串，含所有校验源 URL)
  - `last_check_error: Option<String>` (`40`) — 上次检查失败快照
  - `last_check_time: Option<u64>` (`45`) — 上次检查时间戳
- `DownloadProgress` (`50`): 下载进度推送结构
  - `downloaded: u64`
  - `total: u64`
  - `speed: u64`
  - `percent: f64`

#### 函数

- `compare_versions(current: &str, latest: &str) -> bool` (`56`): 语义化版本号比较。去除 'v' 前缀，按 '.' 分割为数字段，逐段比较（支持 4 段 hotfix，如 `2.3.0.1` > `2.3.0`）。
- `build_short_timeout_http_client() -> Result<&'static reqwest::Client, String>` (`93`): 构建 10s 超时的模块级共享 HTTP 客户端（`OnceLock` 缓存），不强制 `no_proxy()` 以保持系统代理语义。
- `decide_checksum_missing(all_client_errors: bool, had_transport_error: bool, allow_skip: bool) -> Result<(), String>` (`112`): 所有校验源失败时的决策逻辑。仅当全部 4xx 且 `allow_skip=true` 时降级通过；否则拒绝。
- `extract_checksum(url: &str, text: &str) -> Option<String>` (`131`): 从响应文本提取 64 位小写 SHA256。支持 GitHub API JSON `digest` 字段、shasum 风格纯文本、BSD 风格纯文本。
- `verify_download_sha256(file_path: &str, checksum_urls: &[String], allow_skip_missing: bool) -> Result<bool, String>` (`153`): 下载后 SHA256 校验。按序尝试所有 `checksum_urls` 获取期望哈希，再分块（64KB buffer）计算本地文件哈希并比对。
- `update_temp_dir() -> std::path::PathBuf` (`183`): 返回 `%TEMP%/campus-login-update` 路径。
- `schedule_update_cleanup(app_handle: &tauri::AppHandle)` (`188`): 调度 24h 后清理临时目录。审计 A13：若检测到 `is_downloading` 标志活跃则跳过本轮清理。
- `cleanup_stale_update_dir()` (`215`): 启动期兜底清理。扫描临时目录，删除 mtime 超过 24h 的 exe/msi/sha256 残留文件。
- `now_epoch_ms() -> u64` (`262`): 当前 epoch 毫秒时间戳。
- `backoff_delay_secs(fail_count: u32) -> u64` (`273`): 根据连续失败次数返回等待秒数：0→24h, 1→5min, 2→15min, 3→1h, ≥4→24h（重置计数）。
- `check_and_record(mirror_first: bool) -> Result<UpdateInfo, String>` (`284`): 执行检查并记录状态（`LAST_CHECK_ERROR`/`LAST_CHECK_TIME_MS`）。自动循环与手动命令共用此入口。
- `start_update_check_loop(app_handle: &tauri::AppHandle)` (`300`): 启动后台自动检查循环。启动延迟 5s，之后每 24h 或退避间隔检查一次；支持 `CancellationToken` 中断。
- `do_update_check(app_h: &tauri::AppHandle, state: &AppState) -> bool` (`335`): 执行单次检查并发送 Toast/系统通知。Windows 使用 WinRT toast 点击跳转关于界面。
- `fetch_latest_release(mirror_first: bool) -> Result<(bool, String, String, Option<String>), String>` (`365`): 按配置排序源列表，依次请求 `version.json`，任一成功即返回 `(has_update, latest_tag, notes, asset_filename)`。
- `fetch_version_from_url(url: &str) -> Result<(bool, String, String, Option<String>), String>` (`395`): 从单个 URL 拉取并解析 `version.json`，提取 `version`、`notes`、`asset` 字段。
- `check_update_inner(mirror_first: bool) -> Result<UpdateInfo, String>` (`425`): 核心检查逻辑。调用 `fetch_latest_release` 后，若 `has_update=true` 则对 GitHub Release 资产执行 HEAD 探测：若返回 404 则静默设 `has_update=false`（解决 version.json 先于 Release 发布的问题）。构造 `UpdateInfo` 返回，包含 SHA256 校验源列表。

### `tauri-app/src-tauri/src/commands/updater.rs`

#### 全局状态
- `DOWNLOAD_CANCEL_TOKEN`: `14` — `parking_lot::Mutex<Option<Arc<CancellationToken>>>`，单槽存放当前下载任务的取消令牌。
- `CancelSlotGuard` (`21`): RAII 守卫，Drop 时清空 `DOWNLOAD_CANCEL_TOKEN` 槽位。

#### Tauri Commands

- `check_update(app_handle, _state)` (`30`): 手动检查更新入口。调用 `check_and_record` 并序列化返回 `UpdateInfo`。
- `download_update(app_handle, url, _state)` (`37`): 下载更新安装包。
  - 互斥：通过 `is_downloading` TaskGuard 保证同时仅一个下载。
  - 安全：注册 `CancellationToken` 到单槽；校验 URL 协议 (HTTPS) 与 Host 白名单（14 个允许域名）。
  - 文件名清洗：从 URL 路径提取，替换 Windows 非法字符，防止路径穿越。
  - 下载：使用长超时客户端 (connect 30s, total 1800s)，分块异步写入 (`tokio::fs`)，每 200ms 推送 `DownloadProgress`。
  - 取消/错误路径：均清理半成品文件。
- `cancel_download_update()` (`163`): 取消进行中的下载。幂等：无任务时返回 `false`。
- `install_update(app_handle, file_path, checksum_url)` (`173`): 验证并安装。
  - 路径校验：必须位于 `update_temp_dir()` 内（防任意文件执行）。
  - SHA256 校验：解析 `checksum_url`（支持 JSON 数组或单 URL），调用 `verify_download_sha256`，默认拒绝未校验安装（除非 `skipSha256WhenMissing` 开启）。
  - 执行：`.exe` 用 `open::that`；`.msi` 用 `msiexec /i` (Windows 下处理引号转义)。安装成功后调度 `schedule_update_cleanup`。
- `get_mirror_urls(github_url)` (`237`): 根据 GitHub 资产 URL 生成各镜像源的完整 URL 列表（供前端 UI 展示）。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Frontend (UI)                                │
│  ► check_update ◄── Tauri Command ──► check_and_record()           │
│  ► download_update ◄── Tauri Command ──► download_update()         │
│  ► cancel_download_update ◄── Tauri Command ──► cancel_token       │
│  ► install_update ◄── Tauri Command ──► verify + open/msiexec      │
└───────────────┬─────────────────────────────────────────────────────┘
                │ HTTP / JSON
                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     updater.rs (Core Logic)                         │
│                                                                     │
│  fetch_latest_release() ──► fetch_version_from_url()               │
│       ▲        │                                                  │
│       │        ▼ HEAD Probe (404? skip)                            │
│       │   check_update_inner()                                     │
│       │        │                                                  │
│       │        ▼ SHA256 Source List                                │
│       │   UpdateInfo { has_update, assets, sha256_checksum, ... }  │
│                                                                     │
│  start_update_check_loop() ──► tokio::select! (sleep/cancel)       │
│       ▲        │                                                  │
│       │   do_update_check() ──► EventBus / Toast / Notification    │
│                                                                     │
│  verify_download_sha256() ──► extract_checksum()                   │
│       ▲        │                                                   │
│       │   decide_checksum_missing() (4xx vs 5xx/transport)         │
│                                                                     │
│  schedule_update_cleanup() / cleanup_stale_update_dir()            │
└─────────────────────────────────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                  External Sources                                   │
│  ► GitHub API (releases/tags/vX.Y.Z)                               │
│  ► raw.githubusercontent.com / .sha256                              │
│  ► Mirrors: ghfast.top, gh-proxy.com, ghproxy.net                  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **自动检查**: `start_update_check_loop` 启动后延迟 5s 执行首次检查，此后每 24h 循环。失败时按 `5min → 15min → 1h` 退避，用尽后重置为 24h。
2. **版本比对**: `check_and_record` 调用 `fetch_latest_release`，按 `update_source` 配置决定优先尝试官方源还是镜像源。解析 `version.json` 得到 `latest_tag`，与 `env!("APP_VERSION")` 比较。
3. **Asset 探测**: 若 `has_update=true`，对 GitHub Release 资产 URL 发起 HEAD 请求。若返回 404，说明 `version.json` 已推但 Release 未发布，静默将 `has_update` 置为 `false`，不触发通知。
4. **手动检查**: 前端调用 `check_update` 命令，同样走 `check_and_record`，并将 `last_check_error` 和 `last_check_time` 回传给前端展示。
5. **下载**: 前端调用 `download_update`，传入资产 URL。命令层校验 Host 白名单，注册取消令牌，分块异步下载至 `%TEMP%/campus-login-update`，期间每 200ms 推送进度。
6. **安装**: 前端调用 `install_update`，传入文件路径和 `checksum_url`。命令层校验路径在临时目录内，调用 `verify_download_sha256` 校验完整性（支持多源降级、4xx 跳过开关），通过后调用 `open::that` (EXE) 或 `msiexec` (MSI)。
7. **清理**: 安装成功后调度 `schedule_update_cleanup` (24h 后删目录，若下载中则跳过)。下次启动时 `cleanup_stale_update_dir` 兜底清理超过 24h 的残留资产文件。

## Connections

[[release-asset-integrity]]、[[version-single-source-build-rs]]、[[android-version-json-camelcase-parse-drift]]