---
title: android-log-type-enum-drift
type: learning
source_files:
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/shared/ui-types.ts
tags: [教训, 安卓, 双端同步, 枚举, 白屏]
---

## Overview

本文记录安卓端 `login-log` 事件 `type` 字段在后端与前端之间的字符串字面量漂移问题及修复过程。后端 `emit_login_log` 最初使用非标准 `"warn"`，而前端 `LogType` 枚举仅包含 `'info' | 'success' | 'error' | 'warning'`，导致图标映射取 `undefined` 引发白屏。后端已修正为 `'warning'`，前端通过消费侧多层兜底防止同类问题重演。

## Key Components

### 后端：android/src-tauri/src/monitor_loop.rs

*   **`MONITOR`** (`:16`)：全局监控状态单例，类型为 `MonitorState`。
*   **`OUTBOUND_RESTORE_FAIL_COUNT`** (`:23`)：夜间出站切换连续还原失败计数，原子类型 `AtomicU32`。
*   **`OUTBOUND_RESTORE_MAX_FAILS`** (`:25`)：连续还原失败上限，值为 `3`。
*   **`OUTBOUND_SWITCH_FAIL_COUNT`** (`:30`)：夜间出站切换连续注销失败计数，原子类型 `AtomicU32`。
*   **`OUTBOUND_SWITCH_MAX_FAILS`** (`:32`)：连续注销失败上限，值为 `3`。
*   **`MONITOR_GENERATION`** (`:37`)：监控循环代际 token，用于防 start/stop 竞速下的标志误清，原子类型 `AtomicU64`。
*   **`LOGIN_ATTEMPT_LOCK`** (`:42`)：自动登录读-判-写 attempt 段互斥锁，原子类型 `tokio::sync::Mutex<()>`。
*   **`MonitorState`** (`:47-86`)：监控循环的全局状态结构体。
    *   `running`: `AtomicBool`，监控是否运行。
    *   `check_count`: `AtomicU64`，检测总次数。
    *   `consecutive_failures`: `AtomicU32`，连续失败次数（用于掉线重连熔断）。
    *   `reconnect_count`: `AtomicU32`，本次在线周期内自动重连次数。
    *   `last_login_attempt_ms`: `AtomicU64`，最近一次登录尝试时间戳。
    *   `was_online`: `AtomicBool`，上一拍在线状态记忆。
    *   `last_result`: `Mutex<Option<serde_json::Value>>`，最近一次检测结果。
    *   `desired_interval_ms`: `AtomicU64`，目标检测间隔。
    *   `idle_interval_ms`: `AtomicU64`，闲时巡检间隔。
    *   `logout_protected_until_ms`: `AtomicU64`，注销保护期截止时间。
    *   `wifi_event_ms`: `AtomicU64`，最近一次 WiFi 变化事件时间戳。
    *   `notified_online`: `AtomicU8`，常驻通知已展示的在线状态码（0=未展示, 1=在线, 2=未连接, 3=非检测时段, 4=WiFi未连接）。
    *   `scheduled_login_day`: `AtomicI32`，定时登录当日已触发标记。
    *   `scheduled_logout_day`: `AtomicI32`，定时注销当日已触发标记。
