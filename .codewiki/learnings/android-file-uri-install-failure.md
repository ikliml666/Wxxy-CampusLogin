### Compression of Compressed Context

The user wants to update an existing CodeWiki article `learnings/android-file-uri-install-failure.md` based on the provided source code.
The article should be rewritten to reflect the current state of the code, preserving the structure (title, type, tags, sections).
I need to read the provided source files and generate a new markdown article.

Source files provided:
1.  `android/src-tauri/gen/android/app/src/main/AndroidManifest.xml`
2.  `android/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml`
3.  `android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt`
4.  `android/src-tauri/src/update_cmds.rs`

I will analyze these files to extract the necessary information for the article sections: Overview, Key Components, Architecture, Data Flow, Connections, Known Issues.

**Overview:**
- Module handles Android APK installation, update checking/downloading, and foreground service lifecycle.
- Core issue solved: Android 7+ file URI restriction using FileProvider.
- Two main layers: Rust (`update_cmds.rs`) for logic/network/crypto, Kotlin (`MonitorServicePlugin.kt`) for Android-specific APIs (FileProvider, Services, Receivers).

**Key Components:**

*   **Rust: `update_cmds.rs`**
    *   **Constants:**
        *   `VERSION_FILE`: GitHub raw URL for version.json (:30)
        *   `AUTO_CHECK_INTERVAL_SECS`: 24h (:32)
        *   `BACKOFF_RETRY_SECS`: [5min, 15min, 1h] (:36)
        *   `LAST_CHECK_ERROR`: Mutex<Option<String>> for last check failure reason (:43)
        *   `LAST_CHECK_TIME_MS`: AtomicU64 for last check time (:48)
        *   `VERSION_MIRRORS`: List of mirror URLs (:50-53)
        *   `DOWNLOAD_ALLOWED_HOSTS`: Whitelist for download domains (:57-66)
        *   `MAX_DOWNLOAD_BYTES`: 500MB limit (:68)
        *   `APK_STALE_SECS`: 24h for stale APK cleanup (:74)
        *   `UPDATE_LOOP_RUNNING`: AtomicBool for loop guard (:20)
        *   `UPDATE_CHECKSUM`: Mutex<Option<(String, String)>> for (tag, digest) (:21-26)
        *   `DOWNLOAD_RUNNING`: AtomicBool for download concurrency guard (:317)
    *   **Structs:**
        *   `ReleaseAsset`: name, url, size (:79-83)
        *   `UpdateInfo`: has_update, latest_version, release_notes, assets, sha256_checksum, last_check_error, last_check_time (:86-100)
        *   `DownloadProgress`: downloaded, total, speed, percent (:103-107)
        *   `VersionFile`: version, notes (snake_case for parsing version.json) (:133-137)
    *   **Functions:**
        *   `update_dir(app)`: Returns `app_data_dir/files/update` path. Critical for Android compatibility as `app_data_dir` maps to `/data/user/0/<pkg>` which needs `files/update` to match FileProvider (:76-81)
        *   `has_newer_version(current, latest)`: Semantic version comparison, strips 'v', numeric segment comparison (:110-126)
        *   `http_client()`: reqwest client with connect_timeout 5s, timeout 30s (:129-137)
        *   `allowed_url(url)`: Domain whitelist check (:140-156)
        *   `check_update(app)`: Tauri command entry (:162-164)
        *   `backoff_delay_secs(fail_count)`: Exponential backoff logic (:172-179)
        *   `check_and_record(app)`: Executes check and records error/time (:182-194)
        *   `version_urls(mirror_first)`: URL ordering based on user setting (:197-203)
        *   `fetch_apk_assets(client, latest)`: Fetches APK assets and digest from GitHub API (:206-240)
        *   `check_update_inner(app)`: Core check logic, iterates URLs, parses version.json, gets assets (:243-276)
        *   `start_update_check_loop(app)`: Background loop, starts after 5s delay, then every 24h or backoff interval (:279-316)
        *   `get_mirror_urls(github_url)`: Returns mirror URL list (:319-326)
        *   `download_update(app, url)`: Tauri command with concurrency lock (:329-334)
        *   `download_update_inner(app, url)`: Stream download to `update_dir`, emits progress every 200ms, verifies SHA256 at end (:337-404)
        *   `verify_file_sha256(path, expected)`: Async SHA256 verification, uses heap allocated buf vec[65536] to prevent stack overflow (:407-435)
        *   `install_update(app, file_path)`: Tauri command, validates path prefix, calls Kotlin plugin (:438-460)
        *   `cleanup_stale_apks(app)`: Cleans up .apk files older than 24h (:463-491)
    *   **Tests:**
        *   Various unit tests for version comparison, whitelist, deserialization, directory paths, cleanup, backoff, tag extraction, version.json parsing (:494-593)

