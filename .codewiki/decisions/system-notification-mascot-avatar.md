---
title: 系统通知携带看板娘头像（2026-09-12 起）
type: decision
source_files:
  - tauri-app/src-tauri/src/infra/notification.rs
  - tauri-app/src-tauri/src/platform/toast.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/gen/android/app/src/main/res/drawable-xxhdpi/mascot_alert.png
  - android/src-tauri/gen/android/app/src/main/res/drawable-xxhdpi/mascot_offline.png
  - tauri-app/src-tauri/resources/mascot-toast/mascot-alert.png
  - tauri-app/src-tauri/resources/mascot-toast/mascot-busy.png
  - tauri-app/src-tauri/resources/mascot-toast/mascot-celebrate.png
  - tauri-app/src-tauri/resources/mascot-toast/mascot-offline.png
  - tauri-app/src-tauri/resources/mascot-toast/mascot-portrait.png
tags: [决策, 通知, 看板娘, winrt, 安卓]
---

## 背景

系统通知原本只有纯文本，与应用的看板娘形象不一致。

## 决策

`emit_notification(app, title, body, mascot)` 第 4 参为变体名（`mascot-alert` 等）。

- **Windows 桌面**优先走 `platform/toast.rs::show_system_toast` 自组 WinRT toast（appLogoOverride 圆形头像），失败降级插件纯文本；
- **安卓**该函数无图，安卓系统通知的娘大图由 `monitor_loop::notify_system` 的 `large_icon`（drawable 资源名）负责——传入值即资源名，用下划线形态（`mascot_alert` / `mascot_offline`），对应 PNG 放 `gen/android/app/src/main/res/drawable-xxhdpi/`（2026-09-13 补齐这两个，此前 `large_icon` 必抛 `Resources.NotFoundException`，每通知都白走一次"失败降级纯文本"）。

头像 PNG 经 `bundle.resources` 打包（`resources/mascot-toast/`，**仅 PNG——toast 不支持 webp**），运行时 `resource_dir()` 拼成 `file:///` URL。

## 理由

插件 notify-rust 在 Windows 不暴露图片参数，带不出娘，因此桌面必须自组 toast XML。

## 备选方案

旧文档未记录（仅记录"失败降级插件纯文本"这一降级路径）。

## 影响与约束

新增通知变体要同时准备 PNG 资源并放进 `resources/mascot-toast/`（桌面，连字符命名）与 `gen/android/app/src/main/res/drawable-xxhdpi/`（安卓，文件名即 `large_icon` 传入的资源名，下划线命名）；webp 不可用于 toast。资源缺失时无兜底（`ToastContainer` 拼 `/girl/mascot-${mascot}.webp` 无 `onError`）。

## Connections

[[single-notification-channel]]、[[update-notification-per-platform]]、[[mascot-asset-alpha-channel-check]]
