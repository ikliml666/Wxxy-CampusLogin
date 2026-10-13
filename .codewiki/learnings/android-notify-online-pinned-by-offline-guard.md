---
title: 安卓后台监控循环与在线状态机
type: module
source_files:
  - android/src-tauri/src/monitor_loop.rs
tags: [安卓, 监控循环, 状态机, 夜间切换, 定时任务, 通知管理]
---

## Overview

`monitor_loop.rs` 是安卓端 Tauri 应用的后台监控核心，负责维持校园网连接的稳定性与可观测性。其核心职责包括：
1. **周期性巡检**：根据 WiFi/屏幕状态动态调整检测间隔，执行校园网判定与 Portal 探测。
2. **在线状态机**：基于三态护栏（确定判定/校园内失配/非校园）维护 `was_online` 状态，驱动掉线自动重连与通知更新。
3. **夜间自动化**：处理夜间出站切换（注销校园网、等待系统切蜂窝、晨间还原）与晚间断网运营商自动切换。
4. **定时任务**：执行定时登录/注销，并具备启动对账防误触机制。
5. **WiFi 事件响应**：通过 Android NetworkCallback 实时触发检测，具备风暴去抖与延迟稳定机制。

## Key Components

### MonitorState (monitor_loop.rs:28-76)
监控全局状态，所有字段均为线程安全原子类型或 Mutex，供 tokio 循环与命令并发访问。

| 字段 | 类型 | 用途 |
| :--- | :--- | :--- |
| `running` | `AtomicBool` | 监控循环运行标志 |
| `check_count` | `AtomicU64` | 累计检测次数 |
| `consecutive_failures` | `AtomicU32` | 连续失败计数（用于熔断） |
| `reconnect_count` | `AtomicU32` | 当前在线周期内的自动重连次数 |
| `last_login_attempt_ms` | `AtomicU64` | 上次登录尝试时间戳（用于冷却控制） |
| `was_online` | `AtomicBool` | 上一拍在线状态记忆（状态机核心） |
| `last_result` | `Mutex<Option<serde_json::Value>>` | 最近一次检测结果缓存 |
| `desired_interval_ms` | `AtomicU64` | 目标检测间隔（支持热更新） |
| `idle_interval_ms` | `AtomicU64` | 闲时检测间隔（蜂窝/灭屏时使用） |
| `logout_protected_until_ms` | `AtomicU64` | 注销保护期截止时间（防止注销后立即重登） |
| `wifi_event_ms` | `AtomicU64` | 最近一次 WiFi 事件时间戳（用于去抖） |
| `notified_online` | `AtomicU8` | 常驻通知已展示的状态码（仅翻转时重建通知以省电） |
| `scheduled_login_day` | `AtomicI32` | 定时登录当日触发标记（防每日重复触发） |
| `scheduled_logout_day` | `AtomicI32` | 定时注销当日触发标记 |

### 常量与配置 (monitor_loop.rs:9-73)

| 常量/变量 | 值/说明 | 用途 |
| :--- | :--- | :--- |
| `OUTBOUND_RESTORE_MAX_FAILS` | `3` | 夜间出站还原连续失败上限，超限放弃自动还原 |
| `OUTBOUND_SWITCH_MAX_FAILS` | `3` | 夜间出站切换注销确认连续失败上限 |
| `WIFI_EVENT_DELAY_MS` | `2500` | WiFi 连接事件延迟检测时间（等 DHCP/路由稳定） |
| `WIFI_LOST_EVENT_DELAY_MS` | `0` | WiFi 断开事件零延迟（断开即事实） |
| `WIFI_EVENT_DEBOUNCE_MS` | `1000` | WiFi 事件去抖窗口（合并风暴事件） |
| `NIGHT_VERIFY_WAIT_SECS` | `15` | 夜切验证等待时间（eportal 在线表同步延迟） |
| `NIGHT_VERIFY_RETRY_SECS` | `5` | 夜切验证重试间隔 |
| `NIGHT_VERIFY_ATTEMPTS` | `3` | 夜切验证最大重试次数 |
| `NIGHT_VERIFY_HTTP_TIMEOUT_SECS` | `15` | 夜切验证单次 HTTP 超时 |

### 核心函数

#### `effective_interval_ms` (monitor_loop.rs:87-95)
**用途**：计算当前 tick 的有效检测间隔。
**逻辑**：
- 基础间隔下限 5s。
- WiFi 且亮屏：使用基础间隔。
- 蜂窝或灭屏：使用闲时间隔，但不低于基础间隔。

#### `should_attempt_login` (monitor_loop.rs:101-118)
**用途**：纯函数，判定本 tick 是否应尝试自动登录。
**条件**：
1. 当前不在线 (`!online`) 且在校园网 (`on_campus`)。
2. 若非掉线场景（启动即离线），需显式开启 `auto_login_on_preparation`。
3. 重连次数未达上限 (`reconnect_count < max_reconnect`)。
4. 距上次尝试超过冷却时间 (`cooldown_secs`)。

#### `run_check_once` (monitor_loop.rs:1245-1445)
**用途**：单次完整检测流程，是所有检测入口（周期、WiFi 事件、手动触发）的统一汇聚点。
**流程**：
1. **前置检查**：读取配置；若处于夜间出站切换态 (`night_outbound_restore` 非空)，整轮跳过。
2. **静默期门控**：若在配置的非检测时段内，直接返回，但会更新通知为“已暂停检测”。
3. **探针窗口**：获取 WifiLock/WakeLock 防止检测期间断流。
4. **强制绑 WiFi**：确保流量走 WiFi 接口。
5. **校园网判定**：调用 `probe_campus_with_ssid`，获取 `on_campus`, `source_ip`, `wifi_off`。
6. **Portal 探测**：
   - 若在校园网且有 IP，在新建短命线程中绑定小核执行 `check_portal_full`。
   - 若不在校园网或无 IP，短路返回错误。
