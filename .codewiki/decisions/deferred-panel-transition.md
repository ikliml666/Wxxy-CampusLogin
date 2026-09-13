---
title: 面板内容渲染统一用 deferredPanel（useDeferredValue）
type: decision
source_files:
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/components/layout/DockNav.tsx
tags: [决策, 前端, 面板转场, 并发渲染]
---

## 背景

快速连切面板时出现"内容与标题错位"，真机表现为"快速切换短卡顿"；120Hz 帧预算仅 8.3ms，React 并发渲染会跳过中间面板的 mount。旧文档在"关键约定""踩坑记录""决策记录"三处都记了同一件事。

## 决策

面板 switch / 转场 key / 标题 / 方向**全部消费** `useDeferredValue` 后的值（`deferredPanel`）；`activePanel` **仅**用于 DockNav 高亮与 storage 恢复。

## 理由

这是真机"快速切换短卡顿"的主修，120Hz 帧预算下并发跳过中间 mount 是根因。

## 备选方案

旧文档未记录。

## 影响与约束

用错（转场相关逻辑改用 `activePanel`）会内容与标题错位。已知残留分歧：`DockNav` 高亮仍用 `activePanel`（`components/layout/DockNav.tsx:403`），并发渲染延迟期间会出现"Dock 已切换、内容还是旧面板"，属预期取舍。相关硬耦合：`App.tsx:284` 的 60ms 切换锁只覆盖 `mode="wait"` 的退出动画 0.04s，任一侧改动都会静默失配。

## Connections

[[quality-detail-key-contract]]、[[panel-import-strategy]]
