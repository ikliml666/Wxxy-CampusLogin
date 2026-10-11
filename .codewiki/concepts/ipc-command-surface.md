---
title: IPC 命令面与事件总线
type: concept
source_files:
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/app/tray.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/capabilities/default.json
  - android/src-tauri/src/battery_cmds.rs
tags: [概念, ipc, tauri, 命令, 事件总线]
---

# IPC 命令面与事件总线

## Overview

桌面端（Windows）通过 Tauri IPC 暴露 **65 条命令**，安卓端暴露 **53 条命令**。两端共享约 38 条同名命令，另有 3 对跨端映射命令。前端通过 `tauriApi.ts` 封装 invoke 调用，后端通过事件总线（桌面经 `EventBus`，安卓直接 `app.emit`）向前端推送状态变化。

- **桌面 65 条命令**：由 `app/startup.rs:62-128` 的 `generate_handler!` 注册，分布在 `commands/` 目录 8 个模块及 `infra/logger.rs`。
- **安卓 53 条命令**：由 `android/src-tauri/src/lib.rs:57-111` 注册，分布在 15 个模块。
- **桌面事件总线**：`infra/events.rs` 提供 18 个 `emit_*` 方法，是桌面后端向前端发事件的唯一正规出口。
- **安卓事件**：无统一 `events.rs`，直接 `use tauri::Emitter` 裸调 `app.emit`；真实发射的事件是桌面的子集。

## Key Components

### 桌面命令模块 (`tauri-app/src-tauri/src/commands/`)

#### `config_cmd.rs` (9 条)
| 函数 | 行号 | 用途 |
| --- | --- | --- |
| `save_config_to_disk_encrypted` | 10-17 | 落盘配置 + 掩码广播 `config-changed` 事件；异步重建托盘菜单 |
| `load_config_from_file` | 19-47 | 读取并解密配置文件；失败时保留其他字段 |
| `load_config_from_disk_or_default` | 49-78 | 加载配置或默认值；解析失败时备份损坏文件 |
| `build_config_export_payload` | 81-101 | 组装导出 JSON（掩码态或 DPAPI 密文态） |
| `restore_imported_password_field` | 104-120 | 还原导入密码（密文解密、空/MASK 保留当前值） |
| `export_config` | 123-142 | 导出配置到 `exports/` 目录 |
| `import_config` | 145-200 | 导入配置（含大小限制、JSON 校验、密码还原、状态丢弃） |
| `show_window` | 203-206 | 显示或重建主窗口 |
| `get_config` | 209-211 | 返回掩码后配置 |
| `save_config` | 214-252 | 验证、同步全局 Portal URL/日志天数、落盘、更新内存、自动建号 |

**测试**: `masked_for_display_masks_password` (255-268)、`export_payload_never_contains_plaintext_password` (272-294)、`import_password_restore_semantics` (298-321)。

#### `system.rs` (17 条)
| 函数 | 行号 | 用途 |
| --- | --- | --- |
| `minimize_window` | 8-14 | 根据 `minimize_to_tray` 配置隐藏或最小化 |
| `close_window` | 17-31 | 优先隐藏到托盘；轻量化模式下关闭 |
| `open_external` | 34-45 | 打开外部 URL（http/https 校验、长度限制、凭据检查） |
| `get_auto_launch` | 48-51 | 获取开机自启状态 |
| `set_auto_launch` | 54-78 | 设置开机自启（先注册表后配置） |
| `get_notification_enabled` | 81-83 | 获取通知开关 |
| `set_notification_enabled` | 86-95 | 设置通知开关 |
| `cancel_auto_exit` | 98-103 | 取消自动退出和校园网退出 |
| `get_logs` | 106-109 | 读取最近 N 行日志 |
| `clear_logs` | 112-115 | 清空日志 |
| `get_init_data` | 118-152 | 初始化数据（掩码配置、账号列表、GPU、适配器快照等） |
| `render_heartbeat` | 155-165 | 更新前端心跳时间戳 |
| `get_gpu_info` | 168-171 | 检测 GPU 信息 |
| `set_log_retention_days` | 174-177 | 设置日志保留天数 |
| `get_log_retention_days` | 180-182 | 获取日志保留天数 |
| `export_diagnostics` | 185-270 | 导出诊断包（日志、掩码配置、适配器、GPU、manifest） |
| `notify_window_ready` | 273-276 | 前端就绪信号（轻量化重建用） |

