---
title: 更新提醒分端定制：桌面 WinRT 自写 toast + 安卓应用内弹窗
type: decision
source_files:
  - tauri-app/src-tauri/src/platform/toast.rs
  - tauri-app/src-tauri/src/update/updater.rs
  - android/frontend/src/auth/AboutDialogMobile.tsx
tags: [决策, 更新, 通知, 安卓, 桌面]
---

## 背景

需要"点击通知跳到关于界面"的体验；安卓提醒链路原本只有日志 + 版本角标。

## 决策

2026-09-12：更新提醒**分端定制**——

- 桌面：`windows crate` 自组 toast XML（AUMID 借 PowerShell 同款，与既有通知同源），支持点击跳转；失败降级普通通知；
- 安卓：补 `UpdateAvailableDialog` 应用内弹窗（双壳挂载）。

## 理由

`tauri-plugin-notification` 桌面端（notify-rust）**不暴露 Activated 回调**、Windows 无通知点击自定义能力，桌面"点击通知跳关于界面"只能经 windows crate 自组 toast XML。安卓通知无点击回传，因此改走应用内弹窗。

## 备选方案

统一走 `tauri-plugin-notification`（桌面点击跳转无法实现）；安卓沿用日志 + 版本角标（体验不足）——均被弃用。

## 影响与约束

桌面通知 AUMID 借用 PowerShell 的已注册 ID，应用改签名/打包方式或系统策略变化时通知可能静默不显示。更新提示只弹一次（`update/updater.rs:292` 的 `update_notified` 一次性门控，且**全仓库无重置点**）。

## Connections

[[system-notification-mascot-avatar]]、[[single-notification-channel]]、[[version-json-push-before-release-404]]
