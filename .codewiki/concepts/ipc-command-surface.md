---
title: IPC 命令面与事件总线
type: concept
source_files:
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/platform/toast.rs
  - tauri-app/src-tauri/src/commands/updater.rs
  - tauri-app/src-tauri/capabilities/default.json
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/src/battery_cmds.rs
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/auth/AboutDialog.tsx
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/useEventListeners.ts
tags: [概念, ipc, tauri, 命令, 事件总线, 双端]
---

## Overview

IPC 命令面是 webview（React 前端）与 Rust 后端之间的唯一函数调用通道，事件总线是后端向前端单向推送的唯一通道。桌面端由 `tauri-generate_handler!` 注册 **56 条命令**，安卓端由独立的 `generate_handler!` 注册 **48 条命令**，两端命令名逐字对齐，由各自的前端 `tauriApi.ts` 提供同形接口。事件面只有桌面端有统一封装（`infra/events.rs` 的 `EventBus`，16 个事件名），安卓端为桌面子集，且部分事件直接裸调 `app.emit`。

## 机制说明

### 命令注册点（先读这一节）

| 端 | 注册位置 | 注册项数 | 命令定义位置 |
|---|---|---|---|
| 桌面 | `tauri-app/src-tauri/src/app/startup.rs:63-120` | 56 | `commands/` 下 8 文件 54 条 + `infra/logger.rs:336/343` 2 条 |
| 安卓 | `android/src-tauri/src/lib.rs:55-104` | 48 | `android/src-tauri/src/` 下 9 个 `*_cmds` / 状态模块 |

桌面 `commands/mod.rs:1-8` 声明 8 个领域文件：`login` / `background` / `network_cmd` / `system` / `config_cmd` / `account` / `updater` / `self_service`。安卓 `lib.rs:10-23` 声明 14 个模块。

### 命令命名与参数契约

- **命令名**：直接取 Rust 函数名（snake_case），两端同名。前端以 `invoke('do_login', ...)` 调用（`tauri-app/frontend/src/hooks/tauriApi.ts:142`）。
- **参数名**：前端一律传 camelCase，Tauri 层自动映射到 Rust 的 snake_case 参数。例：`invoke('get_adapters', { force })`（`tauriApi.ts:136`）、`invoke('enable_adapter', { adapterName })`（`tauriApi.ts:138`）。
- **返回形状**：通用三态 `CommandResult { success, message?, data? }`（桌面 `infra/state/mod.rs`；安卓在 `self_service_cmds.rs:7-23` 自带同形定义）。
- **自定义命令不需要 capability 声明**：`tauri-app/src-tauri/capabilities/default.json` 只列 core/notification 权限，`desktop.json` 只列 shell/global-shortcut/autostart，说明应用自有命令不受 Tauri 2 能力系统门控。

### 两端命令名不完全一致的三处

| 语义 | 桌面命令 | 安卓命令 | 前端胶水 |
|---|---|---|---|
| 身份验证门 | `verify_windows_identity` | `verify_biometric_identity` | `android/frontend/src/hooks/tauriApi.ts:180-211` 把 `verifyWindowsIdentity` 转调 |
| 开机自启 | `get_auto_launch` / `set_auto_launch` | `get_boot_autostart` / `set_boot_autostart` | `android/frontend/src/hooks/tauriApi.ts:253-254` 转调并包装返回值 |
| 配置读写 | `get_config` 返回 `Config` | `get_config` 返回 `serde_json::Value`（掩码后） | 无胶水，前端按同一 `Config` 类型消费 |

### 桌面独有 / 安卓独有命令

- **仅桌面注册**：`show_window`、`minimize_window`、`close_window`、`open_external`、`cancel_auto_exit`、`render_heartbeat`、`get_gpu_info`、`check_dns_doh_status`、`setup_dns_doh`、`get_adapters`、`get_adapter_details`、`get_disabled_adapters`、`enable_adapter`、`dhcp_*`（3 条）。
- **仅安卓注册**：`ping_test`（`protocol_cmds.rs:169`）、`bind_to_wifi`（`protocol_cmds.rs:175`）、`accept_wifi_network`（`protocol_cmds.rs:193`）、`detect_campus`（`campus_detect.rs:124`）、`get_soc_info`（`system_cmds.rs:71`）、`get_boot_autostart` / `set_boot_autostart`（`monitor_loop.rs:306/323`）、电池优化 3 条（`battery_cmds.rs:20/46/65`）。
- 其中 `ping_test`、`detect_campus`、`accept_wifi_network` **没有前端封装**，属后端内部/调试面。

