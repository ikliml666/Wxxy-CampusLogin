---
title: 安卓应用内更新与安装路径约定——app_data_dir vs filesDir
type: learning
source_files:
  - android/src-tauri/src/update_cmds.rs
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
tags: [安卓, tauri, 更新, 路径, FileProvider, 安装]
---

## Overview

本文档记录 Tauri Android 应用中，Rust 侧（`update_cmds.rs`）与 Kotlin 插件侧（`MonitorServicePlugin.kt`）在文件落盘与安装时的路径约定。核心发现是：Tauri 的 `app_data_dir()` 在 Android 上解析为 `dataDir`（`/data/user/0/<pkg>`），而 Android 的 `filesDir` 对应 `dataDir/files`。由于 `FileProvider` 的 XML 映射仅覆盖 `filesDir` 下的子目录，**所有涉及 APK 下载、安装及后续清理的路径，必须在 `app_data_dir()` 后显式拼接 `"files"` 段**，统一收敛至 `dataDir/files/update/`。

## Key Components

### Rust 侧：`android/src-tauri/src/update_cmds.rs`

**常量与全局状态**
- `VERSION_FILE`: 版本检查源 URL (L31)。
- `AUTO_CHECK_INTERVAL_SECS`: 自动检查周期，24h (L33)。
- `BACKOFF_RETRY_SECS`: 失败退避间隔数组 `[5min, 15min, 1h]` (L37)。
- `VERSION_MIRRORS`: GitHub 镜像降级源列表 (L46-49)。
- `DOWNLOAD_ALLOWED_HOSTS`: 下载域名白名单，用于防 SSRF (L52-60)。
- `MAX_DOWNLOAD_BYTES`: 下载大小上限，500MB (L62)。
- `APK_STALE_SECS`: 残留 APK 清理阈值，24h (L67)。
- `UPDATE_LOOP_RUNNING`: 原子标志，防止自动检查循环重复启动 (L25)。
- `UPDATE_CHECKSUM`: Mutex 保护的最近一次校验值，供下载后比对 (L28)。
- `LAST_CHECK_ERROR`: 最近一次检查失败原因快照 (L40)。
- `LAST_CHECK_TIME_MS`: 最近一次检查完成时间戳 (L43)。
- `DOWNLOAD_RUNNING`: 原子标志，防止并发下载互写 (L245)。

**数据结构**
- `ReleaseAsset`: L69-73。字段：`name`, `url`, `size`。
- `UpdateInfo`: L76-90。字段：`has_update`, `latest_version`, `release_notes`, `assets`, `sha256_checksum`, `last_check_error`, `last_check_time`。
- `DownloadProgress`: L93-97。字段：`downloaded`, `total`, `speed`, `percent`。
- `VersionFile`: L152-156。`snake_case` 结构体，字段：`version`, `notes`。用于解析线上真实的 version.json 格式，避免被 `UpdateInfo` 的 `camelCase` 反序列化误导。

**关键函数**
- `update_dir(app)`: L70-74。**核心路径函数**。返回 `app.path().app_data_dir()?.join("files").join("update")`。注释明确指出必须拼 `files` 段以对齐 Kotlin/FileProvider 约定。
- `has_newer_version(current, latest)`: L100-116。语义对齐桌面端，去 `v` 前缀后逐段数值比较。
- `http_client()`: L119-128。构建 `reqwest::Client`，设置 `connect_timeout=5s` 和 `timeout=30s`，使用 `rustls`。
- `allowed_url(url)`: L131-148。白名单校验漏斗，提取 host 并与 `DOWNLOAD_ALLOWED_HOSTS` 比对。
- `check_update(app)`: L151-153。Tauri 命令入口，委托给 `check_and_record`。
- `backoff_delay_secs(fail_count)`: L173-180。根据连续失败次数计算等待秒数，超过 3 次重置为 24h。
- `check_and_record(app)`: L183-195。执行检查并记录错误/时间戳到全局变量。
- `version_urls(mirror_first)`: L198-204。根据用户设置决定镜像优先还是官方优先。
- `fetch_apk_assets(client, latest)`: L213-248。从 GitHub API 拉取 APK 资产列表和 `sha256` digest。
- `check_update_inner(app)`: L251-287。遍历 URL 源，解析 `VersionFile`，组装 `UpdateInfo`。
- `start_update_check_loop(app)`: L290-328。后台循环：启动延迟 5s，之后每 24h 或按退避间隔重试。有新版本时 emit `update-available` 并触发系统通知。
- `get_mirror_urls(github_url)`: L331-338。Tauri 命令，返回前端可用的镜像 URL 列表。
- `download_update(app, url)`: L341-347。Tauri 命令入口，处理并发互斥锁。
- `download_update_inner(app, url)`: L350-412。流式下载 APK。
    - **路径约定**：调用 `update_dir(&app)?` 获取目标目录 (L375)。
    - **进度上报**：每 200ms emit `update-download-progress` (L398-406)。
    - **完整性校验**：下载完成后调用 `verify_file_sha256` 比对 `UPDATE_CHECKSUM` (L409-415)。
