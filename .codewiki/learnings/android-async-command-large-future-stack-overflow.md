---
title: 安卓异步更新命令——栈溢出修复与完整更新流
type: learning
source_files:
tags: [安卓, tauri, 异步, 栈溢出, 更新, 崩溃, 闪退, SHA256, 白名单]
---

## Overview

`android/src-tai/src/update_cmds.rs` 实现了 Android 平台专属的 Tauri 应用更新流程。与桌面端复用 HTTP 检查/下载逻辑不同，安装环节必须通过 Android `FileProvider` + `Intent` 调用系统包安装器。

核心历史教训：v2.3.8 发布时，`verify_file_sha256` 中使用 64KB 栈数组 `[0u8; 65536]` 作为 async future 的局部状态，导致在 Android WebView 的 **JavaBridge 线程**（栈空间极小）上构造 `download_update` future 时发生栈溢出（SIGSEGV）。修复方案为将其改为堆分配 `vec![0u8; 65536]`。

## Key Components

### 常量与全局状态 (`update_cmds.rs`)

- **`VERSION_FILE` (L33)**: GitHub 原始源 `version.json` 地址。
- **`AUTO_CHECK_INTERVAL_SECS` (L35)**: 自动更新检查周期，24小时。
- **`BACKOFF_RETRY_SECS` (L37)**: 失败退避重试间隔数组：`[5min, 15min, 1h]`。超过3次连续失败后重置为24h周期。
- **`VERSION_MIRRORS` (L49)**: GitHub 镜像源列表，按序降级。
- **`DOWNLOAD_ALLOWED_HOSTS` (L57)**: 下载域名白名单，包含 GitHub 官方域名及三个镜像域名，用于防 SSRF。
- **`MAX_DOWNLOAD_BYTES` (L66)**: 下载大小上限 500MB。
- **`APK_STALE_SECS` (L70)**: 残留 APK 清理阈值，24小时。
- **`UPDATE_LOOP_RUNNING` (L25)**: `AtomicBool`，防止启动多个更新检查循环。
- **`UPDATE_CHECKSUM` (L29)**: `Mutex<Option<String>>`，存储最近一次 check_update 获取的 SHA256 校验值，供 download_update 完成后比对。
- **`LAST_CHECK_ERROR` (L43)**: `Mutex<Option<String>>`，记录最近一次检查失败原因，供前端展示。
- **`LAST_CHECK_TIME_MS` (L46)**: `AtomicU64`，记录最近一次检查完成时间。
- **`DOWNLOAD_RUNNING` (L196)**: `AtomicBool`，下载任务并发互斥锁。

### 数据结构 (`update_cmds.rs`)

- **`ReleaseAsset` (L76)**: 
    - `name: String`: 资产文件名。
    - `url: String`: 下载 URL。
    - `size: u64`: 文件大小。
- **`UpdateInfo` (L82)**: 更新检查结果，序列化使用 `camelCase`。
    - `has_update: bool`: 是否有新版本。
    - `latest_version: String`: 最新版本号。
    - `release_notes: String`: 更新说明。
    - `assets: Vec<ReleaseAsset>`: APK 资产列表。
    - `sha256_checksum: Option<String>`: SHA256 校验值（来自 GitHub API digest）。
    - `last_check_error: Option<String>`: 上次检查失败原因快照。
    - `last_check_time: Option<u64>`: 上次检查完成时间。
- **`DownloadProgress` (L99)**: 下载进度数据。
    - `downloaded: u64`, `total: u64`, `speed: u64`, `percent: f64`.
- **`VersionFile` (L138)**: `version.json` 的反序列化结构，注意字段为 `snake_case` (`version`, `notes`)，与 `UpdateInfo` 的 `camelCase` 不同。

### 公开函数与命令

