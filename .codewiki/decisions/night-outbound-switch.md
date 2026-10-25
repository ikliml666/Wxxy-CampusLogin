---
title: "夜间出站自动切换（出站层夜切）"
type: module
source_files:
  - tauri-app/src-tauri/src/config/outbound_switch.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/night_switch.rs
  - tauri-app/src-tauri/src/platform/metric.rs
  - tauri-app/frontend/src/network/outboundOrder.ts
tags: [决策, 夜间出站, 出站切换, metric, 夜切, 双端同构]
---

## Overview

本模块实现**夜间出站自动切换**的纯函数判定与配置承载，解决校园网（无锡学院）在周日至周四 23:00、周五六 23:30 整体断网时，将系统默认出站流量切到非校园网（如热点、物联网卡）的问题。与运营商夜切（`night_switch.rs`）共用时间表但动作语义正交：本模块只产出 `Switch`/`Restore` 决策，“切到哪张卡（桌面写 metric + 禁用校园网卡）”与“注销 + 重检（安卓）”由各端调用方实现。切换态由配置自身承载（三份快照/标记非空），幂等无需去重标记。

恢复时刻自 2026-10-26 起可配置（`Config::night_restore_minutes`，经 `restore_start_minutes` 归一化），不再硬编码 07:30。生产代码一律调用带 `_with_restore` 后缀的判定函数并显式传入配置值；旧的无后缀版本仅 `#[cfg(test)]` 可用，固定默认 07:30，留作测试用例可读。

核心源码划分：
1. **判定层** `config/outbound_switch.rs`：跨平台纯函数 `evaluate_night_outbound_with_restore` (行37-61)、`is_night_outbound_guard_window_with_restore` (行85-99)、`outbound_restore_active` (行106-108) 等。
2. **配置模型** `config/model.rs`：`Config` 中 `enable_night_outbound_switch` 总开关 (行63-64)、`night_restore_minutes` (行69-70) 及 `outbound_metric_restore` (行80-81) / `outbound_disabled_adapters` (行86-87) / `outbound_standby_route` (行91-92) / `night_outbound_restore` (行109-110) 等快照字段。
3. **时间基类** `config/night_switch.rs`：提供 `DEFAULT_RESTORE_START_MINUTES=450` (行11)、`RESTORE_END_MINUTES=1380` (行13)、`switch_time_for` (行37-43)、`restore_start_minutes` (行18-23) 等共用常数与函数。
4. **平台读取** `platform/metric.rs`：Windows 接口跃点读取 `read_interface_metrics` (行56-65)，供桌面端快照使用。
5. **前端排序** `frontend/src/network/outboundOrder.ts`：`buildOutboundOrder` (行11-29) 与后端优先级语义对齐。

## Key Components

### 1. `tauri-app/src-tauri/src/config/outbound_switch.rs`

- **枚举 `NightOutboundAction`** (行9-17)
  - `None`：本拍无需任何动作（含夜间窗口内已处于切换态）(行11-12)。
  - `Switch`：桌面调目标卡 metric=1 并写快照；安卓注销 + `report_wifi_unusable` 并写标记 (行13-14)。
  - `Restore`：桌面按快照还原 metric；安卓登录当前账号并清标记 (行15-16)。

- **函数 `evaluate_night_outbound`** (行24-33, `#[cfg(test)]` 仅测试期可用)
  - 签名：`pub fn evaluate_night_outbound(enabled: bool, weekday: u32, now_minutes: u32, restore_active: bool) -> NightOutboundAction`
  - 内部调用 `evaluate_night_outbound_with_restore` 并传入 `DEFAULT_RESTORE_START_MINUTES` (行26-32)。

