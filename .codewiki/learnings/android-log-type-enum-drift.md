---
title: 跨树日志事件 type 必须对齐前端 LogType 枚举（"warn" vs "warning" 致白屏）
type: learning
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/hooks/useEventListeners.ts
  - android/frontend/src/components/layout/RightPanel.tsx
  - android/frontend/src/shared/ui-types.ts
tags: [教训, 安卓, 双端同步, 枚举, 白屏]
---

## 现象

安卓端白屏——启动自动登录判定失败可触发。

## 根因

安卓后端 `emit_login_log`（`monitor_loop.rs:130`）发的事件 `type` 是 `"warn"`，而前端 `LogType` 枚举是 `'warning'`（`android/frontend/src/shared/ui-types.ts:4`）；`LOG_ICONS[type]` 取到 `undefined` 直接白屏。

## 解决

后端改为 `'warning'`（现：`monitor_loop.rs:464` "启动自动登录:校园网判定失败"）；前端消费处加兜底防再犯——`android/frontend/src/hooks/useEventListeners.ts:266` 用 `(data.type as LogType) || 'info'`，`android/frontend/src/components/layout/RightPanel.tsx:268`、`:293` 用 `LOG_ICONS[log.type] ?? Info`（`LOG_ICONS` 定义于同文件 :26）。

## 教训

跨树（后端 → 前端）的事件 `type` 属于**字符串字面量契约**：后端发值必须落在前端枚举内，前端消费处一律加兜底默认值。这类漂移不会触发 TS 编译错误，只能靠契约纪律 + 兜底。

兜底尚未对齐到桌面：`tauri-app/frontend/src/components/layout/RightPanel.tsx:268`、`:293` 是裸的 `LOG_ICONS[log.type]`（桌面后端 `infra/events.rs:22` 的 `emit_login_log` 同样接受任意字符串），桌面侧只有 `useEventListeners.ts:266` 的 `|| 'info'` 兜底，`LOG_ICONS` 这一层没有——后端一旦引入新 type，桌面会重演同类白屏。

## Connections

[[tablet-layout-alignment-audit]]、[[android-white-screen-missing-init-fields]]
