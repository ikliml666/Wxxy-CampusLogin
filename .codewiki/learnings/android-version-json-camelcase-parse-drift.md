以下是根据当前源码重写的文章。

---
title: 安卓与桌面更新模块架构对比与版本解析契约
type: learning
source_files:
  - android/src-tauri/src/update_cmds.rs
  - tauri-app/src-tauri/src/update/updater.rs
tags: [教训, 安卓, 桌面, 更新, 反序列化, 测试]
---

## Overview

本文档记录了 `Wxxy-CampusLogin` 项目中 Android 端 (`update_cmds.rs`) 与桌面端 (`updater.rs`) 更新模块的架构差异，以及核心的版本解析契约。

核心发现：Android 端曾因错误使用 camelCase 结构体反序列化 snake_case 的 `version.json` 导致更新检测失效（≤v2.3.6）。当前代码已通过引入专用的 `VersionFile` 结构体修复此问题。两个模块在镜像源配置、退避重试逻辑和 SHA256 校验机制上保持了高度语义对齐，但在安装执行路径（APK Intent vs EXE 替换）和 HTTP 客户端策略上存在平台特异性实现。

## Key Components

### Android: `android/src-tauri/src/update_cmds.rs`

**常量与全局状态:**
- `VERSION_FILE`: 行 40。GitHub 原始 version.json URL。
- `AUTO_CHECK_INTERVAL_SECS`: 行 43。24小时检查周期 (86400s)。
- `BACKOFF_RETRY_SECS`: 行 47。失败退避序列 `[300, 900, 3600]` (5min, 15min, 1h)。
- `VERSION_MIRRORS`: 行 57。三个 GitHub 加速镜像源 URL。
- `DOWNLOAD_ALLOWED_HOSTS`: 行 64。下载域名白名单，用于防止 SSRF。
- `MAX_DOWNLOAD_BYTES`: 行 73。500MB 下载大小上限。
- `APK_STALE_SECS`: 行 79。残留 APK 清理阈值 (24h)。
- `UPDATE_LOOP_RUNNING`: 行 32。原子布尔值，防止启动多个更新检查循环。
- `UPDATE_CHECKSUM`: 行 36。Mutex 保护的 Option<String>，存储最近一次检查获取的 SHA256 校验值，供下载后比对。
- `LAST_CHECK_ERROR`: 行 49。Mutex 保护的 Option<String>，记录最近一次检查失败原因，供前端展示。
- `LAST_CHECK_TIME_MS`: 行 53。AtomicU64，记录最近一次检查完成时间戳。
- `DOWNLOAD_RUNNING`: 行 235。原子布尔值，防止重复下载任务互写文件。

**数据结构 (Structs):**
- `ReleaseAsset`: 行 93。发布资产信息 (`name`, `url`, `size`)。
- `UpdateInfo`: 行 98。更新检测结果 DTO。
    - 字段: `has_update` (bool), `latest_version` (String), `release_notes` (String), `assets` (Vec<ReleaseAsset>), `sha256_checksum` (Option<String>), `last_check_error` (Option<String>), `last_check_time` (Option<u64>)。
    - 注: 使用 `#[serde(rename_all = "camelCase", default)]` 确保 JSON 交互兼容性。
- `DownloadProgress`: 行 114。下载进度 DTO (`downloaded`, `total`, `speed`, `percent`)。
- `VersionFile`: 行 148。**关键修复**。专为解析 `version.json` 设计的内部结构。
    - 字段: `version` (String), `notes` (String)。
    - 注: 使用 snake_case 匹配上游真实数据形状，避免之前的反序列化陷阱。

**公开函数/命令 (Tauri Commands):**
- `check_update(app)`: 行 135。手动触发更新检查。调用 `check_and_record`。
- `get_mirror_urls(github_url)`: 行 222。返回可供前端选择的镜像 URL 列表。
- `download_update(app, url)`: 行 242。流式下载 APK。包含并发控制、白名单校验、进度发射、SHA256 校验。
- `install_update(app, file_path)`: 行 306。通过 `CampusMonitorServiceExt.install_apk` 拉起系统安装器。安装成功后删除本地 APK 文件。
- `cleanup_stale_apks(app)`: 行 332。清理超过 `APK_STALE_SECS` 的残留 APK 文件。

