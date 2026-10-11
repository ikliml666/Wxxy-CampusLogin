---
title: 后台巡检与自动登录全链路
type: concept
source_files:
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/monitor/background_task.rs
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/background_emit.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/infra/notification.rs
tags: [概念, 后台巡检, 自动登录, 断线重连, 生命周期, 通知]
---

## Overview

后台巡检是一条"启动装配 → 周期 tick → 校园网判定 → Portal 探测 → 状态机 → 自动登录 / 断线重连 → 生命周期倒计时 / 通知"的长链路。桌面端实现在 `monitor/` 模块群（`watcher` 门面 + `background_check` 主体），安卓端实现在单个 `monitor_loop.rs`。两端共享同一套阈值语义（连续失败 5 次熔断、重连上限 3、冷却 60s、注销保护 60s），但周期常量、检测时段门控与省电分档策略不同。

## Key Components

### 核心结构体与枚举

**`BackgroundCheckResult`** (`tauri-app/src-tauri/src/monitor/background_emit.rs:14-32`)
后台检测结果聚合，封装 `emit_background_check_result` 所需的全部数据字段：
- `online: bool`: 主适配器在线状态
- `reachable: bool`: 服务器可达性
- `login_available: bool`: 是否可登录
- `message: &'a str`: 主适配器消息
- `adapter1_name: &'a str`, `adapter2_name: &'a str`: 适配器名称
- `secondary_online: Option<bool>`, `secondary_message: &'a str`: 副适配器状态
- `online_operator: Option<String>`, `secondary_online_operator: Option<String>`: 从 Portal uid 推导的运营商后缀
- `dual_adapter: bool`: 是否双适配器模式
- `config: &'a crate::config::model::Config`: 配置引用
- `campus_result: &'a CampusCheckResult`: 校园网检测结果
- `a1_campus_msg: Option<&'a str>`, `a2_campus_msg: Option<&'a str>`: 适配器校园网消息
- `a1_on_campus: Option<bool>`, `a2_on_campus: Option<bool>`: 适配器是否在校园网

**`PortalCheckResult`** (`tauri-app/src-tauri/src/monitor/portal_check.rs:7-16`)
Portal 探测结果枚举：
- `Success { online, message, reachable, login_available, uid }`: 成功结果
- `Error { is_request_failed }`: 错误结果（区分请求失败与其他异常）
- `NotFound`: 未找到适配器

**`CampusCheckResult`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:11-18`)
校园网检测结果：
- `wifi: Option<ConnectionCampusStatus>`: WiFi 状态
- `wired: Option<ConnectionCampusStatus>`: 有线状态
- `on_campus: bool`: 是否在校园网
- `current_ssid: Option<String>`: 当前 SSID 或有线 profile
- `message: String`: 描述消息

**`ConnectionCampusStatus`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:5-9`)
连接校园网状态：
- `on_campus: bool`
- `name: Option<String>`
- `message: String`

### 核心函数

**`run_startup_tasks`** (`tauri-app/src-tauri/src/monitor/watcher.rs:19-73`)
启动时装配所有后台任务：
- 如果 `enable_background_check`，spawn "startup_bg_check" 跟踪任务
- 如果 `enable_network_quality && enable_latency_test`，spawn "startup_latency"
- 如果仅 `enable_network_quality`，spawn "startup_quality_once"
- spawn "startup_auto_login"
- spawn "scheduled_actions"（每日定时登录/注销，独立循环）

**`start_background_check_inner`** (`tauri-app/src-tauri/src/monitor/background_task.rs:11-62`)
启动后台检测循环：
- 幂等置 `enable_background_check=true`；interval < 10000 时补 15000
- spawn "background_check" 任务
- 立即首拍执行 `run_background_check`
- loop 每 tick 重读 `config.background_check_interval` 重建计时器
- persist::save_config_and_broadcast（落盘 + config-changed）

**`run_background_check`** (`tauri-app/src-tauri/src/monitor/background_check.rs:340-348`)
后台巡检异步入口：spawn_blocking 包装阻塞函数