- **`update_dir(app)` (L73)**: 计算更新目录路径，约定为 `app_data_dir/files/update`。这是为了兼容 Android `FileProvider` 的 `filesDir` 映射。
- **`has_newer_version(current, latest)` (L105)**: 语义化版本比较，去除 'v' 前缀后逐段数值比较。
- **`http_client()` (L122)**: 构建 `reqwest::Client`，设置 connect_timeout 5s, timeout 30s，使用 rustls。
- **`allowed_url(url)` (L132)**: 白名单校验漏斗，解析主机名并检查是否在 `DOWNLOAD_ALLOWED_HOSTS` 中。
- **`check_update(app)` (L150)**: `#[tauri::command]`。触发更新检查，统一走 `check_and_record`。
- **`backoff_delay_secs(fail_count)` (L168)**: 根据连续失败次数计算等待秒数。
- **`check_and_record(app)` (L177)**: 执行检查并记录 `LAST_CHECK_ERROR` 和 `LAST_CHECK_TIME_MS`。
- **`version_urls(mirror_first)` (L190)**: 根据用户设置的 `update_source` 决定检查源顺序（镜像优先或 GitHub 优先）。
- **`fetch_apk_assets(client, latest)` (L208)**: 从 GitHub API 拉取特定 tag 的 release 资产，提取 `.apk` 文件和 `sha256:<hex>` digest。
- **`check_update_inner(app)` (L236)**: 核心检查逻辑，遍历 URL 源获取 `version.json`，对比版本，拉取资产，返回 `UpdateInfo`。
- **`start_update_check_loop(app)` (L280)**: 启动后台自动检查循环。启动延迟 5s，之后每 24h 检查。失败时按退避策略重试。有新版本时 emit `update-available` 并发送系统通知。
- **`get_mirror_urls(github_url)` (L326)**: `#[tauri::command]`。返回可用的镜像 URL 列表供前端选择。
- **`download_update(app, url)` (L203)**: `#[tauri::command]`。流式下载 APK。使用 `DOWNLOAD_RUNNING` 互斥。
- **`download_update_inner(app, url)` (L212)**: 
    - 校验 URL 白名单。
    - 创建目录 `files/update`。
    - 流式写入文件，每 200ms emit `update-download-progress`。
    - 下载完成后，若存在 `UPDATE_CHECKSUM`，调用 `verify_file_sha256` 进行完整性校验。
    - **关键修复点**: L255 处使用 `vec![0u8; 65536]` 而非栈数组，避免 JavaBridge 线程栈溢出。
- **`verify_file_sha256(path, expected)` (L272)**: `pub(crate)`。流式计算文件 SHA256。容忍 "hex filename" 格式。非标准 64 位十六进制时跳过校验。
- **`install_update(app, file_path)` (L302)**: `#[tauri::command]`。校验路径必须在 `update_dir` 内，然后通过 `CampusMonitorServiceExt.install_apk` 调用系统安装器。安装器拉起后立即删除本地 APK 文件。
- **`cleanup_stale_apks(app)` (L320)**: 清理 `update_dir` 中超过 24h 的 `.apk` 文件，防止磁盘堆积。

### 测试 (`update_cmds.rs` L340+)

- **`版本比较_逐段数值`**: 验证 `has_newer_version` 处理短版本、预发布后缀等边界情况。
- **`白名单域名校验`**: 验证 `allowed_url` 对合法/非法域名的判定。
- **`UpdateInfo_反序列化_camelCase契约`**: 验证 `UpdateInfo` 能正确反序列化前端传来的 camelCase JSON，包括新增的 `lastCheckError` 和 `lastCheckTime`。
- **`安装目录约定_含files段`**: 验证 `update_dir` 拼接结果包含 `files/update`，确保与 `FileProvider` 一致。
- **`清理残留安装包_超龄删_新鲜留_非apk不动`**: 验证 `cleanup_stale_apks` 的判定逻辑。
- **`退避间隔_步进后回归24h周期`**: 验证 `backoff_delay_secs` 的退避策略。
- **`version_json_真实形状解析`**: 验证 `VersionFile` 能正确解析 snake_case 的 `version.json`，避免之前因字段名不匹配导致的“永远已是最新”缺陷。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      Android App (Tauri)                    │
│                                                             │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │  UI / Frontend│──►│ update_cmds.rs   │──►│ System      │ │
│  │  (About Page) │   │ (Rust Backend)   │   │ Package     │ │
│  └──────────────┘   └────────┬─────────┘   └──────▲──────┘ │
│           ▲                  │                     │       │
│           │                  ▼                     │       │
│           │            ┌─────────┐                │       │
│           │            │ HTTP    │                │       │
│           │            │ Client  │                │       │
│           │            └────┬────┘                │       │
│           │                 │                     │       │
│           │          ┌──────▼──────┐              │       │
│           │          │ GitHub /    │              │       │
│           │          │ Mirrors     │              │       │
│           │          └─────────────┘              │       │
└───────────┴───────────────────────────────────────┴───────┘
         ▲                                    │
         │                                    │
         │                                    │ Intent + FileProvider
         │                                    ▼
         └────────────────────────────────────┤ Android OS
                                              │ Package Installer
