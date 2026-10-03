---
title: "定时登录/注销的判定点必须置于静默期与巡检分档之外"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/schedule.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - android/src-tauri/src/monitor_loop.rs
tags: [决策, 定时动作, 静默期, 双端, 调度]
---

## 背景

新增「每日定时登录 / 定时注销」时，最自然的落点是复用既有每拍巡检循环（桌面 `run_check_once`、安卓 `monitor_tick_loop` 内的 `run_check_once`）。但既有巡检有两道"省电/省流"闸门：

1. **校园网检测静默期**（`campus_check_start_minutes` / `campus_check_end_minutes`）：时段外整拍跳过校园网验证。安卓实现位于 `run_check_once` 入口附近（`android/src-tauri/src/monitor_loop.rs:1269-1289`，`campus_check_start_minutes > 0` 时 `minutes_now < start` 或 `>= end` 即整拍跳过、通知码 3）
2. **安卓巡检分档**（`effective_interval_ms`，`android/src-tauri/src/monitor_loop.rs:83-90`）：亮屏+WiFi 才用基础间隔，否则取 `idle_ms.max(base)`；跳拍判定在 `monitor_tick_loop`（`android/src-tauri/src/monitor_loop.rs:604-610`）

## 决策

定时动作的判定**独立于上述两道闸门**：

- **桌面**用独立循环 `monitor/scheduled.rs::run_scheduled_action_loop`（`tauri-app/src-tauri/src/monitor/scheduled.rs:116`），30s 一拍（`SCHEDULED_TICK_MS = 30_000`，`scheduled.rs:49`），随 `run_startup_tasks` 无条件 spawn（`tauri-app/src-tauri/src/monitor/watcher.rs:54-55`）——注释明确不受 `enable_background_check` 开关管（`watcher.rs:51-52`），"定时动作与巡检是两个功能"。
- **安卓**把 `run_scheduled_actions` 放在 `monitor_tick_loop`（`android/src-tauri/src/monitor_loop.rs:580`）每拍**第一位**执行（`monitor_loop.rs:593`），先于间隔热更新、分档跳拍（`:604-610`）与 `run_check_once` 内的静默期闸门；`run_check_once` 入口注释明示 `run_scheduled_actions` 不经过其出站切换态整轮跳过闸（`:1259-1263`）。
- 两端判定共用同一纯函数 `config::schedule::should_fire_scheduled_action`（`tauri-app/src-tauri/src/config/schedule.rs:18`），模块注释声明本模块在 `lib.rs` 无 cfg 门控、跨平台共用同一实现与同一组测试（`schedule.rs:3-4`）。
- 该循环**同时承载夜间出站切换编排**（桌面 `run_scheduled_action_loop` 内的出站 Switch/Restore 与运营商夜切，`scheduled.rs:213-215` 注释引用本文；安卓 `run_scheduled_actions` 内的夜出站切换与 `evaluate_night_switch`，`monitor_loop.rs:748-892`）——两者同取位、同「独立于静默期闸门」原则。

## 理由

定时动作的典型用户场景恰恰落在这些闸门覆盖的时段内——例如「23:30 自动注销」天然处于夜间（静默期 + 灭屏闲时档）。若把判定放进 `run_check_once`：

- 静默期命中 → 整拍 return，定时注销当天静默失效，用户看不到任何反馈
- 灭屏跳拍 → 拍点间隔从基础值拉长到闲时档级，虽有过点补触发兜底，但失效窗口被放大

定时动作是**用户显式配置的一次性时点行为**，与"省电省流"目标不冲突（判定本身是纯内存计算，不产生网络请求——只有命中时才走一次登录/注销；安卓侧还有双禁用早退：两目标均 `>= 1440` 时免读时钟直接 return，`monitor_loop.rs:893-895`）。因此它不应被巡检的节流闸门约束。

## 影响与约束

