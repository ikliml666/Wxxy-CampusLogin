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

Wxxy-CampusLogin 是无锡学院校园网（Dr.COM / ePortal）的自动登录助手，由 **Windows 桌面端**与**安卓端**两个同构应用组成，均使用 Tauri 2（Rust 后端 + React 19 / TypeScript 前端）。

同构的实现方式不是代码共享，而是**协议单点 + 平台外壳**：登录/注销/Portal 探测/自助服务/网络质量等全部协议实现只存在于桌面协议核心 crate（`tauri-app/src-tauri`，package `campus-login`，lib `campus_login_lib`，见 `tauri-app/src-tauri/Cargo.toml:8-10`），安卓后端以 Cargo path 依赖引用它（`android/src-tauri/Cargo.toml:34`），禁止复制；桌面侧与平台耦合的模块用 `#[cfg(desktop)]` 整体门控（`tauri-app/src-tauri/src/lib.rs:11-20`），安卓侧只补平台探针、状态管理与命令面包装。

前端是两套**独立复刻树**（`tauri-app/frontend` 与 `android/frontend`），靠同形的 `tauriApi.ts` 接口与同名的 IPC 命令名保持行为一致——桌面 56 条命令（`app/startup.rs:63-120`）、安卓 48 条（`android/src-tauri/src/lib.rs:55-104`），命令名逐字对齐。

这一架构的代价集中在"加法要改几处"：通用改进必须在同一次提交内改两端（项目 `AGENTS.md` 第 3 条），而两端的配置字段、面板集合、命令集又是"交集 + 双向差集"关系，无法照抄（见 Known Issues）。

## 系统总览图

