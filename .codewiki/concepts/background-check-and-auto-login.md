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
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/auth/failure_tracker.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/infra/notification.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/config_state.rs
tags: [概念, 后台巡检, 自动登录, 断线重连, 生命周期, 通知]
---

## Overview

后台巡检是一条"启动装配 → 周期 tick → 校园网判定 → Portal 探测 → 状态机 → 自动登录 / 断线重连 → 生命周期倒计时 / 通知"的长链路：桌面实现在 `monitor/` 模块群（`watcher` 门面 + `background_check` 主体），安卓实现在单个 `monitor_loop.rs`。两端共享同一套阈值语义（连续失败 5 次熔断、重连上限 3、冷却 60s、注销保护 60s），但周期常量、检测时段门控与省电分档策略不同。

## 机制说明

### 完整调用链

```text
app/startup.rs:195  crate::monitor::watcher::run_startup_tasks(&app_h)
  └─ monitor/watcher.rs:15-54  run_startup_tasks
       ├─ [enable_background_check]  spawn "startup_bg_check"
       │    └─ monitor/background_task.rs:7-60  start_background_check_inner
       │         ├─ 幂等置 enable_background_check=true；interval<10000 → 15000      (:8-13)
       │         ├─ spawn "background_check" 任务
       │         │    ├─ 立即首拍 run_background_check                               (:24)
       │         │    └─ loop：每 tick 重读 interval.max(10000) 重建计时器            (:29-49)
       │         └─ save_config_to_disk_encrypted（落盘 + config-changed）           (:55-57)
       ├─ [enable_network_quality && enable_latency_test]  spawn "startup_latency"
       │    └─ monitor/latency.rs:52-102  spawn_latency_test_loop
       └─ spawn "startup_auto_login"
            └─ monitor/auto_auth.rs:219-468  run_auto_login_on_start

单拍的主体
  monitor/background_check.rs:340-348  run_background_check（spawn_blocking 包装）
    └─ background_check.rs:14-335  run_background_check_blocking
         ├─ exit.is_quitting / cancel_token 早退                                      (:15-17)
         ├─ tasks.is_checking.try_acquire() 单飞（上一拍未完成直接返回）               (:18-20)
         ├─ get_adapters_cached() 失败回退 get_adapters_force()                       (:28-37)
         ├─ resolve_adapter_names + find_dual_adapters                                (:41-43)
         ├─ is_campus_check_silent(now_min, start, end) 静默期 → 伪 on_campus + cancel_campus_exit (:46-70)
         ├─ check_campus_network(&config, &op_adapters)                               (:74)
         ├─ 状态写回 + campus fail 清 has_logged_online                                (:79-91)
         ├─ [campus fail] emit background-check-result → 无 IP 跳过退出 / start_campus_exit → return (:103-132)
         ├─ cancel_campus_exit                                                         (:136)
         ├─ Portal 探测（dual_adapter 时两个 spawn_blocking + tokio::join! 并行）        (:143-176)
         ├─ [request_failed] handle_portal_request_failure（阈值 5 次 → MAC 重置）      (:185-206)
         ├─ [Success] 按适配器重置失败计数                                              (:211-233)
         ├─ handle_status_change（在线↔离线翻转 → 系统通知，60s 节流）                   (:283-288)
         ├─ emit_background_check_result（checkCount / 注销保护期强制 online=false）    (:299-318)
         ├─ try_auto_login_on_preparation                                             (:320)
         ├─ try_disconnect_reconnect                                                  (:322-326)
         └─ update_network_state（auto_exit_on_online → start_auto_exit）              (:331)
```

### 周期常量与阈值（具体值）