*   **Kotlin: `MonitorServicePlugin.kt`**
    *   **Classes & Annotations:**
        *   `MonitorServicePlugin`: @TauriPlugin, extends Plugin (:28-30)
        *   `StartArgs`, `TextArgs`, `BoolArgs`: @InvokeArg data classes (:17-26)
        *   `BootReceiver`: BroadcastReceiver for boot completion (:303-321)
    *   **Commands:**
        *   `startMonitor(invoke)`: Starts foreground service (:33-45)
        *   `stopMonitor(invoke)`: Stops foreground service (:48-56)
        *   `updateNotification(invoke)`: Updates notification text (:59-68)
        *   `installApk(invoke)`: **Core install method**. Resolves APK from `filesDir/update/`, builds FileProvider URI, launches ACTION_VIEW intent (:71-90)
        *   `beginProbeWindow(ininvoke)` / `endProbeWindow(invoke)`: Probe window keep-alive (:93-102)
        *   `getPowerState(invoke)`: Gets screen on/off and WiFi status (:105-119)
        *   `isWifiConnected()`: Checks if active network is WiFi (:122-130)
        *   `getBatteryOptimizationInfo(invoke)`: Gets battery optimization status and brand info (:189-201)
        *   `requestIgnoreBatteryOptimizations(invoke)`: Requests battery optimization whitelist (:204-215)
        *   `openVendorBatterySettings(invoke)`: Opens vendor-specific battery/autostart settings with fallback chain (:218-265)
        *   `openNotificationSettings(invoke)`: Opens notification settings with fallback chain (:268-293)
        *   `setBootAutostart(invoke)` / `isBootAutostartEnabled(invoke)`: Enables/disables boot receiver (:296-318)
    *   **Private Helpers:**
        *   `vendorTargets`: Map of vendor brands to ComponentName lists (:133-170)
        *   `buildVendorIntent(target)`: Builds intent with package_name extra (:173-178)
        *   `startSafely(intent)`: Safe activity start wrapper (:181-185)

*   **Android Manifest & Config:**
    *   `AndroidManifest.xml`:
        *   `REQUEST_INSTALL_PACKAGES`: Permission for APK installation (:9)
        *   `ForegroundService` configuration with `specialUse` type (:46-51)
        *   `BootReceiver` default disabled (:54-60)
        *   `FileProvider` configuration with authority `${applicationId}.fileprovider` and `grantUriPermissions="true"` (:63-70)
    *   `file_paths.xml`:
        *   `<files-path name="update" path="update/" />`: Defines update directory as shared root (:5)

**Architecture:**
Frontend -> Rust (Logic/Network) -> Kotlin (Android APIs) -> System (Package Installer/FileProvider).

**Data Flow:**
1.  Update Check: Frontend triggers check, Rust fetches version.json and assets, emits event.
2.  Download: Frontend triggers download, Rust streams file, verifies SHA256, saves to `update_dir`.
3.  Install: Frontend triggers install, Rust validates path, calls Kotlin, Kotlin uses FileProvider to launch installer.
4.  Cleanup: On startup, Rust cleans up stale APKs.

