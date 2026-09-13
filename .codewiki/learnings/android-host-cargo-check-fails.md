---
title: host cargo check 在 android/src-tauri 基线即失败
type: learning
source_files:
  - android/src-tauri/Cargo.toml
tags: [教训, 安卓, 构建, 验证]
---

## 现象

在 `android/src-tauri` 下执行 host `cargo check` / `cargo test` 在基线（未改代码）时就失败。

## 根因

mobile-only 插件的权限在 host 环境收集不全（相关实现都是 `#![cfg(mobile)]`）。

## 解决

安卓 Rust 改动只认交叉编译：`cargo check --target aarch64-linux-android --all-targets`（需注入 NDK 工具链环境变量），或一键出包 `pwsh android/build-apk.ps1`。

## 教训

不要用 host `cargo check` 的失败判断安卓改动有问题；先用基线确认它本来就失败。host 上跑测试需注入假桥。

## Connections

[[verification-baseline]]、[[tauri-android-build-report-path-missing]]
