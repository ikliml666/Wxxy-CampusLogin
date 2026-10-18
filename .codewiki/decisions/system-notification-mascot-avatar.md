---
title: 系统通知携带看板娘头像（2026-09-12 起）
type: decision
source_files:
  - tauri-app/src-tauri/src/infra/notification.rs
  - tauri-app/src-tauri/src/platform/toast.rs
tags: [决策, 通知, 看板娘, winrt]
---

## Overview

本决策记录系统通知携带"看板娘"头像的跨平台实现方案。桌面端（Windows）通过自组 WinRT Toast XML 实现 `appLogoOverride` 圆形头像；安卓端由独立模块补充系统通知能力。核心入口为 `tauri-app/src-tauri/src/infra/notification.rs::emit_notification`，该函数负责抑制判定与分发，Windows 端调用 `platform/toast.rs` 发送带图 Toast，非 Windows 端降级为插件纯文本通知。

## Key Components

### 桌面端 (Windows)

**文件**: `tauri-app/src-tauri/src/infra/notification.rs`
- **`pub fn emit_notification(app_handle: &AppHandle, title: &str, body: &str, mascot: &str)`** (`ln 24–73`)
    - 系统通知唯一入口。
    - 检查 `enable_notification` 配置。
    - **抑制判定**: 若窗口可见且未最小化 (`is_visible && !is_minimized`)，视为用户可见，不弹系统通知。
    - Windows 优先调用 `show_system_toast`，失败降级插件纯文本通知。
    - 非 Windows 直接调用插件纯文本通知。

**文件**: `tauri-app/src-tauri/src/platform/toast.rs`
- **常量**:
    - `AUMID: &str = "com.campus.login"` (`ln 35`): 自有 AUMID，用于注册表。
    - `FALLBACK_APP_ID: &str` (`ln 37–38`): 兜底 AUMID (PowerShell)。
    - `APP_DISPLAY_NAME: &str = "校园网登录助手"` (`ln 39`).
    - `AUMID_ICON_RESOURCE: &str = "icons/128x128.png"` (`ln 41`): 来源角标图标。
- **`fn notifier_aumid(app_handle: &AppHandle) -> &'static str`** (`ln 46–56`): 缓存注册结果，返回实际可用的 AUMID。
- **`fn register_aumid(app_handle: &AppHandle) -> Result<(), String>`** (`ln 59–78`): 向 `HKCU\Software\Classes\AppUserModelId\<AUMID>` 写 DisplayName 与 IconUri。
- **`fn resolve_resource_image(app_handle: &AppHandle, relative: &str) -> Option<String>`** (`ln 81–88`): 解析打包资源为 `file:///` URL。
- **`pub fn show_system_toast(app_handle: &AppHandle, title: &str, body: &str, mascot: &str) -> Result<(), String>`** (`ln 91–107`):
    - 构造带 `appLogoOverride` 圆形头像的 Toast XML。
    - 头像路径: `resources/mascot-toast/{mascot}.png`。
    - 使用 `ToastNotificationManager::CreateToastNotifierWithId` 发送。
- **`pub fn show_update_toast(app_handle: &AppHandle, version: &str) -> Result<(), String>`** (`ln 110–148`):
    - 发送"发现新版本" toast，点击回调唤起主窗口并触发前端事件。
    - 头像路径: `resources/mascot-update-toast.png`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      emit_notification                       │
│              (tauri-app/infra/notification.rs)               │
└──────────┬──────────────────────────────────┬───────────────┘
           │                                  │
           ▼                                  ▼
   ┌─────────────────┐                ┌─────────────────┐
   │   Windows       │                │   Non-Windows   │
   │                 │                │                 │
   │ show_system_toast│                │ Plugin Notify   │
   │(platform/toast.rs)│               │ (Pure Text)     │
   └────────┬────────┘                └─────────────────┘
            │
            ▼
   ┌─────────────────┐
   │ WinRT Toast XML │
   │ + AppLogoOverride│
   │ (Circle Avatar) │
   └─────────────────┘
```

## Data Flow

1. **业务触发**: 后台监控、启动自动登录等场景检测到关键状态变化（掉线、重连失败、新版本等）。
2. **构建参数**: 准备标题、正文、以及看板娘变体名（如 `mascot-alert`, `mascot-offline`）。
3. **分发**:
    - **Windows**: `emit_notification` → `show_system_toast` → 查找 `resources/mascot-toast/{mascot}.png` → 转为 `file:///` URL → 嵌入 XML `<image placement="appLogoOverride">` → WinRT API 发送。
    - **Android**: 由独立模块处理系统通知大图逻辑（源码中未提供详细代码片段，但架构上对应 Android 侧的通知实现）。

## Connections

- [[app-window-management]]