| 项 | 桌面 | 安卓 | 位置 |
|---|---|---|---|
| 后台检测间隔默认 | 15000 ms | 60000 ms | 桌面 `config/model.rs:180`；安卓 `config_state.rs:88` |
| 后台检测间隔下限 | 10000 ms（`start_background_check` 内 <10000 时补 15000） | 5000 ms | 桌面 `background_task.rs:10-12`；安卓 `monitor_loop.rs:162` |
| 闲时巡检间隔 | 无（桌面不分档） | 300000 ms | 安卓 `config_state.rs:90`、`monitor_loop.rs:61-68` |
| 质量测试间隔默认 | 60000 ms | 60000 ms | 桌面 `model.rs:187`；安卓 `config_state.rs:101` |
| 质量测试间隔下限 | 10000 ms（启动时 <10000 → 30000） | 10000 ms | 桌面 `watcher.rs:38`、`validate.rs:104`；安卓 `quality_cmds.rs:63` |
| 认证失败熔断（触发 MAC 重置） | 5 次 | 无此链路 | 桌面 `failure_tracker.rs:6` |
| Portal 请求失败熔断 | 5 次（网关可达才计数） | 无 | 桌面 `failure_tracker.rs:194`、`:213-226` |
| 准备自动登录失败熔断 | 5 次（本会话停止） | 5 次 | 桌面 `auto_auth.rs:18`；安卓 `monitor_loop.rs:694` |
| 自动登录冷却 | `auto_login_cooldown_secs` 默认 60 s | 同 60 s | 桌面 `model.rs:148`；安卓 `config_state.rs:93` |
| 断线重连次数上限 | `max_disconnect_reconnect` 默认 3 | 同 3 | 桌面 `model.rs:146`；安卓 `config_state.rs:92` |
| 超限后重复提醒间隔 | 每 10 次 | 无 | 桌面 `auto_auth.rs:13`、`:202` |
| 注销保护期 | 60 s（注释） | 60 s（注释与实现） | 安卓 `monitor_loop.rs:688` |
| 网络状态变更通知节流 | 60000 ms | 无（按状态翻转） | 桌面 `background_emit.rs:95` |
| 自动退出倒计时 | `AUTO_EXIT_DELAY_MS = 20000` | 无此能力 | 桌面 `infra/state/mod.rs:13` |
| 校园网退出倒计时 | 最小化 30000 / 退出 60000 | 无此能力 | 桌面 `lifecycle.rs:12-13` |
| 启动自动登录就绪延迟 | 1500 ms（`--autostart` 时 5000 ms） | 500 ms | 桌面 `auto_auth.rs:227-228`；安卓 `monitor_loop.rs:383` |
| 开机自启适配器重试 | 3 次 × 3000 ms | 无 | 桌面 `auto_auth.rs:266-270` |
| 检测时段默认窗口 | `campusCheckStartMinutes` 460（07:40）、`campusCheckEndMinutes` 0（不限） | 同 460 / 0 | 桌面 `model.rs:202-203`；安卓 `config_state.rs:114-116` |
| 校园网退出生效时段默认 | 480–1380（08:00–23:00） | 无 | 桌面 `model.rs:200-201` |
| 取消退出快捷键 | `CommandOrControl+Shift+C` | 无 | 桌面 `infra/state/mod.rs:14` |
| WiFi 事件延迟 / 去抖 | 无（无事件触发） | 2500 / 1000 ms | 安卓 `monitor_loop.rs:44/47` |

### 各环节行为细节

**检测时段门控**（桌面 `monitor/campus_check.rs:244-252`）：`start == 0` 视为禁用；早于 start 静默；`end > start && now >= end` 静默（end ≤ start 退化为仅受 start 限制）。静默期桌面构造 `on_campus: true` 的伪结果并 `cancel_campus_exit`（`background_check.rs:63-70`），安卓直接 `return` 整拍跳过（`monitor_loop.rs:617-626`），两者都保持上一拍在线记忆。

**状态机与在线判定**：桌面 `prev_online` 取 `any_adapter_online`，`any_online = online || secondary_online == Some(true)`（`background_check.rs:248/282`），历史缺陷注释（`:277-281`）记录曾用"仅主适配器 online"比较导致双适配器"主断副通"每拍误报离线。安卓用三态护栏：只有 `error_kind.is_none()` 的确定判定才翻转状态，Unknown/Failed 保持 `prev_online`（`monitor_loop.rs:669-672`）。

**注销保护期**：桌面 `logout_protected_until` 在 `emit_background_check_result` 里强制 `online=false`（`background_emit.rs:122-130`），并在 `update_network_state` 里跳过状态写回（`:170-176`）；自动登录与重连两条路径都检查它（`auto_auth.rs:56-60`、`:129-133`）。安卓用 `logout_protected_until_ms`，`run_check_once` 里判 `now < logout_protected_until_ms` 则 `should=false`（`monitor_loop.rs:688/693-695`）。

**通知出口**：桌面 `emit_notification`（`infra/notification.rs:22-65`）只在"`enable_notification` 为真 且 主窗口不可见或已最小化"时弹系统通知（`:26-40`）；Windows 桌面用自组 WinRT toast 带看板娘，失败降级插件（`:53-60`）；`mascot` 变体名 `mascot-alert` / `mascot-celebrate` / `mascot-portrait` / `mascot-offline` / `mascot-busy`。安卓 `notify_system`（`monitor_loop.rs:139-156`）用 `large_icon`，两个在用大图 `mascot_alert` / `mascot_offline` 已随 APK 内置（`android/src-tauri/gen/android/app/src/main/res/drawable-xxhdpi/`），资源缺失时降级纯文本仅作兜底。