#### `network_cmd.rs` (18 条)
| 函数 | 行号 | 用途 |
| --- | --- | --- |
| `get_adapters` | 22-28 | 获取适配器列表（可选强制刷新） |
| `get_disabled_adapters` | 31-33 | 获取禁用适配器列表 |
| `enable_adapter` | 36-45 | 启用适配器（允许 COM 提权失败降级） |
| `get_adapter_details` | 48-50 | 获取适配器详情 |
| `outbound_switch_now` | 53-62 | 手动立即切换出站 |
| `outbound_restore_now` | 65-74 | 手动立即还原出站 |
| `get_current_outbound_name` | 77-88 | 获取当前真实出站接口别名 |
| `check_campus_status` | 91-110 | 检查校园网状态（SSID、有线/无线） |
| `check_portal_status` | 113-138 | 检查 Portal 在线状态（含注销保护期） |
| `dhcp_renew_all` | 157-174 | DHCP 续租（仅限有线适配器） |
| `dhcp_release_renew` | 177-198 | DHCP 释放/续租（主/副适配器） |
| `dhcp_release_renew_adapter` | 201-216 | 单适配器 DHCP 释放/续租 |
| `check_network_quality` | 219-250 | 网络质量检测（含节流、历史落盘） |
| `start_latency_test` | 253-273 | 启动延迟测试循环（校验总开关、持久化） |
| `stop_latency_test` | 276-285 | 停止延迟测试循环 |
| `check_dns_doh_status` | 288-298 | 检测 DNS/DoH 状态 |
| `setup_dns_doh` | 301-365 | 一键设置 DNS+DoH（支持管理员/非管理员提权） |
| `reset_dns` | 368-445 | 恢复 DNS 自动获取（支持管理员/非管理员提权） |

#### `background.rs` (4 条)
| 函数 | 行号 | 用途 |
| --- | --- | --- |
| `start_background_check` | 8-12 | 启动后台检测 |
| `stop_background_check` | 15-26 | 停止后台检测并持久化配置 |
| `trigger_background_check` | 29-41 | 手动触发一次后台检测 |
| `get_background_status_value` | 44-102 | 计算后台状态快照（需调用方传入 adapters 避免重复取） |
| `get_background_status` | 105-109 | 获取后台状态（内部调用 get_background_status_value） |

#### `tray.rs` (托盘菜单与事件处理)
| 函数/常量 | 行号 | 用途 |
| --- | --- | --- |
| `TRAY_ID` | 11 | 托盘图标 ID 常量 |
| `SWITCH_ITEM_PREFIX` | 14 | 切换账号子菜单项 ID 前缀 |
| `build_tray_menu` | 17-62 | 构建托盘菜单（快速登录/注销、切换账号子菜单） |
| `refresh_tray_menu_state` | 65-76 | 异步重建并应用托盘菜单 |
| `build_tray` | 79-102 | 构建并注册托盘图标与菜单 |
| `handle_tray_menu_event` | 105-183 | 处理托盘菜单事件（显示、快速登录/注销、切换账号、退出） |
| `handle_tray_icon_event` | 186-193 | 处理托盘图标点击事件（左键显示主窗口） |

### 基础设施

#### `infra/events.rs` (18 个 emit 方法)
| 事件 | events.rs 行号 | payload | 后端发射点 |
| --- | --- | --- | --- |
| `login-log` | 22-27 | `{message, type}` | auth/session.rs, monitor/auto_auth.rs, commands/login.rs 等 |
| `background-check-result` | 30-32 | BackgroundStatus 快照 | monitor/background_emit.rs:136 |
| `auto-login-result` | 35-41 | `{success, message, skipped}` | monitor/auto_auth.rs, app/tray.rs |
| `network-quality-result` | 44-46 | NetworkQuality | network/quality.rs, monitor/quality_scheduler.rs |
| `update-available` | 49-55 | `{hasUpdate, latestVersion, releaseNotes}` | update/updater.rs:354 |
| `update-notification-click` | 58-60 | `{}` | platform/toast.rs:119 |
| `adapter-details-changed` | 63-65 | AdapterDetail[] | monitor/adapter_watch.rs:92 |
| `disabled-adapters-changed` | 68-70 | DisabledAdapter[] | adapter_watch.rs:99 |
| `adapter-disabled-warning` | 73-78 | `{name, message}` | adapter_watch.rs:160 |
| `campus-exit-countdown` | 81-86 | `{minimizeDelay, exitDelay}` | infra/lifecycle.rs:59 |
| `campus-exit-cancelled` | 89-91 | `{}` | infra/lifecycle.rs:178 |
| `auto-exit-countdown` | 94-99 | `{delay, shortcut}` | infra/lifecycle.rs:204 |
| `auto-exit-cancelled` | 102-104 | `{}` | infra/lifecycle.rs:285 |
| `config-changed` | 107-109 | `{config: 掩码后配置}` | config_cmd.rs:8 (save_config_to_disk_encrypted) |
| `adapters-changed` | 112-114 | Adapter[] | adapter_watch.rs:88 |
| `update-download-progress` | 117-119 | 下载进度 | commands/updater.rs:147/178 |

#### `infra/command_context.rs`
| 类型/函数 | 行号 | 用途 |
| --- | --- | --- |
| `CommandContext<'a>` | 11-15 | 命令层公共上下文（AppHandle + AppState 引用） |
| `from_app` | 24-27 | 从 AppHandle 便捷构造 CommandContext |
| `Deref` impl | 30-32 | 让 CommandContext 可直接访问 AppState 字段 |
| `AppHandleExt` trait | 35-37 | AppHandle 扩展 trait（notify_config_changed, notify_update_download_progress） |
| `AppHandleExt` impl | 39-51 | 委托给 EventBus 实现 |

