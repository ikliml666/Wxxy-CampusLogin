---
title: 安卓生成工程纪律（gen/android 入库、identifier 破坏性、签名不可更换）
type: decision
source_files:
  - android/src-tauri/gen/android/app/build.gradle.kts
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
  - android/plugins/foreground-service/permissions/default.toml
  - android/src-tauri/gen/schemas/acl-manifests.json
tags: [决策, 安卓, gradle, 签名, 包名]
---

## 背景

Tauri 的安卓工程由 CLI 生成（`gen/android/`），生成物与手改内容混在一起，且包名/签名证书直接决定用户侧数据能否延续。当前仓库中 `build.gradle.kts` 与 `AndroidManifest.xml` 均位于 `gen/android/` 下并被版本化，同时手写插件依赖与权限声明也在其中兜底。

## 当前代码快照

### build.gradle.kts（`android/src-tauri/gen/android/app/build.gradle.kts`）

- **插件**：`com.android.application`、`org.jetbrains.kotlin.android`、`rust`（第 3–7 行）。
- **namespace / applicationId**：均为 `com.campuslogin.client`（第 18、21 行）。
- **SDK 版本**：compileSdk=36、targetSdk=36、minSdk=29（第 17、22–23 行）。
- **版本号**：从 `tauri.properties` 读取 `tauri.android.versionCode` / `tauri.android.versionName`，默认 1 / 1.0（第 24–25 行）。
- **签名配置**（第 29–36 行）：release 使用本机 `~/.android/debug.keystore`，别名 `androiddebugkey`，密码 `android`——注释说明“个人分发”策略，构建产出即发布物，且“签名证书一经发布不可更换”（第 27–28 行注释）。
- **产物命名**（第 37–44 行）：`applicationVariants.all` 块将 APK 文件名直接改为 `Wxxy-CampusLogin_${versionName}.apk`，与桌面安装包命名同源，不再依赖构建后改名。
- **buildTypes**（第 45–66 行）：
  - debug（第 46–56 行）：`usesCleartextTraffic=true`（第 47 行）、可调试/JNI 调试（第 48–49 行）、不缩小（第 50 行）；`packaging` 块保留所有架构（arm64-v8a、armeabi-v7a、x86、x86_64）的 `.so` 调试符号（第 51–54 行）。
  - release（第 57–65 行）：启用 minify（第 58 行），使用 `signingConfigs.getByName("release")`（第 59 行），proguard 从项目目录收集 `**/*.pro` 并附加默认 `proguard-android-optimize.txt`（第 60–64 行）。
- **kotlinOptions / buildFeatures**（第 67–72 行）：`jvmTarget = "1.8"`（第 68 行），`buildConfig = true`（第 71 行）。
- **Rust 集成**（第 75–77 行）：`rust { rootDirRel = "../../../" }`；文件末尾 `apply(from = "tauri.build.gradle.kts")`（第 97 行）。
- **依赖**（第 79–95 行）：
  - WebView：`androidx.webkit:webkit:1.14.0`（第 80 行）
  - AppCompat：`androidx.appcompat:appcompat:1.7.1`（第 81 行）
  - Activity KTX：`androidx.activity:activity-ktx:1.10.1`（第 82 行）
  - Material：`com.google.android.material:material:1.12.0`（第 83 行）
  - Lifecycle process：`androidx.lifecycle:lifecycle-process:2.10.0`（第 84 行）
  - 手写插件模块（project 依赖）：`:tauri-plugin-campus-network-bind`、`:tauri-plugin-campus-keystore`、`:tauri-plugin-campus-monitor-service`、`:tauri-plugin-biometric`、`:tauri-plugin-opener`（第 87–91 行），注释说明与 `settings.gradle` 持久 include 配套，CLI 重生成可能丢失故在此兜底（第 85–86 行）。
  - 测试依赖：`junit:junit:4.13.2`（第 92 行）、`androidx.test.ext:junit:1.1.4`（第 93 行）、`espresso-core:3.5.0`（第 94 行）。

