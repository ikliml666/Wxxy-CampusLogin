---
title: 安卓 tauri app_data_dir 与 filesDir 错位——应用内更新下载成功但安装报 APK 文件不存在
type: learning
source_files:
  - android/src-tauri/src/update_cmds.rs
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
tags: [安卓, tauri, 更新, 路径, FileProvider, 安装]
---

## 现象

v2.3.9（首个走通应用内下载链路的版本）真机实测：关于页「一键下载更新」下载进度走完、无任何报错，随后点击安装提示「**打开安装器失败，APK 文件不存在**」（`MonitorServicePlugin.kt:89` 的 `invoke.reject`，`${apk.name}` 显示的文件名却是对的）。

## 根因：两套目录假设错位

- **Rust 下载侧**：`app.path().app_data_dir().join("update")`。tauri Android 的 `app_data_dir` 经 JNI `getDataDir` 解析为 **`activity.dataDir`**（上游 `crates/tauri/mobile/android/src/main/java/app/tauri/PathPlugin.kt`，API 24+ 走 `activity.dataDir.absolutePath`）= `/data/user/0/<pkg>`——**不含 `/files` 段**。下载实际写到 `<dataDir>/update/xxx.apk`。
- **Kotlin 安装侧**（`MonitorServicePlugin.installApk`）：`File(activity.filesDir, "update")` = `/data/user/0/<pkg>/files/update/`——**多一层 `/files`**。
- 结果：下载真实存在，安装侧 `apk.exists()` 恒 false →「APK 文件不存在」。basename 从传入路径提取所以文件名显示正确，更具迷惑性。

**为什么 v2.3.9 才暴露**：应用内下载链路 v2.3.8 才第一次被用户触发，且当轮即被 [[android-async-command-large-future-stack-overflow]] 的栈溢出闪退打断——用户从未到达安装步骤，本 bug 潜伏至 v2.3.9。

## 修复

Rust 侧统一到 Kotlin/FileProvider 的约定：`app_data_dir()` 后拼 `files` 段——

```rust
app.path().app_data_dir()?.join("files").join("update")
```

`download_update_inner` 与 `install_update`（白名单校验目录）两处同步。Kotlin 不动：`files-path` 映射（`gen/android/app/src/main/res/xml/file_paths.xml` 的 `<files-path name="update" path="update/"/>`）恰好覆盖 `filesDir/update/`——若反向改 Kotlin 去用 `dataDir/update`，FileProvider 会因路径不在 files 映射内而抛异常，还得动 file_paths.xml，更差。

## 规则

**Android 端涉及文件落盘的路径，凡要与 Kotlin 插件/FileProvider 交互，统一以 `filesDir`（`app_data_dir().join("files")`）为基准，不要裸用 tauri 的 `app_data_dir`**——它是 dataDir，不是 filesDir。同类陷阱还可能在 cache（`app_cache_dir` = dataDir/cache，与 `context.cacheDir` 一致，这个倒无错位）。

## 取证方法

- 上游路径语义不确定时直接查 tauri 源码 `path/android.rs`（`call_resolve("getDataDir")`）+ 对应 Kotlin `PathPlugin.kt`（GitHub dev 分支）。
- 真机验证安装链路的最小闭环：release 包无 run-as/root 不可直接 ls 私有目录，但报错文案 + basename 正确即可锁定「存在性检查失败 = 目录错位」。
