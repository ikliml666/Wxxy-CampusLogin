---
title: 三份 vendored tauri-api 需人工跟进上游
type: learning
source_files:
  - android/plugins/keystore/android/.tauri/tauri-api/src/main/java/app/tauri/plugin/Plugin.kt
  - android/plugins/network-bind/android/.tauri/tauri-api/src/main/java/app/tauri/plugin/Plugin.kt
  - android/plugins/foreground-service/android/.tauri/tauri-api/src/main/java/app/tauri/plugin/Plugin.kt
tags: [教训, 安卓, 插件, tauri, 升级]
---

## 现象

升级 tauri-api 时改了一份，其他插件不受影响。

## 根因

每个插件目录下各有一份独立的 `tauri-api` 源码副本（由 `build.rs` 的 `android_path("android")` 引入，`android/build.gradle.kts` 用 `implementation(project(":tauri-android"))`），三份内容独立、共约 28 个 Kotlin 文件；插件基类命令（`registerListener`/`removeListener`）也来自这里。

## 解决

升级时**三份同步**（人工）。

## 教训

插件目录里的 vendored 副本不是符号链接也不是单一来源，**升级要按"三份"核对**；同类问题还有三个插件都是 `#![cfg(mobile)]`（host 测试环境拿不到实现，host 下加解密恒失败、测试需注入假桥）。

## Connections

[[build-rs-docsrs-masks-android-build-failure]]、[[plugin-permission-dangling-refs]]
