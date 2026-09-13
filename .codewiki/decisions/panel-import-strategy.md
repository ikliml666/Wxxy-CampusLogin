---
title: 常用面板静态导入，仅 3 个低频对话框懒加载 + 启动预取
type: decision
source_files:
  - tauri-app/frontend/src/App.tsx
tags: [决策, 前端, 代码分割, 首屏]
---

## 背景

出于首屏体积考虑曾用 `React.lazy` 分包，但实测切换开销明显。

## 决策

2026-09-04：**常用面板静态导入**，仅 3 个低频对话框懒加载 + 启动预取。

## 理由

`React.lazy` 切换实测 ~366ms（chunk 缓存命中仍如此），13KB 分包远不值这个切换延迟——首屏体积与交互流畅之间取后者。

## 备选方案

全部面板 `React.lazy` 分包（含 `LogPanel`）——因上条实测被弃用；`LogPanel` 已回归静态导入。

## 影响与约束

新增面板默认静态导入，除非确属低频对话框。注意 `App.tsx:20/29` 注释与实现已不一致（注释称"仅低频的 LogPanel/对话框保留懒加载"，实际 LogPanel 是静态导入）。

## Connections

[[deferred-panel-transition]]、[[webview2-args-minimal]]
