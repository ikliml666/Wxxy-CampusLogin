---
title: rust_bridge
type: module
source_files:
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/tauri-mock.js
  - tauri-app/frontend/vite.mock.config.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/vpn.rs
tags: [tauri, ipc, bridge, frontend, rust-backend, invoke, events]
---

## Overview

`rust_bridge` 是前端（React + TypeScript，运行在 Tauri WebView 内）与 Rust 后端（`src-tauri`，Tauri 应用核心）之间唯一的通信抽象层。它封装了 Tauri 的两条 IPC 通道——**命令调用（invoke）** 与 **事件订阅（listen）**——把约 120 个后端 `#[tauri::command]` 暴露成一组带强类型返回值的 TS 方法，供 UI 组件与各个 store 直接调用，UI 层无需关心 `@tauri-apps/api/core` 与 `@tauri-apps/api/event` 的细节。

核心实现位于 `tauri-app/frontend/src/hooks/tauriApi.ts`：

- `interface TauriApi`（第 39–144 行）声明了全部公开方法及其入参/返回类型（命令 + 事件监听器）。
- `const tauriApi: TauriApi`（第 177–282 行）用 `invoke()` 逐条实现每个命令；事件类方法统一委托给工厂 `createEventListener`（第 146–175 行）。
- `export const tauriApiWithRetry: TauriApi`（第 314–320 行）在 `tauriApi` 之上包了一层指数退避重试，仅对 `saveConfig` 保留重试。
- 默认导出的消费入口是 `tauriApiWithRetry`（各 store/hook 以 `import { tauriApiWithRetry as api } from '@/hooks/tauriApi'` 引入）。

后端侧对应的注册与分发在 `tauri-app/src-tauri/src/app/startup.rs` 的 `generate_handler!`（第 66–137 行）完成；事件发射统一走 `tauri-app/src-tauri/src/infra/events.rs` 的 `EventBus`（事件名与前端 `createEventListener` 完全对齐）。命令层公共上下文见 `tauri-app/src-tauri/src/infra/command_context.rs`，运行时共享状态见 `tauri-app/src-tauri/src/infra/state/mod.rs`（`AppState`、`CommandResult`、`AccountItem`、`AccountResult`）。

此外，仓库里还存在一个**纯浏览器 mock** 实现 `tauri-app/frontend/tauri-mock.js`：在真 Tauri 环境缺失时（本地浏览器 / Vitest 取不到 `window.__TAURI_INTERNALS__` 时）接管 `invoke`/`listen`，让所有命令走内存实现。它由 `tauri-app/frontend/vite.mock.config.ts` 通过 `transformIndexHtml` 把 `/tauri-mock.js` 以 `<script>` 注入 `index.html` 头部（先于 module 脚本执行，第 10–18 行），是这条 bridge 在测试与截图场景下的平行实现。

## Key Components

### 前端：`tauri-app/frontend/src/hooks/tauriApi.ts`

**类型声明 `interface TauriApi`（第 39–144 行）** —— 列出全部公开方法。下面按职责分组给出方法名、所在行号与返回/用途：

配置与诊断：
- `getConfig(): Promise<Config>`（第 40 行）— 取全量配置。
- `saveConfig(config, clearPassword?): Promise<SaveConfigResult>`（第 41 行）— 落盘配置；`tauriApiWithRetry` 对其单独套重试。
- `exportConfig(includePassword?): Promise<string>`（第 43 行）— 导出配置到 `<dataDir>/exports/`，桌面专属（密码以本机 DPAPI 密文导出）。
- `importConfig(path): Promise<CommandResult>`（第 45 行）— 从 JSON 文件导入。
- `exportDiagnostics(days?): Promise<string>`（第 47 行）— 导出诊断包到 `<dataDir>/diagnostics/`。
- `getInitData(): Promise<InitData>`（第 128 行）— 启动首屏聚合数据（config/accounts/version/adapters/bgStatus 等）。

