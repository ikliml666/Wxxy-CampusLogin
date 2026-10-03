---
title: 双端同构与协议单点共享
type: concept
tags:
  - 概念
  - 双端同构
  - 协议共享
  - cfg门控
  - cargo path依赖
source_files:
  - tauri-app/src-tauri/Cargo.toml
  - android/src-tauri/Cargo.toml
  - tauri-app/src-tauri/src/lib.rs
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/build.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/network/mod.rs
  - tauri-app/src-tauri/src/config/mod.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/update/updater.rs
  - tauri-app/src-tauri/src/infra/mod.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - tauri-app/frontend/src/settings/types.ts
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/cpu_affinity.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/build.rs
  - android/plugins/keystore/Cargo.toml
  - android/plugins/keystore/src/lib.rs
  - android/plugins/network-bind/src/lib.rs
  - android/plugins/foreground-service/src/lib.rs
  - android/frontend/src/settings/types.ts
---

# 双端同构与协议单点共享

## Overview

本项目用同一套登录协议服务两个前端：桌面（Windows）与安卓。**协议单点在桌面 crate `campus-login`**（`tauri-app/src-tauri`，v2.3.9），安卓壳 crate `campus-login-android`（`android/src-tauri`，v2.3.9）通过 Cargo path 依赖直接复用其模块（`android/src-tauri/Cargo.toml:33-34`），不复制任何协议代码。安卓端只补四类平台件：平台探针（`campus_detect.rs`）、监控与状态编排（`monitor_loop.rs` + `config_state.rs`/`android_state.rs`）、命令包装层（`*_cmds.rs`）与落盘记录（`login_history.rs`/`quality_history.rs`），再加三个 Tauri 插件（network-bind / keystore / foreground-service）承接 Kotlin 平台能力。前端为双复刻树：两端各自维护一份 `settings/types.ts`（桌面 60 字段 `tauri-app/frontend/src/settings/types.ts:9-87`，安卓 47 字段 `android/frontend/src/settings/types.ts:9-73`），与各自 Rust 配置结构同形。

## 机制说明

### 共享边界：Cargo path 依赖配置

- 桌面 crate：`tauri-app/src-tauri/Cargo.toml:3` `campus-login v2.3.9`；lib name `campus_login_lib`，crate-type `cdylib + rlib`（`Cargo.toml:8-10`）——rlib 是被安卓 path 依赖消费的前提。
- 安卓 crate：`android/src-tauri/Cargo.toml:3` `campus-login-android v2.3.9`；`:34` `campus-login = { path = "../../tauri-app/src-tauri" }`（`:33` 注释「跨平台协议核心」）。
- 安卓专属联网依赖挂在 `target.'cfg(target_os = "android")'` 段（`android/src-tauri/Cargo.toml:46-50`：socket2 0.6 / libc 0.2 / network-interface 2），支撑 bound_socket 的 SO_BINDTODEVICE 旁路；桌面不引入。
- 桌面专属插件挂在 `cfg(not(android, ios))` 段（`tauri-app/src-tauri/Cargo.toml:53-57`：shell / autostart / global-shortcut / single-instance），安卓编译不可见。
- release profile 不共享：桌面 `[profile.release]` 在 `tauri-app/src-tauri/Cargo.toml:89-94`（thin LTO）；安卓在 workspace 根（fat LTO + codegen-units=1，`android/src-tauri/Cargo.toml:53-60`，注释说明桌面 profile 不随 path 依赖生效）。
- 版本注入双轨：桌面由 `tauri-app/src-tauri/build.rs:12-37` 解析 tauri.conf.json 并注入 `APP_VERSION`（`startup.rs:189`、`commands/system.rs:124` 消费）；安卓直接用 `env!("CARGO_PKG_VERSION")`（`system_cmds.rs:15`、`update_cmds.rs:278`）。安卓 `build.rs` 仅 3 行 `tauri_build::build()`（`android/src-tauri/build.rs:1-3`）。

### 哪些是共享的（桌面 crate 的模块表）

`tauri-app/src-tauri/src/lib.rs:2-8` 七个跨平台 `pub mod` 是安卓端唯一可见面（`:1` 注释「跨平台协议核心：安卓端(path 依赖)唯一可见面」）：

