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
  - tauri-app/src-tauri/src/auth/service.rs
  - tauri-app/src-tauri/src/auth/session.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/network/client.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/store.rs
  - tauri-app/src-tauri/src/infra/task_manager.rs
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/build.rs
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/shared/ui-constants.ts
  - android/src-tauri/Cargo.toml
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - android/plugins/foreground-service/src/lib.rs
tags: [架构, 总览]
---

## Overview

Wxxy-CampusLogin 是无锡学院校园网（Dr.COM / ePortal）的自动登录助手，由 **Windows 桌面端**与**安卓端**两个同构应用组成，均使用 Tauri 2（Rust 后端 + React 19 / TypeScript 前端）。当前版本 `2.3.9`（`tauri-app/src-tauri/Cargo.toml:2-3`、`android/src-tauri/Cargo.toml:2-3`）。

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
└──────────────────────────────────────────┬──────────────────────────────────────────┘
                                           │ 同形 tauriApi 接口
                        ┌──────────────────▼──────────────────┐
                        │  android/frontend（独立复刻树）       │
                        └─────────────────────────────────────┘
```

共享 crate 内部的依赖方向（由浅至深）：`commands/*`、`app/*`、`monitor/*` → `auth`、`network`、`self_service`、`config`、`update`、`platform` → `infra`（`state`/`events`/`logger`/`task_manager`/`lifecycle`）→ `config::model`。`lib.rs:1` 的注释把这层约束写死为"跨平台协议核心：安卓端（path 依赖）唯一可见面，保持无桌面依赖"。

## 请求与数据流

### 一次登录：UI → Win32/HTTP → 回到 UI（桌面端）

```text
[1] 组件触发
    components/layout/DockNav.tsx:415 取 useAuthStore 的 doLogin，绑定到按钮 onAction
      → hooks/useAuthStore.ts:145 doLogin
          先 saveConfigDirect(loginConfig)（:157）落盘，再 api.doLogin（:165，60s 超时）

[2] 出 IPC
    hooks/tauriApi.ts:161  doLogin: (adapterName) => invoke<LoginResult>('do_login', { adapterName })

[3] 命令入口（并发门）
    app/startup.rs:68 注册的 crate::commands::login::do_login
      → commands/login.rs:55 do_login
          :57 清残留自动退出倒计时 → :59 set_deadline(None)
          → :66 tasks.is_logging_in.try_acquire()
          → :73 auth::service::full_login(&state, &app_handle, adapter)

[4] 业务编排
    auth/service.rs:81 full_login
      :87 读配置并校验 user/password → :94-100 get_adapters_cached / wait_for_adapter(10000)
      → :106 ensure_ethernet_ip_for_login → :110-113 get_adapters_force
        手动指定网卡：按 adapter1/2_account 解析账号凭据（:122-124）
        双适配器：auth/dual_adapter_executor.rs execute_dual（:169 调用，适配器 2 延迟 1s）
        单适配器：auth/session.rs:77 login_adapter_with_log（:184-191）

[5] 协议执行
    auth/session.rs:77 login_adapter_with_log
      :87-99 预检 auth/portal.rs:98 check_portal_full（只读 GET，无凭据；已在线直通+补历史）
      → :108-112 auth/protocol.rs:192 do_login_with_retry(..., 3, ...)
           → auth/protocol.rs:124 do_login_request
                :125-127 config/validate.rs 三处凭据校验
                :129 network/client.rs:8 PORTAL_URL.load() + portal.rs:42 ensure_portal_port
                :147 network/client.rs:120 create_safe_http_client(15s, local_addr)
                （安卓侧 :153-157 走 bound_socket 旁路 → client.rs:95 create_bypass_http_client）
                → HTTP GET /eportal/portal/login（reqwest，无代理）
                :163-168 脱敏后打日志（portal.rs:69 redact_credentials）
                → 限长读体（1MB）→ protocol.rs:227 parse_login_result
                     产出 { code, message, success, retryable }
[6] 结果回流与副作用
    auth/session.rs:114-173 复核（parse_error→查 Portal 改判；"已经在线"但探测不通→降级失败）
      → auth/session.rs:11-75 adapter_action_with_log
           infra/events.rs:22 EventBus::emit_login_log  → 前端 login-log
           config/persist.rs:160 append_login_history      → 磁盘 login-history.json（上限 100）
      → auth/failure_tracker.rs:47 update_auth_failure_count
          成功清零；认证失败累加，达 MAX_FAILURES=5（failure_tracker.rs:6）
          → network/dhcp.rs:460 dhcp_release_renew_single（netsh + 注册表 MAC 重置，可能经提权 helper）
      → auth/service.rs:297 post_login_handler
          解除注销保护期（:301）→ 500ms 后按需触发后台检查（:310-321）
          → 按需 infra/lifecycle.rs:183 start_auto_exit（20s 倒计时，:325）

[7] 回到 UI
    infra/events.rs:17-19 app_handle.emit('login-log', payload)
      → hooks/tauriApi.ts onLoginLog（createEventListener<...>('login-log')）
        → hooks/useEventListeners.ts:269-275 useLogToastStore.addLog(message, type)
          → shared/LogPanel / RightPanel / ToastContainer 渲染
    do_login 的 Promise 同时 resolve CommandResult → useAuthStore 写 status/isLoggingIn
      → 详情面板与状态条经 zustand 订阅重渲染
```

安卓端同一动作的差异点：`invoke('do_login')` → `protocol_cmds.rs:10` 先 `ensure_wifi_bound`（`protocol_cmds.rs:264`，绑定成功且路径非 `already_bound` 时清 HTTP 连接池 `protocol_cmds.rs:283-289`，调共享 crate `network::client::clear_client_pool`）→ `run_login`（`protocol_cmds.rs:63`）→ 同一个 `auth::protocol::do_login_with_retry`（`protocol_cmds.rs:76`）→ `login_history::append`（`login_history.rs:51`）→ `app.emit("login-log")`（`monitor_loop.rs:152-156`）。

### 一次后台巡检拍（桌面端）

```text
monitor/background_task.rs:7 start_background_check_inner 循环（每轮重读 config.background_check_interval，save 前已钳位 ≥10000，validate.rs:103）
  → monitor/background_check.rs:370 run_background_check（spawn_blocking）
      → :15 run_background_check_blocking
          :19 is_checking.try_acquire（单飞）
          → monitor/campus_check.rs:43 check_campus_network（SSID/有线 profile/子网/网关）
          → Portal 探测（双适配器并行）→ auth/portal.rs:98 check_portal_full
          → 请求失败计数（阈值 5 → MAC 重置）
          → monitor/background_emit.rs:109 emit_background_check_result
               → 注销保护期强制 online=false
               → infra/events.rs:30 EventBus::emit_background_check_result
          → monitor/auto_auth.rs:29 try_auto_login_on_preparation（冷却 → 熔断 → 登录锁 → auth::service full_login）
          → monitor/auto_auth.rs:110 try_disconnect_reconnect（计数 CAS ≤ max_disconnect_reconnect）
          → monitor/background_emit.rs:166 update_network_state（必要时 infra/lifecycle.rs:183 start_auto_exit）
```

## 模块索引

### modules/（19 篇）

| 文章 | 一句话职责 |
|---|---|
| [[desktop-auth]] | 桌面 Dr.COM 协议唯一实现点：登录/注销/Radius 注销/MAC 解绑/Portal 探测 + 双适配器编排 + 失败计数与 MAC 重置 |
| [[desktop-config]] | `Config` 60 字段模型、原子写落盘、密码 DPAPI 加密、严格/宽松双校验与迁移 |
| [[desktop-account-selfservice]] | `account::crypto`（DPAPI FFI）与 Dr.COM Self（`/Self`）协议全流程 |
| [[desktop-network-core]] | 适配器发现/5s 缓存/选择规则、DHCP、MAC 重置、子网与网关探测、HTTP 客户端池 |
| [[desktop-network-dns]] | 智能域名解析（评分排序 + DoH 竞速 + 系统兜底）、分段计时、DNS/DoH 一键设置 |
| [[desktop-network-quality]] | 19 项延迟测量、四波调度、中位数/截尾平均聚合与六档等级 |
| [[desktop-monitor]] | 后台巡检中枢：校园网判定、Portal 检测、自动登录/断线重连、质量调度、适配器监听 |
| [[desktop-helper-update]] | `--helper` 提权子进程（改 MAC / 设 DNS）与更新自检/下载/SHA256 校验 |
| [[desktop-infra]] | 运行时基座：`AppState`(ArcSwap)、异步日志、`EventBus`、`BackgroundTaskManager`、退出生命周期、通知 |
| [[desktop-app-lifecycle]] | 进程入口、插件装配、命令注册表、托盘/窗口/快捷键/心跳/优雅退出/WebView2 恢复 |
| [[desktop-commands]] | 桌面 65 条命令的逐条参数、并发门与 JSON 返回形状 |
| [[desktop-platform]] | Win32/WinRT 平台层：注册表自启、DNS/DoH API、UAC 提权、DXGI 探测、Windows Hello、Toast |
| [[desktop-frontend-hooks]] | 前端 IPC 网关 `tauriApi.ts`、6 个领域 store、事件监听与启动编排 |
| [[desktop-frontend-shared]] | shared 业务组件、Radix UI 原语、布局骨架、`lib/` 工具与 i18n |
| [[desktop-frontend-panels]] | `App.tsx` 外壳与 5 个业务域面板（总览/账号/监控/网络/设置） |
| [[android-backend]] | 安卓后端 53 条命令、15 个模块、平台探针与监控状态机 |
| [[android-plugins]] | 三个手写 Tauri 插件的 Rust 壳 + Kotlin 实现 + 权限/构建文件 |
| [[android-frontend-core]] | 安卓前端入口、双外壳、6 个 store、设备分档与 2D 人脸 |
| [[android-frontend-panels]] | 安卓面板层（总览/账号/自助/监控/设置/日志）与向导状态机 |

### concepts/（5 篇）

| 文章 | 一句话职责 |
|---|---|
| [[ipc-command-surface]] | 两端命令注册点、命名对齐规则、16 个事件的双端矩阵 |
| [[dual-platform-sharing]] | path 依赖、`cfg` 门控边界、两端字段/命令差集与同步硬约定 |
| [[security-model]] | 凭据出站唯一出口、落盘加密、验证门分级与日志脱敏 |
| [[background-check-and-auto-login]] | 巡检→探测→状态机→自动登录/重连→倒计时的完整链路与阈值表 |
| [[config-and-persistence]] | 配置字段、磁盘布局、原子写、迁移与 `config-changed` 回流 |

## Known Issues

### 双端同构的固有风险

1. **"通用改进必须双端同步"没有自动化守门**：`AGENTS.md`（项目根）第 3 条要求前端 UI/文案/i18n/命令面/配置字段/事件类型的改动在同一次提交内双端各改一份，但仓库内无 CI 检查，脚本/git hook 都不拦截只改一端的提交；`git diff --stat` 自检是唯一手段。
2. **前端是两套独立树，功能分叉持续累积**：桌面 `tauri-app/frontend/src/hooks/` 与安卓 `android/frontend/src/hooks/` 现各 24 个文件，差集为「安卓有 `useDeviceProfile.ts`/`useAdaptiveFramePace.ts`/`useFormFactor.ts`/`usePullToRefresh.ts`；桌面有 `useGpuCorrection.ts` + 3 个测试文件」。安卓前端全目录**不存在任何测试**（无 `test-setup.ts`、`package.json` 无 `test` 脚本），复刻分叉只能靠 `tsc --noEmit` 与真机验证发现。
3. **配置字段集是"交集 + 双向差集"**：桌面 `Config` 60 字段（`config/model.rs:10-172`）、安卓 `Settings` 47 字段（`android/src-tauri/src/config_state.rs:15-111`），同名交集 43（版本号字段名双端不同：`config_version` ↔ `config_schema_version`，不可互推）；安卓独有 `allow_2d_face_verify`（`config_state.rs:24`）、`background_check_idle_interval`（`:64`）、`enable_boot_autostart`（`:78`）；桌面独有 `adapter1_account`/`adapter2_account`（`model.rs:30-34`）、`auto_exit_after_login`（`:40`）、`outbound_manual_hold_day`（`:94`）、`auto_exit_on_online`（`:105`）等。而两套前端类型又各自是第三份定义（桌面 `settings/types.ts:3-56` 45 字段、安卓 `settings/types.ts:3-58` 45 字段，两者均指各自 `Config` 接口本体的字段总数）。新增字段无法照抄任何一端。
4. **默认值分叉已实际发生**：`background_check_interval` 桌面默认 60000（`config/model.rs:257-258`）与安卓 60000（`android/src-tauri/src/config_state.rs:138`）已对齐（双端迁移都仍会把存量 15000 强刷为 60000：`config_state.rs:316-318`、`validate.rs:152-153`）；`default_panel` 桌面空串（`model.rs:282`）与安卓 `"dashboard"`（`config_state.rs:147`）不同。巡检下限口径也不同：桌面 save 前钳位 10000（`validate.rs:103`），安卓运行时下限 `max(5000)`（`monitor_loop.rs:84`）。**配置 schema 版本号也各走各的**：安卓 `config_schema_version` 现为 7（`config_state.rs:180`，安卓前端 `android/frontend/src/settings/constants.ts:58` 的 `configSchemaVersion` 对齐同一数字；v0→v1 刷巡检间隔、v4 补 `background_check_idle_interval`、v5 刷 `campus_check_end_minutes` 0→1380、v6 刷 `latency_test_interval` 60s→600s、v7 刷夜切默认并把定时 0 刷为 1440 禁用哨兵，迁移链 `config_state.rs:314-366`）；桌面 `config_version` 演进至 5（`model.rs:305-306`，v2→v3 把 `campus_check_end_minutes` 旧默认 0 刷为 1380，`model.rs:202-206`）。两端字段名近义但语义独立，跨端对照时不可互推。
5. **版本号有六个来源，无自动校验**：桌面 `env!("APP_VERSION")` 由 `tauri-app/src-tauri/build.rs:37` 从 `tauri.conf.json` 注入；安卓用 `env!("CARGO_PKG_VERSION")`（`android/src-tauri/src/system_cmds.rs:15`，安卓 `build.rs` 只有一行 `tauri_build::build()`）；两端 `Cargo.toml` 各有一个 `version`；两套前端手写常量 `tauri-app/frontend/src/shared/ui-constants.ts:2` 与 `android/frontend/src/shared/ui-constants.ts:2` 都硬编码 `'2.3.9'`。发布时需人工多处同步；发布侧主脚本是项目根 `make-release.ps1`（包装 `tauri-app/build.ps1`，从 `tauri-app/src-tauri/tauri.conf.json` 读版本，把安装包改名为 `Wxxy-CampusLogin_<版本>_x64-setup.exe` 并生成同名 `.sha256`，收集 APK 与 `RELEASE_NOTES_v<版本>.md`，最后打印 `gh release create/upload` 命令），它本身**不做版本号跨来源一致性校验**。

### 已发现的代码不一致

6. **二进制与 lib 各编译一份模块树**：`tauri-app/src-tauri/src/main.rs:3-14` 用 `mod` 自行声明 12 个模块，`lib.rs:2-20` 另声明一份，`main.rs` 不通过 `campus_login_lib::` 复用。后果是同一份源码编译两遍，且 `#[macro_export]` 宏在 bin 与 lib 各有一份实例（`crate::log_info!` 与 `campus_login_lib::log_info!` 不是同一个东西）。
7. **安卓后端 6 处裸 `app.emit` 绕过事件总线模式**：`android/src-tauri/src/config_state.rs:390`、`android/src-tauri/src/monitor_loop.rs:155`、`:498`、`:1457`、`android/src-tauri/src/update_cmds.rs:323`、`:444`；而安卓的 `network-quality-result` 又反而经共享 crate 走 `EventBus`（`quality_cmds.rs:98` 注释，`network::quality` 内部 emit）。安卓没有集中登记点，事件名散落在各处。
8. **安卓 `save_config` 未持 `CONFIG_IO_LOCK`**：锁定义在 `android/src-tauri/src/account_cmds.rs:17`（`config_io_lock()` :21-23 供跨命令共用），账号写命令（`account_cmds.rs:177/221/287/327`）与 `set_boot_autostart`/`set_notification_enabled`（`monitor_loop.rs:369/393`）都取锁，而读改写全量配置的 `save_config`（`config_state.rs:421-453`）与巡检侧落盘 `persist_settings`（`monitor_loop.rs:955-978`）没取——正是该锁要防的竞态对象。
9. **安卓 `AndroidState` 文档与实现不符**：`android/src-tauri/src/lib.rs:9` 的模块说明写「android_state:进程态(源 IP 缓存/配置内存态/监控循环句柄)」，实际结构体只有 `cached_source_ip` 与 `config` 两个字段（`android_state.rs:8-13`），监控状态在 `monitor_loop.rs:12` 的 `MONITOR` 静态里。
10. **安卓 3 条命令无前端出口**：`ping_test`（`protocol_cmds.rs:172`）、`detect_campus`（`campus_detect.rs:217`）、`accept_wifi_network`（`protocol_cmds.rs:196`）都注册进了 `generate_handler!`，但 `android/frontend` 全仓无调用点（安卓 `tauriApi.ts` 实现对象只有 50 条 invoke 出口）。
11. **foreground-service 插件的三张命令/权限清单互不一致**：Rust 转发面 `plugins/foreground-service/src/lib.rs:33-97` 有 13 个方法；`foreground-service/build.rs` 的 `COMMANDS` 只列 6 项（startMonitor/stopMonitor/updateNotification/setBootAutostart/isBootAutostartEnabled/installApk）；`foreground-service/permissions/default.toml:7-20` 列 12 项（另含 beginProbeWindow/endProbeWindow/getPowerState/三个电池权限，但仍缺 `allow-open-notification-settings`）。network-bind 插件的 `build.rs` 与 `default.toml:7-15` 均为 7 项一致，但 Kotlin 侧 WiFi watcher 通道（`monitor_loop.rs:252-270` `start_wifi_watcher` 经 `tauri::ipc::Channel` 注册，失败退化纯周期探测）不在命令清单内。悬空引用不会在构建期报错，因为 `android/src-tauri/capabilities/default.json` 未引用这些插件的 default 权限集。
12. **两端命令名有三处刻意不一致**（由前端 `tauriApi` 转调抹平）：`verify_windows_identity` ↔ `verify_biometric_identity`、`get/set_auto_launch` ↔ `get/set_boot_autostart`（安卓 `tauriApi.ts:289-290` 转调）、`get_config` 返回 `Config` ↔ 返回掩码后的 `serde_json::Value`（`android/src-tauri/src/config_state.rs:406-418`）。任何按"命令名逐字相同"假设写的自动化核对都会在这三处误报。
13. **文档计数漂移（教训条目，数字已按 2026-10-03 实测更新）**：安卓命令面现为 **53 项**（注册块 `android/src-tauri/src/lib.rs:57-111`），桌面 **65 项**（`app/startup.rs:63-127`），安卓模块 **15 个**（`lib.rs:11-25`）。历史上 `modules/desktop-commands.md` 与 `modules/android-backend.md` 曾分别把安卓写成 45/48 项——**计数必须在主仓库工作区 `E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin` 上测**：滞后于 main 的分支/worktree 缺少新命令模块（如 `battery_cmds.rs`），会误测出旧数字；核实命令 `grep -cE '^\s*[a-z_0-9]+::[a-z_0-9]+,$' android/src-tauri/src/lib.rs` → 53。
14. **`default_panel`"Rust 侧零引用"的说法不准确**：`grep -rn default_panel tauri-app/src-tauri/src/` 命中 3 处，除字段定义与 Default（`config/model.rs:123/282`）外还有 `network/adapter.rs:269`（测试辅助 `make_test_config`）。结论（"仅前端实际消费"）成立，但证据描述需修正（`modules/desktop-config.md` 的 `default_panel` 条目已按此改写证据）。
