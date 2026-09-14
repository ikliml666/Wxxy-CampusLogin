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

验证基线为：`cargo test` 全绿（**2026-09-14 实测：lib 单元 325 + 集成 1 = 326 个独立用例，以此为准**；统计口径见下）/ `cargo clippy -D warnings` / `npx tsc --noEmit --incremental` / `npm test`；安卓只认 `tauri android build` 交叉编译。布局/交互改动需浏览器实测，且用后 `git diff` 必须干净。

### `cargo test` 用例数统计口径

`cd tauri-app/src-tauri && cargo test 2>&1 | grep "test result"` 会输出 4 行（2026-09-14 实测）：

| 目标 | test result 行 |
|---|---|
| lib（单元测试，`campus_login_lib`） | `325 passed` |
| bin（`campus-login-bin`，链接同一套 lib 测试） | `325 passed`（与 lib 重复，**不单独计数**） |
| 集成测试（tests/） | `1 passed` |
| doc-tests | `0 passed` |

**独立用例数 = lib 325 + 集成 1 = 326**。bin 目标与 lib 是同一套测试经双模块树重复链接（见 [[bin-lib-dual-module-tree]]），把 4 行 `test result` 直接相加（651）会把 lib 那套数两遍，不是独立用例数。历史上的"493 用例"说法来历不明（可能为旧版本快照或不同口径），不做臆测；2026-09-13 曾实测 282 + 1 = 283（账号体系功能合入前的口径，随 2026-09-14 的账号/适配器功能新增用例失效），**以最新实测口径为准**；后续新增/删除测试时按同口径更新此数。

## 理由

旧文档未记录（只给出命令清单与"以实际输出为准"的要求）。

## 备选方案

旧文档未记录。

## 影响与约束

`tsc` 一律用 `--noEmit --incremental`（禁止 `tsc -b`）；安卓 host `cargo check` 基线即失败，不作为验证手段。

## Connections

[[tsc-b-emits-contaminated-files]]、[[android-host-cargo-check-fails]]