| 模块 | 桌面职责 | 安卓实际消费 |
| --- | --- | --- |
| `account` | 密码 DPAPI 加密（`account/crypto.rs:1-112`，仅 Windows） | 不消费加密路径；以 keystore 插件同构替代（`crypto.rs:132-141` 非平台存根显式 Err） |
| `auth` | 跨平台 `failure_tracker`/`portal`/`protocol`（`auth/mod.rs:1-3`）；桌面门控 dual_adapter_executor/session/service（`:6-11`） | `protocol::do_login_with_retry`/`do_logout_with_retry`（`protocol_cmds.rs:76-83/:112-117`）、`portal::check_portal_full`（`protocol_cmds.rs:155`） |
| `config` | Config 模型/校验/迁移/定时/夜切/出站 | `should_fire_scheduled_action`（`monitor_loop.rs:904-909`）、`evaluate_night_switch`+`parse_chkstatus`+`uid_matches`（`monitor_loop.rs:856-862/:1232-1234`）、`evaluate_night_outbound`（`monitor_loop.rs:759-778`）、字段语义与哨兵值 |
| `infra` | 日志/通知/生命周期/事件等（`infra/mod.rs:1-8`） | logger 全套：`read_recent_logs`/`clear_logs`/保留天数/debug 开关（`system_cmds.rs:34/:42/:49/:54`）、`log_info!`/`log_warn!` 宏（`protocol_cmds.rs:293/:319`）、安卓日志目录分支（`infra/logger.rs:385-394` app_data_dir/logs） |
| `network` | 适配器/DHCP/DNS 等桌面网络面（`network/mod.rs:1-13`） | `bound_socket::tcp_connect_bounded`（`campus_detect.rs:48`）、`bind_capability`（`protocol_cmds.rs:269-274`）、`vpn_tun_present`（`monitor_loop.rs:185`）、`client::clear_client_pool`（`protocol_cmds.rs:286-290`）、`quality` 全套（`quality_cmds.rs:9`） |
| `platform` | 12 个桌面子模块（`platform/mod.rs:5-36`） | 仅跨平台 `console_output`（`platform/mod.rs:1-2`） |
| `self_service` | 六个协议核心函数 + 纯函数校验 | 逐一同构包装：`bind_operator`/`query_bind_status`/`query_dashboard`/`query_online_log`/`offline_session`/`reveal_credential`（`self_service_cmds.rs:107/:128/:160/:208/:236/:263`）、`is_iso_date`（`:170-183`） |

### 哪些是各端独有

桌面独有（`#[cfg(desktop)]` 或仅桌面编译）：

- 五个壳模块（`tauri-app/src-tauri/src/lib.rs:11-20`）：`app`（`app/startup.rs`：build_runtime `:6-20`、run 插件注册与 65 命令 invoke_handler `:25-128`、setup_app `:152-249`）、`commands`（如 `commands/system.rs` get_init_data `:112-155`、export_diagnostics `:195-280`）、`helper`（`main.rs:30-46` --helper-task 拦截）、`monitor`、`update`（`update/updater.rs` 651 行 exe/msi 更新器）。
- platform 12 个桌面子模块（`platform/mod.rs:5-36`：autostart/ecoqos/dns_config/elevation/gpu/helper_spawn/task_proxy/identity/metric/best_route/icmp_probe/rtss_compat/toast）。
- auth 桌面门控三模块（`auth/mod.rs:6-11`）；网络桌面/Windows 模块（`network/mod.rs:22-23` dhcp 仅 Windows、`:48-49` enable_adapter 仅桌面）。
- 桌面生命周期件：托盘/单实例/GPU 预热/RTSS 预检（`startup.rs:214-247`、`main.rs:51-68`）、轻量化退出拦截（`startup.rs:135-149`）、校园网离校窗口（`infra/lifecycle.rs:17-74`）。

安卓独有（不进桌面编译面）：

