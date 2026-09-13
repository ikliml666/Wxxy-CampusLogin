---
title: npx tsc -b 会 emit 出 vite.config.js 等污染文件
type: learning
source_files:
  - tauri-app/frontend/tsconfig.json
  - tauri-app/frontend/tsconfig.node.json
tags: [教训, 工具链, typescript, 构建]
---

## 现象

执行 `npx tsc -b` 后目录里多出 `vite.config.js` / `vite.config.d.ts` 等生成文件，污染工作区。

## 根因

`tsconfig.node.json` 是 composite 项目，`tsc -b` 按项目引用构建时会 emit 产物。

## 解决

类型检查一律改用 `npx tsc --noEmit --incremental`。

## 教训

本仓库（含安卓前端）的类型检查**禁止 `tsc -b`**，只用 `--noEmit --incremental`。该规则见项目 `AGENTS.md` 第 4 条验证约定。

## Connections

[[broken-types-package-in-root-node-modules]]、[[verification-baseline]]
