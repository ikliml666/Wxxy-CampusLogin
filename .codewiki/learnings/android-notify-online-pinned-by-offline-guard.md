---
title: 安卓后台监控循环与在线状态机
type: module
source_files:
  - android/src-tauri/src/monitor_loop.rs
tags: [安卓, 监控循环, 状态机, 夜间切换, 定时任务, 通知管理]
---

## Overview

`monitor_loop.rs` 是安卓端 Tauri 应用的后台监控核心（模块头部注释见 monitor_loop.rs:1-3），负责维持校园网连接的稳定性与可观测性。其核心职责包括：

1. **周期性巡检**：`monitor_tick_loop`（monitor_loop.rs:668-727）根据 WiFi/屏幕状态动态分档调整检测间隔（`effective_interval_ms` monitor_loop.rs:106-113），执行校园网判定与 Portal 探测。
2. **在线状态机**：`run_check_once`（monitor_loop.rs:1493-1733）基于三态护栏（确定判定/校园内失配/非校园，monitor_loop.rs:1493-1497）维护 `MONITOR.was_online` 状态（monitor_loop.rs:1493），驱动掉线自动重连与常驻通知更新。
3. **夜间自动化**：处理夜间出站切换（注销校园网、等待系统切蜂窝、晨间还原，monitor_loop.rs:756-781, 892-956）与晚间断网运营商自动切换（monitor_loop.rs:969-1011）。
4. **定时任务**：`run_scheduled_actions`（monitor_loop.rs:860-1068）执行定时登录/注销，监控启动时具备启动对账防冷启动误触机制（monitor_loop.rs:677-690）。
5. **WiFi 事件响应**：`handle_wifi_event`（monitor_loop.rs:335-373）通过 Android NetworkCallback 实时触发检测，具备风暴去抖（`wifi_event_is_burst_start` monitor_loop.rs:97-99）与延迟稳定机制（monitor_loop.rs:84-89）。
6. **启动恢复**：`run_startup_tasks`（monitor_loop.rs:468-539）并行拉起后台检测、质量首测、自动登录、更新检查等启动链。

## Key Components

### MonitorState (monitor_loop.rs:49-79)

监控全局状态（派生 Default，见 monitor_loop.rs:49-51），所有字段均为线程安全原子类型或 Mutex，供 tokio 循环与命令并发访问。

| 字段 | 类型 | 用途 | 行号 |
| :--- | :--- | :--- | :--- |
| `running` | `AtomicBool` | 监控循环运行标志 | 52 |
| `check_count` | `AtomicU64` | 累计检测次数 | 53 |
| `consecutive_failures` | `AtomicU32` | 连续失败计数（用于熔断，达 5 次暂停自动重登，见 1503） | 54 |
| `reconnect_count` | `AtomicU32` | 当前在线周期内的自动重连次数（达上限暂停，见 56 注释） | 56 |
| `last_login_attempt_ms` | `AtomicU64` | 上次登录尝试时间戳（用于冷却控制） | 57 |
| `was_online` | `AtomicBool` | 上一拍在线状态记忆（状态机核心） | 58 |
| `last_result` | `Mutex<Option<serde_json::Value>>` | 最近一次检测结果缓存 | 59 |
| `desired_interval_ms` | `AtomicU64` | 目标检测间隔（支持热更新，start 刷新） | 62 |
| `idle_interval_ms` | `AtomicU64` | 闲时检测间隔（蜂窝/灭屏时使用） | 64 |
| `logout_protected_until_ms` | `AtomicU64` | 注销保护期截止时间（防注销后立即重登） | 66 |
| `wifi_event_ms` | `AtomicU64` | 最近一次 WiFi 事件时间戳（去抖与风暴合并） | 68 |
| `notified_online` | `AtomicU8` | 常驻通知已展示的状态码（0=未展示,1=在线,2=未连接,3=非检测时段，仅翻转时重建通知） | 72 |
| `scheduled_login_day` | `AtomicI32` | 定时登录当日触发标记（当日序号 num_days_from_ce，0=从未） | 76 |
| `scheduled_logout_day` | `AtomicI32` | 定时注销当日触发标记（语义同 scheduled_login_day） | 78 |

