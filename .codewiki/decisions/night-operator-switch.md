---
title: "晚间断网自动切换运营商（夜切功能）"
type: decision
source_files:
  - tauri-app/src-tauri/src/config/night_switch.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/frontend/src/settings/types.ts
  - tauri-app/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/account_cmds.rs
  - android/frontend/src/settings/types.ts
  - android/frontend/src/settings/constants.ts
  - android/frontend/src/account/AccountPanel.tsx
tags: [决策, 夜切, 运营商, 定时任务, 双端同构]
---

## 背景

校园网（无锡学院）的运营商线路有固定断网规律：**周日至周四 23:00 与周五、周六 23:30**之后，电信/移动/联通服务下线无法上网（2026-09-20 需求方更正时间表：原实现仅周日/周一 23:00，周二~周四无切换；`switch_time_for` 已改为 `0..=4 => 1380`、`5 | 6 => 1410`），需把登录运营商切到"无锡学院"（`config.operator = ''`）才能继续上网；次日早晨线路恢复后应切回原运营商。2026-09-19 与需求方确认形态为**自动定时切换**：一个开关（默认关），开启后到点自动切换，无需手动。

## 设计

### 配置字段（双端共有）

| 字段 | 默认 | 含义 |
| --- | --- | --- |
| `enableNightOperatorSwitch` | `false` | 夜切总开关（AccountPanel 自动化开关卡第 5 个开关） |
| `nightOperatorRestore` | `''` | 切至无锡学院前暂存的原运营商；非空即"处于切换态" |

无 schema 版本迁移：serde 容器级 `default` 使旧配置缺字段回退默认值（与 `scheduledLoginMinutes` 落地先例一致，桌面 `configVersion` 保持 3、安卓保持 5）。

### 时间表与判定（共享纯函数）

判定集中在桌面 crate 跨平台模块 `config/night_switch.rs::evaluate_night_switch(enabled, weekday, now_minutes, current_operator, restore_operator)`，安卓经 path 依赖复用，禁止复制：

- 切换时刻：weekday（`num_days_from_sunday()`，0=周日）0..=4 → 1380 分钟（23:00，2026-09-20 起含周二~周四），5/6 → 1410 分钟（23:30），每天均有切换时刻。
- 恢复窗口：每日 `[390, 1380)`（06:30 起、最早切换点 23:00 前），与切换时刻无缝衔接。
- 切换条件：开关开 + 当日有切换时刻 + `now_minutes >= 时刻` + **当前 operator 在白名单**（`@telecom`/`@unicom`/`@cmcc`）→ SwitchToCampus（当前值写入 restore、operator 置空）。
- 恢复条件：开关开 + 恢复窗口内 + restore 非空 + 当前 operator 为空 → Restore（取回并清空 restore）。
- 白名单拦截账号档案里的脏 operator 值（`switch_account` 不做 operator 校验，曾有测试夹具用 `"校园"`）。

### 幂等：切换态由配置自身承载

不引入"当日已触发"去重标记：restore 非空 + operator 空 = 已切换态，纯函数重复判定自然返回 None；落盘失败时内存不生效、下一拍（30s）重新判定即天然重试。比定时登录的当日标记更简单，且跨天正确。

### 接入点（两端不同，动作语义一致）

- 桌面：`monitor/scheduled.rs` 的 30s 无条件循环（与后台检测开关无关，见 [[scheduled-actions-outside-silent-window]]）；登录执行体抽为 `spawn_login_action` 与定时登录共用（`is_logging_in` 互斥、`full_login` 无探测闸）。
- 安卓：`monitor_loop.rs::run_scheduled_actions` 内、分档跳拍与静默期闸门之前；登录走 `night_switch_login` **直接登录**（保留账号为空防御），**有意绕过 `auto_login_on_start` 的"不在校园网则跳过"探测闸**——夜切语义是"强制重新认证"，静默跳过会让功能无声失效。落盘经 `persist_settings`（`save_to` 密文 + `AndroidState.config` 缓存刷新 + 账号档案同步）并 `emit_config_changed` 广播。

### 配套修复

- **安卓 `save_config` 命令补发 `config-changed`**（payload 为 `{ "config": masked }`，掩码复用 `masked_for_display`）：此前安卓后端从不广播该事件，夜切后前端旧快照会把新配置覆盖回去（全量回写），且 UI 不反映切换态。
- **桌面 `config-changed` payload 补 `{ "config": ... }` 包裹**（`config_cmd.rs`）：M1 重构（EventBus 收敛）后丢了包裹直发裸 Config，两端前端监听契约均为 `data.config`，导致桌面监听静默失效；夜切功能首次依赖此事件，故随本次修复。
- **切账号清空 `nightOperatorRestore`**（两端 `switch_account`）：恢复目标属于账号语义，跨账号残留会把 A 账号的运营商后缀恢复到 B 账号。

## 已知边界（有意不修）

- **午夜后不补触发**：00:00~06:30 之间启动 App 不会切换（当日切换时刻判定 `now_minutes >= 时刻` 只覆盖到当日 23:59），06:30 恢复窗口自愈。影响极小，不引入跨天状态。
- ~~**桌面 `autoExitAfterLogin` 默认 true**：夜切登录成功即自动退出，恢复被推迟到用户下次启动 App~~（2026-09-20 边界消除：该开关默认值随后台留存优化改为 false，见 [[lightweight-mode-desktop]]；存量配置显式落的 true 不迁移，需用户在设置页自行关闭）。
- **安卓依赖后台检测循环**：`run_scheduled_actions` 由 `monitor_tick_loop` 驱动，`enableBackgroundCheck` 关闭则夜切不运行（桌面循环无条件启动）。文案已提示。

## Connections

[[config-schedule-pure-function]]、[[scheduled-actions-outside-silent-window]]、[[dual-platform-sharing]]、[[config-field-sets-bidirectional-sync]]、[[learnings/gen-schemas-crlf-diff-noise]]
