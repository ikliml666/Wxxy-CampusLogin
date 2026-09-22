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

验证基线为：`cargo test` 全绿（**2026-09-22 实测：lib 单元 396 + 集成 1 = 397 个独立用例，以此为准**；统计口径见下）/ `cargo clippy -D warnings` / `npx tsc --noEmit --incremental` / `npm test`；安卓只认 `tauri android build`（Rust 改动验证见 `decisions/verification-gate-tiers` 与 [[android-host-cargo-check-fails]]）。布局/交互改动需浏览器实测，且用后 `git diff` 必须干净。

### `cargo test` 用例数统计口径

`cd tauri-app/src-tauri && cargo test 2>&1 | grep "test result"` 会输出多行（2026-09-22 实测，夜间出站切换功能合入后）：

| 目标 | test result 行 |
|---|---|
| lib（单元测试，`campus_login_lib`） | `396 passed; 0 failed; 3 ignored`（ignored 为真机冒烟：`elevation.rs` 提权、`platform/metric.rs` 读跃点、`task_proxy.rs` 计划任务，各 `#[ignore]` 手动执行） |
| bin（`campus-login-bin`，链接同一套 lib 测试） | `396 passed`（与 lib 重复，**不单独计数**） |
| 集成测试 `tests/repro_logout_panic.rs` | `1 passed` |
| 集成测试 `tests/task_proxy_smoke.rs` | `0 passed; 1 ignored`（真实注册计划任务冒烟，手动执行） |
| doc-tests | `0 passed` |

**独立用例数 = lib 396 + 集成 1 = 397**。bin 目标与 lib 是同一套测试经双模块树重复链接（见 [[bin-lib-dual-module-tree]]），把多行 `test result` 直接相加会把 lib 那套数两遍，不是独立用例数。历史口径：2026-09-14 为 326（lib 325 + 集成 1）；夜间出站切换功能（2026-09-21~22）新增约 24 个用例（时间表共用、快照幂等、目标卡选择、互斥顺序、还原终态出口、退避阶梯、helper 编解码 round-trip 等）后为 397。更早的"493 用例"说法来历不明，不再采用。**后续新增/删除测试时按同口径更新此数**。

## 理由

旧文档未记录（只给出命令清单与"以实际输出为准"的要求）。

## 备选方案

旧文档未记录。

## 影响与约束

`tsc` 一律用 `--noEmit --incremental`（禁止 `tsc -b`）；安卓 host `cargo check` 基线即失败，不作为验证手段。

## Connections

[[tsc-b-emits-contaminated-files]]、[[android-host-cargo-check-fails]]