### 静态变量与常量 (monitor_loop.rs:15-47, 84-92, 285-287, 1227-1230)

| 常量/变量 | 行号 | 值/说明 | 用途 |
| :--- | :--- | :--- | :--- |
| `MONITOR` | 15-17 | `MonitorState::default()` | 全局监控状态懒初始化 |
| `OUTBOUND_RESTORE_FAIL_COUNT` | 21 | `AtomicU32` | 夜间出站还原连续失败计数 |
| `OUTBOUND_RESTORE_MAX_FAILS` | 23 | `3` | 夜间出站还原连续失败上限，超限放弃自动还原 |
| `OUTBOUND_SWITCH_FAIL_COUNT` | 26 | `AtomicU32` | 夜间出站切换注销确认连续失败计数 |
| `OUTBOUND_SWITCH_MAX_FAILS` | 28 | `3` | 夜间出站切换注销确认连续失败上限 |
| `MONITOR_GENERATION` | 32 | `AtomicU64` | 监控循环代际 token，防 stop→start 竞速误清 running |
| `LOGIN_ATTEMPT_LOCK` | 37 | `tokio::sync::Mutex` | 自动登录读-判-写 attempt 段互斥 |
| `FORCE_CHECK_AFTER_RESTORE` | 47 | `AtomicBool` | 夜间出站还原后无视静默期的一次性放行标记 |
| `WIFI_EVENT_DELAY_MS` | 84 | `2500` | WiFi 连接事件延迟检测时间（等 DHCP/路由稳定） |
| `WIFI_LOST_EVENT_DELAY_MS` | 89 | `0` | WiFi 断开事件零延迟（断开即事实） |
| `WIFI_EVENT_DEBOUNCE_MS` | 92 | `1000` | WiFi 事件去抖窗口（合并风暴事件） |
| `WIFI_WATCHER_CHANNEL_ID` | 285 | `AtomicU32` (cfg mobile) | WiFi 监听注册 channel id |
| `WIFI_WATCHER_ACTIVE` | 287 | `AtomicBool` (cfg mobile) | WiFi 监听激活标志 |
| `NIGHT_VERIFY_WAIT_SECS` | 1227 | `15` | 夜切验证等待时间（eportal 在线表同步延迟） |
| `NIGHT_VERIFY_RETRY_SECS` | 1228 | `5` | 夜切验证重试间隔 |
| `NIGHT_VERIFY_ATTEMPTS` | 1229 | `3` | 夜切验证最大重试次数 |
| `NIGHT_VERIFY_HTTP_TIMEOUT_SECS` | 1230 | `15` | 夜切验证单次 HTTP 超时 |

### 纯函数与工具函数

