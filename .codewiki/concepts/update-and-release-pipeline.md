---
type: concept
title: update-and-release-pipeline
source_files:
- android/src-tauri/src/update_cmds.rs
- docs/make-release.ps1
- docs/clean-build-cache.ps1
- android/build-apk.ps1
tags:
- release
- build
- update
---

## Overview

「update-and-release-pipeline」是系统实现自动化构建、产物分发及应用内更新能力的核心概念。该概念横跨 Android 前端外壳、Rust 后端逻辑以及文档基础设施模块，涵盖了从源码编译、签名打包到运行时版本检测与安装的完整闭环。

在 Android 端，该流程由 `android-tauri-shell-and-plugins` 负责底层 APK 的构建与签名，由 `android-rust-backend` 提供应用内的更新检查与安装能力。同时，`workspace-docs-and-build` 模块提供了标准化的 PowerShell 脚本入口，协调双端构建并生成发布产物。

## Involved Modules

*   **workspace-docs-and-build**: 提供主构建入口 `make-release.ps1` 和缓存清理入口 `clean-build-cache.ps1`，负责协调 Tauri 前端构建、产物定位及 APK 收集。
*   **android-rust-backend**: 提供应用内更新的核心 Rust 命令接口，包括版本检查、下载、安装及后台轮询。
*   **android-tauri-shell-and-plugins**: 提供 Android 专属的 APK 构建脚本 `build-apk.ps1` 及原生插件支持（如前台服务中的安装权限）。
*   **android-frontend-app-shell**: 作为前端入口，依赖后端 API 进行 Mock 或生产环境的更新状态交互，并在 UI 层展示相关面板。

## Flow

1.  **构建阶段 (Build Phase)**:
    *   开发者或 CI 触发 `docs/make-release.ps1` (行 ~38-122)。
    *   脚本执行 Tauri 前端构建，随后调用 `android/build-apk.ps1` (行 11-89) 进行 APK 打包与签名校验。
    *   若需清理环境，运行 `docs/clean-build-cache.ps1` (行 ~35-133)，保留 Local Storage 以维持登录态，但清除 WebView2 及其他缓存。

2.  **分发与发布 (Distribution Phase)**:
    *   生成的 APK 和 Windows Setup 产物被收集至 `target/` 或 `dist/` 目录。
    *   通过 GitHub Release 或内部渠道分发安装包。
    *   更新日志 (`changelogs/`) 和维护记录 (`release/RELEASE_NOTES_*.md`) 随文档资产归档。

3.  **应用内更新 (In-App Update Phase)**:
    *   **检查**: 前端通过 Tauri IPC 调用 `android-rust-backend` 的 `check_update` (L164)。后端查询 GitHub API 获取最新版本信息。
    *   **轮询**: 可选地启动 `start_update_check_loop` (L340) 进行后台定期检测。
    *   **下载**: 用户确认后，调用 `download_update` (L401) 将新版 APK 下载至本地临时目录。
    *   **安装**: 调用 `install_update` (L549)，利用 `MonitorServicePlugin` 提供的 `installApk` 命令触发系统安装界面。
    *   **原子性保障**: 整个过程中，后端使用互斥锁 (`DOWNLOAD_RUNNING`) 防止重复下载任务。

## Implementation Map

*   **Android 更新逻辑**: `android/src-tauri/src/update_cmds.rs`
    *   `check_update`: L164 - 发起版本比对。
    *   `download_update`: L401 - 处理二进制流下载。
    *   `install_update`: L549 - 触发 Android 包安装器。
    *   `start_update_check_loop`: L340 - 定时任务调度。
*   **桌面/通用构建脚本**: `docs/make-release.ps1`
    *   主构建流程: L38-L122 - 整合前后端构建步骤。
*   **Android 专属构建脚本**: `android/build-apk.ps1`
    *   APK 打包: L11-L89 - 调用 Gradle 构建并签名。
*   **缓存清理**: `docs/clean-build-cache.ps1`
    *   清理逻辑: L35-L133 - 安全删除构建中间文件。