- `verify_file_sha256(path, expected)`: L419-444。异步计算 SHA256。使用堆分配 buffer (`vec![0u8; 65536]`) 避免栈溢出 (L436)。
- `install_update(app, file_path)`: L448-468。Tauri 命令入口。
    - **安全校验**：验证 `file_path` 规范后位于 `update_dir` 下 (L456-460)。
    - **安装执行**：调用 `CampusMonitorServiceExt.install_apk` (L463)。
    - **清理**：**注意：此处不再立即删除本地 APK 文件**，而是交由启动期的 `cleanup_stale_apks` 清理超龄残留 (L467)。
- `cleanup_stale_apks(app)`: L471-496。清理超过 `APK_STALE_SECS` 的残留 APK。只删 `.apk` 且 mtime 超龄的文件。

**测试模块**
- `tests` 模块 (L500-600)：包含版本比较、白名单校验、`UpdateInfo` 反序列化、安装目录约定 (`files/update`)、残留清理判定、退避间隔、`version_json` 解析等单元测试。

### Kotlin 侧：`android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt`

**类与接口**
- `MonitorServicePlugin`: L28-270。Tauri 插件类，继承自 `Plugin`。
- `StartArgs`, `TextArgs`, `BoolArgs`: L16-26。Tauri 命令参数 DTO。
- `BootReceiver`: L273-292。开机广播接收器，若开启自启则拉起前台服务并尝试启动 MainActivity。

**关键方法**
- `installApk(invoke)`: L86-103。**核心安装方法**。
    - **路径约定**：`java.io.File(activity.filesDir, "update")` (L88)。直接读取 `filesDir` 下的 `update` 子目录。
    - **文件名过滤**：`java.io.File(args.text).name` 提取 basename，防止路径逃逸 (L89)。
    - **URI 转换**：使用 `FileProvider.getUriForFile` 生成 `content://` URI (L92-94)。
    - **Intent 启动**：`ACTION_VIEW` + `FLAG_GRANT_READ_URI_PERMISSION` (L95-98)。