**Connections:**
Related modules: Android Foreground Service Lifecycle, Tauri Plugin Architecture.

**Known Issues:**
None explicitly mentioned in the current code as open issues, but the history mentions the Android 7+ file URI issue which is now resolved. The code includes robust handling for various edge cases (stack overflow prevention, concurrency guards, path traversal prevention).

I will now construct the markdown article.

```markdown
---
title: Android APK 安装与更新模块
type: module
source_files:
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
  - android/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/src-tauri/src/update_cmds.rs
tags: [安卓, apk 安装, fileprovider, 更新流, foreground service]
---

## Overview

本模块负责 Android 端的应用内 APK 安装流程、自动更新检查与下载、以及后台监控服务的生命周期管理。核心痛点在于 Android 7+ 禁止在应用间共享 `file://` URI，必须通过 `FileProvider` 生成 `content://` URI 才能拉起系统包安装器。

模块包含两个主要部分：
1. **Rust 层 (`update_cmds.rs`)**：处理版本检查、白名单域名流式下载、SHA256 校验、残留 APK 清理，并调用 Kotlin 插件执行安装。
2. **Kotlin 插件层 (`MonitorServicePlugin.kt`)**：提供 Tauri 命令接口，处理前台服务启停、厂商自启/电池优化跳转、以及核心的 `installApk` 逻辑（构建 FileProvider URI）。

## Key Components

### Rust: `update_cmds.rs`

**常量与全局状态**
- `VERSION_FILE`: GitHub 原始 version.json 地址 (:30)
- `AUTO_CHECK_INTERVAL_SECS`: 自动更新检查周期 24h (:32)
- `BACKOFF_RETRY_SECS`: 失败退避重试间隔数组 (5min, 15min, 1h) (:36)
- `LAST_CHECK_ERROR`: Mutex<Option<String>>，记录最近一次检查失败原因 (:43)
- `LAST_CHECK_TIME_MS`: AtomicU64，记录最近一次检查时间 (:48)
- `VERSION_MIRRORS`: 镜像源列表 (:50-53)
- `DOWNLOAD_ALLOWED_HOSTS`: 下载域名白名单 (github.com 及其子域, 镜像站) (:57-66)
- `MAX_DOWNLOAD_BYTES`: 500MB 下载上限 (:68)
- `APK_STALE_SECS`: 残留 APK 清理阈值 24h (:74)
- `UPDATE_LOOP_RUNNING`: 原子锁，防止更新循环重复启动 (:20)
- `UPDATE_CHECKSUM`: Mutex<Option<(String, String)>>，存储最近一次检查到的 (tag, digest) (:21-26)
- `DOWNLOAD_RUNNING`: 原子锁，防止并发下载 (:317)

**结构体**
- `ReleaseAsset`: `name`, `url`, `size` (:79-83)
- `UpdateInfo`: `has_update`, `latest_version`, `release_notes`, `assets`, `sha256_checksum`, `last_check_error`, `last_check_time` (:86-100)
- `DownloadProgress`: `downloaded`, `total`, `speed`, `percent` (:103-107)
- `VersionFile`: `version`, `notes` (snake_case，用于解析 version.json) (:133-137)

