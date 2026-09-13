---
title: 安卓构建后 APK 没更新：tauri CLI 不跑 beforeBuildCommand
type: learning
source_files:
  - android/build-apk.ps1
  - android/src-tauri/tauri.conf.json
tags: [教训, 安卓, 构建, 前端]
---

## 现象

安卓打包完成后安装，发现界面/逻辑还是旧的。

## 根因

tauri CLI 不执行 `beforeBuildCommand`，`frontend/dist` 仍是上一次构建的产物。

## 解决

打包前**先手动** `npx vite build`。

## 教训

安卓出包流程里前端构建是独立一步，别指望 CLI 帮你跑；看到"改动没生效"先查 `dist` 时间戳。

## Connections

[[tauri-android-build-report-path-missing]]、[[android-host-cargo-check-fails]]
