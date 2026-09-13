---
title: 安卓前端没有单元测试
type: learning
source_files:
  - android/frontend/package.json
tags: [教训, 安卓, 测试, 复刻树]
---

## 现象

复刻树的行为分叉只能靠人工发现。

## 根因

桌面前端有 13 个 `.test.ts(x)` 与 `test-setup.ts`，而 `android/frontend` 目录下**不存在** `test-setup.ts` 与任何测试文件，`package.json` 也没有 `test` 脚本。

## 解决

当前安卓前端只靠 `tsc --noEmit` + 真机验证。

## 教训

① 安卓前端的改动**不能声称"测试通过"**——没有测试可跑；② 复刻树的分叉检测依赖提交审计（同一提交是否同时改两端）与真机验证（见 [[dual-tree-sync-human-discipline]]）。

## Connections

[[dual-tree-sync-human-discipline]]、[[verification-baseline]]
