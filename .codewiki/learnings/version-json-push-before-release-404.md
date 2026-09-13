---
title: version.json 推送后全员收到更新通知但下载 404
type: learning
source_files:
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/commands/updater.rs
tags: [教训, 发布, 更新, 流程]
---

## 现象

推送 `version.json` 后，全员客户端提示有更新，但点击下载 404。

## 根因

`version.json` 已推、Release 尚未发布（资产还不存在）——两步操作被拆开执行。

## 解决

版本号提交与 Release 发布**绑定为同一次操作**。

## 教训

发布流程中"通告类"文件（`version.json`）与"资产类"产物（Release）**必须原子完成**；顺序错一步就是全员可感知的故障。详见旧文档附录 H 的版本号流程。

## Connections

[[release-asset-integrity]]、[[version-single-source-build-rs]]、[[android-version-json-camelcase-parse-drift]]