*   **`WIFI_EVENT_DELAY_MS`** (`:91`)：WiFi 连上事件延迟检测毫秒数，值为 `2500`。
*   **`WIFI_LOST_EVENT_DELAY_MS`** (`:98`)：WiFi 断开事件延迟毫秒数，值为 `0`。
*   **`WIFI_EVENT_DEBOUNCE_MS`** (`:101`)：WiFi 事件去抖窗口毫秒数，值为 `1000`。
*   **`wifi_event_is_burst_start`** (`:104-107`)：纯函数，判定 WiFi 事件是否为风暴起点。
*   **`effective_interval_ms`** (`:110-118`)：纯函数，根据屏幕状态和 WiFi 连接状态计算有效检测间隔。
*   **`should_attempt_login`** (`:121-137`)：纯函数，判定是否应执行自动登录。受 `online`, `was_online`, `on_campus`, `auto_login_on_preparation`, `reconnect_count`, `max_reconnect`, `millis_since_last_attempt`, `cooldown_secs` 等多条件门控。
*   **`status_value`** (`:140-158`)：构造后台状态 JSON，展平 `last_result` 中的关键字段到顶层。
*   **`is_running`** (`:161-163`)：检查监控是否运行。
*   **`epoch_ms`** (`:165-169`)：获取当前纪元毫秒时间戳。
*   **`emit_login_log`** (`:171-175`)：核心日志发射函数。构造 `{ "message": ..., "type": ... }` JSON payload 并通过 Tauri `app.emit("login-log", payload)` 广播。`log_type` 参数现严格限定为 `"info"`, `"success"`, `"error"`, `"warning"`。
*   **`notify_system`** (`:178-193`)：系统通知发送，区别于应用内 toast。支持看板娘大图显示，资源缺失时降级。
*   **`with_vpn_hint`** (`:196-202`)：探测/登录失败通知追加 VPN 接管提示。
*   **`start_background_check`** (`:205-250`)：启动后台监控命令。幂等处理，刷新间隔，启动前台服务保活及 WiFi 监听。
*   **`stop_background_check`** (`:253-265`)：停止后台监控命令。
*   **`WIFI_WATCHER_CHANNEL_ID`** (`:269`)：WiFi 变化监听 channel ID。
*   **`WIFI_WATCHER_ACTIVE`** (`:270`)：WiFi 变化监听激活状态。
*   **`start_wifi_watcher`** (`:273-290`)：注册 WiFi 变化监听。
*   **`stop_wifi_watcher`** (`:293-307`)：注销 WiFi 变化监听。
*   **`handle_wifi_event`** (`:310-340`)：WiFi 变化事件处理。去抖 + 延迟后执行一次完整检测。
*   **`trigger_background_check`** (`:343-351`)：手动触发后台检测命令。
*   **`get_background_status`** (`:354-356`)：获取后台状态命令。
*   **`get_boot_autostart`** (`:359-370`)：获取开机自启开关命令。
*   **`set_boot_autostart`** (`:373-392`)：设置开机自启开关命令。
*   **`get_notification_enabled`** (`:395-397`)：获取通知开关命令。
*   **`set_notification_enabled`** (`:400-417`)：设置通知开关命令。
*   **`run_startup_tasks`** (`:420-470`)：启动恢复任务。并行拉起后台检测、质量首测、自动登录等。
*   **`probe_with_retry`** (`:473-492`)：启动探测重试逻辑，3s 后重试一次。
*   **`emit_auto_login_result`** (`:495-503`)：启动自动登录结果走应用内通知。
*   **`auto_login_on_start`** (`:506-543`)：启动自动登录。在校内网环境下触发登录，成功/失败均通过 `emit_login_log` 记录。
*   **`ProbeWindowGuard`** (`:546-554`)：探针窗口 guard，持 WifiLock/WakeLock。
*   **`power_state`** (`:557-565`)：查询电源状态（屏幕交互中、当前活动网络为 WiFi）。
*   **`monitor_tick_loop`** (`:568-621`)：监控循环主体。处理定时动作、间隔热更新、分档跳拍及调用 `run_check_once`。
*   **`outbound_switch_active`** (`:624-629`)：判断是否处于夜间出站切换态。
*   **`latest_settings`** (`:632-637`)：取最新配置用于构造落盘快照。
*   **`logout_for_outbound_switch`** (`:640-676`)：切换侧注销逻辑。
*   **`switch_logout_unconfirmed`** (`:679-697`)：注销未确认生效时的上限判定。
*   **`portal_probe_on_little_cores`** (`:700-749`)：Portal 全量探测，专用短命线程执行并绑定小核。
*   **`run_scheduled_actions`** (`:752-888`)：定时登录/定时注销判定与执行，附自动切换运营商。
*   **`persist_settings`** (`:891-911`)：落盘夜间切换后的配置并刷新内存缓存。
*   **`night_switch_login`** (`:914-946`)：夜切后的登录触发。直接走 run_login 内核。
*   **`trigger_wifi_recheck`** (`:949-971`)：触发系统对当前 WiFi 重新验证。
*   **`ensure_avoid_bad_wifi`** (`:974-1002`)：程序化确保 network_avoid_bad_wifi=1。
*   **`restore_avoid_bad_wifi`** (`:1005-1027`)：按插件快照还原 network_avoid_bad_wifi。
*   **`NIGHT_VERIFY_WAIT_SECS`** (`:1030`)：夜切验证等待秒数，值为 `15`。
*   **`NIGHT_VERIFY_RETRY_SECS`** (`:1031`)：夜切验证重试间隔秒数，值为 `5`。
*   **`NIGHT_VERIFY_ATTEMPTS`** (`:1032`)：夜切验证最大尝试次数，值为 `3`。
*   **`NIGHT_VERIFY_HTTP_TIMEOUT_SECS`** (`:1033`)：夜切验证 HTTP 超时秒数，值为 `15`。
*   **`portal_origin`** (`:1036-1047`)：提取 portal URL 的 origin。
*   **`expected_operator_display`** (`:1050-1055`)：期望运营商的展示名。
*   **`verify_night_switch`** (`:1058-1118`)：夜切后的登录生效自动验证。
*   **`night_switch_verify_round`** (`:1121-1139`)：一轮验证，多次重试。
*   **`night_switch_check_once`** (`:1142-1163`)：单次 chkstatus 查询+uid 核对。
*   **`run_check_once`** (`:1166-1328`)：单次完整检测流程。包含校园网判定、Portal 探测、状态机翻转、自动重登决策及 payload 组装 emit。内部多处调用 `emit_login_log` 记录检测结果、自动登录成败及配置读取失败等信息。
*   **`notify_state`** (`:1331-1338`)：常驻通知状态码与文案纯函数。

### 类型定义：android/frontend/src/shared/ui-types.ts