- **`wifi_event_is_burst_start`** (monitor_loop.rs:97-99)：判定 WiFi 事件是否为风暴起点（首事件或距上次 >= 去抖窗口）。
- **`effective_interval_ms`** (monitor_loop.rs:106-113)：计算有效检测间隔；基础间隔下限 5s；WiFi 且亮屏用基础间隔，否则用闲时间隔（不低于基础）。
- **`should_attempt_login`** (monitor_loop.rs:117-138)：纯函数，判定本 tick 是否应尝试自动登录（不在线且在校园网、非掉线需开 preparation、未达重连上限、过冷却）。
- **`status_value`** (monitor_loop.rs:141-162)：生成 `get_init_data` 和 `get_background_status` 的返回值，展平 `last_result` 关键字段（online/message/onlineOperator/serverAvailable/onCampusNetwork）到顶层。
- **`is_running`** (monitor_loop.rs:165-167)：读取 `MONITOR.running` 标志。
- **`epoch_ms`** (monitor_loop.rs:169-174)：返回当前 UNIX 毫秒时间戳。
- **`emit_login_log`** (monitor_loop.rs:176-180)：发射 `login-log` 事件。
- **`notify_system`** (monitor_loop.rs:185-202)：发送关键事件系统通知，支持 mascot 大图降级（资源缺失 catch 后纯文本）。
- **`with_vpn_hint`** (monitor_loop.rs:208-214)：检测 VPN 接管时追加排除提示到通知正文。
- **`notify_state`** (monitor_loop.rs:1614-1622)：返回常驻通知状态码与文案（1=在线, 2=未连接, 4=WiFi 未连接; 3 被非检测时段占用）。
- **`portal_origin`** (monitor_loop.rs:1233-1245)：提取 portal_url 的 scheme+host 用于 chkstatus 验证，空值回退默认。
- **`expected_operator_display`** (monitor_loop.rs:1248-1254)：空串显示为“无锡学院”，其余原样。
- **`outbound_switch_active`** (monitor_loop.rs:731-736)：异步判定是否处于夜间出站切换态（读配置 night_outbound_restore 非空）。
- **`latest_settings`** (monitor_loop.rs:741-748)：取最新配置用于落盘快照，防 stale 覆盖，读失败回退传入快照。
- **`portal_probe_on_little_cores`** (monitor_loop.rs:809-846)：绑小核的专用短命线程执行 Portal 全量探测，携带 tokio reactor handle 防 panic。
- **`probe_with_retry`** (monitor_loop.rs:545-563)：启动探测带一次重试（3s 后），SSID 感知版。
- **`emit_auto_login_result`** (monitor_loop.rs:569-575)：发射应用内 auto-login-result 事件（前端 toast）。

### Tauri 命令与入口

- **`start_background_check`** (monitor_loop.rs:217-266)：启动后台监控 Rust tokio 循环 + 前台服务保活（幂等），刷新间隔、注册 WiFi 监听，代际 token 推进。
- **`stop_background_check`** (monitor_loop.rs:269-281)：停止后台监控，推进代际、清标志、注销 WiFi 监听、停 FGS。
- **`start_wifi_watcher`** (monitor_loop.rs:292-314, cfg mobile)：注册 WiFi 变化监听，失败降级纯周期检测并置 WATCHER_FAILED。
- **`stop_wifi_watcher`** (monitor_loop.rs:318-330, cfg mobile)：注销 WiFi 变化监听。
- **`handle_wifi_event`** (monitor_loop.rs:335-373, cfg mobile)：处理 WiFi 变化事件，去抖 + 延迟后执行 `run_check_once`，风暴内过期自杀。
- **`trigger_background_check`** (monitor_loop.rs:376-384)：手动触发检测（仅运行中点用，否则 Err）。
- **`get_background_status`** (monitor_loop.rs:387-389)：返回 `status_value()`。
- **`get_boot_autostart`** (monitor_loop.rs:393-407)：开机自启开关读（cfg mobile 查插件，否则 false）。
- **`set_boot_autostart`** (monitor_loop.rs:410-432)：开机自启开关写，同步配置落盘与缓存。
- **`get_notification_enabled`** (monitor_loop.rs:436-438)：通知开关读。
- **`set_notification_enabled`** (monitor_loop.rs:441-456)：通知开关写，同步配置落盘与缓存。

### 核心循环与调度

- **`run_startup_tasks`** (monitor_loop.rs:468-539)：启动恢复，500ms 后并行 spawn 后台检测/质量首测/自动登录，应用日志保留天数，清理残留 APK，启动更新检查循环。
- **`monitor_tick_loop`** (monitor_loop.rs:668-727)：tokio 循环体，处理代际校验、启动对账、间隔热更新、巡检分档跳拍、调用 `run_scheduled_actions` 与 `run_check_once`。
- **`run_check_once`** (monitor_loop.rs:1370-1610)：单次完整检测流程，全入口（周期、WiFi、手动）汇聚点，含出站闸、静默期闸、探针窗口、探测、状态机、自动登录、emit、常驻通知。
- **`run_scheduled_actions`** (monitor_loop.rs:860-1068)：定时登录/注销及夜间切换逻辑编排（出站切换→运营商夜切→定时登录/注销）。

