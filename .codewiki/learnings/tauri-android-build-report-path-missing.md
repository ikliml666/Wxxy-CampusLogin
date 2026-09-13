---
title: tauri android build 完成报告指向的 APK 文件不存在
type: learning
source_files:
  - android/src-tauri/gen/android/app/build.gradle.kts
tags: [教训, 安卓, 构建, gradle]
---

## 现象

`tauri android build` 完成后提示产物是 `app-universal-release.apk`，但该文件不存在。

## 根因

产物名与签名已内置在 gradle 配置里，CLI 的报告路径与实际产物不一致。

## 解决

以**输出目录里的实际文件**为准。

## 教训

打包后不要照抄 CLI 报告的文件名，去输出目录核实；发布资产命名以 gradle 为准。

## Connections

[[android-apk-not-updated-before-build-command]]、[[release-asset-integrity]]