**`run_background_check_blocking`** (`tauri-app/src-tauri/src/monitor/background_check.rs:14-335`)
后台检测阻塞主体：
- exit.is_quitting / cancel_token 早退
- tasks.is_checking.try_acquire() 单飞保护
- get_adapters_cached() 失败回退 get_adapters_force()
- resolve_adapter_names + find_dual_adapters
- is_campus_check_silent(now_min, start, end) 静默期判定
- check_campus_network(&config, &op_adapters) 校园网检测
- 状态写回 + campus fail 清 has_logged_online
- [campus fail] emit background-check-result → 无 IP 跳过退出 / start_campus_exit
- cancel_campus_exit
- Portal 探测（dual_adapter 时两个 spawn_blocking + tokio::join! 并行）
- [request_failed] handle_portal_request_failure（阈值 5 次 → MAC 重置）
- [Success] 按适配器重置失败计数
- handle_status_change（在线↔离线翻转 → 系统通知，60s 节流）
- emit_background_check_result（checkCount / 注销保护期强制 online=false）
- try_auto_login_on_preparation
- try_disconnect_reconnect
- update_network_state（auto_exit_on_online → start_auto_exit）

**`try_auto_login_on_preparation`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:30-99`)
准备自动登录：
- 检查 login_available / online / auto_login_on_preparation 开关
- 检查 has_logged_online
- 检查 prep_login_failures >= PREP_LOGIN_MAX_FAILURES (5次)
- 检查注销保护期
- 检查冷却时间
- 获取登录锁后执行 full_login
- 成功则重置失败计数，失败则累加

**`try_disconnect_reconnect`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:101-223`)
断线重连：
- 检查 any_offline / any_adapter_online / reachable / login_available
- 检查注销保护期与冷却时间
- 获取登录锁
- 读-增-取值在单次 CAS 内完成 reconnect_count
- within_limit 时执行 full_login
- 成功则重置计数、any_adapter_online、has_logged_online
- 超限则释放锁并提醒

**`run_auto_login_on_start`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:219-468`)
开机自启登录流程：
- 检查 auto_login_on_start 开关
- initial_delay: --autostart 时 5000ms，否则 1500ms
- 获取适配器列表（--autostart 时最多重试 3 次 × 3000ms）
- 校园网检测（静默期跳过 / 失败触发 start_campus_exit）
- Portal 探测（双适配器并行）
- 已在线则跳过登录
- 执行 full_login

**`check_campus_network`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:35-170`)
校园网名称名单解析与检测：
- 支持顿号/全半角逗号/分号/空白分隔多个名称
- WiFi SSID 匹配 → 子网检查 (/18) → 网关可达性 → portal 可达性
- 有线 profile 匹配 → 子网检查 → 网关可达性 → portal 可达性
- on_campus = wifi.on_campus || wired.on_campus

**`is_campus_check_silent`** (`tauri-app/src-tauri/src/monitor/campus_check.rs:175-183`)
校园网检测静默期判定：
- start == 0 视为禁用
- now_minutes < start 静默
- end > start && now_minutes >= end 静默（end ≤ start 退化为仅受 start 限制）

**`emit_notification`** (`tauri-app/src-tauri/src/infra/notification.rs:22-65`)
发送系统通知：
- 仅在 enable_notification 为真且主窗口不可见或已最小化时调用
- Windows 桌面用自组 WinRT toast 带看板娘，失败降级插件
- Android 用 large_icon，资源缺失时降级纯文本

