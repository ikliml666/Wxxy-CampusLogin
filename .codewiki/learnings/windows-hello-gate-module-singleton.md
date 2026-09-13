---
title: Hello 门是模块级单例，跨面板与向导共享一份时间戳
type: learning
source_files:
  - tauri-app/frontend/src/account/selfServiceState.ts
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - tauri-app/frontend/src/settings/OnboardingWizard.tsx
tags: [教训, 前端, 验证门, 单例, 状态]
---

## 现象

在向导里验证过一次后，账号面板的绑定/查询在 TTL 内**不再验证**，反之亦然。

## 根因

绑定门只有一个模块级变量（`account/selfServiceState.ts:55` 绑定门、`:89` 会话门）：账号面板绑定卡与两个向导共用固定时间戳；且生产代码**没有显式重置入口**，只能等 `VERIFY_TTL_MS`（570s）过期或时钟回拨，测试必须 `vi.resetModules()` 重置。

## 解决

当前未改（作为已知副作用记录；会话门另有一条"按面板卸载重置"的路径）。

## 教训

① 模块级单例状态要明确写出"谁共享、什么时候重置"；② 新增验证入口时确认是否会意外复用别人的门；③ 面板状态丢失（如 `ErrorBoundary` 兜底替换面板）不会重置门，排查验证异常先想到这里。

## Connections

[[windows-hello-only-identity]]、[[verification-gate-tiers]]
