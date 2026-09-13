---
title: 通知单通道（系统通知与应用内提示分离）
type: decision
source_files:
  - tauri-app/src-tauri/src/infra/notification.rs
tags: [决策, 通知, 事件, 前端]
---

## 背景

同一件事（如自动登录结果）既有系统通知、又有应用内提示、还有日志，多种通道叠加会出现重复提醒。

## 决策

- 系统通知只有 `emit_notification`（窗口不可见才发）；
- 应用内提示走业务专用事件（`auto-login-result` / `login-log` / …）；
- `enable_notification` **只**控制系统通知。

新增提醒先想清楚走哪条通道，**不要双发**。

## 理由

旧文档未记录理由，只写了"不要双发"的规则；`emit_notification` 头部注释记录了"双通道重复是已根除的历史缺陷，不要在本函数内补 emit"（`notification.rs:11-14`）。

## 备选方案

旧文档未记录。

## 影响与约束

新增提醒必须先选通道。`emit_notification` 的 `mascot` 参数在非 Windows 桌面被丢弃（`notification.rs:57-58`），安卓看板娘大图由 `monitor_loop::notify_system` 的 `large_icon` 负责。

## Connections

[[system-notification-mascot-avatar]]、[[update-notification-per-platform]]