- **函数 `evaluate_night_outbound_with_restore`** (行37-61, 公开生产入口)
  - 签名：`pub fn evaluate_night_outbound_with_restore(enabled: bool, weekday: u32, now_minutes: u32, restore_active: bool, restore_start: u32) -> NightOutboundAction`
  - 逻辑（纯函数）：
    - `restore_start` 经 `restore_start_minutes` 归一化 (行44)。
    - `!enabled` → `None` (行45-47)。
    - `restore_active` 且 `now_minutes ∈ [restore_start, RESTORE_END_MINUTES)` → `Restore` (行48-51)。
    - `restore_active` 且不在恢复窗 → `None` (行52-53)。
    - `!restore_active` 且 `now_minutes < restore_start`（凌晨补切，机器睡眠错过切换时视为前一晚窗口尾） → `Switch` (行54-56)。
    - `!restore_active` 且 `switch_time_for(weekday)` 有值且 `now_minutes >= 时刻`（过点补触发） → `Switch` (行57-60)。
    - 其余 → `None` (行59-60 匹配臂)。

- **函数 `is_night_outbound_guard_window`** (行72-80, `#[cfg(test)]` 仅测试期可用)
  - 签名：`pub fn is_night_outbound_guard_window(enabled: bool, weekday: u32, now_minutes: u32) -> bool`
  - 调用 `_with_restore` 版本传默认起点 (行74-79)。

- **函数 `is_night_outbound_guard_window_with_restore`** (行85-99, 公开生产入口)
  - 签名：`pub fn is_night_outbound_guard_window_with_restore(enabled: bool, weekday: u32, now_minutes: u32, restore_start: u32) -> bool`
  - 逻辑：从当日切换时刻起至次日 `restore_start` 恢复窗开为止返回 `true`。`!enabled`→false (行92-94)；`now_minutes < restore_start`→true (行95-97)；`switch_time_for(weekday).is_some_and(|t| now_minutes >= t)`→true (行98)；否则 false。注释 (行63-69) 指出用户在切换前手动禁用的手选适配器无 IP 进不了闸门名单，窗口内巡检一律不出自动启用目标，防顶掉切换（2026-09-27 夜实证）。

- **函数 `outbound_restore_active`** (行106-108, 公开)
  - 签名：`pub fn outbound_restore_active(metric_restore: &str, disabled_adapters: &str, standby_route: &str) -> bool`
  - 逻辑：三者任一非空即切换态 (行107)。注释 (行101-105)：六期起切换动作=写 metric + 禁校园网卡一次落盘；兜底路由自 2026-10-18 事故后不再创建，但字段仍参与判定，历史残留要能识别走精确清理。

- **测试模块** (行110-214)
  - `guard_window_covers_switch_time_to_restore_start` (行115-132)
  - `guard_window_respects_custom_restore_start` (行135-142)
  - `restore_window_follows_custom_start` (行145-158)
  - `switch_time_follows_operator_night_schedule` (行161-167)
  - `restore_active_covers_all_three_snapshots` (行169-176)
  - `switch_fires_after_time_when_not_in_switched_state` (行178-188)
  - `restore_fires_in_restore_window_only` (行190-199)
  - `predawn_missed_switch_is_replayed` (行201-213)

### 2. `tauri-app/src-tauri/src/config/model.rs`

- **常量** (行3-4)
  - `PASSWORD_MASK: &str = "***"` (行3)
  - `AUTO_DETECT_ADAPTER: &str = "自动检测"` (行4)