### 前端封装覆盖关系

| 项 | 桌面 `tauri-app/frontend/src/hooks/tauriApi.ts` | 安卓 `android/frontend/src/hooks/tauriApi.ts` |
|---|---|---|
| `TauriApi` 接口成员总数 | 72（`27-100`） | 76（`56-133`） |
| 其中 invoke 型方法 | 56（= 桌面命令数，一一对应） | 61（= 56 同名 + 5 安卓独有） |
| 其中事件监听器 | 16（`153-216`） | 15（无 `onUpdateNotificationClick`） |
| 实现体 | `133-223` | `166-285` |
| 安卓独有 5 项 | — | `bindToWifi` / `getSocInfo` / `getBatteryOptimizationInfo` / `requestIgnoreBatteryOptimizations` / `openVendorBatterySettings` |
| 桌面专属但安卓仍声明 | — | 15 个用 `desktopOnly<T>(name)` 直接 reject（`android/.../tauriApi.ts:13-15`，清单见 `169-172/216-217/234-236/260/266/279-282`） |
| 桌面专属事件 | — | 8 个用 `noopListener`（`17-18`，清单见 `220-223/261-264`） |

安卓前端保持与桌面**同形接口**，是为了让 `App.tsx` / panels 等共享风格的组件代码两端可复用；差异被压缩在 `tauriApi.ts` 这一层。

### 事件总线

桌面唯一实现在 `tauri-app/src-tauri/src/infra/events.rs`：`EventBus<'a>` 持 `&AppHandle`（`8-10`），私有 `emit()` 做 `app_handle.emit(event, payload)`（`17-19`），对外 16 个 `emit_*` 方法。另有 `AppHandleExt`（`infra/command_context.rs:38-51`）暴露 `notify_config_changed` / `notify_update_download_progress` 两个快捷入口。

| 事件名 | `EventBus` 方法（`infra/events.rs`） | 实际发射点 | 桌面前端监听 | 安卓后端是否 emit |
|---|---|---|---|---|
| `login-log` | `22` | `auth/session.rs:27/124`、`auth/failure_tracker.rs:75/156/255`、`monitor/auto_auth.rs:162`、`monitor/latency.rs:45/48` | `useEventListeners.ts:263` | 是（`monitor_loop.rs:130-134`） |
| `background-check-result` | `30` | `monitor/background_emit.rs:132` | `useEventListeners.ts:77` | 是（`monitor_loop.rs:760`） |
| `auto-login-result` | `35` | `monitor/auto_auth.rs:80/191/250/317/403/441`、`app/tray.rs:62/71` | `useEventListeners.ts:199` | 是（`monitor_loop.rs:444-450`） |
| `network-quality-result` | `44` | `network/quality.rs:507/583/637`、`monitor/quality_scheduler.rs:35` | `useEventListeners.ts:327` | 是（复用共享 crate 的 `quality.rs`） |
| `update-available` | `49` | `update/updater.rs:285` | `useEventListeners.ts:335` | 是（`update_cmds.rs:251`） |
| `update-notification-click` | `58` | `platform/toast.rs:68` | `App.tsx:223` | 否 |
| `adapter-details-changed` | `63` | `monitor/adapter_watch.rs:79` | `useEventListeners.ts:242` | 否（noop） |
| `disabled-adapters-changed` | `68` | `monitor/adapter_watch.rs:86` | `useEventListeners.ts:248` | 否（noop） |
| `adapter-disabled-warning` | `73` | `monitor/adapter_watch.rs:136` | `useEventListeners.ts:254` | 否（noop） |
| `adapters-changed` | `112` | `monitor/adapter_watch.rs:75` | `useEventListeners.ts:215` | 否（noop） |
| `campus-exit-countdown` | `81` | `infra/lifecycle.rs:59` | `useEventListeners.ts:299` | 否（noop） |
| `campus-exit-cancelled` | `89` | `infra/lifecycle.rs:178` | `useEventListeners.ts:320` | 否（noop） |
| `auto-exit-countdown` | `94` | `infra/lifecycle.rs:204` | `useEventListeners.ts:271` | 否（noop） |
| `auto-exit-cancelled` | `102` | `infra/lifecycle.rs:285` | `useEventListeners.ts:292` | 否（noop） |
| `config-changed` | `107` | **唯一发射点** `commands/config_cmd.rs:16` | `useEventListeners.ts:348` | **否** |
| `update-download-progress` | `117` | `commands/updater.rs:150/181` | `AboutDialog.tsx:157`（安卓 `AboutDialogMobile.tsx:86`） | 是（`update_cmds.rs:355`） |

