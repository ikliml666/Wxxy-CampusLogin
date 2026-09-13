---
title: WebView2 浏览器参数精简到单参数并恢复 vsync
type: decision
source_files:
  - tauri-app/src-tauri/src/platform/gpu.rs
tags: [决策, webview2, 渲染, 前端性能]
---

## 背景

曾经给 WebView2 注入 11 个 Chromium 参数，其中 `--disable-gpu-vsync` 导致动画撕裂顿挫（观感掉帧）：BeginFrame 不对齐显示器刷新，经 DWM 合并后紊乱。同一件事旧文档在"踩坑记录（Windows 平台）"与"决策记录 2026-09-03"两处都记了。

## 决策

2026-09-03：WebView2 浏览器参数**精简到仅** `--js-flags=--max-old-space-size=512`，并**恢复 vsync**（vsync 相关参数不要动），渲染交还平台默认。

## 理由

逐项核验原 11 参数：已从 Chromium 移除 / Windows 默认即开 / Windows 不支持 / 实验性强开有渲染异常风险。渲染交还平台默认即"测试最充分的配置"，前端动画本就 rAF/vsync 驱动。

## 备选方案

保留原有 11 个 Chromium 参数（含 `--disable-gpu-vsync`）——因上述逐项核验结论被弃用。删除理由逐项记录在 `platform/gpu.rs:239-246` 注释；若日后确有性能增益需按 `ponytail:` 注释（244 行）逐项实测后加回。

## 影响与约束

不要再动 vsync 相关参数；`build_browser_args` 目前只剩单个参数（`platform/gpu.rs:236-248`）。

## Connections

[[deferred-panel-transition]]、[[panel-import-strategy]]