- **结构体 `Config`** (行6-200) 全部字段（容器级 `#[serde(default)]` 行9）：

  | 字段 | 类型 | 行号 | 说明 |
  |------|------|------|------|
  | user | String | 11 | 用户名 |
  | password | String | 12-13 | 密码（显示打码） |
  | self_hello_enabled | bool | 17 | Windows Hello 验证，默认 true |
  | self_reverify_each_action | bool | 20-21 | 每次操作二次验证，默认 false |
  | operator | String | 22 | 运营商后缀 |
  | adapter1 | String | 23 | 主适配器，默认 AUTO_DETECT_ADAPTER |
  | adapter2 | String | 24 | 副适配器 |
  | adapter1_account | String | 27-28 | 主适配器绑定账号 id |
  | adapter2_account | String | 30-31 | 副适配器绑定账号 id |
  | dual_adapter | bool | 32-33 | 双适配器开关 |
  | auto_login_on_start | bool | 34-35 | 启动自动登录，默认 true |
  | auto_exit_after_login | bool | 36-37 | 登录后退出，默认 false |
  | minimize_to_tray | bool | 38-39 | 最小化托盘 |
  | lightweight_mode | bool | 42-43 | 轻量化模式，默认 true |
  | hidden_start | bool | 44-45 | 隐藏启动 |
  | auto_launch | bool | 46-47 | 自启动，默认 true |
  | enable_background_check | bool | 48-49 | 后台检查，默认 true |
  | background_check_interval | u64 | 50-51 | 间隔，默认 60000 |
  | auto_login_on_preparation | bool | 52-53 | 准备时自动登录，默认 true |
  | enable_night_operator_switch | bool | 56-57 | 运营商夜切总开关，默认 false |
  | night_operator_restore | String | 59-60 | 原运营商暂存，非空=切换态 |
  | enable_night_outbound_switch | bool | 63-64 | 出站夜切总开关，默认 false |
  | night_restore_minutes | u32 | 69-70 | 夜间切换恢复时刻（当日分钟数），0=未自定义回落默认 07:30，安卓消费 |
  | outbound_priority | Vec<String> | 75-76 | 出站网卡优先级，空=未排序，安卓不消费 |
  | outbound_metric_restore | String | 80-81 | 目标卡 metric 快照 JSON，非空=桌面切换态，导入清空 |
  | outbound_disabled_adapters | String | 86-87 | 禁用校园网卡快照 JSON，非空=待还原禁用动作 |
  | outbound_standby_route | String | 91-92 | 兜底默认路由快照 JSON，空=未加路由 |
  | outbound_manual_hold_day | i32 | 98-99 | “立即切换”当日保持标记，0=无，次日清零 |
  | outbound_pending_switch | String | 105-106 | 待切换状态残留 JSON，本版本不再产生，排程清空 |
  | night_outbound_restore | String | 109-110 | 安卓切换态标记恒 "logged_out"，桌面不消费 |
  | dns_optimize_adapters | Vec<String> | 114-115 | DNS 优化目标适配器，安卓不消费 |
  | device_aliases | HashMap<String,String> | 119-120 | 设备自定义名，与安卓同构 |
  | auto_exit_on_online | bool | 121-122 | 在线退出，默认 false |
  | theme_mode | String | 123-124 | 主题，默认 "dark" |
  | enable_notification | bool | 125-126 | 通知，默认 true |
  | active_account | String | 127-128 | 活跃账号 |
  | display_name | String | 131-132 | 显示名 |
  | enable_latency_test | bool | 133-134 | 延迟测试，默认 true |
  | latency_test_interval | u64 | 135-136 | 延迟间隔，默认 600000 |
  | custom_theme_color | String | 137-138 | 主题色 |
  | default_panel | String | 139-140 | 默认面板 |
  | enable_network_quality | bool | 141-142 | 网络质量，默认 true |
  | skip_ttfb_in_latency | bool | 143-144 | 跳过 TTFB，默认 true |
  | skip_content_in_latency | bool | 145-146 | 跳过内容，默认 true |
  | portal_url | String | 147-148 | Portal URL，默认 "http://10.1.99.100" |
  | fixed_gateway | String | 149-150 | 固定网关，默认 "10.2.127.254" |
  | required_network_name | String | 151-152 | 必选网络名 |
  | enable_network_name_check | bool | 153-154 | 网络名检查，默认 true |
  | campus_gateway | String | 155-156 | 校园网关，默认 "10.2.127.254" |
  | update_source | String | 158-159 | 更新渠道，默认 "mirror" |
  | campus_exit_on_fail | bool | 160-161 | 失败退出，默认 true |
  | campus_exit_start_minutes | u16 | 163-164 | 退出时段起点，默认 480 |
  | campus_exit_end_minutes | u16 | 166-167 | 退出时段终点，默认 1380 |
  | campus_check_start_minutes | u16 | 168-169 | 检测起点，默认 460 |
  | campus_check_end_minutes | u16 | 171-172 | 检测终点，默认 1380 |
  | scheduled_login_minutes | u16 | 174-175 | 定时登录，默认 1440 |
  | scheduled_logout_minutes | u16 | 177-178 | 定时注销，默认 1440 |
  | log_retention_days | u32 | 179-180 | 日志保留，默认 7 |
  | max_disconnect_reconnect | u32 | 181-182 | 最大重连，默认 3 |
  | auto_login_cooldown_secs | u64 | 183-184 | 冷却，默认 60 |
  | skip_sha256_when_missing | bool | 185-186 | 跳过 sha，默认 false |
  | vpn_account | String | 188-189 | WebVPN 账号 |
  | vpn_password_enc | String | 192-193 | WebVPN 密码密文 |
  | self_service_via_vpn | bool | 196-197 | 自助服务 VPN 回退，默认 true |
  | config_version | u32 | 198-199 | 配置版本，默认 5 |

