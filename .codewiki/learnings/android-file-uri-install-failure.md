---
title: 应用内下载 APK 后安装失败（Android 7+，file:// URI 被禁）
type: learning
source_files:
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
  - android/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/src-tauri/src/update_cmds.rs
tags: [教训, 安卓, apk 安装, fileprovider]
---

## 现象

应用内下载 APK 后安装失败（Android 7+）。

## 根因

应用私有目录的 `file://` URI 必失败（`FileUriExposedException`）。

## 解决

改用 FileProvider `content://` URI + `FLAG_GRANT_READ_URI_PERMISSION`（`MonitorServicePlugin.kt:92-99`）。

## 教训

跨应用共享文件一律走 FileProvider。约束：authority `${applicationId}.fileprovider` 在 manifest（`AndroidManifest.xml:60-68`，authority 在 :62）与 Kotlin 侧（`MonitorServicePlugin.kt:93`，用 `${activity.packageName}.fileprovider`）必须一致，改 applicationId 后需同时确认两处与 `file_paths.xml`（`files-path name="update"`，:5）；安装路径还有双重限定——Rust 侧 `canonicalize` 后校验必须以 `app_data_dir/update/` 开头（`update_cmds.rs:405-419`，:413 规范化、:415-417 前缀校验），Kotlin 侧用 `File(...).name` 重拼 `filesDir/update/`（`MonitorServicePlugin.kt:86-87`，只取文件名防逃逸）。

## Connections

[[android-identifier-change-breaks-keystore]]、[[release-asset-integrity]]