**`reconnect_should_report`** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:24-26`)
断线重连是否应向上报告"已重连成功"：
- login_success && within_limit 返回 true

### 测试模块

**`auto_auth.rs` tests** (`tauri-app/src-tauri/src/monitor/auto_auth.rs:474-490`)
- `reconnect_success_within_limit_reports_true`: 重连成功且未超限 → 必须返回 true
- `reconnect_failure_never_reports_true`: 失败 never 返回 true
- `reconnect_success_over_limit_reports_false`: 超过重连次数上限即使成功也不再上报

**`campus_check.rs` tests** (`tauri-app/src-tauri/src/monitor/campus_check.rs:191-225`)
- `名称名单_解析多分隔符与空项`: 验证 campus_name_list 解析
- `静默期判定_开始时间单边`: start=0 禁用，早于 start 静默
- `静默期判定_时间段双边`: 生效窗口 [07:40, 23:00) 外静默
- `静默期判定_结束不晚于开始时退化为单边`: end <= start 时仅受 start 限制

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│                        startup.rs                               │
│  setup_app() ──► watcher::run_startup_tasks(app_h)              │
└──────────────┬──────────────────────────────────────────────────┘
               │
       ┌───────┼───────────────┬───────────────┬───────────────┐
       ▼       ▼               ▼               ▼               ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│background_  │ │   latency   │ │  auto_login │ │scheduled_   │ │ updater     │
│task.rs      │ │   .rs       │ │   .rs       │ │ actions.rs  │ │ updater.rs│
│             │ │             │ │             │ │             │ │             │
│start_       │ │spawn_       │ │run_         │ │run_         │ │start_     │
│background_  │ │latency_     │ │auto_login_  │ │scheduled_   │ │update_    │
│check_inner  │ │test_loop    │ │on_start     │ │action_loop  │ │check_loop │
└──────┬──────┘ └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│                   background_check.rs                           │
│                                                                 │
│  run_background_check() ──► spawn_blocking                      │
│       │                                                         │
│       ▼                                                         │
│  run_background_check_blocking()                                │
│       ├─ is_checking.try_acquire() ◄── 单飞保护                  │
│       ├─ get_adapters_cached()/force()                          │
│       ├─ is_campus_check_silent() ──► check_campus_network()    │
│       │       │                                                 │
│       │       ├─ fail ──► start_campus_exit()                   │
│       │       └─ pass                                           │
│       │                                                         │
│       ├─ check_adapter_portal() (dual: tokio::join!)            │
│       │       │                                                 │
│       │       ├─ request_failed×5 ──► handle_portal_request_    │
│       │       │    failure()                                    │
│       │       └─ Success ──► reset failure count                │
│       │                                                         │
│       ├─ handle_status_change() ──► emit_notification (60s节流) │
│       ├─ emit_background_check_result()                         │
│       │       │                                                 │
│       │       ├─ logout_protected ──► force online=false        │
│       │       └─ normal                                         │
│       │                                                         │
│       ├─ try_auto_login_on_preparation()                        │
│       │       │                                                 │
│       │       ├─ cooldown / protected / max_failures guard      │
│       │       ├─ is_logging_in.try_acquire()                    │
│       │       └─ full_login()                                   │
│       │                                                         │
│       ├─ try_disconnect_reconnect()                             │
│       │       │                                                 │
│       │       ├─ reconnect_count read-inc-CAS                   │
│       │       ├─ within_limit ? full_login() : reminder         │
│       │       └─ reconnect_should_report()                      │
│       │                                                         │
│       └─ update_network_state() ──► auto_exit_on_online?        │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow

```text
配置(enable_background_check / interval)
  → 启动装配 run_startup_tasks
    → 周期 tick（桌面 tokio::time::interval 动态重建）
      → 适配器解析（主/副，含 disabled/disconnected 降级文案）
      → 校园网判定（时段门控 → SSID/有线 profile/网关可达性）
         ├─ fail → campus-exit-countdown → 30000ms 最小化 → 60000ms 退出（可 Ctrl+Shift+C 取消）
         └─ pass
      → Portal 探测（只读，绝不携带凭据）
         ├─ request_failed ×5 → 该适配器 MAC 重置 + DHCP 续租
         └─ Success → 清零失败计数
      → 状态机（any_online，注销保护期强制离线）
      → background-check-result 事件 + login-log
      → 情况分支
         ├─ 离线且 login 可用 → 准备自动登录（冷却 → 保护期 → 熔断 → 登录锁）
         │    └─ auto-login-result 事件 / 失败计数 +1
         ├─ 曾在线且现离线 → 断线重连（计数 +1，≤3 次）
         │    ├─ 成功 → 计数清零 + login-history("reconnect") + 事件
         │    └─ 超限 → 第 4 次起每 10 次提醒
         └─ 在线恢复 → auto_exit_on_online → auto-exit-countdown(20000ms)
      → update_network_state（server_available / any_adapter_online / last_a1_online）