网络/适配器：
- `getAdapters(force?): Promise<Adapter[]>`（第 48 行）— 网卡列表。
- `enableAdapter(adapterName): Promise<EnableAdapterResult>`（第 49 行）。
- `getAdapterDetails(): Promise<AdapterDetail[]>`（第 50 行）。
- `checkPortalStatus(adapterIp): Promise<PortalStatusResult>`（第 51 行）。
- `checkCampusStatus(): Promise<CampusStatusResult>`（第 52 行）。
- `checkNetworkQuality(): Promise<NetworkQuality>`（第 105 行）。
- `startLatencyTest() / stopLatencyTest(): Promise<CommandResult>`（第 107–108 行）。
- `dhcpRenewAll(): Promise<DhcpRenewResult>`（第 96 行）、`dhcpReleaseRenew(): Promise<DhcpReleaseRenewResult>`（第 97 行）、`dhcpReleaseRenewAdapter(adapterName): Promise<DhcpReleaseRenewResult>`（第 98 行）。
- `checkDnsDohStatus(): Promise<DnsDohStatus>`（第 138 行）、`setupDnsDoh(family?): Promise<DnsSetupResult>`（第 139 行）、`resetDns(): Promise<CommandResult>`（第 140 行）。
- `outboundSwitchNow() / outboundRestoreNow(): Promise<CommandResult>`（第 100–102 行）、`getCurrentOutboundName(): Promise<string | null>`（第 104 行）。

登录/认证：
- `doLogin(adapterName?): Promise<LoginResult>`（第 53 行）、`doLogout(adapterName?): Promise<LoginResult>`（第 54 行）。
- `bindOperator(params): Promise<CommandResult>`（第 55 行）、`getBindStatus(params): Promise<CommandResult>`（第 56 行）。
- `verifyWindowsIdentity(params): Promise<CommandResult>`（第 57 行）、`revealOperatorCredential(params): Promise<CommandResult>`（第 58 行）。

自助服务 / VPN：
- `querySelfDashboard(params): Promise<CommandResult>`（第 59 行）、`selfOfflineSession(params): Promise<CommandResult>`（第 60 行）、`querySelfOnlineLog(params): Promise<CommandResult>`（第 61 行）、`changeSelfPassword(params): Promise<CommandResult>`（第 62 行）。
- `vpnSaveCredential(params): Promise<CommandResult>`（第 64 行）、`vpnCredentialStatus(): Promise<CommandResult>`（第 66 行）、`vpnClearCredential(): Promise<CommandResult>`（第 68 行）、`setSelfServiceViaVpn(enabled): Promise<CommandResult>`（第 70 行）、`vpnTestConnection(): Promise<CommandResult>`（第 72 行）、`vpnVerifyCredential(params): Promise<CommandResult>`（第 74 行）。

账号档案：
- `listAccounts(): Promise<AccountItem[]>`（第 83 行）、`createAccount(params): Promise<AccountResult>`（第 85 行）、`switchAccount(accountName): Promise<SwitchAccountResult>`（第 86 行）、`saveCurrentAsAccount(accountName): Promise<SaveAccountResult>`（第 87 行）、`deleteAccount(accountName): Promise<DeleteAccountResult>`（第 88 行）、`renameAccount(params): Promise<AccountResult>`（第 90 行）、`getActiveAccount(): Promise<string>`（第 91 行）。

后台检测：
- `startBackgroundCheck() / stopBackgroundCheck() / triggerBackgroundCheck(): Promise<CommandResult>`（第 92–94 行）、`getBackgroundStatus(): Promise<BackgroundStatus>`（第 95 行）。

窗口/系统：
- `minimizeWindow() / closeWindow(): Promise<void>`（第 75–76 行）、`showWindow(): Promise<void>`（第 124 行）、`openExternal(url): Promise<boolean>`（第 109 行，含 http(s)/长度/URL 解析校验与 shell 插件兜底）、`setAutoLaunch(enabled): Promise<AutoLaunchResult>`（第 110 行）、`getCoreAutostart(): Promise<boolean>`（第 111 行）、`setCoreAutostart(enabled): Promise<CoreAutostartResult>`（第 112 行）、`setNotificationEnabled(enabled): Promise<boolean>`（第 113 行）、`cancelAutoExit(): Promise<CommandResult>`（第 114 行）。
- `getLogs(lines?): Promise<string>`（第 125 行）、`clearLogs(): Promise<boolean>`（第 126 行）、`setDebugMode(enabled): Promise<boolean>`（第 127 行）、`setLogRetentionDays(days): Promise<void>`（第 143 行）、`renderHeartbeat(): Promise<{online, checking}>`（第 141 行）、`notifyWindowReady(): Promise<CommandResult>`（第 142 行，轻量化重建窗口的 ready 门）。

