---
title: "absolute 定位的菜单/按钮被卡片裁掉（contain: paint）"
type: learning
source_files:
  - tauri-app/frontend/src/index.css
  - tauri-app/frontend/src/components/ui/animated-card.tsx
tags: [教训, 前端, css, portal, 定位]
---

## 现象

卡片内 absolute 定位的下拉菜单/按钮被卡片边界裁掉，显示不全。

## 根因

`animated-card-interactive` 的 `contain: paint` 会裁剪后代越界内容。

## 解决

菜单改用 `createPortal(document.body)` 渲染到 body。

## 教训

在带 `contain: paint` 的卡片内不要放 absolute 浮层；浮层一律走 `createPortal(document.body)`。

## Connections

[[tailwind-space-y-margin-specificity]]、[[radix-select-empty-string-value]]
