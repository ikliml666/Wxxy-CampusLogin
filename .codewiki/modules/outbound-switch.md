---
title: 夜间出站自动切换判定（outbound_switch）
type: module
source_files:
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/src-tauri/src/config/night_switch.rs
tags: [夜间出站, 夜切, 纯函数, 跨平台, 时间表, config]
---

## Overview

`config/outbound_switch.rs` 是「夜间出站自动切换」的跨平台判定纯函数层：与运营商夜切（`night_switch`，见 [[night-operator-switch|夜间运营商切换决策]]）共用同一时间表，但动作语义正交——本模块只产出 Switch/Restore 决策，"切到哪张卡（桌面：metric 压至 1 + 禁用校园网卡 + 加兜底路由）"与"注销+重检（安卓）"由各端调用方实现。切换态由配置自身承载：`outbound_metric_restore` / `outbound_disabled_adapters` / `outbound_standby_route` 三份快照任一非空即切换态（经 `outbound_restore_active` 折算），幂等无需去重标记。

本模块在 `tauri-app/src-tauri/src/config/mod.rs:3` 注册，随 `config` 模块对安卓端可见（path 依赖）。

桌面侧读写底座与接线均已落地：读 `platform::metric::read_interface_metrics`（`platform/metric.rs:56`，免提权快照）、写经 helper `HelperOp::SetMetric`（`helper/mod.rs:179`）提权执行 `SetIpInterfaceEntry`（见 [[desktop-platform]]、[[desktop-helper-update]]）。排程消费方是 `monitor/scheduled.rs`：`outbound_action_for`（scheduled.rs:276）做判定编排——快照残留但功能已关闭时强制 Restore 清理（scheduled.rs:290-292，不受冻结）、`outbound_manual_hold_day` 命中当日时冻结自动 Switch/Restore（scheduled.rs:297）——随后由 `apply_outbound_switch`（scheduled.rs:538）执行三件套切换；`needs_replay`（scheduled.rs:383）用切换态∧失败历史决定未生效补齐重放；夜间看门狗 `watchdog_re_disable_campus`（scheduled.rs:897）与启动对账 `reconcile_outbound_on_startup`（scheduled.rs:1313）均先经本模块判定切换态/守护窗口。守护窗口另被 `monitor/adapter_watch.rs:186`（窗内不出自动启用目标）与 `monitor/background_check.rs:50`（切换态跳过巡检）消费。字段对照与写入坑位见 [[set-ip-interface-entry-metric]]。

## Key Components

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `outbound_switch.rs:10` | pub enum | `NightOutboundAction { None, Switch, Restore }` | 本拍动作；`None` 含"切换态凌晨/夜间保持" |
| `outbound_switch.rs:27` | pub fn | `evaluate_night_outbound(enabled, weekday, now_minutes, restore_active)` | 判定纯函数：`!enabled` → None；`restore_active` 且 now ∈ [450, 1380) → Restore；`!restore_active` 且 now < 450（凌晨）→ Switch（补切：机器在切点前后睡眠/关机错过切换时，凌晨视为前一晚切换窗尾部）；`!restore_active` 且当日有切换时刻且 now ≥ 时刻 → Switch（过点补触发）；其余（白天、切换态夜间保持）→ None |
| `outbound_switch.rs:53` | pub fn | `is_night_outbound_guard_window(enabled, weekday, now_minutes)` | 守护窗口判定（桌面巡检闸门）：`!enabled` → false；now < 450 → true（凌晨分支覆盖前一晚切换时刻之后全部时段，跨日天然成立）；当日有切换时刻且 now ≥ 时刻 → true |
| `outbound_switch.rs:68` | pub fn | `outbound_restore_active(metric_restore, disabled_adapters, standby_route)` | 切换态判定：三份快照（metric / 禁用名单 / 兜底路由）任一非空 → true |
| `night_switch.rs:10-12` | pub(crate) const | `RESTORE_START_MINUTES=450` / `RESTORE_END_MINUTES=1380` | 恢复窗口 [07:30, 23:00)，两端共用 |
| `night_switch.rs:26` | pub(crate) fn | `switch_time_for(weekday)` | 时间表：weekday 0..=4 → 1380（23:00），5/6 → 1410（23:30），其余 None |

## Patterns & Conventions

- **时间表单点**：出站切换与运营商夜切共用 `switch_time_for`，禁止两处硬编码漂移；运营商时刻调整时只改 `night_switch.rs` 一处。
- **判定与状态解耦**：纯函数只吃 `restore_active: bool`（调用方用 `outbound_restore_active` 把配置中三份快照折算传入，见 scheduled.rs:259 的 `outbound_switch_active`），本模块不直接读配置，方便单测与跨端复用。
- **边界语义**：恢复窗口 [07:30, 23:00) 下界 450 含、上界 1380 不含（恰为最早切换时刻，两窗口无缝衔接）。凌晨（now < 450）未切换态补切 Switch——与运营商夜切"凌晨不补"刻意不同：出站线路上凌晨校园线路已断、热点可用，切在 06:29 也会在 1 分钟后随恢复窗还原，无害（`outbound_switch.rs:22-24` 注释）。
- **守护窗口与恢复窗口的分工**：守护窗口 [当日切换时刻（含过点补触发），次日 07:30 恢复窗开) 内，巡检闸门（adapter_watch 自动启用、夜间看门狗）不出自动启用目标、只做禁用复核；还原统一交给恢复窗的 scheduled.rs（`outbound_switch.rs:46-52` 注释）。

## Learnings

- 判定纯函数层先于调用方落地时，`pub fn` 会报 `never used` 警告——属任务序列中间态；接线（scheduled / adapter_watch / background_check）完成后消失，未加 `#[allow(dead_code)]` 掩盖。同序列的 `platform/metric.rs::read_interface_metrics` 与 `MetricRow` 同理。
- 守护窗口的由来（2026-09-27 夜实证）：用户在切换前手动禁用的手选适配器无 IP、进不了六期巡检闸门名单（select 只收有 IP 的卡），名单为空时闸门失效，adapter_watch 的自动启用会顶掉切换——故改为时间窗 ∧ 切换态双条件（adapter_watch.rs:177-194），叠加切换态判定避免整夜休眠/应用未运行错过切换时凌晨误阻塞自动启用（纯时间窗过宽，k2.8 审计 P3-2）。
- 测试内联于源文件，无独立测试文件：`outbound_switch.rs:72-150` 六个用例（守护窗口边界 / 时间表与运营商夜切共用 / 三快照任一非空即切换态 / 过点切换与幂等 / 恢复窗口触发与窗口外不还原 / 凌晨补切）；`night_switch.rs:112-291` 为运营商夜切与 chkstatus 解析用例（归属 [[night-operator-switch|夜间运营商切换决策]]）。