**内部辅助函数:**
- `update_dir(app)`: 行 84。计算更新目录路径 `app_data_dir/files/update`。
- `http_client()`: 行 119。构建带超时设置的 reqwest Client。移动端强制 rustls。
- `allowed_url(url)`: 行 130。白名单域名校验漏斗。
- `has_newer_version(current, latest)`: 行 104。语义对齐桌面的版本比较逻辑（去 v 前缀，逐段数值比较）。
- `backoff_delay_secs(fail_count)`: 行 169。计算退避等待时间。
- `check_and_record(app)`: 行 176。执行检查并记录状态（错误/时间戳）。
- `fetch_apk_assets(client, latest)`: 行 156。从 GitHub API 拉取 APK 资产和 digest。
- `verify_file_sha256(path, expected)`: 行 286。流式计算文件 SHA256 并比对。注意使用堆分配 buffer 避免栈溢出。

**测试模块 (`mod tests`):**
- `版本比较_逐段数值`: 验证 `has_newer_version` 的各种边界情况。
- `白名单域名校验`: 验证 `allowed_url` 的正反向用例。
- `UpdateInfo_反序列化_camelCase契约`: **关键测试**。验证 UpdateInfo 能正确反序列化 camelCase JSON，且缺失 `lastCheckError` 等字段时默认为 None。
- `安装目录约定_含files段`: 验证 `update_dir` 拼接结果包含 `files` 段，适配 Android FileProvider。
- `清理残留安装包_超龄删_新鲜留_非apk不动`: 验证 `cleanup_stale_apks` 的文件筛选和过期判定逻辑。
- `退避间隔_步进后回归24h周期`: 验证 `backoff_delay_secs` 的退避序列。
- `version_json_真实形状解析`: **关键测试**。使用真实的 snake_case JSON 数据测试 `VersionFile`，确保不再出现“永远已是最新”的 bug。

### Desktop: `tauri-app/src-tauri/src/update/updater.rs`

**常量与全局状态:**
- `GITHUB_REPO`: 行 7。仓库名 `ikliml666/Wxxy-CampusLogin`。
- `AUTO_CHECK_INTERVAL_SECS`: 行 8。24小时 (86400s)。
- `BACKOFF_RETRY_SECS`: 行 11。退避序列 `[300, 900, 3600]`。
- `STARTUP_CHECK_DELAY_SECS`: 行 14。启动延迟 5s。
- `LAST_CHECK_ERROR`: 行 20。Mutex 保护，记录最近检查失败原因。
- `LAST_CHECK_TIME_MS`: 行 23。AtomicU64，记录最近检查时间。
- `VERSION_FILE_URL`: 行 25。GitHub 原始 version.json URL。
- `VERSION_MIRRORS`: 行 28。三个 GitHub 加速镜像源 URL。
- `SHORT_TIMEOUT_CLIENT`: 行 82。OnceLock 缓存的短超时 HTTP Client (10s)。

**数据结构 (Structs):**
- `ReleaseAsset`: 行 34。与 Android 端一致。
- `UpdateInfo`: 行 39。与 Android 端一致，同样使用 `rename_all = "camelCase"`。
- `DownloadProgress`: 行 54。与 Android 端一致。

**公开函数:**
- `compare_versions(current, latest)`: 行 62。桌面端版本比较逻辑。支持多段版本号（如 hotfix），不截断为 3 段。
- `decide_checksum_missing(all_client_errors, had_transport_error, allow_skip)`: 行 96。决策是否跳过 SHA256 校验。区分 4xx（文件不存在）和传输错误。
- `extract_checksum(url, text)`: 行 116。从 GitHub API JSON 或 .sha256 文本中提取哈希值。支持 BSD 风格。
- `verify_download_sha256(file_path, checksum_urls, allow_skip_missing)`: 行 136。尝试多个校验源获取期望哈希，并在 spawn_blocking 中计算实际哈希进行比对。
- `schedule_update_cleanup()`: 行 186。安排 24h 后清理临时更新目录。
- `start_update_check_loop(app_handle)`: 行 212。启动后台检查循环。集成 `TaskManager` 以支持取消令牌中断。
- `check_update_inner(mirror_first)`: 行 266。执行内部检查逻辑，包括资产存在性探测 (HEAD request)。

**内部辅助函数:**
- `build_short_timeout_http_client()`: 行 76。构建并缓存短超时 Client。
- `check_and_record(mirror_first)`: 行 202。执行检查并记录状态。
- `do_update_check(app_h, state)`: 行 234。执行单次检查并发送通知/Toast。
- `fetch_latest_release(mirror_first)`: 行 254。按优先级顺序请求 version.json。
- `fetch_version_from_url(url)`: 行 276。从单个 URL 解析 version.json。

