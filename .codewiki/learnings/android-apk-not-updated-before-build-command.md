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

tauri CLI 不执行 `beforeBuildCommand`，`frontend/dist` 仍是上一次构建的产物。`tauri.conf.json` 的 `frontendDist` 指向 `../frontend/dist`（`android/src-tauri/tauri.conf.json:7`），该目录不会自动刷新。

## 解决

打包前**先手动** `npx vite build`。`build-apk.ps1` 已将其纳入一键流程：第 15 行注释标注"tauri CLI 不跑 beforeBuildCommand,必须单独构建"，第 17-19 行在 `$PSScriptRoot\frontend` 下执行 `npx vite build`（默认行为；可通过 `-SkipFrontend` 跳过，使用现有 dist）。交叉编译由第 28 行 `tauri.js android build --target aarch64 --apk` 完成。

## 教训

安卓出包流程里前端构建是独立一步，别指望 CLI 帮你跑；看到"改动没生效"先查 `dist` 时间戳。`build-apk.ps1` 已将手动构建封装进自动化脚本，常规出包无需额外操作。

## Connections

[[tauri-android-build-report-path-missing]]、[[android-host-cargo-check-fails]]
