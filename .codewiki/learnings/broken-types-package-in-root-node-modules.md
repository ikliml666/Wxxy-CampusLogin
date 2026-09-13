---
title: tsc 全量检查必报错 exit 2：自动加载了工作区根 node_modules 里损坏的 @types/yauzl
type: learning
source_files:
  - tauri-app/frontend/tsconfig.json
tags: [教训, typescript, 依赖, 工作区]
---

## 现象

tsc 全量检查必然报错并 exit 2，来自工作区根 `node_modules` 里损坏的 `@types/yauzl`。

## 根因

TS 默认自动加载 `node_modules/@types` 下的全部类型包，工作区根的那份损坏包被一并加载。

## 解决

tsconfig 显式写 `types: ["vite/client"]`（node 侧 `["node"]`），阻止自动加载全部 `@types`。

## 教训

工作区（多项目集合目录）下做类型检查时，**必须显式声明 `types`**，不要依赖自动加载——根目录平级的依赖会污染子项目。

## Connections

[[tsc-b-emits-contaminated-files]]