更新：
- `checkUpdate(): Promise<UpdateInfo>`（第 129 行）、`downloadUpdate(url): Promise<string>`（第 130 行）、`cancelDownloadUpdate(): Promise<boolean>`（第 132 行）、`installUpdate(filePath, checksumUrl?): Promise<boolean>`（第 133 行）、`getMirrorUrls(githubUrl): Promise<MirrorSource[]>`（第 134 行）。

事件订阅（均为「注册回调、返回 unlisten 清理函数」形态）：
- `onBackgroundCheckResult`（第 77 行）、`onAutoLoginResult`（第 78 行）、`onAdaptersChanged`（第 79 行）、`onAdapterDetailsChanged`（第 80 行）、`onAdapterDisabledWarning`（第 81 行）、`onLoginLog`（第 82 行）、`onNetworkQualityResult`（第 106 行）、`onAutoExitCountdown`（第 115 行）、`onAutoExitCancelled`（第 116 行）、`onCampusExitCountdown`（第 117 行）、`onCampusExitCancelled`（第 118 行）、`onScheduledActionSkipped`（第 120 行）、`onMacResetCycle`（第 122 行）、`onConfigChanged`（第 123 行）、`onDownloadProgress`（第 135 行）、`onUpdateAvailable`（第 136 行）、`onUpdateNotificationClick`（第 137 行）。

**工厂 `createEventListener<T>(eventName)`（第 146–175 行）** —— 返回 `(cb) => () => void`。内部先 `listen<T>(eventName, cb)` 拿到 `UnlistenFn`，并用 `cancelled` 标志防御「监听注册完成前就调用清理」的竞态：若清理时 `unlisten` 还是 `null`，则在 `listenPromise.then` 里补一次解绑（第 164–173 行）。`listen` 失败时仅 DEV 环境 `console.error` 并吞掉（第 159–162 行），不抛。

**实现体 `const tauriApi: TauriApi`（第 177–282 行）** —— 每条命令都是 `invoke<ReturnType>('command_name', { args })` 的薄包装。命令名与 `app/startup.rs` 的 `generate_handler!` 完全一致（例如第 178 行 `get_config`、第 188 行 `do_login`、第 199 行 `vpn_credential_status`）。`openExternal`（第 233–250 行）是唯一的非纯 invoke：先做 http(s)/长度/URL 解析校验，失败后退回 `tauri-apps/plugin-shell` 的 `shellOpen` 兜底。

**重试封装 `export const tauriApiWithRetry: TauriApi`（第 314–320 行）** —— `{ ...tauriApi }` 展开后仅覆盖 `saveConfig`：用 `withRetry` 包裹（第 296–312 行，默认 `maxRetries=2`、`baseDelay=500ms` 指数退避 + 随机抖动）。`isRetryableError`（第 284–294 行）仅在错误文本含 `timeout/network/fetch/connection` 时触发重试；`checkPortalStatus`/`checkNetworkQuality` 因已有后台循环或事件流兜底，**故意不加重试**（注释第 316–318 行），避免放大高频调用流量。

**前端本地类型（仅在本文件内声明、参与类型收口）：**
- `interface ConnectionCampusStatus { onCampus; name | null; message }`（第 11–15 行）。
- `export interface AdapterCampusStatus { name; wireless; onCampus; matchedBy; message }`（第 18–25 行），`matchedBy` 取值 `name | subnet | gateway | portal | null`。
- `interface CampusStatusResult`（第 27–37 行），其中 `adapters?: AdapterCampusStatus[]` 为可选（旧版后端可能不返回）。

### 后端命令注册：`tauri-app/src-tauri/src/app/startup.rs`

