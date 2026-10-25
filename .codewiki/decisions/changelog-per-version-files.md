---
title: 更新日志按版本文件维护（changelogs/，默认写入下一版号）
type: decision
source_files:
  - AGENTS.md
  - tauri-app/src-tauri/tauri.conf.json
tags: [决策, 文档, changelog, 发布]
---

## 背景

原机制是项目根单文件 `CHANGELOG.md`：所有条目堆积在 `[Unreleased]` 下按日期倒序追加（2026-09-15 重组时已积 73 条），发布版本时由 `make-release.ps1` 把整份文件快照为 `changelogs/CHANGELOG_v<版本>.md`。快照时机滞后于条目写入——v2.3.6 发布（2026-09-12 晚）后新条目继续写进 CHANGELOG.md，而 v2.3.6 的快照做晚了，实际混入 68 条 v2.3.7 开发期条目，只能靠日期切分手工拆分（2026-09-15 重组实证）。

## 决策

- 更新日志**直接按版本文件维护**：`changelogs/CHANGELOG_v<版本>.md`，一个发布版本一个文件。
- **写之前必须先确认当前版本号**：以 `tauri-app/src-tauri/tauri.conf.json` 的 `version` 字段为唯一权威源（见 [[version-single-source-build-rs]]），不凭会话记忆或既有文件名顺延猜测。**目标文件 = 当前版本号 + 1**（如当前 2.4.1 → 写 `changelogs/CHANGELOG_v2.4.2.md`）；该文件即下一版发布时的更新日志——已存在就把新条目并入该文件（changelogs/ 里可能有多个未发布版本累积文件时，并入最早的那个），**绝不顺延开新的版本文件**，发布后封版，新内容再从新的"当前版本号 + 1"起步（见 AGENTS.md 第 7 行）。
- 任何代码 / 配置 / 构建 / 文档改动，完成并验证后均须记入版本化 changelog，沿用既有分类（行为调整 / 缺陷修复 / 前端性能优化 / 架构改进 等），说清"改了什么 + 为什么 + 验证数据"（见 AGENTS.md 第 7 行）。
- 不再维护统一的 `CHANGELOG.md`（2026-09-15 起废弃、已删除），发布流程不再有快照步骤（见 AGENTS.md 第 7 行）。

## 理由

写入时即确定版本归属，消除"快照滞后→内容混版"的整类问题；单文件也无需定期瘦身。快照机制下每个版本文件的内容边界天然清晰。

## 备选方案

- 维持单文件 + 发布时快照：被否，快照滞后已实际造成混版（v2.3.6 快照混入 68 条），且单文件无限膨胀。
- 发布时按 tag 日期切分单文件：被否，等于每次发布都做一次本可避免的拆分工作，且条目无时刻精度、日期边界条目归属模糊。

## 影响与约束

- `changelogs/` 目录在 `.gitignore` 中，仅本地留存、不提交（AGENTS.md 第 7 行）。
- 版本号基准是 `tauri-app/src-tauri/tauri.conf.json` 的 `version` 字段为唯一权威源（见 [[version-single-source-build-rs]]）；新内容写"当前版本 + 1"的文件，若 changelogs/ 中已有未发布版本累积文件则并入最早的那个，绝不顺延开新文件。
- 版本号升级提交时宜同步创建下一版 changelog 文件（空壳或首条），避免后续条目写错文件。
- 历史归档口径（2026-09-15 重组）：日期 ≥ 2026-09-12 的条目归 v2.3.7 文件，之前的归 v2.3.6 文件。

## Connections

[[release-asset-integrity]]、[[version-single-source-build-rs]]