### 夜间切换与登录内核

- **`logout_for_outbound_switch`** (monitor_loop.rs:756-781)：切换侧注销，返回离线是否生效（radiusOk || unbindOk），前置在线检查。
- **`switch_logout_unconfirmed`** (monitor_loop.rs:786-803)：注销未确认生效的上限判定（达上限按断网规律建态）。
- **`persist_settings`** (monitor_loop.rs:1073-1096)：落盘夜间切换配置并刷新内存缓存、广播 config-changed、建账号档案。
- **`night_switch_login`** (monitor_loop.rs:1102-1128)：夜切后登录触发，走 run_login 内核，无校园网探测闸。
- **`trigger_wifi_recheck`** (monitor_loop.rs:1134-1155)：触发系统 WiFi 重检以切蜂窝（spawn_blocking JNI）。
- **`ensure_avoid_bad_wifi`** (monitor_loop.rs:1160-1196)：程序化确保 network_avoid_bad_wifi=1（授权后）。
- **`restore_avoid_bad_wifi`** (monitor_loop.rs:1199-1223)：按快照还原 network_avoid_bad_wifi（pub(crate) async）。
- **`verify_night_switch`** (monitor_loop.rs:1260-1314)：夜切登录生效验证独立任务，含出站态中止、注销重登复验。
- **`night_switch_verify_round`** (monitor_loop.rs:1318-1335)：一轮验证（重试 3 次）。
- **`night_switch_check_once`** (monitor_loop.rs:1338-1367)：单次 chkstatus 查询+uid 核对。

### 探针窗口与电源

- **`ProbeWindowGuard`** (monitor_loop.rs:642-650, cfg mobile)：探针窗口 guard，Drop 释放 WifiLock/WakeLock（调 end_probe_window）。
- **`power_state`** (monitor_loop.rs:656-666)：查询屏幕与 WiFi 状态（cfg mobile 查插件，失败保守 (true,true)；非 mobile 恒 (true,true)）。

### 单元测试 (monitor_loop.rs:1624-1746)

