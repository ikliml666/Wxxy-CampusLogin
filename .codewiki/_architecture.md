---
title: 系统架构总览
type: architecture
source_files:
  - AGENTS.md
  - tauri-app/src-tauri/Cargo.toml
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/app/heartbeat.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/auth/session.rs
  - tauri-app/src-tauri/src/network/client.rs
  - tauri-app/src-tauri/src/platform/mod.rs
tags: [架构, 总览]
---

## Overview

Wxxy-CampusLogin 是无锡学院校园网（Dr.COM / ePortal）的自动登录助手，由 **Windows 桌面端**与**安卓端**两个同构应用组成，均使用 Tauri 2（Rust 后端 + React 19 / TypeScript 前端）。当前版本 `2.4.1`（`tauri-app/src-tauri/Cargo.toml:3`）。

同构的实现方式不是代码共享，而是**协议单点 + 平台外壳**：登录/注销/Portal 探测/自助服务/网络质量等全部协议实现只存在于桌面协议核心 crate（`tauri-app/src-tauri`，package `campus-login`，lib `campus_login_lib`，见 `tauri-app/src-tauri/Cargo.toml:8-10`），安卓后端以 Cargo path 依赖引用它（`android/src-tauri/Cargo.toml:34`），禁止复制；桌面侧与平台耦合的模块用 `#[cfg(desktop)]` 整体门控（`tauri-app/src-tauri/src/lib.rs:11-20`），安卓侧只补平台探针、状态管理与命令面包装。

前端是两套**独立复刻树**（`tauri-app/frontend` 与 `android/frontend`），靠同形的 `tauriApi.ts` 接口与同名的 IPC 命令名保持行为一致——桌面 65 条命令（`app/startup.rs:63-127`）、安卓 53 条（`android/src-tauri/src/lib.rs:57-111`），命令名逐字对齐（仅三处刻意例外，见 Known Issues）。

这一架构的代价集中在"加法要改几处"：通用改进必须在同一次提交内改两端（项目 `AGENTS.md` 第 3 条），而两端的配置字段、面板集合、命令集又是"交集 + 双向差集"关系，无法照抄（见 Known Issues）。

## 系统总览图

