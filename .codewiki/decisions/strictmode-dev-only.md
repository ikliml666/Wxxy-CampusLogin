---
title: 生产环境不启用 StrictMode
type: decision
source_files:
  - tauri-app/frontend/src/main.tsx
tags: [决策, 前端, strictmode]
---

## 背景

React 的 `StrictMode` 会 double-invoke effect（setup → cleanup → setup），面板组件里大量存在"StrictMode setup→cleanup→setup 恢复 `mountedRef`"的防御代码。

## 决策

生产环境**不启用** `StrictMode`——仅 DEV 包 `StrictMode`（`main.tsx:115-117`）。

## 理由

旧文档未记录（wiki 模块文章未记录权衡过程）。

## 备选方案

旧文档未记录。

## 影响与约束

生产环境这些 double-invoke 类问题不受考验，**这类问题只在 DEV 暴露**。排查生产"挂载/卸载后状态丢失"类问题时先确认是否只在 DEV 可复现。

## Connections

[[onmousedown-preventdefault-race]]