模块内含 `#[cfg(test)]` 测试，覆盖 `notify_state` 状态码（含 WiFi 短路码 4）、`should_attempt_login` 各分支、`wifi_event_is_burst_start` 去抖、`effective_interval_ms` 分档与下限、`portal_origin` 解析、`expected_operator_display`、`status_value` 展平运营商等纯函数逻辑。

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                  MonitorLoop (tokio)                      │
│  ┌──────────┐  ┌───────────┐  ┌──────────────────────┐    │
│  │ Interval │  │ WiFi Event│  │ Manual Trigger       │    │
│  │ Tick     │  │ Handler   │  │ (Command)            │    │
│  │ 668-727  │  │ 335-373   │  │ 376-384              │    │
│  └────┬─────┘  └─────┬─────┘  └──────────┬───────────┘    │
│       ▼              ▼                    ▼                 │
│  ┌────────────────────────────────────────────────────┐    │
│  │          run_check_once (1370-1610)                │    │
│  │ Outbound Gate → Silent Gate → Probe → Portal Probe │    │
│  │ (809-846) → State Machine (1477-1481) → Auto Login │    │
│  │ (1504-1559) → Emit & Notify (1596-1609)            │    │
│  └────────────────────────────────────────────────────┘    │
│  ┌────────────────────────────────────────────────────┐    │
│  │   run_scheduled_actions (860-1068 per tick)        │    │
│  │ Outbound Switch / Operator Switch / Sched Login     │    │
│  └────────────────────────────────────────────────────┘    │
│  ┌────────────────────────────────────────────────────┐    │
│  │   run_startup_tasks (468-539) parallel spawns      │    │
│  │ BG Check / Quality / Auto Login / Update Loop      │    │
│  └────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────┘
```

## Data Flow

1. **配置加载**：`crate::config_state::current_settings` 解密读取配置（如 monitor_loop.rs:471, 861），影响间隔、开关、定时时间、静默期等。
2. **探测数据**：
   - `probe_campus_with_ssid` 返回 `(CampusProbe, Option<String>)` (包含 `on_campus`, `source_ip`, `wifi_off`)，见 monitor_loop.rs:1437-1447。
   - `portal_probe_on_little_cores` 调用 `check_portal_full` 返回 `PortalStatus` (包含 `reachable`, `login_available`, `online`, `message`, `uid`, `error_kind`)，见 monitor_loop.rs:814-851, 1456-1464。
3. **状态决策**：
   - `online` 由三态逻辑决定（monitor_loop.rs:1481-1485）：Ok 且无 error_kind 取 s.online；on_campus 失配保持 prev_online；非校园网置 false。更新 `MONITOR.was_online`（monitor_loop.rs:1481）。
   - 自动登录由 `should_attempt_login` 与注销保护/熔断闸决定（monitor_loop.rs:1504-1516），`LOGIN_ATTEMPT_LOCK` 互斥（1519），结果回写 `MONITOR.last_result`（1592-1594）。
4. **输出**：
   - `background-check-result` 事件携带完整状态供前端渲染（monitor_loop.rs:1596），含 onlineOperator 从 uid 推导（1566-1570）。
   - `login-log` 事件记录关键动作日志（monitor_loop.rs:179）。
   - 系统通知/常驻通知根据状态翻转更新（monitor_loop.rs:1491-1493, 1604-1608）；非检测时段常驻通知码 3（1411-1415）。

## Connections

- [[android-backend]] — `MonitorState` 的全局状态管理与命令接口
- [[background-check-and-auto-login]] — 在线判定逻辑与自动重连策略
- [[android-keepalive-fgs-architecture]] — 前台服务保活与常驻通知机制
- [[android-vpn-bypass-infeasible|不做"绕过 VPN 直连 WiFi"]] — VPN 环境下的判定限制与提示
- [[android-config-state]] — `current_settings` / `save_to` / `CryptoBridge` 配置链路
- [[android-campus-detect]] — `probe_campus_with_ssid` 校园网判定与 SSID 感知
- [[android-protocol-cmds]] — `run_login` / `do_logout` / `ensure_wifi_bound` 协议执行

## Known Issues

- **VPN 流量劫持**：全量 VPN（Clash/UU 等）未排除本应用时，socket 探测与登录流量被 tun 劫持，SO_BINDTODEVICE 旁路失效（monitor_loop.rs:204-214）。当前仅靠 `with_vpn_hint` 在通知正文追加提示，需用户手动在 VPN 中排除。
- **裸线程 Reactor 依赖**：Portal 探测在独立裸线程执行（monitor_loop.rs:809-846），必须携带 tokio reactor handle 进入（819-823），否则 panic "there is no reactor running"（注释见 814-818）。
- **WiFi 监听注册失败降级**：`start_wifi_watcher` 若注册失败（monitor_loop.rs:307-312），仅置 `WATCHER_FAILED` 标志并退化为纯周期检测，无实时 WiFi 变化感知。
- **出站切换态跳过巡检**：处于夜间出站切换态时 `run_check_once` 整轮跳过（monitor_loop.rs:1385-1388），若标记残留且插件无兜底还原，巡检会永久压住（已由 `evaluate_outbound_action_with_restore` 内关闭兜底缓解，见 878-891）。
- **静默期与还原时点冲突**：夜间出站还原时刻早于静默期起点时，原会等到静默期结束才恢复登录；已通过 `FORCE_CHECK_AFTER_RESTORE` 一次性放行解决（monitor_loop.rs:1395-1398, 927-928）。
- **定时注销过点防冷启**：监控启动若当日注销时刻已过，启动对账消耗今日额度不执行（monitor_loop.rs:677-690），避免首拍立即注销踢用户下线。