**函数**
- `update_dir(app)`: 返回 `app_data_dir/files/update` 路径 (:76-81)。注意 Android 上 `app_data_dir` 对应 `/data/user/0/<pkg>`，需拼接 `files/update` 以匹配 FileProvider 配置。
- `has_newer_version(current, latest)`: 语义化版本比较，去 'v' 前缀，逐段数值比较 (:110-126)
- `http_client()`: 构建 reqwest 客户端，设置 connect_timeout 5s, timeout 30s (:129-137)
- `allowed_url(url)`: 白名单校验漏斗，提取 host 并与 `DOWNLOAD_ALLOWED_HOSTS` 比对 (:140-156)
- `check_update(app)`: Tauri 命令入口 (:162-164)
- `backoff_delay_secs(fail_count)`: 失败退避逻辑：0→24h, 1→5min, 2→15min, 3→1h, ≥4→24h (:172-179)
- `check_and_record(app)`: 执行检查并记录错误/时间 (:182-194)
- `version_urls(mirror_first)`: 根据用户设置决定官方/镜像优先顺序 (:197-203)
- `fetch_apk_assets(client, latest)`: 从 GitHub API 拉取 APK 资产和 digest (:206-240)
- `check_update_inner(app)`: 核心检查逻辑，遍历 URL 源，解析 version.json，获取 assets (:243-276)
- `start_update_check_loop(app)`: 启动后台循环，启动延迟 5s，之后每 24h 或退避间隔检查 (:279-316)
- `get_mirror_urls(github_url)`: 返回镜像 URL 列表 (:319-326)
- `download_update(app, url)`: Tauri 命令，带并发锁 (:329-334)
- `download_update_inner(app, url)`: 流式下载，写入 `update_dir`，每 200ms emit 进度，最后校验 SHA256 (:337-404)
- `verify_file_sha256(path, expected)`: 异步计算 SHA256，使用堆分配 buf vec[65536] 防栈溢出 (:407-435)
- `install_update(app, file_path)`: Tauri 命令，校验路径前缀，调用 Kotlin 插件安装 (:438-460)
- `cleanup_stale_apks(app)`: 清理超过 24h 的残留 .apk (:463-491)

**测试**
- `tests` 模块: 版本比较、白名单校验、UpdateInfo 反序列化、目录约定、清理逻辑、退避间隔、version.json 解析 (:494-593)

### Kotlin: `MonitorServicePlugin.kt`

**类与注解**
- `MonitorServicePlugin`: `@TauriPlugin`, 继承 `Plugin` (:28-30)
- `StartArgs`, `TextArgs`, `BoolArgs`: `@InvokeArg` 数据类 (:17-26)
- `BootReceiver`: `BroadcastReceiver`, 处理开机自启 (:303-321)

**方法 (Command)**
- `startMonitor(invoke)`: 启动前台服务 (:33-45)
- `stopMonitor(invoke)`: 停止前台服务 (:48-56)
- `updateNotification(invoke)`: 更新通知内容 (:59-68)
- `installApk(invoke)`: **核心安装方法**。从 `filesDir/update/` 获取 APK，构建 FileProvider URI，拉起 `ACTION_VIEW` 意图 (:71-90)
- `beginProbeWindow(invoke)` / `endProbeWindow(invoke)`: 探针窗口保活 (:93-102)
- `getPowerState(invoke)`: 获取屏幕状态和 WiFi 连接 (:105-119)
- `isWifiConnected()`: 实时查询当前网络是否为 WiFi (:122-130)
- `getBatteryOptimizationInfo(invoke)`: 获取电池优化状态和品牌信息 (:189-201)
- `requestIgnoreBatteryOptimizations(invoke)`: 请求加入电池优化白名单 (:204-215)
- `openVendorBatterySettings(invoke)`: 跳转厂商自启/省电页，降级链：厂商专用 → 应用详情 → 通用设置 (:218-265)
- `openNotificationSettings(invoke)`: 跳转通知设置页，降级链 (:268-293)
- `setBootAutostart(invoke)` / `isBootAutostartEnabled(invoke)`: 开关开机自启 Receiver (:296-318)

**私有辅助**
- `vendorTargets`: 各品牌自启页 ComponentName 映射 (:133-170)
- `buildVendorIntent(target)`: 构建带 package_name extra 的 Intent (:173-178)
- `startSafely(intent)`: 安全启动 Activity (:181-185)

### Android Manifest & Config

- `AndroidManifest.xml`:
    - `REQUEST_INSTALL_PACKAGES`: 允许安装 APK (:9)
    - `ForegroundService` 配置, `specialUse` 类型 (:46-51)
    - `BootReceiver` 默认 disabled (:54-60)
    - `FileProvider` 配置, authority `${applicationId}.fileprovider`, `grantUriPermissions="true"` (:63-70)