**生命周期倒计时**：自动退出 `start_auto_exit`（`lifecycle.rs:183-264`）用 `auto_exit_deadline` + `auto_exit_cancelled` 双状态防重复触发；校园网退出 `start_campus_exit`（`lifecycle.rs:23-135`）先 CAS `campus_exit_started` 再设 deadline（注释 `:45-50` 记录原顺序导致的永久卡死缺陷），受 `campus_exit_on_fail` 与生效时段双重门控（`:25-41`）。两者退出前都 `task_manager.detach` 自身避免 shutdown 自等死锁（`:126`、`:256`）。

**质量循环与告警**：`spawn_latency_test_loop`（`latency.rs:52-102`）用 `MissedTickBehavior::Delay`（`:59`），ready 前每 2s 短重试不消耗周期（`:79-90`），检测前固定等 1s（`:92-95`）；档位变化只在 `good → poor/bad` 或反向时告警（`latency.rs:16-36`，`BAD_LEVELS = ["poor","bad"]`），并经 `quality_scheduler` 复核。

## 关键约束

- **单飞保护**：`tasks.is_checking.try_acquire()`（`background_check.rs:18`）保证同一时刻只有一拍在跑；`trigger_background_check` 也先查 `is_checking.is_active()`（`commands/background.rs:30-32`）。
- **登录互斥**：所有登录路径（准备自动登录 / 断线重连 / 开机自启 / 手动）必须先拿 `tasks.is_logging_in.try_acquire()`（`auto_auth.rs:69/141/425`）。
- **冷却与保护期必须在登录锁之前判**：`last_auto_login_attempt` 只在真正要登录前才更新（`auto_auth.rs:71`、`:172`、`:429`），否则跳过路径也会重置冷却。
- **重连计数必须读-增-判定在同一 CAS 内**：`update_with_result`（`auto_auth.rs:151-154`、`failure_tracker.rs:64-67/137-146/230-239`），历史缺陷注释记录 load + update 两次快照导致双重触发 MAC 重置。
- **campus fail 必须重置 `has_logged_online`**：`background_check.rs:85-89` 注释说明不重置会导致离开校园网再回来时准备自动登录被永久拦截。
- **重连成功必须向上报告 `reconnected=true`**：`reconnect_should_report`（`auto_auth.rs:24-26`）与三处测试（`:474-490`）；否则调用方用重连前旧 Portal 快照覆盖 `any_adapter_online`，用户被困离线。
- **间隔必须动态读取**：桌面 loop 每 tick 重读 `config.background_check_interval`（`background_task.rs:29-34`），安卓对比 `desired_interval_ms` 重建计时器（`monitor_loop.rs:538-543`），否则设置面板改间隔不生效。
- **通知重复抑制**：桌面"自动登录成功"走应用内事件 `auto-login-result` 不留系统通知重复（`auto_auth.rs:93-95` 只在配置开启时发系统通知），状态变更通知 60s 节流（`background_emit.rs:88-99`）；安卓注释明确启动自动登录与首拍重登连续发生，成功走事件、失败才留系统通知（`monitor_loop.rs:441-443`）。
- **弹窗文案与 i18n**：后端系统通知文案为中文硬编码（`infra/notification.rs:17-20` 说明后端无法感知前端语言，且仅在用户未看界面时出现）。
- **shutdown 必须限时**：`shutdown_and_exit` 对 `task_manager.shutdown()` 加 10s 上限（`lifecycle.rs:315-317`）。

## Data Flow

