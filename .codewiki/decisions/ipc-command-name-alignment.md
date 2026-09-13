---
title: 命令面两端同名对齐（新增命令三处同改）
type: decision
source_files:
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/tauriApi.ts
  - tauri-app/src-tauri/src/app/startup.rs
  - android/src-tauri/src/lib.rs
tags: [决策, ipc, 双端, 命令面]
---

## 背景

前端需要同时消费桌面端与安卓端的后端能力，两端命令若各自命名，前端就要按平台分叉。

## 决策

前端 `tauriApi` 一套接口两端消费；安卓命令与桌面同名同参（`do_login`/`save_config`…），差异只在实现。

新增命令时：桌面在 `app/startup.rs` 注册，安卓在 `android/src-tauri/src/lib.rs` 注册，并在 `hooks/tauriApi.ts` 加方法——共三处。

## 理由

旧文档未记录（"差异只在实现"是结果，未写权衡过程）。

## 备选方案

旧文档未记录。

## 影响与约束

"新增命令必须三处同改"是硬约束。当前规模：安卓 48 条（`android/src-tauri/src/lib.rs:55` 宏，条目 `:56-103`）、桌面 56 条（`app/startup.rs:63` 宏，条目 `:64-119`）；差值 = 平台专属命令面（仅单端注册，不进"三处同改"）。

**例外（平台专属能力）**：安卓电池优化白名单三命令 `get_battery_optimization_info` / `request_ignore_battery_optimizations` / `open_vendor_battery_settings`（`android/src-tauri/src/battery_cmds.rs`，桌面无对应 API）只在安卓 `lib.rs` 与安卓树 `tauriApi.ts` 各注册一处，不要求桌面同名占位。

注意前端 `TauriApi` 类型仍保留安卓恒 reject 的桌面方法（`android/frontend/src/hooks/tauriApi.ts:56-133` 的 interface，实现处走 `desktopOnly` 统一拒绝），调用方在编译期无法察觉，实际强度低于命名对齐的表面承诺。

## Connections

[[protocol-core-single-source]]、[[ipc-command-surface]]
