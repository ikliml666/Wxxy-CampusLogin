---
title: 校园网登录助手系统总览
type: architecture
source_files:
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/vpn_cmds.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/face_model_cmds.rs
  - android/plugins/network-bind/src/lib.rs
  - android/plugins/keystore/src/lib.rs
  - android/plugins/foreground-service/src/lib.rs
  - android/src-tauri/tauri.conf.json
  - android/src-tauri/src/config_state.rs (tests)
  - android/src-tauri/src/campus_detect.rs (tests)
  - android/src-tauri/src/monitor_loop.rs (tests)
  - android/src-tauri/src/account_cmds.rs (tests)
  - android/src-tauri/src/quality_cmds.rs (tests)
  - android/src-tauri/src/quality_history.rs (tests)
  - android/src-tauri/src/update_cmds.rs (tests)
  - android/src-tauri/src/login_history.rs (tests)
  - android/src-tauri/src/identity_gate.rs (tests)
  - android/src-tauri/src/vpn_cmds.rs (tests)
  - android/src-tauri/src/self_service_cmds.rs (tests)
  - android/src-tauri/src/face_model_cmds.rs (tests)
  - android/src-tauri/src/cpu_affinity.rs (tests)
  - tauri-app/src-tauri/Cargo.toml
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/build.rs
  - tauri-app/src-tauri/tauri.conf.json
  - tauri-app/src-tauri/src/app/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/infra/mod.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/store.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/config/mod.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/auth/service.rs
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/network/mod.rs
  - tauri-app/src-tauri/src/network/client.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/vpn/mod.rs
tags:
  - architecture
  - system-overview
  - campus-login
  - tauri
  - android
  - desktop
  - rust
  - cross-platform
---

## Overview

校园网登录助手（CampusLogin）是一个跨平台（Android 移动端 + Windows 桌面端）的校园网认证与网络质量监控工具。系统由两套独立的 Tauri v2 应用工程构成：`android/`（安卓端）与 `tauri-app/`（桌面端），二者通过 Cargo path 依赖同一个主仓库的跨平台核心 crate `campus_login_lib`（内含 `auth::protocol`、`auth::portal`、`self_service`、`vpn`、`network::quality`、`config::schedule` 等纯函数），严禁在端侧复制协议逻辑。本文面向零上下文的未来 agent，汇总两工程模块文章（见 `source_files`）中的职责、连接与数据流，并给出三张架构框图。

## System Overview

以下总框图展示两个端工程与共享核心的分层结构：

```text
┌──────────────────────────── Android 端 (android/) ────────────────────────────┐
│ ┌────────────── Rust 后端 (android/src-tauri/src/*.rs) ──────────┐            │
│ │ lib.rs:33 run() 装配 invoke_handler! 命令面                     │            │
│ │ android_state.rs / config_state.rs (Settings)                  │            │
│ │ campus_detect.rs / protocol_cmds.rs (登录注销)                 │            │
│ │ monitor_loop.rs (后台保活检测) / account_cmds.rs               │            │
│ │ system_cmds.rs / battery_cmds.rs / quality_cmds.rs            │            │
│ │ update_cmds.rs / login_history.rs / identity_gate.rs          │            │
│ │ vpn_cmds.rs / self_service_cmds.rs / face_model_cmds.rs       │            │
│ │ cpu_affinity.rs (小核绑核)                                     │            │
│ └───────────────────────────┬───────────────────────────────────┘            │
│                             │ 调用                                            │
│ ┌────────────── 移动插件 (#[cfg(mobile)]) ──────────────┐                    │
│ │ network-bind (绑 WiFi) / keystore (AES-GCM)           │                    │
│ │ foreground-service (FGS 保活/开机自启/APK安装)        │                    │
│ └──────────────────────────────────────────────────────┘                    │
│                             │ 经 hooks/tauriApi.ts IPC                       │
│ ┌────────────── React 前端壳 (android/frontend) ─────────┐                    │
│ │ 手机双外壳 + 平板 Dock，复用桌面件                      │                    │
│ └──────────────────────────────────────────────────────┘                    │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       │ 依赖跨平台纯函数 (cargo path)
                                       ▼
┌───────────────── 共享核心 crate: campus_login_lib ───────────────────────────┐
│ auth::protocol / auth::portal / self_service / vpn / network::quality        │
│ config::schedule 等 (无平台 API 依赖)                                        │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       │ 同样依赖
                                       ▼
┌──────────────────────────── Desktop 端 (tauri-app/) ─────────────────────────┐
│ ┌────────────── Rust 后端 (tauri-app/src-tauri/src/*) ──────┐                │
│ │ main.rs:17 → app/startup.rs:23 run() 装配 generate_handler!│                │
│ │ infra/state (AppState, ConfigStore) / commands/*          │                │
│ │ config/* / auth/service.rs / monitor/watcher.rs           │                │
│ │ network/* / account/crypto.rs (DPAPI) / update/updater.rs│                │
│ │ vpn/mod.rs (WebVPN 核心)                                  │                │
│ └───────────────────────────┬───────────────────────────────┘                │
│                             │ 提权子进程 / 无头核心                           │
│ ┌────────────── 辅助二进制 ──────────────┐ ┌──────────────────────────────┐  │
│ │ helper (elevated subproc, netsh/MAC)  │ │ campus-login-core (SYSTEM任务)│  │
│ └───────────────────────────────────────┘ └──────────────────────────────┘  │
│                             │ WebView2 IPC                                   │
│ ┌────────────── React+TS 前端 (tauri-app/frontend) ─────────┐                │
│ │ SPA 通过 Tauri v2 IPC 调用 commands/*                     │                │
│ └──────────────────────────────────────────────────────────┘                │
└─────────────────────────────────────────────────────────────────────────────┘
```

