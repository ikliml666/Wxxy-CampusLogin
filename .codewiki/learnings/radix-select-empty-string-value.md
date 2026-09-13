---
title: Radix Select 报错/不渲染某项：Item 不接受空串 value
type: learning
source_files:
  - tauri-app/frontend/src/settings/SettingsPanel.tsx
tags: [教训, 前端, radix, 表单]
---

## 现象

Radix `Select` 报错，或某一项不渲染。

## 根因

`SelectItem` 的 `value` 不接受空串。

## 解决

"记住上次"类空值语义改用**哨兵值映射**（例如 `'__remember__'`）。

## 教训

Radix 组件里空串是非法 value，需要空值语义时一律用哨兵字符串映射，并在读写两侧同时转换。

## Connections

[[contain-paint-clips-absolute-menu]]