### AndroidManifest.xml（`android/src-tauri/gen/android/app/src/main/AndroidManifest.xml`）

- **权限声明**（第 3–18 行）：
  - `INTERNET`（第 3 行）、`POST_NOTIFICATIONS`（第 4 行）、`FOREGROUND_SERVICE`（第 5 行）、`FOREGROUND_SERVICE_SPECIAL_USE`（第 6 行）、`RECEIVE_BOOT_COMPLETED`（第 7 行）、`WAKE_LOCK`（第 8 行）
  - `REQUEST_INSTALL_PACKAGES`（第 10 行，APK 更新安装，用户确认）
  - `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`（第 12 行，电池优化白名单，需前端引导确认）
  - `CAMERA`（第 14 行，2D 人脸验证）
  - `WRITE_SECURE_SETTINGS`（第 16–18 行，带 `tools:ignore="ProtectedPermissions"`，仅 adb grant 生效）
- **AndroidTV 特性**（第 21 行）：`android.software.leanback` 非必需。
- **应用配置**（第 23–27 行）：`icon`、`label`、`theme="@style/Theme.campus_login_android"`（第 26 行），`usesCleartextTraffic="${usesCleartextTraffic}"`（第 27 行）。
- **MainActivity**（第 28–40 行）：`.MainActivity`、`singleTask` launchMode（第 30 行）、`exported="true"`（第 33 行），intent-filter 包含 `LAUNCHER` 与 `LEANBACK_LAUNCHER`（第 36–38 行）。
- **ForegroundService**（第 42–50 行）：`com.campuslogin.plugin.monitorservice.ForegroundService`，`exported="false"`，`foregroundServiceType="specialUse"`（第 45–46 行），property `android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE=campus-network-monitor-keepalive`（第 47–49 行）。
- **BootReceiver**（第 52–60 行）：`com.campuslogin.plugin.monitorservice.BootReceiver`，默认 `enabled="false"`（第 55 行），监听 `BOOT_COMPLETED`（第 58 行）。
- **FileProvider**（第 62–70 行）：`androidx.core.content.FileProvider`，authority 为 `${applicationId}.fileprovider`（第 64 行），resource 引用 `@xml/file_paths`（第 69 行）。

### foreground-service 权限清单（`android/plugins/foreground-service/permissions/default.toml`）

`[default]` 描述校园网后台监控保活（第 1–5 行）。`permissions` 数组列出 12 个 Rust 命令暴露给 JS（第 7–20 行）：
`allow-startMonitor`（第 8 行）、`allow-stopMonitor`（第 9 行）、`allow-updateNotification`（第 10 行）、`allow-setBootAutostart`（第 11 行）、`allow-isBootAutostartEnabled`（第 12 行）、`allow-installApk`（第 13 行）、`allow-beginProbeWindow`（第 14 行）、`allow-endProbeWindow`（第 15 行）、`allow-getPowerState`（第 16 行）、`allow-getBatteryOptimizationInfo`（第 17 行）、`allow-requestIgnoreBatteryOptimizations`（第 18 行）、`allow-openVendorBatterySettings`（第 19 行）。

### acl-manifests.json（`android/src-tauri/gen/schemas/acl-manifests.json`）

单行 JSON（第 1 行），包含 16 个 plugin manifest key：
`biometric`、`campus-keystore`、`campus-monitor-service`、`campus-network-bind`、`core`、`core:app`、`core:event`、`core:image`、`core:menu`、`core:path`、`core:resources`、`core:tray`、`core:webview`、`core:window`、`notification`、`opener`。

