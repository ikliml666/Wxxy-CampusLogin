---
title: host cargo check 在 android/src-tauri 基线即失败
type: learning
source_files:
  - android/src-tauri/Cargo.toml
  - android/src-tauri/src/lib.rs
tags: [教训, 安卓, 构建, 验证]
---

## 现象

在 `android/src-tauri` 下执行 host `cargo check` / `cargo test` 在基线（未改代码）时就失败。

## 根因

安卓专属 crate（`campus-login-android`）通过 path 依赖桌面协议核心 `campus-login`（`Cargo.toml:34`），且其 `src/lib.rs` 中大量使用 `#![cfg(mobile)]` 门控：`lib.rs:33-39` 的 `#[cfg(mobile)]` 块注册网络绑定、Keystore、前台服务、生物识别与通知插件；`lib.rs:41-42` 的 `#[cfg(not(mobile))]` 分支仅 manage state 不注册任何插件。host 编译时走 `not(mobile)` 路径，缺少这些插件初始化会导致链接失败或命令面不完整——因此基线就无法通过 host cargo check。

此外 `Cargo.toml:28-31` 的 `[target.'cfg(target_os = "android")'.dependencies]` 段声明了仅 android 目标编译的依赖（`android_system_properties`、`libc`），host 环境同样收集不到这些符号。

## 解决

安卓 Rust 改动只认交叉编译：`cargo check --target aarch64-linux-android --all-targets`（需注入 NDK 工具链环境变量），或一键出包 `pwsh android/build-apk.ps1`。

## 教训

不要用 host `cargo check` 的失败判断安卓改动有问题；先用基线确认它本来就失败。host 上跑测试需注入假桥。

## Connections

[[verification-baseline]]、[[tauri-android-build-report-path-missing]]