- 状态与配置壳：`config_state.rs`（Settings 47 字段 `:15-111`、CryptoBridge `:186-223`、编解码/迁移 v0→v7/掩码 `:234-403`、get/save_config `:405-453`、测试 `:476-765`）、`android_state.rs`（AndroidState 源 IP + 配置双缓存 `:8-13`）。
- 监控编排：`monitor_loop.rs`（1607 行；tick 循环 `:580-615`、单次检测 `run_check_once :1245-1471`、定时/夜切 `run_scheduled_actions :748-950`、WiFi 事件 `:290-329`、小核探针线程 `:697-734`）。
- 平台探针：`campus_detect.rs`（/18 子网 + Portal TCP + SSID 三层判定 `:66-128`）。
- 命令包装层：`protocol_cmds.rs`/`self_service_cmds.rs`/`account_cmds.rs`/`system_cmds.rs`/`quality_cmds.rs`/`update_cmds.rs`/`battery_cmds.rs` + campus_detect/config_state/monitor_loop 命令，合计 53 条（`lib.rs:57-111`）。
- 落盘记录：`login_history.rs`（头插上限 100 `:51-76`）、`quality_history.rs`（`:52-75`）。
- 平台工具：`cpu_affinity.rs`（cpuinfo_max_freq 分簇绑小核 `:8-50`）、`identity_gate.rs`（生物识别 TTL 600s `:8`）。
- 三个 Tauri 插件（Kotlin 桥）：`plugins/network-bind/src/lib.rs`（WiFi 绑定/事件/SSID 权限 `:19-106`）、`plugins/keystore/src/lib.rs`（AndroidKeyStore AES-GCM `:24-50`，`Cargo.toml:4`「替代桌面 DPAPI」）、`plugins/foreground-service/src/lib.rs`（前台服务/WifiLock/开机自启/APK 安装 `:33-97`，注释 `:1-3`「只负责让进程活着，监控/自动重登业务逻辑全在 Rust 侧 monitor_loop」）。

### 双端同步硬性约定

1. **命令面同构**：安卓命令名/参数/返回形状与桌面同名对齐——`protocol_cmds.rs:1-3`「命令名与参数与桌面版对齐(前端近零适配)」、`system_cmds.rs:1`「与桌面 commands/system.rs 同构的安卓子集」、`account_cmds.rs:1-2`「与桌面 commands/account.rs 同构」、`self_service_cmds.rs:1-3`「命令名/参数/返回形状与桌面 commands/self_service.rs 同构」；桌面无的命令（battery/campus_detect/monitor_loop 系）在头注释单独标注（`battery_cmds.rs:1-4`）。
2. **配置契约同形**：桌面 `Config` 60 字段（`config/model.rs:10-172`，config_version=5 `:306`）↔ 桌面前端 types.ts 60 字段（`:9-87`）；安卓 `Settings` 47 字段（`config_state.rs:15-111`，config_schema_version=7 `:180`）↔ 安卓前端 types.ts 47 字段（`:9-73`）。camelCase serde 契约有测试背书（`model.rs:366-395`、`config_state.rs:716-724`）。
3. **交集与差集**：两配置交集 43 个同名同义字段；桌面独有 17（adapter1/adapter2/dual_adapter、adapter1Account/adapter2Account、auto_exit_after_login/auto_exit_on_online、minimize_to_tray、lightweight_mode、hidden_start、auto_launch、outbound_manual_hold_day、campus_exit_on_fail/campus_exit_start_minutes/campus_exit_end_minutes、skip_sha256_when_missing、config_version）；安卓独有 4（allow_2d_face_verify `config_state.rs:24`、background_check_idle_interval `:64`、enable_boot_autostart `:78`、config_schema_version `:110`）。
4. **镜像不消费字段**：outbound_priority / outbound_metric_restore / outbound_disabled_adapters / outbound_standby_route / dns_optimize_adapters 双端同名，安卓侧逐字段注释「安卓端不消费此字段」（`model.rs:66-103` ↔ `config_state.rs:38-55`），保持 JSON 同形以便配置语义互换；`night_outbound_restore` 反向：安卓消费（恒 "logged_out" 的夜切标记，`config_state.rs:52`）、桌面不消费（`model.rs:95-98`）。安卓 `get_init_data` 对桌面专属字段补空默认（`system_cmds.rs:16-22`）。
5. **落盘格式同构**：登录历史 `{time,success,message,adapter,user,"type"}` 头插上限 100 条（`login_history.rs:12-21/:51-76`，"type" 字段名是桌面契约，测试 `:136`）；质量历史 `{timestamp,gatewayLatency,externalLatency,quality}` 负延迟落盘 null（`quality_history.rs:1-3/:14-22`，禁 snake_case 测试 `:154`）；原子写统一 tmp+rename（`config_state.rs:279-281`、`login_history.rs:72-74`、`quality_history.rs:71-73`）。
6. **语义对齐有注释锚点**：`has_newer_version` 对齐桌面 compare_versions（`update_cmds.rs:86-107`）、下载白名单与桌面 updater 同构防 SSRF（`:41-51`）、`night_switch_login` 与桌面 full_login 对齐（`monitor_loop.rs:980-983`）、`ensure_identity_gate` 与桌面同构（`self_service_cmds.rs:37-47`）、`auto_create_account_in` 规则与桌面同构（`account_cmds.rs:404-452`）、`emit_config_changed` 与桌面 save_config_to_disk_encrypted 同语义（`config_state.rs:388-391`、`monitor_loop.rs:976`）。