7. **三态在线判定** (monitor_loop.rs:1347-1351)：
   - `Ok(s) if s.error_kind.is_none()`: 取 Portal 判定的 `s.online`。
   - `_ if on_campus`: 校园网内但探测异常（如页面特征失配），保持 `prev_online`（反误报）。
   - `_` (else): 非校园网或探测彻底失败，判为 `false`（离线）。
8. **状态机更新**：
   - 在线：清零重连计数、失败计数、注销保护期。
   - 掉线通知：若从在线翻离线且在校园网，发送系统通知。
9. **自动登录**：
   - 检查注销保护期与连续失败熔断。
   - 满足条件则执行 `run_login`，记录结果并更新通知/日志。
10. **结果 Emit**：组装 JSON payload 发射 `background-check-result` 事件。
11. **通知更新**：仅当 `notified_online` 状态码翻转时，才重建常驻通知。

#### `run_scheduled_actions` (monitor_loop.rs:856-1035)
**用途**：执行定时登录、定时注销及夜间切换逻辑。
**顺序约束**：
1. **夜间出站切换**：判定 `evaluate_night_outbound`，执行 Switch（注销+切蜂窝）或 Restore（登录+还原）。
2. **晚间断网运营商切换**：若未处于出站切换态，执行运营商切换逻辑。
3. **定时登录/注销**：复用 `campus_login_lib::config::schedule` 判定，执行登录或注销。

#### `start_background_check` / `stop_background_check` (monitor_loop.rs:185-225)
**用途**：Tauri 命令，启停后台监控循环与前台服务保活。
**细节**：启动时初始化间隔、注册 WiFi 监听、重置通知状态；停止时清理资源。

#### `handle_wifi_event` (monitor_loop.rs:265-295)
**用途**：处理 Android NetworkCallback 传来的 WiFi 变化事件。
**逻辑**：
- 校验监控运行中。
- 解析事件类型 (`available/lost/validated/unvalidated`)。
- 去抖判定：若是风暴内后续事件，忽略。
- 延迟安排：`lost` 事件零延迟立即检测；`available` 事件延迟 2.5s 后检测。
- 执行 `run_check_once`。

#### `notify_system` / `with_vpn_hint` (monitor_loop.rs:155-180)
**用途**：发送关键事件系统通知。
**细节**：`with_vpn_hint` 检测 VPN 存在时，在通知正文追加 VPN 排除提示。

#### `status_value` (monitor_loop.rs:110-128)
**用途**：生成 `get_init_data` 和 `get_background_status` 的返回值。
**细节**：展平 `last_result` 中的关键字段（`online`, `onlineOperator` 等）到顶层，供前端启动时读取。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      MonitorLoop (tokio)                     │
│                                                              │
│  ┌─────────────┐   ┌──────────────┐   ┌──────────────────┐  │
│  │ Interval    │   │ WiFi Event   │   │ Manual Trigger   │  │
│  │ Tick        │   │ Handler      │   │ (Command)        │  │
│  └──────┬──────┘   └──────┬───────┘   └────────┬─────────┘  │
│         │                 │                     │            │
│         ▼                 ▼                     ▼            │
│  ┌─────────────────────────────────────────────────────┐     │
│  │              run_check_once                         │     │
│  │  ├─ Outbound Switch Gate (Skip if active)           │     │
│  │  ├─ Silent Period Gate (Update Notify & Return)     │     │
│  │  ├─ Probe Campus (SSID + Subnet/Gateway/Portal)     │     │
│  │  ├─ Portal Probe (Little Core Thread)               │     │
│  │  ├─ Online State Machine (3-State Logic)            │     │
│  │  ├─ Auto Login (Cooldown + Fuse)                    │     │
│  │  └─ Emit Result & Update Notification               │     │
│  └─────────────────────────────────────────────────────┘     │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐     │
│  │           run_scheduled_actions (per tick)          │     │
│  │  ├─ Night Outbound Switch (Switch/Restore)          │     │
│  │  ├─ Night Operator Switch                           │     │
│  │  └─ Scheduled Login/Logout                          │     │
│  └─────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **配置加载**：`current_settings` 解密读取配置，影响间隔、开关、定时时间等。
2. **探测数据**：
   - `probe_campus_with_ssid` 返回 `CampusProbe` (包含 `on_campus`, `source_ip`, `wifi_off`)。
   - `check_portal_full` 返回 `PortalStatus` (包含 `online`, `reachable`, `login_available`, `uid`, `error_kind`)。
3. **状态决策**：
   - `online` 由三态逻辑决定，更新 `MONITOR.was_online`。
   - `login_result` 由 `should_attempt_login` 决定是否发起，结果回写 `MONITOR.last_result`。
4. **输出**：
   - `background-check-result` 事件携带完整状态供前端渲染。
   - `login-log` 事件记录关键动作日志。
   - 系统通知/常驻通知根据状态翻转更新。

## Connections

- [[android-backend]] — `MonitorState` 的全局状态管理与命令接口
- [[background-check-and-auto-login]] — 在线判定逻辑与自动重连策略
- [[android-keepalive-fgs-architecture]] — 前台服务保活与常驻通知机制
- [[android-vpn-bypass-infeasible|不做"绕过 VPN 直连 WiFi"]] — VPN 环境下的判定限制与提示