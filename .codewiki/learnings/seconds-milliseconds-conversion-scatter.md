---
title: 秒与毫秒的双向换算点分散在各处
type: learning
source_files:
  - tauri-app/frontend/src/monitor/MonitorPanel.tsx
  - android/frontend/src/components/mobile/MobileDashboard.tsx
  - tauri-app/frontend/src/monitor/useMonitor.ts
tags: [教训, 前端, 单位, 配置, 换算]
---

## Overview

`config.backgroundCheckInterval` 的 UI 呈现（秒）与后端落盘（毫秒）之间的换算逻辑，仍然分散在三个文件中。任一处遗漏或逻辑不一致，即出现"设 60 秒实际 60000 秒"类偏差。此外，桌面端与移动端对同一字段的最小值约束不同，且处理模式存在差异。

## Key Components

- **tauri-app/frontend/src/monitor/MonitorPanel.tsx**
    - `AdapterStatusCard` (L31): 适配器在线状态迷你卡组件。
    - `GroupCardHeader` (L64): 设置卡片头组件。
    - `MonitorPanel` (L75): 监控面板主组件。
        - `intervalSec` (L83): `useMemo` 计算秒数：`(config.backgroundCheckInterval || 60000) / 1000`。
        - `intervalDraft` (L86): 数字输入本地草稿状态，避免输入回灌。
        - `commitInterval` (L97): 提交间隔，下限 `Math.max(10, ...)`，上限 `Math.min(600, ...)`，乘以 1000 后更新配置。
        - `toggleBackgroundCheck` (L104): 启动/停止加锁处理。
    - `bgStatus.adapterStatuses` (L108): 适配器状态列表。

- **android/frontend/src/components/mobile/MobileDashboard.tsx**
    - `MobileStatusCard` (L42): 移动总览页状态卡。
    - `MobileMonitorCard` (L86): 移动监控摘要卡。
        - `intervalSec` (L100): `Math.max(5, Math.round((config.backgroundCheckInterval ?? 60_000) / 1000))`。直接读取现值，无草稿模式。
        - `handleToggleBackgroundCheck` (L111): 通过 `useMonitor` hook 调用。
        - `handleTriggerCheck` (L112): 通过 `useMonitor` hook 调用。
    - `MobileDashboard` (L120): 移动仪表板组件。
        - `extraCards` (L130): 定义两张移动专属卡。

- **tauri-app/frontend/src/monitor/useMonitor.ts**
    - `useMonitor` (L11): 监控逻辑 Hook。
        - `handleToggleBackgroundCheck` (L26): 切换后台检查。
            - 写入：`intervalSec * 1000` 存入 store 并持久化。
            - 先持久化再启停，避免首次启动用旧间隔。
        - `handleTriggerCheck` (L53): 触发单次后台检查。
        - `handleToggleLatencyTest` (L56): 切换延迟测试（类似逻辑）。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                       Config Store                           │
│              backgroundCheckInterval (ms)                    │
└──────────┬──────────────────────────────────┬───────────────┘
           │                                  │
           ▼                                  ▼
┌──────────────────────┐         ┌────────────────────────┐
│ MonitorPanel.tsx     │         │ MobileDashboard.tsx    │
│ (Desktop/Tauri)      │         │ (Android)              │
│                      │         │                        │
│ intervalSec = ms/1000│         │ intervalSec = ms/1000  │
│ Draft Mode: Yes      │         │ Draft Mode: No         │
│ Min: 10s             │         │ Min: 5s                │
│ Commit: v*1000       │         │ Pass to useMonitor     │
└──────────┬───────────┘         └──────────┬─────────────┘
           │                                │
           ▼                                ▼
┌─────────────────────────────────────────────────────────────┐
│                       useMonitor.ts                          │
│                                                              │
│  handleToggleBackgroundCheck(enabled, intervalSec)           │
│    ├─ saveConfigDirect({ ..., backgroundCheckInterval: sec*1000 })
│    ├─ api.start/stopBackgroundCheck()                        │
│    └─ updateConfigLocal(...)                                 │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **读取**:
   - `MonitorPanel.tsx`: `(config.backgroundCheckInterval || 60000) / 1000` (L83)。
   - `MobileDashboard.tsx`: `Math.round((config.backgroundCheckInterval ?? 60_000) / 1000)` (L100)。
2. **写入 (桌面)**:
   - `MonitorPanel.tsx`: `commitInterval` 中 `v * 1000` 调用 `onUpdateConfig` (L99)。
   - 下限：10 秒 (L98)。
3. **写入 (移动)**:
   - `MobileDashboard.tsx`: 将 `intervalSec` 传给 `handleToggleBackgroundCheck`。
   - 下限：5 秒 (L100)。
4. **持久化**:
   - `useMonitor.ts`: `handleToggleBackgroundCheck` 中 `intervalSec * 1000` 存入 store 并持久化 (L33)。

## Known Issues

- **下限不一致**: 桌面端限制 10s，移动端限制 5s。
- **草稿模式差异**: `MonitorPanel` 使用 `intervalDraft` 避免输入回灌，`MobileDashboard` 直接读取现值。
- **换算分散**: `/1000` 和 `*1000` 散落在各处，缺乏统一工具函数。

## Connections

[[android-interval-default-schema-migration]]