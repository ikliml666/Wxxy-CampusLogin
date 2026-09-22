---
title: 夜间出站自动切换判定（outbound_switch）
type: module
source_files:
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/src-tauri/src/config/night_switch.rs
tags: [夜间出站, 夜切, 纯函数, 跨平台, 时间表, config]
---

## Overview

`config/outbound_switch.rs` 是「夜间出站自动切换」的跨平台判定纯函数层（2026-09-22 新增，任务 1/13）：与运营商夜切（`night_switch`，见 [[night-operator-switch|夜间运营商切换决策]]）共用同一时刻表，但动作语义正交——本模块只产出 Switch/Restore 决策，"切到哪张卡（桌面改 metric）"与"注销+重检（安卓）"由各端调用方实现（后续任务接线）。切换态由配置自身承载（restore 快照/标记非空），幂等无需去重标记。

本模块在 `tauri-app/src-tauri/src/config/mod.rs:3` 注册，随 `config` 模块对安卓端可见（path 依赖）。

桌面侧的读写底座已落地（任务 3）：读 `platform::metric::read_interface_metrics`（免提权快照）、写经 helper `HelperOp::SetMetric` 提权执行 `SetIpInterfaceEntry`（见 [[desktop-platform]]、[[desktop-helper-update]]）；判定层与底座之间的接线在任务 5。字段对照与写入坑位见 [[set-ip-interface-entry-metric]]。

## Key Components

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `outbound_switch.rs:10` | pub enum | `NightOutboundAction { None, Switch, Restore }` | 本拍动作；`None` 含"夜间窗口内已处于切换态" |
| `outbound_switch.rs:24` | pub fn | `evaluate_night_outbound(enabled, weekday, now_minutes, restore_active)` | 判定纯函数：`!enabled` → None；`restore_active` 且 now ∈ [390, 1380) → Restore；`!restore_active` 且当日有切换时刻且 now ≥ 时刻 → Switch（过点补触发）；其余 → None（午夜后不补触发、切换态夜间保持） |
| `night_switch.rs:10-12` | pub(crate) const | `RESTORE_START_MINUTES=390` / `RESTORE_END_MINUTES=1380` | 恢复窗口 [06:30, 23:00)，两端共用 |
| `night_switch.rs:26` | pub(crate) fn | `switch_time_for(weekday)` | 时间表：weekday 0..=4 → 1380（23:00），5/6 → 1410（23:30），其余 None |

## Patterns & Conventions

- **时间表单点**：出站切换与运营商夜切共用 `switch_time_for`，禁止两处硬编码漂移；运营商时刻调整时只改 `night_switch.rs` 一处。
- **判定与状态解耦**：纯函数只吃 `restore_active: bool`（由调用方把"配置中 restore 快照/标记非空"折算传入），不直接读配置，方便单测与跨端复用。
- **边界沿既有语义**：恢复窗口下界 390 含、上界 1380 不含（与最早切换时刻无缝衔接）；午夜后（次日 00:00-06:29）不补触发恢复，与运营商夜切一致。

## Learnings

- 纯函数层先于调用方落地时，`pub fn` 会报 `never used` 警告——属任务序列中间态，接线后消失，不加 `#[allow(dead_code)]` 掩盖。同一序列的 `platform/metric.rs::read_interface_metrics` 与 `MetricRow` 同理（任务 3 落地、任务 5 消费），警告只出现在 bin target。
