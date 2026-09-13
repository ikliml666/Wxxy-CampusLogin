---
title: 容器内 margin 间距不生效（space-y-4 的 specificity 锁死 margin-bottom）
type: learning
source_files:
  - tauri-app/frontend/src/index.css
tags: [教训, 前端, css, tailwind, 间距]
---

## 现象

子元素上写的 margin 间距不生效（间距被吃掉）。

## 根因

`space-y-4 > * + *` 的 specificity 锁死了 `margin-bottom`，覆盖子元素自己的 margin。

## 解决

间距改用 **padding**，不用 margin。

## 教训

在 Tailwind 的 `space-y-*` 容器内，子元素间距一律用 padding；用 margin 会静默失效。

## Connections

[[contain-paint-clips-absolute-menu]]、[[css-comma-selector-shared-body-pitfall]]
