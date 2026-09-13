---
title: 平板布局对齐 Windows 版（差异审计驱动）
type: decision
source_files:
  - android/frontend/src/components/layout/TitleBar.tsx
  - tauri-app/frontend/src/components/layout/TitleBar.tsx
tags: [决策, 安卓, 前端, 双端同步, 布局]
---

## 背景

安卓前端是**独立复刻树**，桌面修复不会自动同步到安卓，视觉/行为分叉会持续累积。

## 决策

2026-09-12：平板布局**对齐 Windows 版**，用「同一提交是否同时改 `android/frontend`」做审计驱动，审计出 5 处未同步并补齐：

- 日志 ×N 折叠计数；
- 日志卡茶娘水印；
- TitleBar 娘头像（平板 `TOPBAR_ZOOM=1.3`，用 `w-8` 对齐桌面 `w-10` 视觉）；
- AboutDialog 高度自适应 + 娘图；
- 侧边娘认断点：左娘 900→1008px（720 主区 + 2×(128+16)，900-1008 区间会压进内容列）、右娘竖屏不偏移 304px。

## 理由

旧文档记录的理由是"安卓前端是独立复刻树，桌面修复不会自动同步"；用提交审计逐条找出缺口是选定手段。

## 备选方案

旧文档未记录。

## 影响与约束

双端通用改动必须同一次提交双端各改一份。验证限制：Redmi 25060RK16C(dali) 短边约 334dp 属手机壳，平板壳验证靠浏览器 mock 平板视口。

## Connections

[[android-log-type-enum-drift]]、[[network-quality-default-off]]、[[dual-tree-sync-human-discipline]]