- **判定纯函数与禁用哨兵**：`should_fire_scheduled_action(now_minutes, target_minutes, last_fired_day, today_day)`（`schedule.rs:18`）：`target_minutes >= 1440` 禁用哨兵永不触发（`schedule.rs:19-21`；2026-09-20 起 0 表示真实的 00:00 时刻、禁用改用哨兵 1440，存量 0 由双端 schema 迁移一次性刷为 1440，`schedule.rs:7-8`）；`now < target` 不触发；否则 `last_fired_day != today_day` 即触发。跨天重置靠日期序号天然实现。
- **过点补触发**是配套设计：`now_minutes >= target_minutes` 即触发（而非等值比较），因为循环拍间隔、系统休眠都可能错过精确分钟；等值比较会让功能整天静默失效。见 `[[config-schedule-pure-function]]` 的纯函数注释。
- **当日去重**靠日期序号标记（`num_days_from_ce`，0=从未触发）：命中即置标记，避免凭据错误/非校园网环境下每拍重发请求。桌面为 `state.scheduled.login_day/logout_day`，由 `evaluate_and_mark`（`scheduled.rs:1376-1396`）统一判定并落标记；安卓为 `MONITOR.scheduled_login_day/scheduled_logout_day`（`monitor_loop.rs:53-55`，`:911`/`:930` 置标记）。
- **与夜间出站切换的门控关系**：桌面 `gated_night_action`（`scheduled.rs:1403-1417`）——`fire_login = !outbound_active && should_fire_scheduled_action(...)`，`fire_logout` 不受切换态影响；切换态下跳过定时登录且**不置当日标记、不消耗当日额度**（`evaluate_and_mark`，测试 `scheduled.rs:1722`）。安卓同构：定时登录要求 `!outbound_active`（`monitor_loop.rs:903-909`），定时注销照常（`:924-929`）；夜出站切换与运营商夜切在 `run_scheduled_actions` 内先于双禁用早退执行（`:760-892`）。
- **与既有自动登录/断线重连/自动退出的隔离**：判定只依赖配置目标值 + 自身当日标记，不读写冷却（`last_auto_login_attempt`）、重连计数、`has_logged_online`。动作互斥：桌面登录抢 `tasks.is_logging_in`、注销抢 `tasks.is_logging_out`（`spawn_login_action` `scheduled.rs:1435-1459` 的 `:1441`、`perform_logout` `scheduled.rs:1505-1534` 的 `:1508`），抢不到则本拍跳过、下拍过点补触发兜底；安卓定时登录复用 `auto_login_on_start` 全编排（`monitor_loop.rs:921`，凭据检查/强制绑 WiFi/校园网探测，注释 `:739`）、定时注销复用 `protocol_cmds::do_logout`（`:941`）。
- **定时注销成功后必须对齐全量注销后处理**，否则会被"可登录即自动登录"立刻登回，形成拉锯：桌面 `perform_logout` 成功后置 `exit.auto_exit_cancelled = true`（`scheduled.rs:1523`）、取消自动退出 deadline、`failure_tracker::reset_all`、`network.update{has_logged_online=false, disconnect_reconnect_count=0, last_auto_login_attempt=now, logout_protected_until=now+60s}`（`:1523-1532`，即 60s 注销保护期）；安卓 `do_logout` 同样含两步注销 / login_history 落账 / 60s 保护期语义（`monitor_loop.rs:741` 注释）。
- **测试同文件**（无独立测试文件）：`schedule.rs:28-74` 6 例（禁用哨兵/00:00 即触发/未到点/到点触发/同日去重/跨天重置）；`scheduled.rs:1660-1905`（禁用目标不触发 `:1682`、到点触发置标记 `:1692`、跨天重置 `:1712`、切换态不消耗当日标记 `:1722`、`gated_night_action` 门控 `:1749`、退避/重放/快照/hold 纯函数 `:1768-1904`）；`monitor_loop.rs:1487-1607`（通知状态码、`should_attempt_login`、分档 3 例等）。

## Connections

[[config-schedule-pure-function]]、[[background-check-and-auto-login]]、[[deliberate-background-check-skips]]、[[config-field-sets-bidirectional-sync]]