*   **`StatusState`** (`:1`)：状态类型枚举。`'loading' | 'online' | 'offline' | 'error'`。
*   **`PanelName`** (`:2`)：面板名称枚举。`'dashboard' | 'account' | 'selfservice' | 'network' | 'monitor' | 'quality' | 'settings' | 'log' | 'speedtest'`。
*   **`ThemeName`** (`:3`)：主题名称枚举。`'default' | 'vibrant' | 'forest' | 'midnight' | 'ocean' | 'cherry' | 'custom'`。
*   **`LogType`** (`:4`)：日志类型枚举。定义为 `'info' | 'success' | 'error' | 'warning'`。这是前后端契约的关键部分。
*   **`GpuTier`** (`:5`)：GPU 等级枚举。`'low-igpu' | 'mid-igpu' | 'high-igpu' | 'discrete' | 'unknown'`。
*   **`GpuInfo`** (`:7-14`)：GPU 信息接口。包含 `vendor`, `model`, `vram_mb`, `is_integrated`, `tier`, `gpu_preference`。
*   **`LogEntry`** (`:17-22`)：日志条目接口，包含 `id`, `time`, `message`, `type` (即 `LogType`) 及可选的 `count` (重复折叠计数)。
*   **`ToastMessage`** (`:24-38`)：Toast 消息接口。包含 `id`, `title`, `description`, `type`, `duration`, `mascot`, `action`。
*   **`AdapterDisabledWarningData`** (`:40-43`)：适配器禁用警告数据接口。
*   **`AutoExitCountdownData`** (`:45-48`)：自动退出倒计时数据接口。
*   **`SaveConfigResult`** (`:50-54`)：保存配置结果接口。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Android Backend                         │
│  (android/src-tauri/src/monitor_loop.rs)                    │
│                                                             │
│  ┌──────────────────┐       ┌──────────────────────────┐    │
│  │ run_check_once() │──────►│ emit_login_log(msg,type) │    │
│  │ run_scheduled_   │       │ emits "login-log" event  │    │
│  │ actions()        │       └──────────┬───────────────┘    │
│  │ auto_login_on_   │                  │ (Tauri IPC)        │
│  │ start()          │                  ▼                    │
│  └──────────────────┘       ┌──────────────────────────┐    │
│                             │ Tauri Event Bus           │    │
│                             │ "login-log"               │    │
│                             └──────────┬───────────────┘    │
└────────────────────────────────────────┼───────────────────┘
                                         │
                                         ▼
┌─────────────────────────────────────────────────────────────┐
│                      Frontend                               │
│  (android/frontend/)                                        │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ useEventListeners.ts                                  │   │
│  │                                                       │   │
│  │ onLoginLog(data)                                      │   │
│  │   ├─ type: data.type as LogType || 'info'             │   │
│  │   └─ addLog(message, type)                            │   │
│  └───────────────────────┬──────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ ui-types.ts                                           │   │
│  │                                                       │   │
│  │ LogType = 'info'|'success'|'error'|'warning'          │   │
│  │ LogEntry { type: LogType }                            │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **后端生成日志**：`monitor_loop.rs` 中的 `run_check_once`、`run_scheduled_actions` 或 `auto_login_on_start` 根据运行状态（如检测失败、自动登录成功、定时任务触发）调用 `emit_login_log(app, message, log_type)`。
2.  **事件发射**：`emit_login_log` 构造 JSON 对象 `{ "message": "...", "type": "warning" }`（注：现已修正为 `"warning"`），并通过 `app.emit("login-log", payload)` 发送给前端。
3.  **前端接收**：`useEventListeners.ts` 中的 `api.onLoginLog` 回调捕获数据。
4.  **类型转换与存储**：回调中将 `data.type` 转换为 `LogType` 类型（使用 `as LogType` 断言，若为空则 fallback 到 `'info'`），然后调用 `useLogToastStore.getState().addLog(message, type)` 存入前端状态。
5.  **UI 渲染**：日志列表组件从 store 读取 `LogEntry`，根据其 `type` 匹配对应的图标和颜色样式进行渲染。

## Known Issues

*   **历史白屏根因**：过去 `emit_login_log` 曾使用 `"warn"` 作为 `type` 值，该值不在前端 `LogType` 枚举中。前端 `LOG_ICONS[type]` 取值为 `undefined`，导致 React 组件渲染崩溃（白屏）。
*   **当前状态**：后端代码已全面修正，所有 `emit_login_log` 调用均使用 `"info"`, `"success"`, `"error"`, `"warning"` 四个合法值。
*   **防御性编程**：前端 `useEventListeners.ts` 增加了 `(data.type as LogType) || 'info'` 的兜底；`ui-types.ts` 定义了严格的 `LogType` 枚举。这确保了即使未来后端发生漂移，前端也不会崩溃，而是静默降级。
*   **桌面端风险**：桌面端 (`tauri-app`) 尚未完全对齐此防御机制，其 `RightPanel.tsx` 中部分 `LOG_ICONS` 访问仍缺乏 `?? Info` 兜底，存在潜在的类似白屏风险。

## Connections

[[tablet-layout-alignment-audit]]、[[android-white-screen-missing-init-fields]]