系统由两个独立的 Tauri 应用组成，命令名/参数刻意对齐使前端近乎零适配；Android 端强制流量走 WLAN、密码用 AndroidKeyStore，桌面端用 DPAPI 与 Windows 计划任务/提权子进程。

## Data Flow

下图描述一次典型登录请求与后台事件的数据流（以 Android 与 Desktop 对应命令为例）：

```text
前端 (Android/frontend 或 Desktop/frontend)
   │ invoke("doLogin" / "do_login")
   ▼
┌─────────────────── Tauri 后端命令层 ────────────────────────┐
│ Android: protocol_cmds::do_login (android/src-tauri/src/protocol_cmds.rs:10)  │
│   → ensure_wifi_bound (protocol_cmds.rs:269) 绑进程到 WiFi                   │
│ Desktop: commands/login::do_login (tauri-app/src-tauri/src/commands/login.rs:63)│
│   → spawn_blocking → auth::service::full_login (auth/service.rs:81)          │
└───────────────────────────┬───────────────────────────────────┘
                            │ 调用跨平台纯函数
                            ▼
┌─────────────────── campus_login_lib ───────────────────────┐
│ auth::protocol::do_login_with_retry / portal 探测          │
│ (Android 经 campus_detect 复用 is_subnet_18 等)            │
└───────────────────────────┬─────────────────────────────────┘
                            │ HTTP/TCP 请求 (SO_BINDTODEVICE / 适配器)
                            ▼
                  校园网 Portal / WebVPN 网关
                            │
                            │ emit 事件回流
                            ▼
┌─────────────────── EventBus / invoke_handler ─────────────┐
│ Android: monitor_loop emit background-check-result / login-log│
│ Desktop: infra/events.rs emit_login_log / emit_background_check_result │
└───────────────────────────┬─────────────────────────────────┘
                            ▼
                  前端监听更新 UI (toast / 状态卡)
```

配置写路径：前端 `saveConfig` → 后端 `save_config` (`android/src-tauri/src/config_state.rs:491` 或 `tauri-app/src-tauri/src/commands/config_cmd.rs:8`) → 加密落盘（Keystore / DPAPI）→ 广播 `config-changed` → 前端刷新。账号、质量历史、登录历史均类似经原子写与头插法持久化。

## Dependency Graph

下图展示模块间依赖方向（箭头表示“依赖于/调用”）：

```text
┌────────────────┐    ┌────────────────────┐    ┌────────────────────────┐
│ Android 前端   │───►│ Android 后端命令   │───►│                        │
└────────────────┘    │ (lib.rs 注册)      │    │                        │
       │              └─────────┬──────────┘    │  campus_login_lib      │
       │                        │               │  (跨平台纯函数核心)    │
┌────────────────┐    ┌────────▼──────────┐    │                        │
│ Desktop 前端   │───►│ Desktop 后端命令  │───►│                        │
└────────────────┘    │ (generate_handler)│    └───────────┬────────────┘
                      └─────────┬──────────┘                │
                                │                           │
                                ▼                           ▼
                      ┌──────────────────┐          ┌────────────────────┐
                      │ 平台专属能力      │          │ 外部网络实体       │
                      │ Android plugins  │          │ Portal / WebVPN /  │
                      │ Windows helper    │          │ Radius / ICMP      │
                      │ campus-login-core │          └────────────────────┘
                      └──────────────────┘

依赖规则：
  ► 前端依赖后端命令契约 (invoke_handler!/generate_handler! 注册的同名命令)
  ► 后端命令依赖 campus_login_lib 纯函数 (禁止复制协议)
  ► Android 后端额外依赖 #![cfg(mobile)] 插件封装的 Android Java/Kotlin 能力
  ► Desktop 后端依赖 Windows API (DPAPI/计划任务/helper 提权)
  ◄ 核心库不反向依赖任何端侧模块 (零平台耦合)
```

