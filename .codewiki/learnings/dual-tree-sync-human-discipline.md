---
title: 双端同步依赖人工纪律，仓库里没有自动拦截
type: learning
source_files:
  - AGENTS.md
tags: [教训, 双端同步, 流程, 复刻树]
---

## 现象

"只改一端"的提交不会被任何机制拦下，双端行为/视觉分叉会持续累积。

## 根因

项目 `AGENTS.md` 第 3 条要求同提交双端各改一份，但**仓库中没有 CI 检查**；旧文档也只把它写成约定（`CODE_WIKI.md:289-290`），无法自动拦截只改一端的提交。

## 解决

靠提交前 `git diff --stat` 自检是否同时触及两端；2026-09-12 的平板布局对齐就是用「同一提交是否同时改 `android/frontend`」做审计驱动，查出 5 处未同步并补齐。

## 教训

① 通用改动（前端 UI/交互、文案与 i18n、图标素材、IPC 命令面、配置字段与默认值、事件与日志类型）提交前用 `git diff --stat` 自检双端；② 只有两类例外：协议实现单点存在于桌面 crate（安卓经 Cargo path 依赖自动继承）、平台专属能力（DPAPI/Keystore、托盘/前台服务）各端自理。

## Connections

[[tablet-layout-alignment-audit]]、[[config-field-sets-bidirectional-sync]]、[[android-log-type-enum-drift]]
