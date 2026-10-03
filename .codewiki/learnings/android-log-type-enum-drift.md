---
title: 跨树日志事件 type 必须对齐前端 LogType 枚举（"warn" vs "warning" 致白屏）
type: learning
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/hooks/useEventListeners.ts
  - android/frontend/src/hooks/useLogToastStore.ts
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/shared/ui-types.ts
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/components/layout/RightPanel.tsx
tags: [教训, 安卓, 双端同步, 枚举, 白屏]
---

## 现象

安卓端白屏——启动自动登录判定失败可触发。

## 根因

安卓后端 `emit_login_log`（`android/src-tauri/src/monitor_loop.rs:152`，emit "login-log" 在 :155）发的事件 `type` 是 `"warn"`，而前端 `LogType` 枚举只有 `'info' | 'success' | 'error' | 'warning'`（`android/frontend/src/shared/ui-types.ts:4`）；`LOG_ICONS[type]` 取到 `undefined` 直接白屏。

## 解决

后端已改为 `'warning'`（现：`android/src-tauri/src/monitor_loop.rs:516` "启动自动登录:校园网判定失败"）。当前 `monitor_loop.rs` 全部 30+ 处 `emit_login_log` 调用只落 `info`/`success`/`error`/`warning` 四值，均在前端枚举内。

前端消费侧加三层兜底防再犯：

1. `android/frontend/src/hooks/useEventListeners.ts:137`：`(data.type as LogType) || 'info'`——只兜住 undefined/空串等 falsy 值，兜不住 truthy 的错误字面量（如 `"warn"`）。
2. `android/frontend/src/hooks/useLogToastStore.ts:32`：`addLog(message, type = 'info')` 参数默认值。
3. `android/frontend/src/components/layout/RightPanel.tsx:217`、`:242`：`LOG_ICONS[log.type] ?? Info`（`LOG_ICONS` 定义于同文件 :20）——即使错误字面量穿透前两层，图标取 `Info`、`LOG_BG_COLORS[log.type] ?? ''`（:223/:254）与 `LOG_BAR_COLORS[log.type] ?? 'bg-zinc-400'`（:226/:258）兜住，`LOG_COLORS[log.type]`（:227/:235/:259/:267）取不到只是丢 class，均不再崩溃。

## 教训

跨树（后端 → 前端）的事件 `type` 属于**字符串字面量契约**：后端发值必须落在前端枚举内，前端消费处一律加兜底默认值。这类漂移不会触发 TS 编译错误（`as LogType` 断言会静默放行任意字符串），只能靠契约纪律 + 分层兜底。

兜底尚未对齐到桌面，桌面会重演同类白屏：`tauri-app/frontend/src/components/layout/RightPanel.tsx:268`、`:293` 仍是裸的 `LOG_ICONS[log.type]`（定义于同文件 :26，无 `?? Info`）——后端一旦引入新 type，图标取 `undefined` 直接崩。桌面后端 `tauri-app/src-tauri/src/infra/events.rs:22-27` 的 `EventBus::emit_login_log(&self, message: &str, log_type: &str)`（emit 在 :23）同样接受任意字符串。桌面侧只有 `tauri-app/frontend/src/hooks/useEventListeners.ts:272` 的 `|| 'info'` 一层兜底。

## Connections

[[tablet-layout-alignment-audit]]、[[android-white-screen-missing-init-fields]]