```text
配置(enable_background_check / interval)
  → 启动装配 run_startup_tasks
    → 周期 tick（桌面 tokio::time::interval 动态重建 | 安卓 interval + 分档判定）
      → 适配器解析（主/副，含 disabled/disconnected 降级文案）
      → 校园网判定（时段门控 → SSID/有线 profile/网关可达性）
         ├─ fail → campus-exit-countdown → 30000ms 最小化 → 60000ms 退出（可 Ctrl+Shift+C 取消）
         └─ pass
      → Portal 探测（只读，绝不携带凭据）
         ├─ request_failed ×5 → 该适配器 MAC 重置 + DHCP 续租
         └─ Success → 清零失败计数
      → 状态机（any_online，注销保护期强制离线，安卓三态护栏）
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

### 安卓侧对应实现的差异（`android/src-tauri/src/monitor_loop.rs`）

| 维度 | 安卓做法 | 位置 |
|---|---|---|
| 入口 | `lib.rs:51` → `run_startup_tasks`，先 sleep 500ms，再**并行** spawn 后台检测 / 质量 / 自动登录 / 更新检查循环 | `lib.rs:51`、`monitor_loop.rs:381-419` |
| 状态容器 | 全局 `lazy_static MONITOR: MonitorState`（进程内原子量），非桌面 `AppState` | `monitor_loop.rs:11-39` |
| 分档省电 | `effective_interval_ms(base, idle, screen_on, wifi_connected)`：亮屏 + WiFi 用 base，否则 idle | `monitor_loop.rs:61-68` |
| 事件驱动 | WiFi 变化监听（Kotlin NetworkCallback → Channel）触发即时检测，去抖 1000ms + 延迟 2500ms，风暴内最后一条生效 | `monitor_loop.rs:52-54`、`:218-291` |
| 保活 | 前台服务常驻通知；仅在线状态翻转时 `update_notification`（`notified_online` 记忆），不每拍重建 | `monitor_loop.rs:159-194`、`:764-774` |
| 探针窗口锁 | `begin_probe_window` / `ProbeWindowGuard` Drop 保证 WifiLock + WakeLock 释放 | `monitor_loop.rs:497-508`、`:630-635` |
| 绑 WiFi | 每拍探测与重登前 `ensure_wifi_bound` | `monitor_loop.rs:459`、`:639` |
| 绑小核 | Portal 探测跑在专用短命线程并 `pin_current_thread_to_little_cores`（线程内 `handle.enter()` 补 reactor） | `monitor_loop.rs:563-600` |
| 决策纯函数 | `should_attempt_login`（在线/非校园网/开关/上限/冷却五条件） | `monitor_loop.rs:72-93` |
| 无多适配器 | payload 无 `secondaryOnline`，`adapter2Name` 恒空 | `monitor_loop.rs:743-755` |
| 无退出倒计时 | 无校园网退出、无自动退出、无 MAC 重置链路 | — |
| 启动自动登录 | `probe_with_retry`（3s 重试一次）→ 登录成功预置 `was_online=true` + 清注销保护期 | `monitor_loop.rs:424-439`、`:481-482` |
| 状态初值 | `status_value()` 把最近一次结果展平到顶层，供 `get_init_data` 提供启动初值（首轮 emit 早于 WebView 监听建立） | `monitor_loop.rs:96-116` |

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

- **安卓无 MAC 重置链路**：桌面在认证失败 5 次（`failure_tracker.rs:6`）与 Portal 请求失败 5 次（`:194`）时触发 `dhcp_release_renew_single`，安卓完全没有对应实现——安卓的 `consecutive_failures`（`monitor_loop.rs:694`）只用于停止自动重登。
- **安卓断线重连次数在 `should_attempt_login` 与调用侧重复判上限**：`should_attempt_login` 内已有 `reconnect_count >= max_reconnect`（`monitor_loop.rs:89-91`），`run_check_once` 又在自增后判 `count >= settings.max_disconnect_reconnect` 发通知（`:711-713`），两处阈值语义（≥ vs 自增后）容易理解错位。
- **桌面 `background_check_interval` 与安卓默认值不一致**：桌面 15000（`model.rs:180`），安卓 60000（`config_state.rs:88`），且安卓迁移逻辑会把 15000 的旧值刷成 60000（`config_state.rs:243-245`）。同一字段两端默认值分叉，双端同步约定（`AGENTS.md` 第 3 条）在此处未落实。
- **桌面 `updater` 更新检查循环与 `run_startup_tasks` 分离**：桌面在 `app/startup.rs:193` 单独调 `start_update_check_loop`，安卓在 `run_startup_tasks` 内调（`monitor_loop.rs:408`），两端启动装配顺序不同。
- **`CAMPUS_MINIMIZE_DELAY_MS` 硬编码且不可配**：`lifecycle.rs:12-13` 与 `AUTO_EXIT_DELAY_MS` 都是常量，用户无法调整倒计时长度，只能整体开关（`campus_exit_on_fail` / `auto_exit_after_login`）。
- **静默期语义两端不同**：桌面静默期构造 `on_campus: true` 并继续走完整 Portal 探测（`background_check.rs:63-70`），安卓直接整拍 `return` 不探测（`monitor_loop.rs:617-626`）。非在校时段的"在线状态新鲜度"因此不可比。
- **安卓 `run_startup_tasks` 就绪窗口 500ms 无重试兜底**：注释（`monitor_loop.rs:376-380`）说明网络未就绪靠"自动登录一次重试 + 后台检测下一拍"，即启动即失败会静默等到下一拍（最长 idle 300s）。
- **`try_disconnect_reconnect` 参数多达 10 个**（`auto_auth.rs:109-119`），`#[allow(clippy::too_many_arguments)]` 已经压不住可读性问题；`background_check.rs:322-326` 的调用点需逐个对齐位置参数，改动风险高。