- `file_paths.xml`:
    - `<files-path name="update" path="update/" />`: 定义 update 目录为共享根 (:5)

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│  Frontend (Tauri WebView)                                           │
│  - listen: update-available, update-download-progress               │
│  - call: check_update, download_update, install_update              │
└──────────────────────────┬──────────────────────────────────────────┘
                           │ Tauri Command IPC
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Rust Layer (update_cmds.rs)                                        │
│  ┌──────────────┐   ┌──────────────┐   ┌─────────────────────────┐ │
│  │ check_update │   │download_upd  │   │ install_update          │ │
│  │ (HTTP/API)   │   │ (Stream+SHA) │   │ (Path Check)            │ │
│  └──────┬───────┘   └──────┬───────┘   └────────┬────────────────┘ │
│         │                  │                    │                   │
│         │                  │                    │ CampusMonitorSvc  │
│         │                  │                    │ .install_apk()    │
└─────────┼──────────────────┼────────────────────┼───────────────────┘
          │                  │                    │ JNI/Plugin Call
          ▼                  ▼                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Kotlin Layer (MonitorServicePlugin.kt)                             │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ installApk():                                                 │   │
│  │ 1. Resolve filesDir/update/{filename}                         │   │
│  │ 2. FileProvider.getUriForFile(activity, pkg+".fileprovider",f)│   │
│  │ 3. startActivity(ACTION_VIEW, content://uri, FLAG_GRANT_READ) │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Android System                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ FileProvider (authorities="${applicationId}.fileprovider")    │   │
│  │  - maps content:// to files/update/ inside app private dir    │   │
│  └──────────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ Package Installer (ACTION_VIEW application/vnd.android...)  │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **更新检查**:
   - 前端触发 `check_update` 或后台循环触发 `check_and_record`。
   - Rust 按优先级（镜像/官方）请求 `version.json`。
   - 解析 `VersionFile` (snake_case)，对比当前版本。
   - 若新版本，从 GitHub API 获取 `assets` 和 `sha256_checksum`。
   - Emit `update-available` 事件给前端。

2. **下载 APK**:
   - 前端调用 `download_update(url)`。
   - Rust 校验域名白名单，流式下载至 `app_data_dir/files/update/`。
   - 每 200ms emit `update-download-progress`。
   - 下载完成后，若存在 checksum，调用 `verify_file_sha256` 校验。
   - 成功返回文件路径。

3. **安装 APK**:
   - 前端调用 `install_update(file_path)`。
   - Rust 校验 `file_path` 规范化后必须以 `update_dir()` 开头（防路径逃逸）。
   - 调用 `CampusMonitorServiceExt::install_apk(path)`。
   - Kotlin `MonitorServicePlugin.installApk` 执行：
     - 从 `activity.filesDir/update/` 定位 APK（仅用文件名防逃逸）。
     - 调用 `FileProvider.getUriForFile` 生成 `content://` URI。
     - 发起 `Intent.ACTION_VIEW`，携带 `FLAG_GRANT_READ_URI_PERMISSION`。
   - 系统包安装器弹出确认界面。
   - Rust 删除本地 APK 文件（`tokio::fs::remove_file`）。

4. **残留清理**:
   - 启动时调用 `cleanup_stale_apks`。
   - 遍历 `update_dir`，删除 mtime 超过 24h 的 `.apk` 文件。

## Connections

[[android-file-uri-install-failure]] (历史教训，已在此模块实现中解决)
[[android-foreground-service-lifecycle]] (相关：前台服务管理)
[[tauri-plugin-architecture]] (相关：Tauri 插件通信机制)

## Known Issues

无已知未解决 issue。原 Android 7+ `file://` URI 限制已通过 `FileProvider` 方案完全解决。代码中包含对并发下载、栈溢出、路径遍历等边缘情况的防护。
```