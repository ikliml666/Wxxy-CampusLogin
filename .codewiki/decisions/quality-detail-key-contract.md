---
title: 质量检测键用英文标识符 + 驱动者收敛与三处联动
type: decision
source_files:
  - tauri-app/src-tauri/src/network/quality.rs
  - tauri-app/frontend/src/monitor/QualityPanel.tsx
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
tags: [决策, 质量检测, i18n, 前端]
---

## 背景

质量检测结果里的检查项需要前端展示中文名称，而键名若直接写中文会与跨语言/跨端契约纠缠。

## 决策

- `details` / `metrics` 字典键用**英文标识符**（`gateway` / `aliDns` / `bilibili`…），显示名走 i18n `quality.names.*`；前后端键同步改（同仓库同发版无兼容窗口）。
- 周期质量检测的**唯一驱动者是定时测试循环**。
- 新增受开关控制的面板需**三处联动**：App 渲染 null / DockNav 过滤入口 / useInitialDataLoad 跳过恢复。

## 理由

旧文档只写了"同仓库同发版无兼容窗口"这一前提，未记录权衡过程。

## 备选方案

旧文档未记录。

## 影响与约束

新增检查项时键名必须英文且与 i18n 词条同步；已知脆弱点：`details`/`metrics` 中所有项都以 `name` 为键（`network/quality.rs:381-384`），重名会静默覆盖，须自行保证唯一。

2026-09-13 补充：`check_network_quality_async` 新增 `lightweight` 轻量复核模式（`network/quality.rs:455`，仅执行预览波网关 + 1 个外网站点即提前返回，分支在 `:526-528`），其结果同样经统一的 `build_quality_result` 以 `name` 为键构造 `details`/`metrics`（`:524` 注释明确"聚合语义不变"），本契约不受影响。

## Connections

[[quality-check-single-driver]]、[[network-quality-default-off]]、[[deferred-panel-transition]]
