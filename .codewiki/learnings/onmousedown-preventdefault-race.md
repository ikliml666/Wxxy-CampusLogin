---
title: 清空类按钮需要 onMouseDown preventDefault（blur 提交会覆盖清除）
type: learning
source_files:
  - tauri-app/frontend/src/account/AccountPanel.tsx
tags: [教训, 前端, 竞态, 表单, 密码]
---

## 现象

点击"清除已保存密码"后，密码实际没被清掉。

## 根因

用户此刻在密码框里刚输入草稿时，点击按钮会**先触发 `handlePasswordBlur`** 提交 `onUpdateConfig({ password: draft })`（进入 500ms debounce），随后才执行清空保存。叠加后端「空密码 = 保留旧密码」语义，清除被随后触发的 debounce 保存覆盖。

## 解决

给按钮加 `onMouseDown={(e) => e.preventDefault()}`，阻止 blur 抢跑（同文件的自助密码清除按钮、`SelfServicePanel` 的清除/眼睛按钮都是这么做的；登录密码清除按钮曾是唯一遗漏）。

## 教训

凡是"清空/覆盖"类按钮，都要显式加 `onMouseDown preventDefault` 防 blur 抢先提交；新增同类按钮时对照已有的三处写法。

## Connections

[[strictmode-dev-only]]