- **反序列化辅助** (行202-226)
  - `deserialize_non_empty_or` (行202-212)
  - `deserialize_campus_gateway` (行214-219)
  - `deserialize_required_network_name` (行221-226)

- **默认函数** (行228-260)
  - `default_true` (行228), `default_campus_check_start_minutes` (行230), `default_campus_check_end_minutes` (行234), `default_campus_exit_start_minutes` (行236), `default_campus_exit_end_minutes` (行238), `default_fixed_gateway` (行240-242), `default_log_retention_days` (行244), `default_max_disconnect_reconnect` (行246), `default_auto_login_cooldown_secs` (行248), `default_portal_url` (行250-252), `default_required_network_name` (行254-256), `default_campus_gateway` (行258-260)

- **`impl Default for Config`** (行262-344) 初始化上述默认值（含夜切相关默认：enable_night_outbound_switch=false 行291，night_restore_minutes=0 行293，outbound_* 快照空 行295-299，night_outbound_restore="" 行302，config_version=5 行341）。

- **`impl Config`** (行346-366)
  - `masked_for_display` (行350-354)：返回打码副本。
  - `mask_in_place` (行356-365)：密码/VPN密文非空替换为 `PASSWORD_MASK`。

- **测试模块** (行368-433)
  - `default_reconnect_and_cooldown_config` (行372-377)
  - `serde_missing_reconnect_field_uses_default` (行379-389)
  - `serde_custom_reconnect_config` (行391-399)
  - `serde_account_fields_json_names_and_defaults` (行403-432)

### 3. `tauri-app/src-tauri/src/config/night_switch.rs`

- **常量** (行11-13)
  - `DEFAULT_RESTORE_START_MINUTES: u32 = 450` (行11)
  - `RESTORE_END_MINUTES: u32 = 1380` (行13)

- **函数 `restore_start_minutes`** (行18-23, 公开)
  - 签名：`pub fn restore_start_minutes(configured: u32) -> u32`
  - 逻辑：配置值 0 或 ≥ RESTORE_END_MINUTES 回落默认 450，否则原样 (行19-22)。

- **枚举 `NightSwitchAction`** (行25-33)
  - `None` (行27-28)、`SwitchToCampus` (行29-30)、`Restore` (行31-32)。

- **函数 `switch_time_for`** (行37-43, `pub(crate)`)
  - 签名：`pub(crate) fn switch_time_for(weekday: u32) -> Option<u32>`
  - 逻辑：0..=4 → `Some(1380)`；5|6 → `Some(1410)`；否则 `None`。

- **函数 `is_switchable_operator`** (行48-50, 私有)
  - 白名单：`@telecom`/`@unicom`/`@cmcc`。

