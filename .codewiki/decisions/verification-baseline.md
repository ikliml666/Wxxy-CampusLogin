---
title: 统一验证基线（测试与类型检查命令）
type: decision
source_files:
  - AGENTS.md
tags: [决策, 验证, 测试基线]
---

## 背景

需要一套固定的、"跑过才算完"的验证命令，避免每次凭"应该没问题"交差。

## 决策

验证基线为：`cargo test` 493 全绿 / `cargo clippy -D warnings` / `npx tsc --noEmit --incremental` / `npm test`；安卓只认 `tauri android build` 交叉编译。布局/交互改动需浏览器实测，且用后 `git diff` 必须干净。

## 理由

旧文档未记录（只给出命令清单与"以实际输出为准"的要求）。

## 备选方案

旧文档未记录。

## 影响与约束

`tsc` 一律用 `--noEmit --incremental`（禁止 `tsc -b`）；安卓 host `cargo check` 基线即失败，不作为验证手段。

## Connections

[[tsc-b-emits-contaminated-files]]、[[android-host-cargo-check-fails]]