`invoke_handler(tauri::generate_handler![ ... ])` 在第 66–137 行列出**所有**暴露给前端的命令，分模块映射回 `crate::commands/*`：
- `config_cmd::{get_config, show_window, save_config, export_config, import_config}`（第 67–71 行）
- `login::{do_login, do_logout}`（第 72–73 行）
- `network_cmd::{get_adapters, get_adapter_details, check_campus_status, check_portal_status, enable_adapter, dhcp_renew_all, dhcp_release_renew, dhcp_release_renew_adapter, check_network_quality, start_latency_test, stop_latency_test, check_dns_doh_status, setup_dns_doh, reset_dns, outbound_switch_now, outbound_restore_now, get_current_outbound_name}`（第 74–90 行）
- `account::{list_accounts, switch_account, rename_account, save_current_as_account, create_account, delete_account, get_active_account}`（第 91–97 行）
- `background::{start_background_check, stop_background_check, trigger_background_check, get_background_status}`（第 98–101 行）
- `system::{set_auto_launch, set_core_autostart, get_core_autostart, set_notification_enabled, cancel_auto_exit, minimize_window, close_window, open_external, get_logs, clear_logs, get_init_data, render_heartbeat, set_log_retention_days, export_diagnostics, notify_window_ready}`（第 102–116 行）
- `updater::{check_update, download_update, cancel_download_update, install_update, get_mirror_urls}`（第 117–121 行）
- `self_service::{bind_operator, query_bind_status, verify_windows_identity, reveal_operator_credential, query_self_dashboard, query_self_online_log, self_offline_session, change_self_password}`（第 122–129 行）
- `vpn::{vpn_save_credential, vpn_credential_status, vpn_clear_credential, set_self_service_via_vpn, vpn_test_connection, vpn_verify_credential}`（第 130–135 行）
- `infra::logger::set_debug_mode`（第 136 行）

注意：`generate_handler!` 仅注册命令（请求-响应）；所有 `onXxx` 事件由后端 `EventBus` 主动 `emit`，与前端 `createEventListener` 的 `eventName` 一一对应（见下）。

### 后端事件总线：`tauri-app/src-tauri/src/infra/events.rs`

`pub struct EventBus<'a>`（第 8–19 行）封装 `app_handle.emit(event, payload)`。每个 `emit_*` 方法对应前端一个 `onXxx` 监听器（事件名字符串必须完全一致）：

| 前端监听器 | 事件名 | 后端 emit 方法（行号） |
|---|---|---|
| `onBackgroundCheckResult` | `background-check-result` | `emit_background_check_result`（第 30–32 行） |
| `onAutoLoginResult` | `auto-login-result` | `emit_auto_login_result`（第 46–52 行） |
| `onAdaptersChanged` | `adapters-changed` | `emit_adapters_changed`（第 118–120 行） |
| `onAdapterDetailsChanged` | `adapter-details-changed` | `emit_adapter_details_changed`（第 74–76 行） |
| `onAdapterDisabledWarning` | `adapter-disabled-warning` | `emit_adapter_disabled_warning`（第 79–84 行） |
| `onLoginLog` | `login-log` | `emit_login_log`（第 22–27 行） |
| `onNetworkQualityResult` | `network-quality-result` | `emit_network_quality_result`（第 55–57 行） |
| `onAutoExitCountdown` | `auto-exit-countdown` | `emit_auto_exit_countdown`（第 100–105 行） |
| `onAutoExitCancelled` | `auto-exit-cancelled` | `emit_auto_exit_cancelled`（第 108–110 行） |
| `onCampusExitCountdown` | `campus-exit-countdown` | `emit_campus_exit_countdown`（第 87–92 行） |
| `onCampusExitCancelled` | `campus-exit-cancelled` | `emit_campus_exit_cancelled`（第 95–97 行） |
| `onScheduledActionSkipped` | `scheduled-action-skipped` | `emit_scheduled_action_skipped`（第 36–38 行） |
| `onMacResetCycle` | `mac-reset-cycle` | `emit_mac_reset_cycle`（第 41–43 行） |
| `onConfigChanged` | `config-changed` | `emit_config_changed`（第 113–115 行） |
| `onDownloadProgress` | `update-download-progress` | `emit_update_download_progress`（第 123–125 行） |
| `onUpdateAvailable` | `update-available` | `emit_update_available`（第 60–66 行） |
| `onUpdateNotificationClick` | `update-notification-click` | `emit_update_notification_click`（第 69–71 行） |

### 后端命令上下文与共享状态