### 两端命令面差异表

| 端 | 注册点 | 命令数 | 构成 |
| --- | --- | --- | --- |
| 桌面 | `app/startup.rs:62-128` | 65 | self_service 7（`:119-125`）+ 系统/账号/配置/网络/更新/托盘等 58 |
| 安卓 | `lib.rs:57-111` | 53 | protocol 8（`:58-65`）/ campus_detect 3（`:66-68`）/ config_state 2（`:69-70`）/ self_service 7（`:71-77`）/ account 6（`:78-83`）/ system 8（`:84-91`）/ monitor_loop 8（`:92-99`）/ battery 4（`:100-103`）/ quality 3（`:104-106`）/ update 4（`:107-110`） |

安卓退出策略与桌面不同：`lib.rs:114-121` 在 ExitRequested 一律 `api.prevent_exit()`（前台服务常驻，tauri #15671），桌面则是轻量化退出拦截（`startup.rs:135-149`）。

## 关键约束

1. **协议逻辑禁止双实现**：登录/注销/Portal/自助服务/夜切判定/定时判定一律调 `campus_login_lib`；安卓侧只允许「取配置 → 调共享函数 → 落历史 → 发事件」的包装（`protocol_cmds.rs:63-87`、`monitor_loop.rs:904-909/:1232-1234`）。
2. **跨平台代码保持纯**：bound_socket 纯函数全平台编译供桌面单测（`network/mod.rs:3-4`）；`cpu_affinity` host 恒 false（`cpu_affinity.rs:48-49`）；keystore 插件 `#![cfg(mobile)]`（`plugins/keystore/src/lib.rs:1`）；各安卓命令 not(mobile) 分支显式 Err。
3. **凭据安全**：日志/错误/事件 payload 不得携带 password（`protocol_cmds.rs:3`）；桌面 DPAPI（`crypto.rs:1-112`）与安卓 AndroidKeyStore AES-GCM（keystore 插件）互为同构替代；加密失败绝不落明文（`config_state.rs:261-272`）；配置出口一律掩码（`model.rs:315-328`、`config_state.rs:373-382`）。
4. **配置 camelCase + 迁移双轨**：桌面 v1→v5（`config/validate.rs:130-174`），安卓 v0→v7（`config_state.rs:314-366`），双端迁移均幂等且有测试。
5. **配置写路径唯一且串行**：安卓经 `config_io_lock` 串行化（`account_cmds.rs:15-23`），保存后刷 AndroidState 缓存并发 config-changed；账号档案与主配置同格式（EncodedSettings），仅目录不同（`account_cmds.rs:1-2`）。
6. **安卓平台缺口显式降级**：ICMP 原始 socket 被 SELinux 禁止，网关可达用 Portal TCP 可达表达（`campus_detect.rs:1-3/:41-53`）；surge_ping 安卓静默失败但 TCP 优先策略下不影响结果（`quality_cmds.rs:1-2`）。
7. **同步靠约定不靠机制**：命令面/配置字段/前端 types.ts 均为人工镜像 + 注释锚点 + 各端单测，无跨 crate 契约测试（见 Known Issues）。

## Data Flow

