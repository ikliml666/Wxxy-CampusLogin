---
title: "自动切换运营商（原晚间断网自动切换）"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/night_switch.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/frontend/src/settings/types.ts
  - tauri-app/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - tauri-app/frontend/src/monitor/StatusBar.tsx
  - tauri-app/frontend/src/monitor/StatusBar.onlineOperator.test.tsx
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/account_cmds.rs
  - android/frontend/src/settings/types.ts
  - android/frontend/src/settings/constants.ts
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/monitor/StatusBar.tsx
tags: [决策, 夜切, 运营商, 定时任务, 双端同构]
---

## 背景

**改名说明（2026-09-22）**：本功能原 UI 名「晚间断网自动切换」，随「夜间出站自动切换」功能（[[night-outbound-switch]]）落地而更名为「自动切换运营商」——整体断网场景由出站切换接管，本功能只覆盖运营商线路断网，名字与语义对齐。**仅改文案与本文档标题，配置字段名 `nightOperatorSwitch` 等一律不变**（兼容性）；时间表与下文 [[night-outbound-switch]] 共用。

校园网（无锡学院）的运营商线路有固定断网规律：**周日至周四 23:00 与周五、周六 23:30**之后，电信/移动/联通服务下线无法上网（2026-09-20 需求方更正时间表：原实现仅周日/周一 23:00，周二~周四无切换；`switch_time_for` 已改为 `0..=4 => 1380`、`5 | 6 => 1410`），需把登录运营商切到"无锡学院"（`config.operator = ''`）才能继续上网；次日早晨线路恢复后应切回原运营商。2026-09-19 与需求方确认形态为**自动定时切换**：一个开关（默认关），开启后到点自动切换，无需手动。

## 设计

### 配置字段（双端共有）

| 字段 | 默认 | 含义 |
| --- | --- | --- |
| `enableNightOperatorSwitch` | `false`（2026-10-02 起默认改回关闭；2026-09-20~10-01 期间默认 true，存量落盘 true 不迁移） | 夜切总开关（桌面在 AccountPanel 自动化开关卡内，安卓在 SettingsPanel 自动化分组卡内，见下文接入点） |
| `nightOperatorRestore` | `''` | 切至无锡学院前暂存的原运营商；非空即"处于切换态" |

- 桌面字段定义：`config/model.rs` `enable_night_operator_switch`（serde rename `enableNightOperatorSwitch`，`default`）:57-60、`night_operator_restore`（rename `nightOperatorRestore`）:61-63；`Config::default()` 中 `enable_night_operator_switch: false` :262。前端镜像 `settings/types.ts`:31-34、`settings/constants.ts` `DEFAULT_CONFIG`:25-26。
- 安卓字段定义：`config_state.rs` `Settings::enable_night_operator_switch` :29（文档注释 :26-28，标注复用共享 crate 纯函数）、`night_operator_restore` :31；`Default` 中 false :123-125。前端镜像 `android/frontend/src/settings/types.ts`:20-23、`constants.ts` `DEFAULT_CONFIG`:13-14。

字段落地时（2026-09-19）无 schema 版本迁移；2026-09-20 默认 false→true 随「旧默认值语义变化」走桌面 v4→v5（`config/validate.rs:163-174`，`if !config.enable_night_operator_switch { config.enable_night_operator_switch = true }` :164-166）/ 安卓 v6→v7（`config_state.rs::migrate_legacy_defaults` :353-365，同式 :354-356，随定时登录/注销 0→1440 哨兵一起）迁移一次性刷值（上线一天即改默认，显式设过 false 的极少数会被误刷，先例语义）。2026-10-02 默认改回 false（用户裁定回归默认关闭）：**存量落盘值不再迁移**——沿用 [[lightweight-mode-desktop]] 的 auto_exit 先例，已显式落盘的 true 保持不动，由用户在设置页自行关闭；v4→v5 / v6→v7 迁移块保留原样（迁移记录语义，更旧的存量配置升级仍按当时默认刷开）。测试锁定：桌面 `validate.rs:648`（默认关断言 `"夜切默认关闭(2026-10-02)"`）、:653-673（v4→v5 迁移刷开且用户显式值不被覆盖）；安卓 `config_state.rs:604`（默认关）、:651（v7 迁移刷开）。

### 时间表与判定（共享纯函数）