- `tauri-app/src-tauri/src/infra/command_context.rs`：`pub struct CommandContext<'a> { app: &AppHandle, state: &AppState }`（第 10–13 行），`from_app` 自动取 state（第 22–25 行），并 `Deref` 到 `AppState`（第 30–33 行）；`AppHandleExt` trait（第 38–51 行）把 `notify_config_changed`/`notify_update_download_progress` 委托给 `EventBus`。
- `tauri-app/src-tauri/src/infra/state/mod.rs`：`pub struct AppState`（第 184–192 行，含 `config`/`tasks`/`task_manager`/`network`/`exit`/`update_stats`/`scheduled`）。出站返回结构：`CommandResult { success, message?, data? }`（第 291–310 行，`command_cmd` 的 `ok/ok_msg/err` 构造器）、`AccountItem { id, displayName, user, operator, passwordSaved }`（第 314–327 行）、`AccountResult`（第 329–353 行）。

### 浏览器 mock：`tauri-app/frontend/tauri-mock.js`

- 在 `window.__TAURI_INTERNALS__` 缺失时注入（第 6 行），重写 `invoke`（第 333–352 行）与 `transformCallback`/`emitEvent`（第 356–374 行）。
- `var handlers = { ... }`（第 88–314 行）按命令名分发内存实现；未知命令只 `console.warn` 不崩（第 350–351 行）。
- 自带质量循环：启动 1.2s 后首推 `network-quality-result`，之后每 30s 一次（第 317–331 行）。
- 支持 query 参数场景覆盖：`?mockempty=1` 空账号、`?mocksingle=1` 单账号、`?active=<id>` 预设激活、`?vpnsaved=1` 已存 VPN 凭据、`?vpnfail=1` 校验失败、`?bound=none|cmcc|telecom|unicom` 绑定状态、`?panel=`/`?theme=` 启动面板/主题、`?nomotion=1` 禁动画、`?probe=1` 几何探针、`?dialog=create|edit|bind` 自动点入口按钮（第 28–49、125–153、376–423 行）。

### 事件订阅接线：`tauri-app/frontend/src/hooks/useEventListeners.ts`

`useEventListeners()`（第 18 行起）在 `useEffect` 里逐个调用 `api.onXxx(...)` 注册监听，把后端事件写入相应 store/zustand。关键接线：
- `onBackgroundCheckResult`（第 59 行）→ `useAuthStore.setBgStatus` / `setStatus`；带 1s 节流（第 63 行）与上线/离线日志去抖（第 107–116 行）。
- `onAutoLoginResult`（第 187 行）→ toast + 触发 `checkOnline`。
- `onAdaptersChanged`（第 203 行）→ `useAdapterStore.setAdapters`，带 500ms 尾随定时（trailing throttle，第 213–226 行）。
- `onAdapterDetailsChanged`（第 230 行）→ `setAdapterDetails`，内容未变跳过 setState。
- `onConfigChanged`（第 336 行）→ `mergeConfigFromBackend` + 同步激活账号 + 刷新账号列表（第 343–354 行）。
- 其余 `onAdapterDisabledWarning`/`onLoginLog`/`onAutoExit*`/`onCampusExit*`/`onNetworkQualityResult`/`onUpdateAvailable`/`onScheduledActionSkipped`/`onMacResetCycle` 分别在第 240、249、257、278、286、307、315、323、360、368 行接线。
- 注释明确禁止注册 `onCloseRequested`（第 49–54 行）：Tauri core 检测到 JS 关闭监听会无条件 `prevent_close`，导致关闭按钮失效（v2.4.2 缺陷根因）。

## Architecture

前端 UI / store 通过 `tauriApiWithRetry`（瘦封装 + 重试）发出命令、订阅事件；请求经 Tauri IPC 落到 `generate_handler!` 注册的 Rust 命令，响应携带 `CommandResult` 等结构化数据；后台状态变化由 `EventBus` 主动 `emit`，经同一 IPC 通道以 `listen` 回传，最终由 `useEventListeners` 写入各 store。