```text
┌───────────────────────────── 桌面端 tauri-app/frontend/src ─────────────────────────────┐
│ UI 层   App.tsx(外壳/9 面板路由)  auth/ account/ monitor/ network/ settings/             │
│         components/{ui,layout}  shared/  i18n/                                          │
│ 状态层  hooks/useAuthStore · useConfigStore · useAdapterStore · useQualityStore ·        │
│         useLogToastStore · useThemeStore   +   useEventListeners/useInitialDataLoad      │
│ IPC 层  hooks/tauriApi.ts（56 invoke + 16 事件订阅，唯一出口）                            │
└─────────────────────────────┬───────────────────────────────────────────────────────────┘
                              │ invoke('snake_case')            ▲ listen('kebab-name')
                              ▼                                 │
┌──────────────── tauri-app/src-tauri（crate campus-login / lib campus_login_lib）─────────┐
│ 入口    main.rs(进程/helper 拦截/WebView2 参数) → app/startup.rs:23 run()                │
│ 命令层  commands/{config_cmd,login,network_cmd,system,account,background,self_service,    │
│         updater}.rs 54 条 + infra/logger.rs 2 条 = 56（startup.rs:63-120）               │
│ 业务层  auth/(service,session,protocol,portal,failure_tracker) · monitor/(background_check│
│         auto_auth,latency,quality_scheduler,adapter_watch) · self_service/ · update/      │
│         helper/（--helper 提权子进程）· app/（tray,window,shortcut,heartbeat,shutdown,    │
│         webview_recovery）                                                               │
│ 基础层  infra/{state,logger,events,task_manager,lifecycle,notification,command_context,   │
│         async_util}.rs · config/{model,persist,validate}.rs · network/（adapter,client,    │
│         dhcp,dns,quality,subnet,timing,discovery）· platform/                            │
│ 系统层  platform/{autostart,dns_config,elevation,gpu,helper_spawn,identity,toast}.rs     │
│         （Win32 / WinRT / UAC / 注册表）+ network/dhcp.rs（netsh / MAC 重置）             │
└─────────────────────────────┬───────────────────────────────────────────────────────────┘
                              │ Cargo path 依赖（android/src-tauri/Cargo.toml:34）
                              ▼
┌──────────────────── android/src-tauri（crate campus-login-android）─────────────────────┐
│ 入口    lib.rs:28 run() → lib.rs:42-53 setup → monitor_loop::run_startup_tasks           │
│ 命令层  48 条（lib.rs:55-104）：protocol_cmds · campus_detect · config_state ·            │
│         self_service_cmds · account_cmds · system_cmds · quality_cmds · update_cmds ·     │
│         battery_cmds · monitor_loop                                                        │
│ 状态层  android_state.rs（cached_source_ip + config 内存态）· monitor_loop::MONITOR 静态   │
│         identity_gate.rs（600s 验证门）· login_history.rs · cpu_affinity.rs               │
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

两端 UI 层都通过 `React.lazy` 懒加载低频弹窗（桌面 `App.tsx:53-55` 的 AboutDialog/ThemeDialog/OnboardingWizard），`LogPanel` 则回归静态导入。

### 状态管理层

| 端 | 实现 | 位置 |
|---|---|---|
| 桌面前端 | 6 个 zustand 领域 store + 模块级锁变量 | `tauri-app/frontend/src/hooks/{useAuthStore,useConfigStore,useAdapterStore,useQualityStore,useLogToastStore,useThemeStore}.ts`；兼容壳 `useAppStore.ts:1-3` 仅 re-export |
| 安卓前端 | 同名 6 个 store（独立复刻）+ 设备性能分档 | `android/frontend/src/hooks/` 同名文件；另有 `useDeviceProfile.ts`、`useAdaptiveFramePace.ts`（安卓独有） |
| 桌面后端 | `AppState` = `ConfigStore`(ArcSwap) + `TaskFlags`(5 个原子锁) + `BackgroundTaskManager` + `NetworkState` + `ExitStateStore` + `UpdateStats` | `tauri-app/src-tauri/src/infra/state/mod.rs:125-132`；托管点 `app/startup.rs:50` |
| 安卓后端 | 两个进程级静态容器：`AndroidState`（源 IP + 配置内存态）与 `MONITOR`（12 个原子/互斥字段） | `android/src-tauri/src/android_state.rs:8-13`、`android/src-tauri/src/monitor_loop.rs:12-39` |

安卓后端**不复用** `AppState`/`EventBus`/`BackgroundTaskManager`：`AppState` 在安卓代码中零引用，安卓只用共享 crate 的 `infra::logger`（`android/src-tauri/src/lib.rs:49`）。

### IPC 通信层

- **命令通道**：`invoke('<snake_case 命令名>', { camelCase 参数 })`；注册表分别唯一存在于 `tauri-app/src-tauri/src/app/startup.rs:63-120`（56 条）与 `android/src-tauri/src/lib.rs:55-104`（48 条）。
- **前端出口**：桌面 `tauri-app/frontend/src/hooks/tauriApi.ts`（`interface TauriApi` 27-100，实现 133-223，含 56 invoke + 16 事件订阅）；安卓 `android/frontend/src/hooks/tauriApi.ts`（接口 56-133，实现 166-285，桌面专属项走 `desktopOnly` reject、`noopListener`）。
- **事件通道**：桌面唯一实现 `tauri-app/src-tauri/src/infra/events.rs`（`EventBus`，16 个 `emit_*`，私有 `emit()` 在 `events.rs:18` —— 全仓唯一的 `.emit(` 调用点），消费侧 `useEventListeners.ts`；安卓后端没有 `events.rs`，5 处裸 `app.emit`（见 Known Issues）。
- **返回契约**：通用三态 `CommandResult { success, message?, data? }`（桌面 `infra/state/mod.rs:212-231`；安卓自带同形定义 `android/src-tauri/src/self_service_cmds.rs:7-14`）。

### 业务逻辑层

| 能力 | 桌面实现 | 安卓实现 |
|---|---|---|
| 登录/注销/Portal 协议 | `auth/{protocol,portal,failure_tracker}.rs`（跨平台，安卓经 path 依赖复用） | 只包装：`protocol_cmds.rs:73` 调 `auth::protocol::do_login_with_retry` |
| 登录编排 | `auth/{service,session,dual_adapter_executor}.rs`（`#[cfg(desktop)]`） | 无（`monitor_loop.rs` 自行编排） |
| 后台巡检与自动登录 | `monitor/`（`watcher` 门面 + `background_check` 主体 + `auto_auth` + `latency` + `quality_scheduler` + `adapter_watch`），`lib.rs:17-18` 门控 | `monitor_loop.rs` 单文件（WiFi 事件驱动 + 前台服务保活 + **按电源状态分档**：WiFi 且亮屏走基础间隔，蜂窝/灭屏走闲时档 5min，纯函数 `effective_interval_ms`（`monitor_loop.rs:61-68`）） |
| 自助服务协议 | `self_service/mod.rs`（跨平台） | 包装：`self_service_cmds.rs` 六命令 |
| 配置模型 | `config/{model,persist,validate}.rs`（跨平台，但 `persist` 依赖 DPAPI，安卓不用） | `config_state.rs`（`Settings` 35 字段 + Keystore 桥 + 迁移，当前 `config_schema_version` 4：新增 `background_check_idle_interval` 默认 300000） |
| 更新 | `update/updater.rs`（exe/msi + 5 源 SHA256） | `update_cmds.rs`（APK + GitHub API 资产） |
| 提权 | `helper/mod.rs`（`--helper dns|mac`）+ `platform/elevation.rs`（COM ICMLuaUtil / ShellExecuteW） | 无（改为 `foreground-service` 插件 + 系统授权） |

### 系统交互层

| 端 | 能力 | 位置 |
|---|---|---|
| 桌面 | 注册表自启、DNS/DoH 写入（`SetInterfaceDnsSettings`）、UAC 提权、DXGI/GDI 硬件探测、Windows Hello、WinRT Toast、`GetAdaptersAddresses` 网卡枚举、`netsh`/`ipconfig`/`surge_ping` | `platform/{autostart,dns_config,elevation,gpu,helper_spawn,identity,toast}.rs`、`network/{discovery,dhcp,subnet}.rs`；模块门控见 `platform/mod.rs:1-18`（唯一带 `target_os = "windows"` 的是 `toast`） |
| 安卓 | AndroidKeyStore、`bindProcessToNetwork` + WiFi NetworkCallback、前台服务 + **探针窗口内按需持有的** WifiLock/WakeLock、开机自启（BootReceiver）、APK 安装（FileProvider）、电池优化白名单（查询/申请/厂商自启页跳转）、绑小核（`sched_setaffinity`） | `android/plugins/{keystore,network-bind,foreground-service}/`（三个 crate 首行均 `#![cfg(mobile)]`，Rust 薄壳 + Kotlin 实现）、`android/src-tauri/src/battery_cmds.rs`、`cpu_affinity.rs` |

### 功耗与按需驱动约束

2026-09-13 安卓省电改造（真机实测：前台静置 10s 由 2438 帧、RenderThread 40~44% 降到 0 帧、全线程 <1%）确立了一条跨层硬约束：**不留常驻驱动源**——渲染侧不得保留 pending `requestAnimationFrame`，系统锁不得在整个进程生命周期持有。改造后的分工见下表（详见 [[rAF-blocks-compositor-idle]] 与 [[android-power-three-fixes]]）。

| 约束 | 桌面 | 安卓 |
|---|---|---|
| 渲染存活判定不引入常驻 rAF | `tauri-app/frontend/src/lib/renderLiveness.ts:36-40`：距上次探测超过 `PROBE_REFRESH_MS=4_000`（`:18`）才经 `startProbe()`（`:24-34`）开一个 2 帧（≈33ms）窗口；10s 停滞判定阈值不变 | 同款实现 `android/frontend/src/lib/renderLiveness.ts:36-40` |
| 帧率按需调整 | 无（桌面无帧控 hook） | `useAdaptiveFramePace.ts` 以 `setInterval(applyPace, PACE_POLL_MS=250)`（`:64`，常量 `:18`）替代 rAF 递归轮询，交互起始由 `markInteraction()`（`:26-29`）即时提帧 |
| 系统锁按需持有 | 无 | 巡检每拍进入探针窗口：`begin_probe_window()` + `ProbeWindowGuard`（`monitor_loop.rs:630-635`），Drop（含 early return / panic）必调 `end_probe_window()`（guard 定义 `:497-508`）；Kotlin 侧 `acquireProbeLocks()` 持 `WIFI_MODE_FULL_HIGH_PERF` + 30s 超时 `PARTIAL_WAKE_LOCK` |
| 事件唤醒去重 | 无 | `ForegroundService.kt:198-206` 仅在关注字段（`TRANSPORT_WIFI`/`TRANSPORT_CELLULAR`/`NET_CAPABILITY_VALIDATED` 位掩码）翻转时 nudge，5s 节流降为第二道闸 |
| 巡检频率分档 | 无 | `effective_interval_ms(base, idle, screen_on, wifi_connected)`（`monitor_loop.rs:61-68`）：唤醒周期恒为基础间隔（保证亮屏/回 WiFi 最迟一拍恢复），蜂窝或灭屏时跳拍、实际间隔取闲时档（`background_check_idle_interval` 默认 300000），电源状态经插件 `get_power_state()` 查询、失败按 `(true, true)` 保守处理 |
| 电池优化白名单命令面 | 无（平台专属，三命令均返回"仅安卓端可用"） | `android/src-tauri/src/battery_cmds.rs` 三命令（`get_battery_optimization_info` / `request_ignore_battery_optimizations` / `open_vendor_battery_settings`），经 foreground-service 插件转发 Kotlin（厂商页候选表 + 三级降级链）；`AndroidManifest.xml` 增 `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` |

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
│   system_cmds / lib.rs:49 ──→ infra::logger::*                                        │
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
    components/layout/DockNav.tsx:413 取 useAuthStore 的 doLogin，绑定到按钮 onAction
      → hooks/useAuthStore.ts:133-192 doLogin
          先 saveConfigDirect(loginConfig)（:145）落盘，再

[2] 出 IPC
    hooks/tauriApi.ts:142  doLogin: (adapterName) => invoke<LoginResult>('do_login', { adapterName })

[3] 命令入口（并发门）
    app/startup.rs:67 注册的 crate::commands::login::do_login
      → commands/login.rs:57 do_login
          :58-60 清残留自动退出倒计时 → :67 tasks.is_logging_in.try_acquire()
          → :74 auth::service::full_login(&state, &app_handle, adapter)

[4] 业务编排
    auth/service.rs:20 full_login
      :21-28 读配置并校验 user/password → :32-38 get_adapters_cached / wait_for_adapter(10000)
      → :44 ensure_ethernet_ip_for_login → :48-51 get_adapters_force
      → 单适配器：auth/session.rs:77 login_adapter_with_log
        双适配器：auth/dual_adapter_executor.rs:46 execute_dual（适配器 2 延迟 1s）

[5] 协议执行
    auth/session.rs:87 预检 auth/portal.rs:95 check_portal_full（只读 GET，无凭据）
      → auth/session.rs:111 auth/protocol.rs:155 do_login_with_retry(..., 3, ...)
           → auth/protocol.rs:98 do_login_request
                :99-101 config/validate.rs:10/23/31 三处凭据校验
                :104 network/client.rs:8 PORTAL_URL.load() + portal.rs:42 ensure_portal_port
                :110 network/client.rs:91 create_safe_http_client(15s, local_addr)
                → HTTP GET /eportal/portal/login（reqwest，无代理）
                :119-120 脱敏后打日志；:127 auth/portal.rs:69 redact_credentials
                → 限长读体（1MB）→ protocol.rs:190 parse_login_result
                     产出 { code, message, success, retryable }

[6] 结果回流与副作用
    auth/session.rs:119-173 复核（parse_error→查 Portal；"已经在线"但探测不通→降级失败）
      → auth/session.rs:11-75 adapter_action_with_log
           infra/events.rs:22 EventBus::emit_login_log  → 前端 login-log
           config/persist.rs:99 append_login_history      → 磁盘 login-history.json（上限 100）
      → auth/failure_tracker.rs:47 update_auth_failure_count
          成功清零；认证失败累加，达 MAX_FAILURES=5（failure_tracker.rs:6）
          → network/dhcp.rs:459 dhcp_release_renew_single（netsh + 注册表 MAC 重置，可能经提权 helper）
      → auth/service.rs:218 post_login_handler
          解除注销保护期 → 500ms 后按需触发后台检查 → 按需 infra/lifecycle.rs:183 start_auto_exit（20s 倒计时）

[7] 回到 UI
    infra/events.rs:18 app_handle.emit('login-log', payload)
      → hooks/tauriApi.ts:159 createEventListener<...>('login-log')
        → hooks/useEventListeners.ts:263 useLogToastStore.addLog(message, type)
          → shared/LogPanel / RightPanel / ToastContainer 渲染
    do_login 的 Promise 同时 resolve CommandResult → useAuthStore 写 status/isLoggingIn（:155-163）
      → 详情面板与状态条经 zustand 订阅重渲染
```

安卓端同一动作的差异点：`invoke('do_login')` → `protocol_cmds.rs:10` 先 `ensure_wifi_bound`（`protocol_cmds.rs:220`，绑定成功且路径非 `already_bound` 时清 HTTP 连接池 `protocol_cmds.rs:232-236`）→ `run_login`（`protocol_cmds.rs:60`）→ 同一个 `auth::protocol::do_login_with_retry` → `login_history::append`（`login_history.rs:51`）→ `app.emit("login-log")`（`monitor_loop.rs:133`）。

### 一次后台巡检拍（桌面端）

```text
monitor/background_task.rs:29-49 循环（每轮重读 config.background_check_interval.max(10000)）
  → monitor/background_check.rs:340 run_background_check（spawn_blocking）
      → :14-335 run_background_check_blocking
          :18 is_checking.try_acquire（单飞）→ :28-37 取适配器
          → monitor/campus_check.rs:33 check_campus_network（SSID/有线 profile/子网/网关）
          → :143-176 Portal 探测（双适配器并行）→ auth/portal.rs:95 check_portal_full
          → :185-206 请求失败计数（阈值 5 → MAC 重置）
          → monitor/background_emit.rs:105 emit_background_check_result
               → :123-130 注销保护期强制 online=false
               → infra/events.rs:30 EventBus::emit_background_check_result
          → monitor/auto_auth.rs:28 try_auto_login_on_preparation（冷却 → 熔断 → 登录锁 → auth::service.rs:20 full_login）
          → monitor/auto_auth.rs:109 try_disconnect_reconnect（计数 CAS ≤ max_disconnect_reconnect）
          → monitor/background_emit.rs:159 update_network_state（必要时 infra/lifecycle.rs:183 start_auto_exit）
```

## 模块索引

### modules/（19 篇）

| 文章 | 一句话职责 |
|---|---|
| [[desktop-auth]] | 桌面 Dr.COM 协议唯一实现点：登录/注销/MAC 解绑/Portal 探测 + 双适配器编排 + 失败计数与 MAC 重置 |
| [[desktop-config]] | `Config` 44 字段模型、原子写落盘、密码 DPAPI 加密、严格/宽松双校验与迁移 |
| [[desktop-account-selfservice]] | `account::crypto`（DPAPI FFI）与 Dr.COM Self（`/Self`）协议全流程 |
| [[desktop-network-core]] | 适配器发现/5s 缓存/选择规则、DHCP、MAC 重置、子网与网关探测、HTTP 客户端池 |
| [[desktop-network-dns]] | 智能域名解析（评分排序 + DoH 竞速 + 系统兜底）、分段计时、DNS/DoH 一键设置 |
| [[desktop-network-quality]] | 19 项延迟测量、四波调度、中位数/截尾平均聚合与六档等级 |
| [[desktop-monitor]] | 后台巡检中枢：校园网判定、Portal 检测、自动登录/断线重连、质量调度、适配器监听 |
| [[desktop-helper-update]] | `--helper` 提权子进程（改 MAC / 设 DNS）与更新自检/下载/SHA256 校验 |
| [[desktop-infra]] | 运行时基座：`AppState`(ArcSwap)、异步日志、`EventBus`、`BackgroundTaskManager`、退出生命周期、通知 |
| [[desktop-app-lifecycle]] | 进程入口、插件装配、命令注册表、托盘/窗口/快捷键/心跳/优雅退出/WebView2 恢复 |
| [[desktop-commands]] | 桌面 54→56 条命令的逐条参数、并发门与 JSON 返回形状 |
| [[desktop-platform]] | Win32/WinRT 平台层：注册表自启、DNS/DoH API、UAC 提权、DXGI 探测、Windows Hello、Toast |
| [[desktop-frontend-hooks]] | 前端 IPC 网关 `tauriApi.ts`、6 个领域 store、事件监听与启动编排 |
| [[desktop-frontend-shared]] | shared 业务组件、Radix UI 原语、布局骨架、`lib/` 工具与 i18n |
| [[desktop-frontend-panels]] | `App.tsx` 外壳与 5 个业务域面板（总览/账号/监控/网络/设置） |
| [[android-backend]] | 安卓后端 48 条命令、14 个模块、平台探针与监控状态机 |
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
2. **前端是两套独立树，功能分叉持续累积**：桌面 `tauri-app/frontend/src/hooks/` 24 个文件、安卓 `android/frontend/src/hooks/` 23 个文件，差集为「安卓有 `useDeviceProfile.ts`/`useAdaptiveFramePace.ts`/`useFormFactor.ts`；桌面有 `useGpuCorrection.ts` + 3 个测试文件」。安卓前端全目录**不存在任何测试**（无 `test-setup.ts`、`package.json` 无 `test` 脚本），复刻分叉只能靠 `tsc --noEmit` 与真机验证发现。
3. **配置字段集是"交集 + 双向差集"**：桌面 `Config` 44 字段（`config/model.rs:10-104`）、安卓 `Settings` 35 字段（`android/src-tauri/src/config_state.rs:15-72`），共有 31；而两套前端类型又各自是第三份定义（桌面 `settings/types.ts:3-51` 43 字段、安卓 `settings/types.ts:3-52` 43 字段，两者均指各自 `Config` 接口本体的字段总数）。新增字段无法照抄任何一端。
4. **默认值分叉已实际发生**：`background_check_interval` 桌面 15000（`config/model.rs:180`）与安卓 60000（`android/src-tauri/src/config_state.rs:88`）不一致，且安卓迁移会把 15000 强刷为 60000（`config_state.rs:243-245`）；`default_panel` 桌面空串（`model.rs:189`）与安卓 `"dashboard"`（`config_state.rs:97`）不同。桌面 `validate.rs` 的 clamp 下限 10000（`config/validate.rs:103`）对不上安卓的 5000。**配置 schema 版本号也各走各的**：安卓前端 `android/frontend/src/settings/constants.ts:52` 的 `DEFAULT_CONFIG.configVersion` 曾照抄桌面的 2，2026-09-13 改为对齐安卓后端 `config_schema_version`（现 4，`config_state.rs:120`，v3→v4 新增 `background_check_idle_interval`）；桌面同名 `configVersion` 仍是 2（`tauri-app/src-tauri/src/config/model.rs:208`、`tauri-app/frontend/src/settings/constants.ts:49`）。两端字段名近义（`config_version` ↔ `config_schema_version`）但语义独立，跨端对照时不可互推。
5. **版本号有六个来源，无自动校验**：桌面 `env!("APP_VERSION")` 由 `tauri-app/src-tauri/build.rs:37` 从 `tauri.conf.json` 注入；安卓用 `env!("CARGO_PKG_VERSION")`（`android/src-tauri/src/system_cmds.rs:14`，安卓 `build.rs` 只有一行 `tauri_build::build()`）；两端 `Cargo.toml` 各有一个 `version`；两套前端手写常量 `tauri-app/frontend/src/shared/ui-constants.ts:2` 与 `android/frontend/src/shared/ui-constants.ts:2` 都硬编码 `'2.3.6'`。发布时需人工多处同步；发布侧主脚本是项目根 `make-release.ps1`（包装 `tauri-app/build.ps1`，从 `tauri-app/src-tauri/tauri.conf.json` 读版本，把安装包改名为 `Wxxy-CampusLogin_<版本>_x64-setup.exe` 并生成同名 `.sha256`，收集 APK 与 `RELEASE_NOTES_v<版本>.md`，快照 CHANGELOG 到 `changelogs/`，最后打印 `gh release create/upload` 命令），它本身**不做版本号跨来源一致性校验**。

### 已发现的代码不一致

6. **二进制与 lib 各编译一份模块树**：`tauri-app/src-tauri/src/main.rs:3-14` 用 `mod` 自行声明 12 个模块，`lib.rs:2-20` 另声明一份，`main.rs` 不通过 `campus_login_lib::` 复用。后果是同一份源码编译两遍，且 `#[macro_export]` 宏在 bin 与 lib 各有一份实例（`crate::log_info!` 与 `campus_login_lib::log_info!` 不是同一个东西）。
7. **安卓后端 5 处裸 `app.emit` 绕过事件总线模式**：`android/src-tauri/src/monitor_loop.rs:133`、`:446`、`:760`、`android/src-tauri/src/update_cmds.rs:251`、`:355`；而安卓的 `network-quality-result` 又反而经共享 crate 走 `EventBus::new(ah).emit_network_quality_result`（`tauri-app/src-tauri/src/network/quality.rs:507/583/637`）。安卓没有集中登记点，事件名散落在各处。
8. **安卓不广播 `config-changed`**：`config_state.rs:312-338` 的 `save_config` 只落盘 + 刷 `AndroidState.config` 缓存，不发事件；但安卓前端仍注册监听（`android/frontend/src/hooks/useEventListeners.ts:350`），`mergeConfigFromBackend` 的分支在安卓永不触发。
9. **安卓 `save_config` 未持 `CONFIG_IO_LOCK`**：锁定义在 `android/src-tauri/src/account_cmds.rs:17/21`，`switch_account`/`save_current_as_account`/`delete_account`（`account_cmds.rs:127/156/179`）、`set_boot_autostart`（`monitor_loop.rs:331`）都取锁，而读改写全量配置的 `save_config`（`config_state.rs:312-338`）没取——正是该锁要防的竞态对象。
10. **安卓 `AndroidState` 文档与实现不符**：`android/src-tauri/src/lib.rs:8` 的模块说明写「android_state:进程态(源 IP 缓存/配置内存态/监控循环句柄)」，实际结构体只有 `cached_source_ip` 与 `config` 两个字段（`android_state.rs:10-12`），监控状态在 `monitor_loop.rs:12` 的 `MONITOR` 静态里。
11. **安卓 3 条命令无前端出口**：`ping_test`（`protocol_cmds.rs:169`）、`detect_campus`（`campus_detect.rs:124`）、`accept_wifi_network`（`protocol_cmds.rs:193`）都注册进了 `generate_handler!`，但 `android/frontend` 全仓无调用点（`campus_detect.rs:122` 的注释「前端旧 UI 仍在用」已过期）。
12. **三个插件的权限清单与命令集不一致**：`android/plugins/network-bind/permissions/default.toml:7` 写的 `allow-bind-to-wifi`/`allow-accept-wifi-network` 与生成器产出的 `allow-bindToWifi`/`allow-acceptWifiNetwork` 名不匹配；`network-bind/build.rs:1` 只声明 3 个命令，Kotlin 侧还有 `startWifiWatcher`/`stopWifiWatcher`；`foreground-service/permissions/default.toml:7-20` 列 12 项而 `build.rs:1` 只注册 6 个命令。悬空引用不会在构建期报错，因为 `android/src-tauri/capabilities/default.json` 未引用这些插件的 default 权限集。
13. **两端命令名有三处刻意不一致**（由前端 `tauriApi` 转调抹平）：`verify_windows_identity` ↔ `verify_biometric_identity`、`get/set_auto_launch` ↔ `get/set_boot_autostart`、`get_config` 返回 `Config` ↔ 返回掩码后的 `serde_json::Value`（`android/src-tauri/src/config_state.rs:296-309`）。任何按"命令名逐字相同"假设写的自动化核对都会在这三处误报。
14. **文档计数漂移（2026-09-13 两轮核对定稿）**：`modules/desktop-commands.md` 原称安卓注册 45 项、行号 `lib.rs:54-100`——该数字对应**电池白名单命令面落地前**的旧状态（`battery_cmds` 三命令由 main 提交 `6cbee74` 引入）。以主仓库工作区实测，安卓命令面为 **48 项**：注册块 `android/src-tauri/src/lib.rs:55-104`（`:55` = `.invoke_handler(tauri::generate_handler![`，`:56-103` = 48 条命令条目，`:104` = `])`），与桌面 `app/startup.rs:63-120`（条目 `64-119`，56 条）引用口径一致；核实命令 `grep -cE '^\s*[a-z_0-9]+::[a-z_0-9]+,$' android/src-tauri/src/lib.rs` → 48。**计数必须在主仓库工作区 `E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin` 上测**：滞后于 main 的分支/worktree 缺 `battery_cmds.rs`，会误测成 45 项/`54-100`（同日曾有一轮据此想把 48 改回 45，已被实况驳回）；`modules/android-backend.md` 原称 `lib.rs:10-23` 声明 15 个模块，实际 14 个（`campus_detect`/`android_state`/`config_state`/`cpu_affinity`/`identity_gate`/`login_history`/`protocol_cmds`/`self_service_cmds`/`monitor_loop`/`account_cmds`/`system_cmds`/`quality_cmds`/`update_cmds`/`battery_cmds`）。两处均已改为实计数。
15. **`default_panel`"Rust 侧零引用"的说法不准确**：`grep -rn default_panel tauri-app/src-tauri/src/` 命中 3 处，除 `config/model.rs:61/189` 外还有 `network/adapter.rs:255`（测试辅助 `make_test_config`）。`self_reverify_each_action` 同因命中 `network/adapter.rs:235`（同为测试辅助）。结论（"仅前端实际消费"）成立，但证据描述需修正（`modules/desktop-config.md` 的 `default_panel` 条目已于 2026-09-13 按此改写证据）。
