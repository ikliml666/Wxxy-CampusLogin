---
title: "定时登录/注销的判定点必须置于静默期与巡检分档之外"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/schedule.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - android/src-tauri/src/monitor_loop.rs
tags: [决策, 定时动作, 静默期, 双端, 调度]
---

## 背景

新增「每日定时登录 / 定时注销」时，最自然的落点是复用既有每拍巡检循环（桌面 `run_check_once`、安卓 `monitor_tick_loop` 内的 `run_check_once`）。但既有巡检有两道"省电/省流"闸门：

1. **校园网检测静默期**（`campus_check_start_minutes` / `campus_check_end_minutes`）：时段外整拍跳过校园网验证
2. **安卓巡检分档**（`effective_interval_ms`）：蜂窝或灭屏时按 `backgroundCheckIdleInterval` 跳拍

## 决策

定时动作的判定**独立于上述两道闸门**：桌面对话用独立循环 `monitor/scheduled.rs`（30s 一拍，随 `run_startup_tasks` 无条件启动）；安卓把 `run_scheduled_actions` 放在 `monitor_tick_loop` 中**分档跳拍判定与静默期之前**。

## 理由

定时动作的典型用户场景恰恰落在这些闸门覆盖的时段内——例如「23:30 自动注销」天然处于夜间（静默期 + 灭屏闲时档）。若把判定放进 `run_check_once`：

- 静默期命中 → 整拍 return，定时注销当天静默失效，用户看不到任何反馈
- 灭屏跳拍 → 拍点间隔从 60s 拉长到 5min 级，虽有过点补触发兜底，但失效窗口被放大

定时动作是**用户显式配置的一次性时点行为**，与"省电省流"目标不冲突（判定本身是纯内存计算，不产生网络请求——只有命中时才走一次登录/注销）。因此它不应被巡检的节流闸门约束。

## 影响与约束

- 「过点补触发」是配套设计：`now_minutes >= target_minutes` 即触发（而非等值比较），因为循环拍间隔、系统休眠都可能错过精确分钟；等值比较会让功能整天静默失效。见 `[[config-schedule-pure-function]]` 的纯函数注释。
- 当日去重靠 `last_fired_day`（日期序号）标记：命中即置标记，避免凭据错误/非校园网环境下每拍重发请求。
- 与既有自动登录/断线重连/自动退出的隔离：判定只依赖配置目标值 + 自身当日标记，不读写冷却（`last_auto_login_attempt`）、重连计数、`has_logged_online`；动作经 `is_logging_in`/`is_logging_out` 与手动登录、托盘、掉线重连同锁互斥（抢不到则本拍跳过、下拍过点补触发兜底）。
- 定时注销成功后必须对齐全量注销后处理（取消自动退出、重置重连计数、60s 注销保护期），否则会被"可登录即自动登录"立刻登回，形成拉锯。

## Connections

[[config-schedule-pure-function]]、[[background-check-and-auto-login]]、[[deliberate-background-check-skips]]、[[config-field-sets-bidirectional-sync]]