```text
┌──────────────────────────────┐         ┌──────────────────────────────┐
│  React UI / Zustand stores   │         │  tauriApi.ts (frontend/TS)   │
│  (useNetwork / useAuthStore…)│────────►│  TauriApi iface (L39-144)    │
└──────────────┬───────────────┘         │  tauriApi impl  (L177-282)   │
               │ onXxx subscribe          │  tauriApiWithRetry (L314-320)│
               │ (useEventListeners L18)  └───────────┬──────────────────┘
               │                                      │ invoke() / listen()
               ▼                                      ▼
┌──────────────────────────────┐         ┌──────────────────────────────┐
│  useEventListeners.ts         │         │  Tauri IPC core (WebView2)   │
│  write stores from events     │◄────────│  window.__TAURI_INTERNALS__   │
└──────────────────────────────┘         └───────────┬──────────────────┘
                                                      │
                                                      ▼
                              ┌───────────────────────────────────────────┐
                              │  src-tauri (Rust)                           │
                              │  generate_handler! (startup.rs L66-137)     │
                              │    ├ commands/* (#[tauri::command])         │
                              │    │   └ CommandContext (command_context.rs)│
                              │    └ AppState (infra/state/mod.rs)          │
                              │  EventBus.emit_* (infra/events.rs L8-126)   │
                              └───────────────────────────────────────────┘

测试/截图平行实现：
  Browser ── tauri-mock.js (L1-427, vite.mock.config.ts 注入) ── handlers[] (L88-314)
```

组件数量（命令/事件/类型/上下文/状态/mock）远超 3，故上图用 UTF-8 框图刻画主链路与平行 mock 分支。

## Data Flow

**请求-响应（命令调用）：**
1. UI/store 调用 `tauriApiWithRetry.someCommand(args)`（`tauriApi.ts` 第 314 行起），落到 `tauriApi.someCommand` 的 `invoke<ReturnType>('command_name', { ...args })`（如第 178–282 行）。
2. Tauri core 按命令名路由到 `generate_handler!` 注册的 Rust 函数（`startup.rs` 第 66–137 行），例如 `do_login` → `crate::commands::login::do_login`。
3. 后端命令经 `State<'_, AppState>` / `CommandContext` 读写共享状态（`command_context.rs`、`infra/state/mod.rs`），返回 `Result<CommandResult/serde_json::Value, String>`。
4. 若返回 `Err(String)`，Tauri 让 `invoke` 的 Promise reject，前端捕获后由调用方决定重试（`withRetry` 仅对 `saveConfig`）或弹 toast。
5. 仅 `saveConfig` 经过 `withRetry` 指数退避重试（第 296–312、319 行）；`checkPortalStatus`/`checkNetworkQuality` 明确不重试（注释第 316–318 行）。

