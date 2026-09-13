---
title: 秒与毫秒的双向换算点分散在各处
type: learning
source_files:
  - tauri-app/frontend/src/monitor/MonitorPanel.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - tauri-app/frontend/src/monitor/useMonitor.ts
tags: [教训, 前端, 单位, 配置, 换算]
---

## 现象

`config.backgroundCheckInterval` 被 UI 以**秒**呈现、以**毫秒**落盘，换算点分散在 5 处（`MonitorPanel.tsx:85` 读、`:105` 写、`MobileDashboard.tsx:85` 读、`:102` 写、`useMonitor.ts:31` 写）——任一处遗漏即出现"设 60 秒实际 60000 秒"类偏差。

## 根因

单位换算没有收敛到单一位置，靠每个调用点各自 `/1000`、`*1000`。

## 解决

无（当前靠人工核对；相关条目的下限还不一致：`MobileDashboard` 用 `Math.max(5, ...)`，`MonitorPanel` 的 commit 下限是 10s）。

## 教训

① 改任何 interval 相关 UI 时**逐处核对单位换算**（grep 该字段名找全部换算点）；② 新增读取/写入点时保持同一个单位约定；③ 注意同一字段在不同 UI 的下限不同，从总览卡切开关可能用"下限 5s 的值"覆盖配置。

## Connections

[[android-interval-default-schema-migration]]
