---
title: android-log-type-enum-drift
type: learning
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/shared/ui-types.ts
tags: [教训, 安卓, 枚举, 契约]
---

## Overview
本文记录 Android 端后端日志事件 `type` 字段与前端 `LogType` 枚举的字符串字面量漂移问题。核心教训是：跨树（后端 → 前端）的事件 `type` 属于**字符串字面量契约**，后端发值必须落在前端枚举内，前端消费处需加兜底默认值，因为这类漂移不会触发 TS 编译错误（`as LogType` 断言会静默放行任意字符串）。

## Key Components

### 后端日志发射 (Android)
- **文件**: `android/src-tauri/src/monitor_loop.rs`
- **函数**: `emit_login_log(app: &tauri::AppHandle, message: &str, log_type: &str)` (约 L138-L141)
  - **用途**: 向 Tauri 事件总线发射 `login-log` 事件。
  - **Payload**: `{ "message": message, "type": log_type }`
  - **当前状态**: 所有调用点（30+ 处）均使用 `'info'`, `'success'`, `'error'`, `'warning'` 四个值。

### 前端类型定义 (Android)
- **文件**: `android/frontend/src/shared/ui-types.ts`
- **类型**: `LogType` (L4)
  - **定义**: `type LogType = 'info' | 'success' | 'error' | 'warning'`

### 前端事件监听 (Android)
- **文件**: `android/frontend/src/hooks/useEventListeners.ts`
- **处理逻辑**: `api.onLoginLog?.((data) => { ... })` (约 L137)
  - **转换**: `(data.type as LogType) || 'info'`
  - **局限性**: 仅兜住 falsy 值（undefined/空串），无法拦截 truthy 的错误字面量（如旧版的 `"warn"`）。

## Architecture

```text
┌──────────────────────────┐      ┌─────────────────────────────┐
│   android/src-tauri      │      │   android/frontend          │
│   monitor_loop.rs        │      │                             │
│                          │ emit │   useEventListeners.ts      │
│ emit_login_log(msg, type)│─────►│   onLoginLog(handler)       │
│ (e.g., "warning")        │      │   data.type as LogType      │
└──────────────────────────┘      └──────────────┬──────────────┘
                                                 │
                                                 ▼
                                         ┌─────────────────────┐
                                         │   ui-types.ts         │
                                         │   LogType enum        │
                                         │   (info|success|... ) │
                                         └─────────────────────┘
```

## Data Flow
1. **后端发射**: `monitor_loop.rs` 中的业务逻辑（如启动自动登录、定时注销等）调用 `emit_login_log`，携带 `log_type` 字符串。
2. **Tauri 传输**: 事件通过 Tauri IPC 发送至前端 WebView。
3. **前端接收**: `useEventListeners.ts` 中的 `onLoginLog` 回调接收到数据。
4. **类型断言**: 将 `data.type` 强制转换为 `LogType`。
5. **消费展示**: 日志存储组件根据 `LogType` 渲染图标和颜色。若类型不匹配且无兜底，可能导致 UI 崩溃或白屏。

## Known Issues
- **桌面端隐患**: 桌面端 (`tauri-app`) 尚未完全对齐此防护。`RightPanel.tsx` 中仍存在裸的 `LOG_ICONS[log.type]` 访问，若后端引入新 type 可能导致白屏。
- **类型安全缺失**: TypeScript 的 `as LogType` 断言允许任意字符串通过，缺乏编译期约束。

## Connections
[[tablet-layout-alignment-audit]]、[[android-white-screen-missing-init-fields]]