**事件流（后端 → 前端）：**
1. 后端在某状态变化时调用 `EventBus::new(app).emit_*（`events.rs` 各方法），例如配置落盘后 `emit_config_changed`（第 113–115 行），对应 mock 里 `setTimeout(emitEvent('config-changed', …))`（`tauri-mock.js` 第 109 行）。
2. 前端 `createEventListener`（第 146–175 行）用 `listen<T>(eventName)` 注册，回调把 `e.payload` 交给业务 cb。
3. `useEventListeners`（第 18 行起）的 `onXxx` 回调把 payload 写入 `useAuthStore`/`useAdapterStore`/`useConfigStore`/`useQualityStore`/`useLogToastStore`，驱动 UI 刷新；并带去抖/节流（如 `onAdaptersChanged` 500ms 尾随、`onBackgroundCheckResult` 1s 节流）。

**心跳与窗口就绪（特殊循环）：**
- `renderHeartbeat`（前端第 141 行 → 后端 `system.rs` 第 260–275 行）更新 `update_stats.last_render_heartbeat_ms`，供 `app/heartbeat.rs` 检测 WebView 崩溃。
- `notifyWindowReady`（前端第 142 行 → 后端 `system.rs` 第 389–393 行）发 `lightweight::signal_window_ready`，用于轻量化重建窗口的 ready 门。

**浏览器 mock 平行流程（`tauri-mock.js`）：**
- `vite.mock.config.ts`（第 10–18 行）把 `/tauri-mock.js` 注入 `index.html`，在真 Tauri 缺失时先定义 `window.__TAURI_INTERNALS__`。
- 前端 `invoke` 落到 mock 的 `handlers[cmd]`（`tauri-mock.js` 第 88–314 行）；`listen` 经 `transformCallback`/`eventListeners` 表 + `emitEvent` 回送（第 338–364 行），事件名与真后端完全一致，因此 UI 无需区分环境。

## Connections

- [[tauri-app]] — 本模块所属的桌面应用整体；`tauri-app.md` 描述前端/后端目录结构与构建。
- [[src-tauri-commands]] — 后端 `#[tauri::command]` 实现全集（account/background/network_cmd/system/config_cmd/updater/self_service/vpn），即 `generate_handler!` 实际落地的业务。
- [[infra-event-bus]] — `infra/events.rs` 的 `EventBus` 与各 `emit_*` 事件名契约（如存在独立模块文档时）。
- [[app-state]] — `infra/state/mod.rs` 的 `AppState`/`CommandResult`/`AccountItem`/`AccountResult` 共享状态与出站结构。
- [[frontend-stores]] — `useAuthStore`/`useAdapterStore`/`useConfigStore`/`useQualityStore`/`useLogToastStore`，事件最终写入处（`useEventListeners.ts`）。
- [[network-module]] — 网络/适配器相关命令（`get_adapters`/`check_campus_status`/`dhcp_*`/`check_dns_doh_status`，`network_cmd.rs` + 前端 `network/`）。
- [[vpn-module]] — WebVPN 凭据与路由命令（`vpn_*`、`self_service_via_vpn`，`commands/vpn.rs`）。
- [[self-service-module]] — 自助服务命令（`bind_operator`/`query_self_*`/`change_self_password`）。
- [[updater-module]] — 更新命令（`check_update`/`download_update`/`get_mirror_urls`）。
- [[frontend-mock]] — 浏览器测试 mock（`tauri-mock.js` + `vite.mock.config.ts`）的契约与场景参数。

## Known Issues

- **事件名必须两端严格对齐**：前端 `createEventListener(eventName)`（`tauriApi.ts` 第 206–275 行）与后端 `EventBus.emit_*`（`events.rs`）的事件字符串硬编码相同。新增事件时若只改一端，监听会静默收不到（mock 的 `emitEvent` 同样依赖同名，第 356–364 行）。无编译期校验。
- **`onCloseRequested` 被明确禁止注册**（`useEventListeners.ts` 第 49–54 行注释）：Tauri core 一旦检测到 JS 关闭监听就无条件 `prevent_close`（tauri-2.11.0 `window.rs:170-174`），会把窗口销毁转嫁异步收尾链，导致 v2.4.2 关闭按钮 6 连击失效。配置 flush 改由 `App.tsx handleCloseWindow` 就地完成，失焦兜底 flush 见 `handleWindowBlur`（第 55–57 行）。
- **重试仅保留 `saveConfig`**：`tauriApiWithRetry` 对 `checkPortalStatus`/`checkNetworkQuality` 等高频命令刻意不套 `withRetry`（第 316–318 行），避免放大流量；若有人误给高频命令加重试，会在后台检测循环上叠加重复请求。
- **`openExternal` 双重校验/兜底**：前端 `tauriApi.ts` 第 233–250 行、后端 `system.rs` 第 44–57 行都对 URL 做 http(s)/长度/解析/用户名密码校验，二者语义需同步维护；前端还额外在 invoke 失败后退回 `shellOpen`，校验口径不一致会导致一处通过另一处失败。
- **`adapters` 事件尾随节流可能延迟 UI**：`onAdaptersChanged` 在 500ms 内多次触发只保留最后一次（第 213–226 行），适配器频繁抖动时 UI 更新有最多 ~500ms 滞后。
- **mock 与真后端契约漂移风险**：`tauri-mock.js` 的 `handlers` 是命令返回结构的手搓副本（第 88–314 行），后端 `CommandResult`/`AccountResult` 字段变动（如 `config-changed` 新字段）需同步改 mock，否则浏览器/截图场景与真机表现分叉（注释第 107–109、211 行已记录 `config-changed` 回传全量配置的同步需求）。
- **`createEventListener` 解绑竞态**：若 `listen` 尚未完成就调用返回的清理函数，`unlisten` 为 `null`，需等 `listenPromise.then` 补解绑（第 164–173 行）；极端时序下仍可能短暂残留一个监听。