#### `capabilities/default.json`
- 声明权限：`core:default`, `core:window:allow-start-dragging`, `core:window:allow-toggle-maximize`, `notification:default` + allow-is-permission-granted/allow-request-permission/allow-notify。
- **所有自有命令不经能力系统**，注册即可调用。

### 安卓命令模块 (`android/src-tauri/src/battery_cmds.rs`)
| 函数 | 行号 | 用途 |
| --- | --- | --- |
| `get_battery_optimization_info` | 19-41 | 查询电池优化白名单状态（仅安卓） |
| `request_ignore_battery_optimizations` | 45-61 | 申请加入电池优化白名单（仅安卓） |
| `open_vendor_battery_settings` | 64-81 | 跳转厂商自启/省电页（仅安卓） |
| `open_notification_settings` | 86-102 | 跳转应用通知设置页（仅安卓） |

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Frontend (React)                               │
│  ┌──────────────┐  ┌──────────────────┐  ┌──────────────────────────────┐  │
│  │ tauriApi.ts  │  │ useEventListeners│  │ zustand store               │  │
│  │ (invoke封装) │  │ (事件订阅/处理)   │  │ (状态管理)                   │  │
│  └──────┬───────┘  └────────┬─────────┘  └──────────────┬───────────────┘  │
│         │                   │                           │                  │
└─────────┼───────────────────┼───────────────────────────┼──────────────────┘
          │                   │                           │
          ▼                   ▼                           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Tauri Backend                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                        generate_handler!                             │  │
│  │  (startup.rs:62-128 / lib.rs:57-111)                                 │  │
│  │  注册所有 #[tauri::command] 函数                                      │  │
│  └───────────────────────────┬───────────────────────────────────────────┘  │
│                              │                                             │
│  ┌──────────────┐  ┌───────┴───────┐  ┌────────────────────────────────┐  │
│  │  commands/   │  │   infra/      │  │       app/                     │  │
│  │  ├──config    │  │  ├──events.rs │  │  ├──tray.rs (托盘菜单/事件)    │  │
│  │  ├──system    │  │  └──cmd_ctx  │  │  ├──window.rs                 │  │
│  │  ├──network   │  │              │  │  ├──shutdown.rs               │  │
│  │  ├──account   │  │              │  │  └──lightweight.rs            │  │
│  │  ├──background│  │              │  │                                │  │
│  │  └──...       │  │              │  │                                │  │
│  └──────────────┘  └───────────────┘  └────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

**配置写路径（桌面）**：
```text
React 组件 → zustand store（debounce 聚合）→ tauriApi.saveConfig
  → invoke('save_config') → save_config (config_cmd.rs:214-252)
    → validate_config → save_config_to_disk_encrypted (config_cmd.rs:10)
          → persist::save_config_and_broadcast (加密落盘 + EventBus::emit_config_changed)
          → refresh_tray_menu_state (异步重建托盘菜单)
    → state.config.store (更新内存)
    → auto_create_account_for_current (R2 自动建号)
  → 前端 onConfigChanged (tauriApi.ts:226)
  → useEventListeners.ts:354 → mergeConfigFromBackend → store → UI
```

**状态读路径**：UI 挂载 → `get_init_data` 一次拉全（config 掩码 + accounts + 快照状态）→ 渲染初值；此后事件做增量。

**监控推送路径**：桌面 monitor 体系（scheduled.rs / background_check.rs / auto_auth.rs / adapter_watch.rs）→ `EventBus::emit_*` → 前端 `createEventListener` → `useEventListeners` 各 handler → zustand → UI。

## Connections

- [[desktop-commands]] —— 桌面 65 条命令在各 commands/*.rs 的领域实现与校验规则。
- [[desktop-infra]] —— AppState/CommandContext/EventBus/AppHandleExt/logger 等基础设施。
- [[desktop-frontend-hooks]] —— 桌面 tauriApi.ts / useEventListeners.ts / zustand store 全家桶。
- [[config-and-persistence]] —— 配置加密落盘、掩码出站、导入导出与双端同步。
- [[background-check-and-auto-login]] —— 后台检测循环与自动登录，是 login-log/auto-login-result/background-check-result 的主要生产者。
- [[security-model]] —— DPAPI/keystore 加密、凭据掩码、export/import 的安全边界。

## Known Issues

1. **安卓部分功能缺失**：安卓端无 `events.rs`，部分事件（如适配器族、退出倒计时族）在安卓前端使用 `noopListener` 桩；部分桌面专属命令（如 `exportConfig`、`getAdapters`）在安卓前端为 `desktopOnly` 桩。
2. **事件名无编译期校验**：`events.rs` 方法内的事件名、前端 `createEventListener('...')` 的字符串、安卓裸 emit 的字面量，三处必须手工同步。
3. **安卓 `get_init_data` 需补齐桌面专属字段**：`system_cmds.rs:16-22` 注释指出需补齐 `autoLaunch`、`gpuInfo: null`、`refreshRate: 60`、`adapters/adapterDetails/disabledAdapters` 空数组，否则前端可能崩溃。