## Module Roles

### Android 端模块角色（依据 `modules/android.md`）

- **入口与生命周期** (`android/src-tauri/src/lib.rs:33`)：`run()` 挂载移动插件、`manage(AndroidState)`、`setup` 初始化日志与启动恢复，`ExitRequested` 中 `prevent_exit()` 保活。
- **全局状态** (`android_state.rs:16` `AndroidState` 存源 IP/配置；`config_state.rs:26` `Settings` 全量配置，含 `CryptoBridge` 接 Keystore 加解密)。
- **校园网探针** (`campus_detect.rs:65` `probe_campus` 枚举 IPv4、TCP 探 Portal、SSID 判定)。
- **协议命令** (`protocol_cmds.rs:10` `do_login`/`do_logout` 复用 `campus_login_lib::auth::protocol`，强制 `ensure_wifi_bound` 绑 WiFi)。
- **后台监控** (`monitor_loop.rs:16` `MONITOR` 状态机，`run_check_once` 掉线重登、夜间出站/运营商切换、定时动作，前台服务保活由插件负责)。
- **账号/系统/电池** (`account_cmds.rs` 多账号 CRUD；`system_cmds.rs` 聚合 `get_init_data`；`battery_cmds.rs` 跳电池优化页)。
- **质量/历史/更新** (`quality_cmds.rs` + `quality_history.rs` 网络质量；`update_cmds.rs` 版本检查/APK 安装；`login_history.rs` 登录历史)。
- **身份/WebVPN/自助** (`identity_gate.rs` 生物识别 TTL 门；`vpn_cmds.rs` + `self_service_cmds.rs` 自助服务与 WebVPN 自动共存路由，依赖 `campus_login_lib::self_service`/`vpn`)。
- **人脸模型/CPU亲和** (`face_model_cmds.rs` 按需下模型；`cpu_affinity.rs` 绑小核)。
- **移动插件** (`plugins/network-bind` 绑进程到 WLAN；`plugins/keystore` AndroidKeyStore AES-GCM；`plugins/foreground-service` FGS 保活/开机自启/APK 安装)，均 `#![cfg(mobile)]` host 编译剔除。
- **前端壳** (`android/frontend` 子模块)：`tauriApi.ts` 命令契约层，桌面专属命令 reject 为 `desktopOnly`，双外壳布局。

### Desktop 端模块角色（依据 `modules/tauri-app.md`）

- **进程入口** (`main.rs:17` 拦截提权模式、设 WebView2 参数；`app/startup.rs:23` 装配插件与 `setup_app`)。
- **共享状态** (`infra/state/mod.rs:184` `AppState` 聚合 `ConfigStore`(ArcSwap)、`TaskFlags`、`NetworkState` 等；`infra/command_context.rs:10` `CommandContext` 提供统一访问；`infra/events.rs:8` `EventBus` 推送事件)。
- **配置模型** (`config/model.rs:10` `Config` 全字段，密码 DPAPI 加密、出站 `masked_for_display`；`config/persist.rs` 原子写与账号档案加解密)。
- **命令层** (`commands/login.rs` 登录注销；`commands/account.rs` 账号 CRUD；`commands/config_cmd.rs` 落盘广播)。
- **认证/监控/网络** (`auth/service.rs:81` `full_login` 编排；`monitor/watcher.rs:16` 启动 5 类后台任务；`network/client.rs` 客户端池与 Portal URL)。
- **凭据加密** (`account/crypto.rs` Windows DPAPI 加解密)。
- **更新/WebVPN** (`update/updater.rs` 版本镜像降级；`vpn/mod.rs` 深澜 Srun WebVPN 核心，校外回退)。
- **辅助进程** (`campus-login-core` 无头 SYSTEM 任务；`helper` 提权子进程处理 MAC/DNS/适配器)。
- **前端** (`tauri-app/frontend` 子模块)：WebView2 SPA 经 IPC 调命令。

两工程通过一致的命令契约与共享 `campus_login_lib` 保持协议同源；Android 以插件弥补移动平台限制，Desktop 以 Windows 专属子进程/计划任务覆盖桌面场景。未来 agent 修改协议逻辑时必须只改核心 crate，端侧仅调用。