**测试模块 (`mod tests`):**
- `all_4xx_without_explicit_optin_is_hard_error`: 验证未开启跳过时，所有校验源 4xx 应拒绝安装。
- `backoff_retry_schedule_steps_then_24h_cycle`: 验证退避序列。
- `update_info_serializes_last_check_fields_as_camel_case`: 验证 UpdateInfo 序列化/反序列化兼容性。
- `hotfix_four_segment_version_is_detected`: 验证 `compare_versions` 正确处理 4 段版本号（历史 bug 回归）。
- `version_compare_numeric_and_v_prefix`: 验证基本版本比较。
- `extract_checksum_supports_bsd_style`: 验证提取逻辑支持多种格式。
- `extract_checksum_from_github_api_json`: 验证从 GitHub API 响应中提取 digest。
- `server_error_or_transport_error_always_blocks`: 验证服务端错误或传输错误总是阻止安装。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                                       Update Module                                        │
├───────────────────────┬─────────────────────────────────────────────────────────────────────┤
│       Android         │                               Desktop                               │
│ (update_cmds.rs)      │                              (updater.rs)                           │
├───────────────────────┼─────────────────────────────────────────────────────────────────────┤
│ • VersionFile (snake) │ • fetch_version_from_url (data["version"])                          │
│   ← FIX: Real Shape   │                                                                     │
│                       │                                                                     │
│ • check_update_cmd    │ • check_update_inner                                                │
│   → check_and_record  │   → fetch_latest_release                                            │
│                       │     → HEAD probe for asset existence                                │
│                       │                                                                     │
│ • download_update_cmd │ • verify_download_sha256                                            │
│   → allowed_url       │   → extract_checksum (API + .sha256)                                │
│   → verify_file_sha256│   → decide_checksum_missing                                         │
│   → emit progress     │                                                                     │
│                       │                                                                     │
│ • install_update_cmd  │ • schedule_update_cleanup                                           │
│   → CampusMonitorSvc  │                                                                     │
│     .install_apk()    │                                                                     │
│   → remove_file()     │                                                                     │
│                       │                                                                     │
│ • start_update_loop   │ • start_update_check_loop                                           │
│   → tokio::spawn      │   → TaskManager.spawn (cancelable)                                  │
│   → AtomicBool guard  │   → select! with cancel_token                                       │
└───────────────────────┴─────────────────────────────────────────────────────────────────────┘
          │                               │
          ▼                               ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                                    Shared Contract                                         │
│  • version.json Source: GitHub Raw + 3 Mirrors                                              │
│  • Check Interval: 24h with Backoff Retry (5m -> 15m -> 1h)                                 │
│  • State Tracking: LAST_CHECK_ERROR, LAST_CHECK_TIME_MS                                     │
│  • DTO: UpdateInfo (camelCase JSON fields)                                                  │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Update Check**:
    *   Both platforms iterate through a prioritized list of URLs (`VERSION_FILE_URL` then mirrors, or vice versa based on user config).
    *   Fetch `version.json`.
    *   **Android**: Deserializes into `VersionFile` (snake_case `version`).
    *   **Desktop**: Extracts `data["version"]` directly from `serde_json::Value`.
    *   Compare current version with latest using platform-specific comparison functions.
    *   Record result in `LAST_CHECK_ERROR` and `LAST_CHECK_TIME_MS`.
    *   Return `UpdateInfo` (camelCase) to frontend.

2.  **Download**:
    *   Frontend calls `download_update` (Android) or triggers desktop download.
    *   Validate URL against whitelist (`allowed_url`).
    *   Stream download to disk (`app_data_dir/files/update` on Android, temp dir on Desktop).
    *   Emit progress events.
    *   **Android**: Uses `UPDATE_CHECKSUM` (set during check) to verify SHA256 via `verify_file_sha256`.
    *   **Desktop**: Tries multiple checksum sources (GitHub API digest, `.sha256` file, mirrors) via `verify_download_sha256`.

3.  **Install**:
    *   **Android**: Calls `install_update`, which invokes `CampusMonitorServicePlugin.install_apk()` to launch the system installer via Content URI. Deletes the APK file after launching.
    *   **Desktop**: Typically involves replacing the executable or running an installer. The codebase shows cleanup scheduling for temp directories.

4.  **Background Loop**:
    *   Spawns a background task on startup.
    *   Sleeps 5s, then checks.
    *   On success, resets failure count. On failure, increments and applies backoff delay.
    *   Emits `update-available` event if new version detected.

## Connections

[[android-version-json-camelcase-parse-drift]] (Previous lesson on the specific bug this module fixed)
[[config-field-sets-bidirectional-sync]] (Related to update_source config field used for mirror prioritization)