判定集中在桌面 crate 跨平台模块 `config/night_switch.rs::evaluate_night_switch(enabled, weekday, now_minutes, current_operator, restore_operator)`（:51-73），安卓经 path 依赖复用（`campus_login_lib::config::night_switch`），禁止复制：

- 切换时刻：`switch_time_for(weekday)`（:26-32）——weekday（`num_days_from_sunday()`，0=周日）`0..=4` → 1380 分钟（23:00，2026-09-20 起含周二~周四）、`5 | 6` → 1410 分钟（23:30），每天均有切换时刻。
- 恢复窗口：常量 `RESTORE_START_MINUTES = 450`（:10）、`RESTORE_END_MINUTES = 1380`（:12），每日 `[450, 1380)`（07:30 起，2026-10-02 839d3a0 由 06:30 调整、早于最早切换点 23:00），与切换时刻无缝衔接。
- 切换条件：开关开 + 当日有切换时刻 + `now_minutes >= 时刻` + **当前 operator 在白名单**（`is_switchable_operator` :37-39：`@telecom`/`@unicom`/`@cmcc`）→ `NightSwitchAction::SwitchToCampus`（当前值写入 restore、operator 置空）。
- 恢复条件：开关开 + 恢复窗口内 + restore 非空 + 当前 operator 为空 → `NightSwitchAction::Restore`（取回并清空 restore）。
- 白名单拦截账号档案里的脏 operator 值（`switch_account` 不做 operator 校验，曾有测试夹具用 `"校园"`，`commands/account.rs:437-445`）。
- 辅助纯函数：`parse_chkstatus`（:91-104，剥 JSONP 壳解析 eportal 在线表）与 `uid_matches`（:108-110，学号+运营商后缀精确比对）供双端验证链复用；内联测试 :112-291 锁定窗口边界、白名单、幂等与 chkstatus 样例。出站切换模块 `config/outbound_switch.rs:7` 直接 import 本模块的时间表常量与 `switch_time_for`，测试 :96-103 锁定 1380/1410 时间表共用。

### 幂等：切换态由配置自身承载

不引入"当日已触发"去重标记：restore 非空 + operator 空 = 已切换态，纯函数重复判定自然返回 None；落盘失败时内存不生效、下一拍（桌面 30s `SCHEDULED_TICK_MS`，`monitor/scheduled.rs:49`；安卓随巡检 tick）重新判定即天然重试。比定时登录的当日标记更简单，且跨天正确。

### 接入点（两端不同，动作语义一致）

- **桌面**：`monitor/scheduled.rs` 的 30s 无条件循环 `run_scheduled_action_loop`（:116-233，与后台检测开关无关，见 [[scheduled-actions-outside-silent-window]]）。出站切换动作先行（:167-209），其后 `outbound_switch_active` 成立时运营商夜切让位（:212-218），否则 `evaluate_night_switch` 判定（:221-227）并 `apply_night_switch_action`（:229-231）。`apply_night_switch_action`（:1464-1491）：SwitchToCampus 时暂存 operator→置空、Restore 时取回 restore，经 `save_config_to_disk_encrypted` 落盘（内含 config-changed 广播与托盘刷新）后 `spawn_login_action`（:1435-1459，`is_logging_in` 互斥、`full_login` 无探测闸，与定时登录共用执行体）并 spawn 验证任务。
- **安卓**：`monitor_loop.rs::run_scheduled_actions`（:748，由 `monitor_tick_loop` :580-604 每拍驱动 :593，位于分档跳拍与静默期闸门之前）。出站判定先行（:753-778，含"功能关但标记残留→Restore 兜底"），出站动作后重读最新配置（:841-843，`latest_settings` :629-636 防陈旧快照复活标记）；运营商夜切段门控 `if settings.enable_night_operator_switch && !outbound_active`（:852），`evaluate_night_switch`（:856-862）后按动作分支：SwitchToCampus :864-877 / Restore :878-888——改动内存快照 → `persist_settings`（:955-978：`save_to` 密文落盘 :964 + `AndroidState.config` 缓存刷新 :968-971 + `auto_create_account_for_current` 账号档案同步 :972 + `emit_config_changed` 广播 :976）成功才执行 `night_switch_login`（:984-1010，**直接登录**保留账号为空防御 :985-988、强制绑 WiFi :990、`run_login` 内核 :992，成功置 `was_online` 并清注销保护期 :1000-1001）**有意绕过 `auto_login_on_start` 的"不在校园网则跳过"探测闸**——夜切语义是"强制重新认证"，静默跳过会让功能无声失效。
- **登录生效自动验证（双端同构）**：等 15s（eportal 在线表非即时）→ 查 `{portal_origin}/drcom/chkstatus?callback=dr1003` 核对在线账号 uid 与期望运营商后缀（重试 3 次、间隔 5s、单次超时 15s；桌面常量 `monitor/scheduled.rs:51-56`、安卓 `monitor_loop.rs:1109-1112`）→ 仍不符则注销重登一轮再复验（复登前复查 operator 防用户已手动改走）→ 最终不符发 warning 日志 + 系统通知「夜切验证失败」，生效写 success 日志。实现：桌面 `verify_night_switch` :1540-1577、`night_switch_verify_round` :1589-1607、`night_switch_check_once` :1610-1634；安卓 `verify_night_switch` :1135-1189（另加出站切换态双闸 :1138-1140/:1158-1160，避免打断出站等待窗口）、`night_switch_verify_round` :1193-1210、`night_switch_check_once` :1213-1242。uid 核对复用共享纯函数 `config::night_switch::{parse_chkstatus, uid_matches}`。