```

## Connections

- [[desktop-monitor]] — `monitor/` 模块群逐文件详解
- [[android-backend]] — `monitor_loop.rs` 状态机与前台服务关系
- [[desktop-auth]] — `auth::service::full_login` 与失败计数器的实现
- [[desktop-app-lifecycle]] — `infra/lifecycle.rs` 退出流程与 `task_manager` 清理
- [[config-and-persistence]] — 间隔 / 阈值 / 时段字段的定义与迁移
- [[security-model]] — 自动登录使用内存明文凭据的边界
- [[ipc-command-surface]] — `background-check-result` / `auto-login-result` 事件的消费侧
- [[desktop-network-quality]] — `quality_scheduler` 与质量告警档位

## Known Issues

- **安卓无 MAC 重置链路**：桌面在认证失败 5 次（`failure_tracker.rs:6`）与 Portal 请求失败 5 次（`:194`）时触发 `dhcp_release_renew_single`，安卓完全没有对应实现——安卓的 `consecutive_failures`（`monitor_loop.rs:789`）只用于停止自动重登。
- **安卓断线重连次数在 `should_attempt_login` 与调用侧重复判上限**：`should_attempt_login` 内已有 `reconnect_count >= max_reconnect`（`monitor_loop.rs:89-91`），`run_check_once` 又在自增后判 `count >= settings.max_disconnect_reconnect` 发通知（`:806-808`），两处阈值语义（≥ vs 自增后）容易理解错位。
- **桌面 `background_check_interval` 与安卓默认值不一致**：桌面 15000（`model.rs:180`），安卓 60000（`config_state.rs:88`），且安卓迁移逻辑会把 15000 的旧值刷成 60000（`config_state.rs:243-245`）。同一字段两端默认值分叉，双端同步约定（`AGENTS.md` 第 3 条）在此处未落实。
- **桌面 `updater` 更新检查循环与 `run_startup_tasks` 分离**：桌面在 `app/startup.rs:193` 单独调 `start_update_check_loop`，安卓在 `run_startup_tasks` 内调（`monitor_loop.rs:408`），两端启动装配顺序不同。
- **`CAMPUS_MINIMIZE_DELAY_MS` 硬编码且不可配**：`lifecycle.rs:12-13` 与 `AUTO_EXIT_DELAY_MS` 都是常量，用户无法调整倒计时长度，只能整体开关（`campus_exit_on_fail` / `auto_exit_after_login`）。
- **静默期语义两端不同**：桌面静默期构造 `on_campus: true` 并继续走完整 Portal 探测（`background_check.rs:63-70`），安卓直接整拍 `return` 不探测（`monitor_loop.rs:695-715`）。2026-09-13 起安卓在静默期 return 前会把常驻通知重建为「已暂停检测(非检测时段)」，两端"在线状态"在静默期都保持上一拍记忆，但安卓通知面不再显示陈旧在线；在线状态本身的新鲜度两端仍不可比。
- **安卓 `run_startup_tasks` 就绪窗口 500ms 无重试兜底**：注释（`monitor_loop.rs:382-386`）说明网络未就绪靠"自动登录一次重试 + 后台检测下一拍"，即启动即失败会静默等到下一拍（最长 idle 300s）。
- **`try_disconnect_reconnect` 参数多达 10 个**（`auto_auth.rs:109-119`），`#[allow(clippy::too_many_arguments)]` 已经压不住可读性问题；`background_check.rs:322-326` 的调用点需逐个对齐位置参数，改动风险高。