- `startMonitor(invoke)`: L53-64。启动前台服务。
- `stopMonitor(invoke)`: L67-75。停止前台服务。
- `updateNotification(invoke)`: L78-84。更新通知内容。
- `beginProbeWindow(invoke)` / `endProbeWindow(invoke)`: L107-116。探针窗口管理，持有 WifiLock 和唤醒锁。
- `getPowerState(invoke)`: L126-134。获取屏幕状态和 WiFi 连接状态。
- `isWifiConnected()`: L137-146。实时查询当前活动网络是否为 WiFi。
- `vendorTargets`: L153-192。厂商自启页候选组件映射表。
- `buildVendorIntent(target)`: L195-199。构建带包名 extra 的厂商 Intent。
- `startSafely(intent)`: L201-205。安全启动 Activity 辅助方法。
- `getBatteryOptimizationInfo(invoke)`: L208-219。获取电池优化状态和品牌信息。
- `requestIgnoreBatteryOptimizations(invoke)`: L229-237。申请电池优化白名单。
- `openVendorBatterySettings(invoke)`: L240-278。跳转厂商自启/省电页，含降级链。
- `openNotificationSettings(invoke)`: L281-312。跳转通知设置页，含降级链。
- `setBootAutostart(invoke)` / `isBootAutostartEnabled(invoke)`: L315-338。开机自启开关管理。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      Tauri App Handle                       │
│                                                             │
│  ┌─────────────────────────┐    ┌────────────────────────┐  │
│  │   Rust: update_cmds.rs  │    │  Kotlin: MonitorSvcPlg │  │
│  │                         │    │                        │  │
│  │ check_update()          │    │ installApk(invoke)     │  │
│  │ download_update()       │    │   ├─ activity.filesDir │  │
│  │ install_update()        │    │   ├─ "files/update"    │  │
│  │ cleanup_stale_apks()    │    │   └─ FileProvider      │  │
│  │                         │    │                        │  │
│  │ update_dir() ◄──────────┼────┤ (路径约定对齐点)       │  │
│  │   app_data_dir()        │    │                        │  │
│  │   .join("files")        │    │                        │  │
│  │   .join("update")       │    │                        │  │
│  └──────────┬──────────────┘    └────────────────────────┘  │
│             │                                                │
│             ▼                                                │
│  ┌────────────────────────────────────────────────────────┐ │
│  │              Android Data Directory                    │ │
│  │  /data/user/0/<pkg>/                                   │ │
│  │  ├── data/                                              │ │
│  │  ├── files/                                             │ │
│  │  │   └── update/                                        │ │
│  │  │       └── campus-login-x.x.x.apk  ◄── 实际落盘位置   │ │
│  │  └── ...                                                │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **检查更新**：
    - `check_update` 触发 → `check_and_record` → `check_update_inner`。
    - 按优先级顺序请求 `version.json` (官方源或镜像)。
    - 成功解析后，并行通过 GitHub API 拉取 `assets` 和 `sha256` digest。
    - 将结果存入 `UpdateInfo`，其中 `sha256_checksum` 存入 `UPDATE_CHECKSUM` 全局变量。
    - 若有新版本，emit `update-available` 事件并显示系统通知。

2.  **下载 APK**：
    - 前端触发 `download_update` → `download_update_inner`。
    - 校验 URL 白名单 → 构建 HTTP 客户端 → 流式下载。
    - 目标路径由 `update_dir(&app)` 确定：`app_data_dir().join("files").join("update")`。
    - 写入过程中每 200ms emit 进度事件。
    - 下载完成后，读取 `UPDATE_CHECKSUM` 进行 SHA256 校验。
    - 校验失败则删除文件并报错；成功则返回文件路径。

3.  **安装 APK**：
    - 前端触发 `install_update(file_path)`。
    - Rust 侧校验 `file_path` 是否位于 `update_dir` 规范目录下。
    - 调用 `CampusMonitorServiceExt.install_apk(canonical_path)`。
    - **Kotlin 侧 `installApk`**：
        - 从 `activity.filesDir` 拼接 `"update"` 目录。
        - 从传入路径提取 basename，构造 `File(filesDir, basename)`。
        - 通过 `FileProvider` 将绝对路径转为 `content://` URI。
        - 启动 `ACTION_VIEW` Intent 唤起系统包安装器。
    - **注意**：Rust 侧在 Intent 拉起后，**不再立即删除**本地 APK 文件，以避免竞态条件导致安装失败。清理工作交由启动期的 `cleanup_stale_apks` 处理。

4.  **清理残留**：
    - 启动时或特定触发点调用 `cleanup_stale_apks`。
    - 遍历 `update_dir`，查找 mtime 超过 `APK_STALE_SECS` (24h) 的 `.apk` 文件并删除。

## Connections

- [[android-app-data-dir-vs-filesdir-mismatch]] (历史遗留标题，内容已同步至此文档)
- [[android-async-command-large-future-stack-overflow]] (相关：`verify_file_sha256` 中堆分配 buffer 的栈溢出修复)