安卓没有 `events.rs`：`monitor_loop.rs` 直接 `use tauri::Emitter; app.emit(...)`，`update_cmds.rs` 同理，所以安卓事件名没有任何集中登记点。

### 新增一条命令要改哪几处

**桌面端**（若两端都要有，下面两组都要做）：

1. `tauri-app/src-tauri/src/commands/<领域>.rs` 写 `#[tauri::command] pub fn/async fn`。
2. 若是新文件：`tauri-app/src-tauri/src/commands/mod.rs:1-8` 加 `pub mod <文件>`。
3. `tauri-app/src-tauri/src/app/startup.rs:63-120` 的 `generate_handler!` 加一行 `crate::commands::<领域>::<fn>`。
4. `tauri-app/frontend/src/hooks/tauriApi.ts:27-100` 的 `TauriApi` 接口加成员签名。
5. `tauri-app/frontend/src/hooks/tauriApi.ts:133-223` 的实现对象加 `invoke<T>('<命令名>', { 参数 })`。
6. 调用点（`hooks/` 或对应 panel）；若该命令会改配置，走 `save_config_to_disk_encrypted` 以自动广播 `config-changed`（`commands/config_cmd.rs:9-18`）。

**安卓端**：

1. `android/src-tauri/src/<模块>.rs` 写 `#[tauri::command]`。
2. 若是新模块：`android/src-tauri/src/lib.rs:10-23` 加 `mod <模块>`。
3. `android/src-tauri/src/lib.rs:55-104` 的 `generate_handler!` 加一行。
4. `android/frontend/src/hooks/tauriApi.ts:56-133` 接口 + `166-285` 实现；若该能力桌面专属，实现写 `desktopOnly<T>('<name>')`（`13-15`）。
5. 若命令读写配置：走 `config_state::save_to` + 更新 `android_state::AndroidState.config` 缓存（参考 `monitor_loop.rs:331-344`），**注意安卓不会自动广播 `config-changed`**。

**新增一个事件**：

1. `tauri-app/src-tauri/src/infra/events.rs` 加 `emit_<x>` 方法（含注释说明业务场景）。
2. 业务处调用；前端 `tauriApi.ts` 加 `onXxx: createEventListener<T>('<event-name>')`（桌面 `153-216`）。
3. 安卓要单独在前端加监听、在 `monitor_loop.rs` / `update_cmds.rs` 里裸 `app.emit`；若希望安卓也发，则需在安卓侧手写发射。

## 关键约束

- **注册即全部**：命令不写进 `generate_handler!` 就等于不存在，`invoke` 会报 "command not found"。桌面唯一注册点 `startup.rs:63-120`，安卓唯一注册点 `lib.rs:55-104`。
- **两端命令名必须逐字相同**（除上文三处已知映射），否则共享风格的组件在另一端失效。
- **事件名是字符串常量，无编译期校验**：`events.rs` 的方法名与字符串分离（例：`emit_login_log` → `"login-log"`，`events.rs:22-27`），前端 `createEventListener('login-log')` 是又一处独立字面量，改名三处必须同步。
- **桌面事件必须经 `EventBus` 发**：`events.rs:6` 注释明确"封装所有 `app_handle.emit` 调用"，直接裸调会绕过该约定。
- **配置变更必须掩码后广播**：`commands/config_cmd.rs:13-16` 规定 `masked_for_display()` 后再发 `config-changed`；`get_config`（`config_cmd.rs:88`）与 `get_init_data`（`commands/system.rs:117`）同样必须掩码。
- **安卓 `get_init_data` 必须补齐桌面专属字段**：`android/src-tauri/src/system_cmds.rs:15-21` 把 `gpuInfo` 置 null、`adapters` 置空数组等，否则共享的 `useInitialDataLoad` 读到 `undefined` 会崩。
- **前端必须处理监听建立前的丢事件**：安卓 `status_value()`（`monitor_loop.rs:96-116`）把最近一次结果展平到顶层，供 `getInitData` 提供启动初值。