```

1. **检查流程**: `check_update` -> `check_and_record` -> 遍历 `version_urls` (GitHub/Mirrors) -> 解析 `VersionFile` -> 调用 `fetch_apk_assets` (GitHub API) -> 返回 `UpdateInfo`。
2. **下载流程**: `download_update` -> 白名单校验 -> 流式下载 -> SHA256 校验 (`verify_file_sha256`) -> 落盘。
3. **安装流程**: `install_update` -> 路径安全校验 -> `CampusMonitorService.install_apk` -> 删除本地 APK。
4. **后台循环**: `start_update_check_loop` -> 定时检查 -> 失败退避 -> 有新版本则 Emit 事件并通知。

## Data Flow

1. **版本检查**:
   - 前端调用 `check_update`。
   - Rust 侧根据 `update_source` 配置决定检查源顺序。
   - 获取 `version.json` (snake_case) 得到 `latest_version`。
   - 调用 GitHub API 获取对应 tag 的 assets，提取 `.apk` URL 和 `sha256` digest。
   - 组装 `UpdateInfo` (camelCase)，其中 `last_check_error` 携带上次失败快照。
   - 返回前端，若 `has_update=true`，前端弹窗。

2. **APK 下载**:
   - 前端选择镜像 URL 调用 `download_update`。
   - 流式接收数据，每 200ms 发射 `update-download-progress`。
   - 下载完成后，若 `UPDATE_CHECKSUM` 有值，计算文件 SHA256 并比对。
   - 校验失败则删除文件并报错；成功则返回文件路径。

3. **APK 安装**:
   - 前端调用 `install_update` 传入文件路径。
   - 校验路径位于 `files/update` 目录下。
   - 通过 `CampusMonitorService` 发起 `ACTION_VIEW` Intent，MIME type `application/vnd.android.package-archive`。
   - Android 系统弹出安装界面。
   - Rust 侧立即删除本地 APK 文件，节省空间。

4. **自动检查**:
   - 启动时延迟 5s 后执行一次检查。
   - 之后每 24h 检查一次。
   - 若失败，按 5min -> 15min -> 1h 退避重试。
   - 若成功发现新版本，emit `update-available` 并发送系统通知（若开启）。

## Connections

- [[android-monitor-service]]: `CampusMonitorServiceExt.install_apk` 依赖此模块提供 `FileProvider` 和权限管理。
- [[config-state]]: `current_settings` 用于获取 `update_source` 和 `enable_notification` 配置。
- [[campus-login-lib]]: 提供日志宏 `log_warn!`, `log_info!`。

## Known Issues

- **栈溢出历史问题 (已修复)**: v2.3.8 之前，`verify_file_sha256` 中的 `[0u8; 65536]` 栈数组导致 Android JavaBridge 线程栈溢出。已改为 `vec![]`。今后任何 `#[tauri::command]` async fn 内禁止使用 ≥4KB 的栈数组。
- **version.json 字段名陷阱**: `version.json` 使用 `snake_case` (version/notes)，而 `UpdateInfo` 使用 `camelCase`。不能直接反序列化 `version.json` 到 `UpdateInfo`，否则 `latest_version` 恒为空。必须使用独立的 `VersionFile` struct。
- **下载超时**: GitHub API 在无网络或受限环境下可能 TCP 挂起。`connect_timeout` 设为 5s 是关键，防止单次源检查卡满 30s 总超时。