- **函数 `evaluate_night_switch`** (行67-82, `#[cfg(test)]` 仅测试期可用)
  - 签名：`pub fn evaluate_night_switch(enabled: bool, weekday: u32, now_minutes: u32, current_operator: &str, restore_operator: &str) -> NightSwitchAction`
  - 调用 `_with_restore` 传默认起点 (行74-81)。

- **函数 `evaluate_night_switch_with_restore`** (行90-114, 公开生产入口)
  - 签名：`pub fn evaluate_night_switch_with_restore(enabled: bool, weekday: u32, now_minutes: u32, current_operator: &str, restore_operator: &str, restore_start: u32) -> NightSwitchAction`
  - 逻辑：`!enabled`→None (行99-101)；`restore_operator` 非空且 `current_operator` 空且窗口内→Restore (行102-107)；当日有切换时刻且 `now>=时刻` 且白名单→SwitchToCampus (行108-112)；否则 None (行113)。

- **结构体 `ChkStatusInfo`** (行119-126)
  - 字段：`online: bool` (行121), `uid: String` (行122), `oltime: Option<i64>` (行125)。

- **函数 `parse_chkstatus`** (行132-145, 公开)
  - 签名：`pub fn parse_chkstatus(body: &str) -> Option<ChkStatusInfo>`
  - 剥 BOM/空白/`dr1003()` 壳解析 JSON，任何输入不 panic。

- **函数 `uid_matches`** (行149-151, 公开)
  - 签名：`pub fn uid_matches(uid: &str, user: &str, expected_operator: &str) -> bool`
  - trim、忽略 ASCII 大小写精确相等。

- **测试模块** (行153-367) 含大量用例：`开关关闭_一律不触发` (行199-203), `周日1380恰好命中_触发切换` (行205-208), `恢复窗口内_切换态_恢复` (行233-239), `parse_chkstatus_实测在线样例` (行298-304) 等。

### 4. `tauri-app/src-tauri/src/platform/metric.rs`

- **结构体 `MetricRow`** (行15-20)
  - 字段：`family: u16` (行17), `automatic: bool` (行18), `metric: u32` (行19)。

- **函数 `interface_rows_for_guid`** (行25-53, `pub(crate)`)
  - 签名：`pub(crate) fn interface_rows_for_guid(guid: &str) -> Result<Vec<MIB_IPINTERFACE_ROW>, String>`
  - 调用 `GetIpInterfaceTable` 枚举，LUID→GUID 匹配，成功后 `FreeMibTable`。

- **函数 `read_interface_metrics`** (行56-65, 公开)
  - 签名：`pub fn read_interface_metrics(guid: &str) -> Result<Vec<MetricRow>, String>`
  - 高层封装，无需管理员权限。

- **测试模块** (行67-83)
  - `read_interface_metrics_smoke` (行74-83, `#[ignore]` 真机冒烟)

### 5. `tauri-app/frontend/src/network/outboundOrder.ts`

- **函数 `buildOutboundOrder`** (行11-29)
  - 签名：`export function buildOutboundOrder(priority: string[] | undefined, detected: string[], wirelessNames?: string[]): string[]`
  - 逻辑：`priority` 非空则保留其中存在的网卡并追加其余 (行16-21)；否则 `wirelessNames` 非空则无线优先 (行22-28)；否则返回检测顺序 (行29)。

## Architecture

```text
┌──────────────────────────────────────────────┐
│ frontend/src/network/outboundOrder.ts        │
│ buildOutboundOrder (行11)                     │
└───────────────┬──────────────────────────────┘
                │ 计算 outbound_priority 顺序
                ▼
┌──────────────────────────────────────────────┐
│ config/model.rs                              │
│ Config 夜切字段 (行63-110)                    │
│ enable_night_outbound_switch 等              │
└───────────────┬──────────────────────────────┘
                │ 切换态由快照字段承载
                ▼
┌──────────────────────────────────────────────┐
│ config/outbound_switch.rs                    │
│ evaluate_night_outbound_with_restore (行37)  │
│ is_night_outbound_guard_window_with_restore  │
│ (行85)                                        │
│ outbound_restore_active (行106)              │
└───────────────┬──────────────────────────────┘
                │ 依赖时间表与恢复窗常数
                ▼
┌──────────────────────────────────────────────┐
│ config/night_switch.rs                       │
│ switch_time_for (行37)                       │
│ restore_start_minutes (行18)                 │
│ DEFAULT/RESTORE_END_MINUTES (行11,13)        │
└───────────────┬──────────────────────────────┘
                │ 桌面端需读跃点快照
                ▼
┌──────────────────────────────────────────────┐
│ platform/metric.rs                           │
│ read_interface_metrics (行56)                │
└──────────────────────────────────────────────┘
```

