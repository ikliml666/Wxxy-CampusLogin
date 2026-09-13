---
title: 版本号以 tauri.conf.json 为唯一权威源，build.rs 编译期注入
type: decision
source_files:
  - tauri-app/src-tauri/tauri.conf.json
  - tauri-app/src-tauri/build.rs
  - android/src-tauri/Cargo.toml
tags: [决策, 版本号, 构建, 发布]
---

## 背景

应用内多处引用版本号，手改必漏——旧文档明确记载"已发生事故"。

## 决策

`tauri.conf.json` 是唯一权威源，`build.rs` 编译期注入 `APP_VERSION`，代码里用 `env!("APP_VERSION")` 取值；升级按发布清单逐项走（含安卓端，勿只改桌面）；`version.json` 推送与 Release 发布必须同一次操作完成。

## 理由

旧文档记载的理由即"应用内多处版本号引用手改必漏（已发生事故）"。

## 备选方案

旧文档未记录。

## 影响与约束

升级版本号不是改一个文件：安卓端版本号另有独立来源（`env!("CARGO_PKG_VERSION")` 取自 `Cargo.toml`），当前两侧版本相同但需人工在多个文件同步，**无自动校验**。`version.json` 与 Release 必须同一次操作（否则全员收到更新通知但下载 404）。

## Connections

[[version-json-push-before-release-404]]、[[release-asset-integrity]]