各插件默认权限与命令面要点：
- `campus-monitor-service`：12 个 default permission（与 default.toml 完全对应），覆盖前台服务控制、开机自启、APK 安装、电池优化、探针窗口等全部命令。
- `campus-network-bind`：7 个 default permission（`allow-bind-to-wifi`、`allow-unbind`、`allow-accept-wifi-network`、`allow-ensure-avoid-bad-wifi`、`allow-restore-avoid-bad-wifi`、`allow-restore-written-settings`、`allow-get-secure-settings-status`），含 WiFi 绑定/解绑、坏网规避设置读写、安全设置状态查询。
- `campus-keystore`：2 个 default permission（`allow-encrypt`、`allow-decrypt`），对应 AndroidKeyStore AES-GCM。
- `biometric`：2 个 default permission（`allow-authenticate`、`allow-status`）。
- `notification`：16 个 default permission（完整通知管理：`is-permission-granted`、`request-permission`、`notify`、`register-action-types`、`register-listener`、`cancel`、`get-pending`、`remove-active`、`get-active`、`check-permissions`、`show`、`batch`、`list-channels`、`delete-channel`、`create-channel`、`permission-state`）。
- `opener`：3 个 default permission（`open-url`、`reveal-item-in-dir`、`default-urls`）。
- Tauri core 模块（`core` 及 `core:*`）各自携带其标准命令集（如 `core:window` 含 `get-all-windows`、`scale-factor` 等大量命令）。

## 决策

- `gen/android/` **版本化提交**，但 build 产物不入库；
- `settings.gradle` 插件声明为**手改**（会被构建覆盖，需还原），与 `build.gradle.kts` 第 85–91 行兜底声明配套；
- 构建再生的 `\n`→`\r\n` 行尾差异是噪音，提交前 `git checkout --` 还原；
- **改 identifier（包名）= 用户卸载重装**（Keystore 密钥按包名隔离），必须同步 gradle `namespace`（第 18 行） / `applicationId`（第 21 行） / `MainActivity` 包路径三处并 `git mv` Kotlin 目录；
- **签名证书一经发布不可更换**（见 `build.gradle.kts` 第 27–28 行注释），release 固定使用本机 `debug.keystore`（第 29–36 行）；
- **插件命令面权限三件套必须同提交**：新增插件命令要同步改 `android/plugins/<插件>/permissions/default.toml`（如第 7–20 行）、`permissions/autogenerated/reference.md`、`permissions/schemas/schema.json`，并带上 `android/src-tauri/gen/schemas/` 的 `acl-manifests.json`（第 1 行）等生成物——本仓 tauri build.rs 不为新插件命令生成 per-command toml，生成物就是这三类，漏带即命令被 ACL 拒绝。

## 理由

旧文档记载包名破坏性的依据是“Keystore 密钥按包名隔离”；当前代码仍印证该约束：`namespace` 与 `applicationId` 均为 `com.campuslogin.client`（第 18、21 行），且 `FileProvider` authority 依赖 `${applicationId}`（第 64 行）。签名不可更换的理由：更换证书会导致已安装应用的更新覆盖失败（Android 要求同签名），用户必须卸载重装，且 `build.gradle.kts` 第 27–28 行注释明确“签名证书一经发布不可更换(换证书=用户必须卸载重装)”。

## 备选方案

无替代方案记录。可考虑使用独立 release keystore 并走 zipalign/apksigner 后处理，但当前选择“构建产出即发布物”策略以降低分发复杂度。

## 影响与约束

任何触碰 identifier 的改动都要按破坏性发布对待；FileProvider authority `${applicationId}.fileprovider`（第 64 行）与 `file_paths.xml` 也必须随之确认。`gen/android/.../AndroidManifest.xml` 属版本化的手改点：新增系统权限必须落在这里（本轮电池白名单补 `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` 见第 12 行），与 Kotlin 侧调用成对，缺一则运行时静默失败或抛异常。手写插件依赖在 `build.gradle.kts` 第 87–91 行兜底，若 `settings.gradle` 丢失 include 则编译失败。

## Connections

[[android-identifier-change-breaks-keystore]]、[[gen-schemas-crlf-diff-noise]]、[[release-asset-integrity]]