## Data Flow

1. **配置开关**：`Config::enable_night_outbound_switch` (model.rs 行63-64) 为总开关，默认 false；`night_restore_minutes` (行69-70) 提供可配置恢复时刻。
2. **状态判定**：调用方从 `Config` 读取 `outbound_metric_restore` / `outbound_disabled_adapters` / `outbound_standby_route` (model.rs 行80-92) 传入 `outbound_restore_active` (outbound_switch.rs 行106) 得到 `restore_active`。
3. **决策产出**：将 `enabled`, `weekday`, `now_minutes`, `restore_active`, `night_restore_minutes` 传入 `evaluate_night_outbound_with_restore` (outbound_switch.rs 行37) 得到 `NightOutboundAction`；或先用 `is_night_outbound_guard_window_with_restore` (行85) 判断巡检避让。
4. **前端优先级**：`buildOutboundOrder` (outboundOrder.ts 行11) 根据 `outbound_priority` (model.rs 行75-76) 与探测到的网卡生成顺序，供桌面端选择目标卡。
5. **桌面快照读取**：执行切换前通过 `read_interface_metrics` (metric.rs 行56) 获取当前跃点写进 `outbound_metric_restore` 快照。
6. **跨端动作**：判定为 `Switch` 时桌面写 metric+禁用校园卡、安卓注销；`Restore` 时桌面还原、安卓重登。动作实现不在本模块源码内。

## Connections

[[night-operator-switch]]、[[dual-platform-sharing]]、[[config-field-sets-bidirectional-sync]]、[[set-ip-interface-entry-metric]]、[[outbound-switch]]、[[desktop-config]]

## Known Issues

1. **`model.rs` 与 `metric.rs` 注释口径冲突**：model.rs 行77-81 称“运行时修改重启即还原”，metric.rs 行1-5 实证 `SetIpInterfaceEntry` 落持久配置重启不还原。文档以 metric.rs 为准，代码注释待修（快照还原兜住行为）。
2. **桌面 `night_outbound_restore` 死字段**：model.rs 行109-110 定义为安卓切换态标记，桌面端不消费，仅为满足双端同构纪律存在；桌面真实切换态靠 `outbound_metric_restore` 三快照。
3. **兜底路由废弃但字段仍参与判定**：outbound_switch.rs 行101-105 注释，自 2026-10-18 事故后不再创建 `outbound_standby_route`（model.rs 行91-92），但 `outbound_restore_active` (行106-108) 仍检查它，历史残留须识别清理。
4. **守护窗口内巡检干扰**：outbound_switch.rs 行63-69 注释，2026-09-27 夜实证用户手动禁用适配器无 IP 进不了闸门名单，巡检自动启用会顶掉切换；故 `is_night_outbound_guard_window_with_restore` 返回 true 时巡检不出自动启用目标。
5. **凌晨补切语义**：outbound_switch.rs 行19-23 注释及行54-56 逻辑，机器在切点前后睡眠/关机错过切换时，凌晨（now<restore_start）视为前一晚切换窗口尾部补切，校园线路已断、热点可用，恢复窗自会收敛。
6. **功能关闭+态残留**：若 `enable_night_outbound_switch=false` 但三快照非空，`evaluate_night_outbound_with_restore` 直接返回 `None` (行45-47)，切换态不会被自动还原，需调用方或启动对账处理（调用方代码不在本批源码）。