```text
┌───────────────────────────── 桌面端 tauri-app/frontend/src ─────────────────────────────┐
│ UI 层   App.tsx(外壳/9 面板路由)  auth/ account/ monitor/ network/ settings/             │
│         components/{ui,layout}  shared/  i18n/                                          │
│ 状态层  hooks/useAuthStore · useConfigStore · useAdapterStore · useQualityStore ·        │
│         useLogToastStore · useThemeStore   +   useEventListeners/useInitialDataLoad      │
│ IPC 层  hooks/tauriApi.ts（67 invoke + 16 事件订阅，唯一出口）                            │
└─────────────────────────────┬───────────────────────────────────────────────────────────┘
                              │ invoke('snake_case')            ▲ listen('kebab-name')
                              ▼                                 │
┌──────────────── tauri-app/src-tauri（crate campus-login / lib campus_login_lib）─────────┐
│ 入口    main.rs(进程/helper 拦截/WebView2 参数) → app/startup.rs:23 run()                │
│ 命令层  commands/{config_cmd,login,network_cmd,system,account,background,self_service,    │
│         updater}.rs 63 条 + infra/logger.rs 2 条 = 65（startup.rs:63-127）               │
│ 业务层  auth/(service,session,protocol,portal,failure_tracker) · monitor/(background_check│
│         auto_auth,latency,quality_scheduler,adapter_watch,scheduled) · self_service/ ·    │
│         update/ · helper/（--helper 提权子进程）· app/（tray,window,shortcut,heartbeat,   │
│         shutdown,webview_recovery）                                                       │
│ 基础层  infra/{state,logger,events,task_manager,lifecycle,notification,command_context,   │
│         async_util}.rs · config/{model,persist,validate,schedule}.rs · network/（adapter,  │
│         client,dhcp,dns,quality,subnet,timing,discovery）· platform/                      │
│ 系统层  platform/{autostart,dns_config,elevation,gpu,helper_spawn,identity,metric,toast}.rs │
│         （Win32 / WinRT / UAC / 注册表 / 接口跃点 metric）+ network/dhcp.rs（netsh / MAC 重置） │
└─────────────────────────────┬───────────────────────────────────────────────────────────┘
                              │ Cargo path 依赖（android/src-tauri/Cargo.toml:34）
                              ▼
┌──────────────────── android/src-tauri（crate campus-login-android）─────────────────────┐
│ 入口    lib.rs:30 run() → lib.rs:44-55 setup → monitor_loop::run_startup_tasks           │
│ 命令层  53 条（lib.rs:57-111）：protocol_cmds · campus_detect · config_state ·            │
│         self_service_cmds · account_cmds · system_cmds · quality_cmds · update_cmds ·     │
│         battery_cmds · monitor_loop                                                       │
│ 状态层  android_state.rs（cached_source_ip + config 内存态）· monitor_loop::MONITOR 静态   │
│         identity_gate.rs（600s 验证门）· login_history.rs · quality_history.rs ·          │
│         cpu_affinity.rs                                                                  │
│ 插件层  plugins/keystore（AES-GCM） · network-bind（bindProcessToNetwork + WiFi 事件） ·   │
│         foreground-service（前台服务/探针窗口锁/电池白名单/开机自启/APK 安装），Kotlin 侧  │
└─────────────────────────────┬───────────────────────────────────────────────────────────┘
                              │ IPC（同形 tauriApi，桌面专属项 desktopOnly reject）
                              ▼
┌───────────────────────────── 安卓端 android/frontend/src ────────────────────────────────┐
│ App.tsx 双外壳：短边 <600px → AppInner（手机）+ BottomNav/MobileMore；                    │
│                 ≥600px → components/tablet/TabletShell（DockNav/RightPanel/TitleBar）      │
│ 状态层  六个同名领域 store（useAuthStore 等）+ useDeviceProfile / useAdaptiveFramePace      │
│ IPC 层  hooks/tauriApi.ts（同形接口 + desktopOnly/noopListener 抹平差异）                  │
│ 附加    face/（应用内 2D 人脸验证）· shared/ · components/{ui,layout,mobile,tablet}         │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

## 分层架构

### UI 层

| 端 | 组成 | 代表文件 |
|---|---|---|
| 桌面 | 应用外壳（面板路由 + 转场）+ 5 个业务域目录 + 共享展示层 | `tauri-app/frontend/src/App.tsx`（9 面板 switch）、`auth/`、`account/`、`monitor/`、`network/`、`settings/`、`shared/`、`components/{ui,layout}/`、`i18n/` |
| 安卓 | 双外壳：手机「轻 header + 单列滚动 + BottomNav」与平板「TitleBar + DockNav + RightPanel」 | `android/frontend/src/App.tsx`（`useFormFactor` 二选一）、`components/mobile/`、`components/tablet/`、`face/` |

两端 UI 层都通过 `React.lazy` 懒加载低频弹窗（桌面 `App.tsx:49-55` 的 AboutDialog/ThemeDialog/OnboardingWizard，`LogPanel` 曾 lazy 后回归静态导入，注释 `App.tsx:45-48`）。

### 状态管理层

| 端 | 实现 | 位置 |
|---|---|---|
| 桌面前端 | 6 个 zustand 领域 store + 模块级锁变量 | `tauri-app/frontend/src/hooks/{useAuthStore,useConfigStore,useAdapterStore,useQualityStore,useLogToastStore,useThemeStore}.ts`；兼容壳 `useAppStore.ts:1-3` 仅 re-export |
| 安卓前端 | 同名 6 个 store（独立复刻）+ 氛围动画帧率节流 | `android/frontend/src/hooks/` 同名文件；另有 `useDeviceProfile.ts`、`useAdaptiveFramePace.ts`（安卓独有） |
| 桌面后端 | `AppState` = `ConfigStore`(ArcSwap) + `TaskFlags`(5 个原子锁) + `BackgroundTaskManager` + `NetworkState` + `ExitStateStore` + `UpdateStats`(9 个原子字段) + `ScheduledFired`(定时登录/注销当日触发标记) | `tauri-app/src-tauri/src/infra/state/mod.rs:164-172`；托管点 `app/startup.rs:49` |
| 安卓后端 | 两个进程级静态容器：`AndroidState`（源 IP + 配置内存态）与 `MONITOR`（`MonitorState` 14 个原子/互斥字段，含定时登录/注销当日触发标记 2 个） | `android/src-tauri/src/android_state.rs:8-13`、`android/src-tauri/src/monitor_loop.rs:12`（静态）与 `:28-56`（结构体） |

安卓后端**不复用** `AppState`/`EventBus`/`BackgroundTaskManager`：`AppState` 在安卓代码中零引用，安卓只用共享 crate 的 `infra::logger`（`android/src-tauri/src/lib.rs:51`）。

### IPC 通信层

- **命令通道**：`invoke('<snake_case 命令名>', { camelCase 参数 })`；注册表分别唯一存在于 `tauri-app/src-tauri/src/app/startup.rs:62-128`（65 条，条目 `:63-127`）与 `android/src-tauri/src/lib.rs:57-111`（53 条）。
- **前端出口**：桌面 `tauri-app/frontend/src/hooks/tauriApi.ts`（`interface TauriApi :27-116` 共 84 成员 = 67 invoke + `openExternal` + 16 事件订阅，实现 `:149-248`）；安卓 `android/frontend/src/hooks/tauriApi.ts`（接口 `:56-148`，实现 `:197-326`：53 条命令中 50 条有真实 invoke 出口，其余 20 个桌面专属成员走 `desktopOnly` reject；16 个事件订阅中 7 个真实监听、8 个 `noopListener`）。
- **事件通道**：桌面唯一实现 `tauri-app/src-tauri/src/infra/events.rs`（`EventBus` :8-10，`emit()` :17-19，16 个 `emit_*` 方法 `:22-117` —— 全仓唯一的 `.emit(` 调用点），消费侧 `useEventListeners.ts`；安卓后端没有 `events.rs`，6 处裸 `app.emit`（见 Known Issues），但 `config-changed` 现已由 `emit_config_changed`（`config_state.rs:388-391`）广播。
- **返回契约**：通用三态 `CommandResult { success, message?, data? }`（桌面 `infra/state/mod.rs:271-290`；安卓自带同形定义 `android/src-tauri/src/self_service_cmds.rs:7-14`）。

### 业务逻辑层

| 能力 | 桌面实现 | 安卓实现 |
|---|---|---|
| 登录/注销/Portal 协议 | `auth/{protocol,portal,failure_tracker}.rs`（跨平台，安卓经 path 依赖复用；`protocol.rs` 684 行，注销侧含 Radius 两轮注销 + MAC 解绑） | 只包装：`protocol_cmds.rs:76` 调 `auth::protocol::do_login_with_retry` |
| 登录编排 | `auth/{service,session,dual_adapter_executor}.rs`（`#[cfg(desktop)]`；R1 起支持每张网卡绑定独立账号：`adapter1_account`/`adapter2_account`） | 无（`monitor_loop.rs` 自行编排） |
| 后台巡检与自动登录 | `monitor/`（`watcher` 门面 + `background_check` 主体 + `auto_auth` + `latency` + `quality_scheduler` + `adapter_watch` + `scheduled` 每日定时登录/注销），`lib.rs:17-18` 门控 | `monitor_loop.rs` 单文件（WiFi 事件驱动 + 前台服务保活 + **按电源状态分档**：纯函数 `effective_interval_ms`（`monitor_loop.rs:83-90`）亮屏且 WiFi 走基础间隔（下限 5000），蜂窝/灭屏走闲时档；离网拍 Portal 三态短路把失败周期 11s 压到 3s（`:1326-1330`）；另含每日定时/夜切动作 `run_scheduled_actions`（`:748-950`），判定复用共享 crate 的 `config::schedule::should_fire_scheduled_action`） |
| 自助服务协议 | `self_service/mod.rs`（跨平台） | 包装：`self_service_cmds.rs` 六命令 |
| 配置模型 | `config/{model,persist,validate,schedule}.rs`（跨平台，但 `persist` 依赖 DPAPI，安卓不用；`schedule` 是定时动作纯函数）；`Config` 60 字段（`config/model.rs:10-172`），`config_version` 5（`model.rs:305-306`；v2→v3 把 `campus_check_end_minutes` 旧默认 0 刷为 1380，`model.rs:202-206`） | `config_state.rs`（`Settings` 47 字段 + Keystore 桥 + 迁移，`config_schema_version` 7（`config_state.rs:180`）：v0→v1 旧默认 15s→60s（`:316-318`）、v4 新增 `background_check_idle_interval` 300000（`:332-338`）、v5 把 `campus_check_end_minutes` 0→1380（`:339-345`）、v6 把 `latency_test_interval` 60s→600s（`:346-352`）、v7 夜切默认改 true 并把定时 0 刷为禁用哨兵 1440（`:353-365`），每步迁移即落盘） |
| 更新 | `update/updater.rs`（exe/msi + 5 源 SHA256；自动检查失败按 `BACKOFF_RETRY_SECS = &[5*60, 15*60, 3600]` 退避重试，`lastCheckError`/`lastCheckTime` 随返回体回传前端） | `update_cmds.rs`（APK + GitHub API 资产；24h 周期 + 失败退避 `[5min,15min,1h]`（`:22/:25`）；4 个版本源逐个尝试（`:260-303`）；下载白名单 8 域（`:42-51`）、500MB 上限（`:53`）、SHA256 校验（`:453-459`）、进度 200ms 节流（`:436-447`）、安装前 canonicalize 白目录校验（`:497-519`）） |
| 提权 | `helper/mod.rs`（`--helper dns|clear_dns|mac|enable_adapter|enable_device|set_metric|register_task|selfcheck`）+ `platform/elevation.rs`（COM ICMLuaUtil / ShellExecuteW） | 无（改为 `foreground-service` 插件 + 系统授权） |

### 系统交互层

| 端 | 能力 | 位置 |
|---|---|---|
| 桌面 | 注册表自启、DNS/DoH 写入（`SetInterfaceDnsSettings`）、接口跃点读写（`GetIpInterfaceTable` / `SetIpInterfaceEntry`，夜间出站切换用）、UAC 提权、DXGI/GDI 硬件探测、Windows Hello、WinRT Toast、`GetAdaptersAddresses` 网卡枚举、`netsh`/`ipconfig`/`surge_ping` | `platform/{autostart,dns_config,elevation,gpu,helper_spawn,identity,metric,toast}.rs`、`network/{discovery,dhcp,subnet}.rs`；模块门控见 `platform/mod.rs:1-36`（带 `target_os = "windows"` 的是 `task_proxy` / `identity` / `metric` / `best_route` / `icmp_probe` / `rtss_compat` / `toast`） |
| 安卓 | AndroidKeyStore、`bindProcessToNetwork` + WiFi NetworkCallback、前台服务 + **探针窗口内按需持有的** WifiLock/WakeLock、开机自启（BootReceiver）、APK 安装（FileProvider）、电池优化白名单（查询/申请/厂商自启页跳转/系统通知设置）、绑小核（`sched_setaffinity`） | `android/plugins/{keystore,network-bind,foreground-service}/`（三个 crate 首行均 `#![cfg(mobile)]`，Rust 薄壳 + Kotlin 实现）、`android/src-tauri/src/battery_cmds.rs`、`cpu_affinity.rs` |

### 功耗与按需驱动约束

2026-09-13 安卓省电改造（真机实测：前台静置 10s 由 2438 帧、RenderThread 40~44% 降到 0 帧、全线程 <1%）确立了一条跨层硬约束：**不留常驻驱动源**——渲染侧不得保留 pending `requestAnimationFrame`，系统锁不得在整个进程生命周期持有。改造后的分工见下表（详见 [[rAF-blocks-compositor-idle]] 与 [[android-power-three-fixes]]）。

| 约束 | 桌面 | 安卓 |
|---|---|---|
| 渲染存活判定不引入常驻 rAF | `tauri-app/frontend/src/lib/renderLiveness.ts:36-40`：距上次探测超过 `PROBE_REFRESH_MS=4_000`（`:18`）才经 `startProbe()`（`:24-34`）开一个 2 帧（≈33ms）窗口；10s 停滞判定阈值不变 | 同款实现 `android/frontend/src/lib/renderLiveness.ts:36-40` |
| 帧率按需调整 | 无（桌面无帧控 hook） | `useAdaptiveFramePace.ts` 以 `setInterval(applyPace, PACE_POLL_MS=250)`（`:64`，常量 `:18`）替代 rAF 递归轮询，交互起始由 `markInteraction()`（`:26-29`）即时提帧 |
| 系统锁按需持有 | 无 | 巡检每拍进入探针窗口：`begin_probe_window()` + `ProbeWindowGuard`（`monitor_loop.rs:1293-1298`），Drop（含 early return / panic）必调 `end_probe_window()`（guard 定义 `:554-562`）；Kotlin 侧 `acquireProbeLocks()`（`ForegroundService.kt:172`）持 `WIFI_MODE_FULL_HIGH_PERF` + 30s 超时 `PARTIAL_WAKE_LOCK` |
| 事件唤醒去重 | 无 | `ForegroundService.kt:250-252` 仅在关注字段（`TRANSPORT_WIFI`/`TRANSPORT_CELLULAR`/`NET_CAPABILITY_VALIDATED` 位掩码，掩码位定义 `:225`）翻转时 nudge，5s 节流（`NUDGE_THROTTLE_MS` `:39`）降为第二道闸 |
| 巡检频率分档 | 无 | `effective_interval_ms(base, idle, screen_on, wifi_connected)`（`monitor_loop.rs:83-90`）：亮屏周期恒为基础间隔（保证亮屏/回 WiFi 最迟一拍恢复，下限 `max(5000)`），蜂窝或灭屏时跳拍、实际间隔取闲时档（`background_check_idle_interval` 默认 300000，`config_state.rs:140`），电源状态经插件 `get_power_state()` 查询、失败按 `(true, true)` 保守处理（`monitor_loop.rs:568-578`） |
| 电池优化/系统通知命令面 | 无（平台专属，四命令均返回"仅安卓端可用"） | `android/src-tauri/src/battery_cmds.rs` 四命令（`get_battery_optimization_info` :20 / `request_ignore_battery_optimizations` :46 / `open_vendor_battery_settings` :65 / `open_notification_settings` :87——覆盖 API 13 以下、永久拒绝与系统设置关闭三类场景），经 foreground-service 插件转发 Kotlin（厂商页候选表 + 三级降级链）；`AndroidManifest.xml` 增 `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` |

## 依赖图

```text
                        ┌───────────────────────────────┐
                        │  tauri-app/frontend（独立树）   │
                        └──────────────┬────────────────┘
                                       │ invoke / listen（IPC 字符串契约，无编译期约束）
┌──────────────────────────────────────▼─────────────────────────────────────────────┐
│ tauri-app/src-tauri  package "campus-login" / lib "campus_login_lib"                │
│                                                                                     │
│  main.rs:3-14（bin 自带 12 个 mod，不复用 lib）        lib.rs:2-8（跨平台可见面）      │
│    └─ app / commands / monitor / helper / update ──→  account.auth.config.infra.     │
│        （桌面专属，lib.rs:11-20 #[cfg(desktop)]）       network.platform.self_service │
│                          │                                        ▲                 │
│                          └───────────────┬────────────────────────┘                 │
│                                          │ commands/ app/ monitor/ 调用共享模块       │
└──────────────────────────────────────────┼──────────────────────────────────────────┘
                                           │ campus-login = { path = "../../tauri-app/src-tauri" }
                                           │ （android/src-tauri/Cargo.toml:34）
┌──────────────────────────────────────────▼──────────────────────────────────────────┐
│ android/src-tauri  package "campus-login-android" / lib "campus_login_android_lib"   │
│   protocol_cmds ──→ auth::protocol / auth::portal                                    │
│   campus_detect ──→ network::subnet::is_same_subnet_18                               │
│   quality_cmds  ──→ network::quality::check_network_quality_async                     │
│   self_service_cmds ──→ self_service::*                                              │
│   system_cmds / lib.rs:51 ──→ infra::logger::*                                        │
│   account_cmds / config_state / monitor_loop ──→ 安卓自有实现（同构，不共享）           │
│   plugins/{keystore,network-bind,foreground-service}（Kotlin 侧实现，Rust 侧薄壳）      │
└──────────────────────────────────────────────────────────────────────────────────────┘
```