```
桌面 UI (React)                          安卓 UI (React)
 tauri-app/frontend                        android/frontend
 settings/types.ts (60 字段)               settings/types.ts (47 字段)
   │ invoke（65 命令）                       │ invoke（53 命令）
   ▼                                        ▼
桌面壳 app/startup.rs ── commands/*       安卓壳 *_cmds.rs（包装层）
 helper / monitor / update                protocol/account/self_service/campus_detect/
   │           │                          battery/quality/system/update/monitor_loop
   │ DPAPI     │ 托盘/单实例/GPU              │            │             │
   ▼           ▼                            │ Keystore   │ FGS/WiFi    │ 落盘记录
                                             ▼ (插件)     ▼ (插件)      ▼ login/quality_history
┌───────────────── campus-login（桌面 crate，协议单点）─────────────────┐
│ auth::protocol / auth::portal      network::bound_socket / quality    │
│ config::{model,schedule,night_switch,outbound_switch,validate}        │
│ self_service / infra::logger / platform::console_output               │
└───────────────────────────▲───────────────────────────────────────────┘
                            │ Cargo path 依赖（android/src-tauri/Cargo.toml:34）
```

配置面：桌面 config.json（DPAPI 密文）↔ 安卓 config.json（Keystore 密文），EncodedSettings 同格式；主配置外各有账号档案（桌面 commands/account 系 ↔ 安卓 `app_data_dir/accounts`，`account_cmds.rs:89-95`）。后台检测链（安卓）：FGS 前台服务 → `monitor_tick_loop:580-615` → `run_check_once:1245-1471`（校园网判定 → Portal 探测 → 自动重登）→ emit background-check-result / login-log；桌面等价逻辑在 `monitor` + `background_check` 模块（见 [[background-check-and-auto-login]]）。

## Connections

- [[ipc-command-surface]] —— 两端命令注册与 IPC 形状的完整清单
- [[android-backend]] —— 安卓壳 crate 与命令包装层细节
- [[android-plugins]] —— 三个 Tauri 插件（Kotlin 桥）职责边界
- [[desktop-platform]] —— 桌面平台模块与桌面专属能力
- [[config-and-persistence]] —— 配置模型、迁移链与落盘原子性
- [[security-model]] —— DPAPI / Keystore 加密与凭据不落盘约定
- [[background-check-and-auto-login]] —— 后台检测与自动重登状态机
- [[desktop-app-lifecycle]] —— 桌面启动/托盘/轻量化退出

## Known Issues

1. **Cargo.toml 注释与实现错位**：`tauri-app/src-tauri/Cargo.toml:43-45` 注释描述的是 `:46-50` 安卓目标依赖段（SO_BINDTODEVICE 旁路），段落在桌面 crate 内易误读；`android/src-tauri/Cargo.toml:53-55` 关于 profile 不生效的说明依赖读者理解 workspace 语义。
2. **self_service_cmds 头注释六/七不一致**：`self_service_cmds.rs:1-3` 仍写「六个命令包装」，实际 7 命令（`:62-271`，`lib.rs:71-77`）——注释未随 `reveal_operator_credential` 的加入更新。
3. **版本号多处人工同步**：`tauri-app/src-tauri/Cargo.toml:3` 与 `android/src-tauri/Cargo.toml:3` 均为 2.3.9，需与 tauri.conf.json 手工一致；且桌面走 build.rs 注入的 `APP_VERSION`（`build.rs:37`）而安卓走 `CARGO_PKG_VERSION`（`system_cmds.rs:15`），两条版本宏链并行，漏改任意一处即静默漂移。
4. **配置面交集 + 双向差集靠注释纪律**：交集 43 字段、桌面独有 17、安卓独有 4，外加 5 个「同名不消费」镜像字段与 1 个反向标记字段（night_outbound_restore），全部靠逐字段注释维护，无编译期或测试期防漂移手段。
5. **安卓网络探测平台缺口**：ICMP 被 SELinux 禁止后，网关可达性以 Portal TCP 可达表达（`campus_detect.rs:1-3/:103-110`），与桌面三层判定（SSID → /18 → ICMP）语义有差；surge_ping 在安卓静默失败（`quality_cmds.rs:1-2`）。
6. **双端同步靠人工纪律**：前端 types.ts 双份手工镜像、命令返回形状靠各端单测锁定（`model.rs:366-395`、`config_state.rs:716-724`、`account_cmds.rs:701-709`）；共享 crate 的函数签名变更有编译期保护，但注释承诺的「同构」语义（字段含义、事件名、错误文案）无自动校验。