## Data Flow

```text
React 组件
  → useXxxStore / hooks
    → tauriApi.<method>()            (tauri-app|android/frontend/src/hooks/tauriApi.ts)
      → invoke('<snake_case>', { camelCaseArgs })
        → Tauri IPC
          → generate_handler! 分发    (startup.rs:63-120 | android lib.rs:55-104)
            → #[tauri::command] fn
              → CommandContext / AppState / android_state
              → 领域逻辑 (auth / network / monitor / self_service / update)

后端事件
  → EventBus::emit_xxx(payload)      (infra/events.rs，安卓为裸 app.emit)
    → app_handle.emit('<kebab-name>', payload)
      → 前端 createEventListener('<kebab-name>') 回调
        → useEventListeners.ts → zustand store → UI
```

## Connections

- [[desktop-commands]] — 桌面命令层的逐文件详解与本概念的实现侧
- [[android-backend]] — 安卓 48 条命令的模块分工
- [[desktop-infra]] — `EventBus` / `CommandContext` / 日志所在的基础设施层
- [[desktop-frontend-hooks]] — `tauriApi.ts` 与 `useEventListeners.ts` 的组织方式
- [[android-frontend]] — 安卓前端如何用 `desktopOnly` / `noopListener` 抹平命令面差异
- [[config-and-persistence]] — `config-changed` 事件与 `save_config` 落盘路径
- [[background-check-and-auto-login]] — `background-check-result` / `auto-login-result` 的载荷来源
- [[security-model]] — 命令返回值掩码与事件载荷的密码禁令

## Known Issues

- **安卓后端从不发射 `config-changed`**，但安卓前端仍注册了监听（`android/frontend/src/hooks/useEventListeners.ts:350`、`android/frontend/src/hooks/tauriApi.ts:265`）。安卓 `save_config`（`android/src-tauri/src/config_state.rs:311-338`）只落盘并更新内存缓存，不广播。后果：安卓端 `useConfigStore` 的 `mergeConfigFromBackend` / `dirtyFields` 合并逻辑（`android/frontend/src/hooks/useConfigStore.ts:108`）永远不触发，后端配置回写完全依赖命令返回值。
- **安卓 8 个事件监听是空壳**：`noopListener` 覆盖 4 个适配器事件与 4 个退出倒计时事件（`android/frontend/src/hooks/tauriApi.ts:220-223/261-264`），但对应的 `useEventListeners` 消费代码（`android/frontend/src/hooks/useEventListeners.ts:215-325`）仍在，属死代码。
- **文档与代码计数不一致**：旧的人工文档 `CODE_WIKI.md` 曾称安卓命令面"44 个"（其 §6.1.1），实际 `lib.rs` 注册 **48 条**（注册块 `lib.rs:55-104`，命令条目 `:56-103`）；`modules/desktop-commands.md` 原称安卓注册项 45、行号 54-100——那是 `battery_cmds` 三命令引入前的旧状态，2026-09-13 已修正为 48 项、注册块 55-104。计数核对须在主仓库工作区进行：滞后于 main 的分支/worktree 缺 `battery_cmds.rs`，会误测为 45 项。
- **`auto-login-result` 载荷两端字段数不同**：桌面 `emit_auto_login_result(success, message, skipped)` 发三字段（`events.rs:35-41`），安卓的同名事件只发 `{success, message}`（`monitor_loop.rs:444-450`），而前端会读 `result.skipped`（`useEventListeners.ts:202`）——安卓端 `skipped` 恒为 `undefined`，走 else 分支。
- **安卓 3 条命令无前端出口**：`ping_test`、`detect_campus`、`accept_wifi_network` 注册了但没有 `tauriApi` 封装，属"注册即暴露给 webview"的残留面。
- **`update-notification-click` 安卓无对应**：桌面靠 WinRT toast 点击回调（`platform/toast.rs:68`），安卓走 `large_icon` 通知（`monitor_loop.rs:139-156`）无点击回传，前端接口也因此少了该监听器。