### UI 呈现（夜切临时态徽标）

双端 StatusBar 前端按 config 自行检测夜切临时态：`configOperator === '' && nightOperatorRestore !== ''`（桌面 `monitor/StatusBar.tsx:46,53-56`、安卓 `android/frontend/src/monitor/StatusBar.tsx:26,41-44`）。徽标始终显示真实在线运营商，夜切临时态仅在 tooltip 标注恢复目标；渲染级测试 `StatusBar.onlineOperator.test.tsx:62-70` 断言临时态下 chip 显示真实线路且 Moon 图标不进 chip。

### 配套修复

- **安卓 `save_config` 命令补发 `config-changed`**（`config_state.rs:451`，payload 为 `{ "config": masked }`，掩码复用 `masked_for_display`，事件构造 `emit_config_changed` :388-391）：此前安卓后端从不广播该事件，夜切后前端旧快照会把新配置覆盖回去（全量回写），且 UI 不反映切换态。
- **桌面 `config-changed` payload 补 `{ "config": ... }` 包裹**（`config_cmd.rs:15-18`，`notify_config_changed(&json!({ "config": emit_cfg }))`）：M1 重构（EventBus 收敛）后丢了包裹直发裸 Config，两端前端监听契约均为 `data.config`，导致桌面监听静默失效；夜切功能首次依赖此事件，故随本次修复。
- **切账号清空 `nightOperatorRestore`**（两端 `switch_account`：桌面 `commands/account.rs:70-72`、安卓 `account_cmds.rs:188-190`）：恢复目标属于账号语义，跨账号残留会把 A 账号的运营商后缀恢复到 B 账号。安卓同函数一并清出站切换态标记 `night_outbound_restore`（:191-193）并收尾 `restore_avoid_bad_wifi` 快照（:204）。

## 已知边界（有意不修）

- **午夜后不补触发**：00:00~07:30 之间启动 App 不会切换（当日切换时刻判定 `now_minutes >= 时刻` 只覆盖到当日 23:59），07:30 恢复窗口自愈。影响极小，不引入跨天状态。
- ~~**桌面 `autoExitAfterLogin` 默认 true**：夜切登录成功即自动退出，恢复被推迟到用户下次启动 App~~（2026-09-20 边界消除：该开关默认值随后台留存优化改为 false，见 [[lightweight-mode-desktop]]；存量配置显式落的 true 不迁移，需用户在设置页自行关闭。桌面 AccountPanel 自动化卡内该开关入口也已移除，见 `AccountPanel.tsx:487-489` 注释）。
- **安卓依赖后台检测循环**：`run_scheduled_actions` 由 `monitor_tick_loop` 驱动，而该循环仅在 `enable_background_check` 开启时于 `run_startup_tasks` 启动（`monitor_loop.rs:429-434`）——开关关闭则夜切不运行（桌面循环无条件启动）。文案已提示。

## Connections

[[config-schedule-pure-function]]、[[scheduled-actions-outside-silent-window]]、[[dual-platform-sharing]]、[[config-field-sets-bidirectional-sync]]、[[night-outbound-switch]]、[[lightweight-mode-desktop]]、[[learnings/gen-schemas-crlf-diff-noise]]
