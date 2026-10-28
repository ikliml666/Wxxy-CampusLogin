---
title: android
type: module
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
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/App.tsx
  - android/frontend/src/main.tsx
  - android/frontend/vite.config.ts
  - android/frontend/package.json
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
tags: [android, tauri, rust, mobile, frontend-bridge]
---

## Overview

`android` 是校园网登录助手（CampusLogin）的安卓端，位于仓库 `android/` 目录。它与 Windows 桌面端（`../tauri-app` / `tauri-app/src-tauri`）**共享协议核心**：`android/src-tauri` 通过 Cargo path 依赖主仓库的 `campus-login` crate（`campus_login_lib`，内含 `auth::protocol`、`auth::portal`、`self_service`、`vpn`、`network::quality`、`config::schedule` 等跨平台纯函数），禁止复制协议逻辑。

整体由三层组成：

1. **Rust 后端**（`android/src-tauri/src/*.rs`）：Tauri v2 移动端命令面，命令名/参数/返回形状刻意与桌面版同名对齐，使前端（详见子模块 [[android-frontend]]）近乎零适配。所有命令在 `lib.rs` 经 `.invoke_handler(tauri::generate_handler![...])` 注册。
2. **三个 Tauri 移动插件**（`android/plugins/{network-bind,keystore,foreground-service}`）：封装安卓专属的 Java/Kotlin 能力（进程绑 WiFi、`AndroidKeyStore` 加解密、前台服务保活、开机自启、APK 安装、电池优化页跳转）。这些插件 `#![cfg(mobile)]`，在 host（桌面/CI）编译期被整体剔除为空实现。
3. **React 前端壳**（`android/frontend`，见独立子文章模块 [[android-frontend]]）：vite + React 19 + Tailwind，手机双外壳（顶部轻 header + 单列卡流 + 底部 5 tab）与平板 Dock 布局（懒加载复用桌面件），`@` 别名指向 `./src`，`@shared` 共享层来自主仓库前端。

关键平台约束（见 `android/README.md` 第 21–27 行）：强制走 WLAN（`ConnectivityManager.bindProcessToNetwork` 的 fwmark 方案），登录流量物理上只走 WiFi；ICMP 原始 socket 在安卓被 SELinux 禁止（`surge-ping` 不可用），Portal 可达改用 TCP 探测；密码经 `AndroidKeyStore` AES-GCM 加密落盘，出站一律掩码。

`lib.rs` 第 33–141 行是整个后端的入口：`run()` 在 `#[cfg(mobile)]` 下依次挂载四个移动插件（`tauri_plugin_campus_network_bind`、`tauri_plugin_campus_keystore`、`tauri_plugin_campus_monitor_service`、`tauri_plugin_biometric`）+ `tauri_plugin_notification`，并 `manage(android_state::AndroidState::default())`；`setup` 里初始化日志（落应用私有目录 `logs`，见第 49–58 行）、调用 `monitor_loop::run_startup_tasks` 拉起启动恢复、清理遗留的配置临时文件（第 62–64 行）；最后在 `RunEvent::ExitRequested` 里 `api.prevent_exit()`（第 137–139 行）——安卓语义是划掉任务只关 UI，进程随前台服务常驻（修复 tauri #15671 默认「最后窗口关闭→`process::exit(0)`」把前台服务一起带走的缺陷）。

## Key Components

### Entry point & process lifecycle

- `lib.rs` — 安卓端 Tauri 入口与命令面装配。
  - `pub fn run()`（第 33–141 行）：构建 `tauri::Builder`，`#[cfg(mobile)]` 下挂载 `tauri_plugin_campus_network_bind::init()`（第 37 行）、`tauri_plugin_campus_keystore::init()`（第 38 行）、`tauri_plugin_campus_monitor_service::init()`（第 39 行）、`tauri_plugin_biometric::init()`（第 40 行，该 crate `#![cfg(mobile)]`，host 编译为空）、`tauri_plugin_notification::init()`（第 41 行），并 `manage(android_state::AndroidState::default())`（第 42 行）。非移动端（第 43–44 行）只 `manage` 状态、不挂插件。`setup`（第 47–66 行）初始化日志（`campus_login_lib::infra::logger::init_logger`，第 56 行，失败仅 `eprintln`）、调用 `monitor_loop::run_startup_tasks(app.handle().clone())`（第 60 行）、启动清理 `config_state::cleanup_stale_config_tmp`（第 63 行）。`.invoke_handler(tauri::generate_handler![...])`（第 68–130 行）注册全部命令，`RunEvent::ExitRequested`（第 137–139 行）`api.prevent_exit()` 保活。
  - 模块声明 `mod ...`（第 12–28 行）：`campus_detect / android_state / config_state / cpu_affinity / identity_gate / login_history / protocol_cmds / self_service_cmds / vpn_cmds / monitor_loop / account_cmds / system_cmds / quality_cmds / quality_history / update_cmds / face_model_cmds / battery_cmds`。

### Global state & configuration

- `android_state.rs` — 进程级全局状态。
  - `pub(crate) static WATCHER_FAILED: AtomicBool`（第 14 行）：WiFi watcher 注册失败降级标志（退化为纯周期检测）；独立于 `AndroidState` 是因为 `status_value()` 可能在 `State` 可用前被调用，`AtomicBool` 免锁。
  - `pub struct AndroidState`（第 16–21 行，派生 `Default`）：`cached_source_ip: Mutex<Option<Ipv4Addr>>`（第 18 行，登录/注销复用的 wlan0 源 IP 缓存，由 `campus_detect::cache_source_ip` 写入）、`config: Mutex<Option<Settings>>`（第 20 行，明文配置内存态）。

- `config_state.rs` — 全量配置（结构对齐桌面 `Config` 的适用子集，camelCase IPC 契约）。
  - `pub const PASSWORD_MASK: &str = "***"`（第 10 行）；`const CONFIG_FILE: &str = "config.json"`（第 11 行）。
  - `pub async fn config_io_lock()`（第 22–24 行）：跨命令的配置 IO 串行锁（`tokio::sync::Mutex`，`CONFIG_IO_LOCK`，第 17–19 行），防 `save_config`/`switch`/`delete` 并发读改写竞态。
  - `pub struct Settings`（第 26–140 行，派生 `Serialize, Deserialize, Clone, Debug`，`#[serde(rename_all = "camelCase", default)]`）：51 个字段，含账号凭据、行为与通知开关、网络与检测时段、更新渠道、WebVPN 字段等。关键字段：`user`/`password`（第 30–31 行）、`allow_2d_face_verify`（第 36 行，2D 人脸回退开关）、`enable_night_operator_switch`/`night_operator_restore`（第 41–43 行，晚间断网自动切换运营商）、`enable_night_outbound_switch`/`night_outbound_restore`（第 47、68 行，夜间出站切换态标记，值恒 `"logged_out"`）、`background_check_interval`（第 80 行，默认 60_000）、`background_check_idle_interval`（第 85 行，闲时巡检默认 300_000）、`campus_check_start_minutes`/`campus_check_end_minutes`（第 113–115 行，检测时段，默认 460/1380）、`scheduled_login_minutes`/`scheduled_logout_minutes`（第 117–119 行，定时动作，默认 1440=禁用哨兵）、`vpn_account`/`vpn_password_enc`（第 128–129 行，WebVPN 凭据密文）、`config_schema_version`（第 139 行，当前新装 = 7）。
  - `impl Default for Settings`（第 142–217 行）：全部默认值，`config_schema_version` 默认 7（第 214 行），`required_network_name` 默认 `"i-wxxy、iwxxy-2、iwxxy-3"`（第 195 行）。
  - `pub struct CryptoBridge`（第 220–224 行）：`encrypt`/`decrypt` 两个 `Arc<dyn Fn>`；`from_app`（第 227–256 行）移动端经 `CampusKeystoreExt` 调用 `AndroidKeyStore`，host 注入「仅移动端可用」错误实现（保证密码路径可测）。
  - `struct EncodedSettings`（第 260–265 行）：磁盘形态，`settings: Settings` + `password_cipher: String`（密码字段落盘时清空、只留密文）。
  - `pub async fn load_file`（第 267–280 行）：读 `config.json`，密文损坏时置空而非报错；`pub fn password_saved_at`（第 287–293 行）：判据必须落在 disk 形态的 `password_cipher` 位上，不能看 `settings.password`（落盘时恒被清空）；`pub async fn save_file`（第 295–325 行）：加密落盘、临时文件名带 `pid` 防遗留 tmp 竞争；`pub async fn cleanup_stale_config_tmp`（第 330–341 行）清理 `*.json.<pid>.tmp` 孤儿；`pub async fn load_from`（第 343–353 行）含旧 `i-wxxy` 单值扩三值的幂等修复。
  - `async fn migrate_legacy_defaults`（第 372–434 行）：v0→v7 一次性的默认值迁移（15s→60s 间隔、开箱即用开关、质量检测默认关、闲时间隔、检测时段终点、质量间隔、夜切开关、定时动作哨兵 1440 等），迁移结果落盘、幂等。
  - `pub async fn save_to`（第 436–438 行）；`pub fn masked_for_display`（第 443–452 行，敏感信息出站唯一出口，密码与 `vpn_password_enc` 打 `***`）；`pub async fn emit_config_changed`（第 458–461 行，广播 `config-changed`）；`pub fn resolve_password_field`（第 464–473 行，空/`MASK`/`clear` 语义回退）。
  - `#[tauri::command] pub async fn get_config`（第 476–488 行）、`pub async fn save_config`（第 491–526 行，全程持 `config_io_lock`、落盘后调 `account_cmds::auto_create_account_for_current` 与 `emit_config_changed`）；`pub async fn current_settings`（第 530–547 行，缓存优先，协议命令回退凭据用）。
  - 测试：`fake_bridge`（第 555–564 行，base64 假加密，输出与输入不同），覆盖「加密落盘不含明文」「解密失败置空」「SSID 三值迁移」「默认值业务语义」「掩码出口」「VPN 密文回环」「密码已保存判据」等（第 613–902 行）。

### Campus network probe

- `campus_detect.rs` — 安卓侧校园网探针（本机 IPv4 枚举 + `/18` 子网判定复用桌面纯函数 `is_same_subnet_18` + Portal TCP 可达 + WiFi SSID 判定）。
  - `const PORTAL_PORT: u16 = 80` / `TCP_TIMEOUT`（第 9–10 行）。
  - `pub fn pick_campus_source_ip`（第 15–37 行）：从网卡枚举结果挑登录源 IPv4，排除蜂窝（`rmnet*`/`ccmni*`）、link-local/回环/未指定，`wlan0` 优先。
  - `pub async fn portal_reachable`（第 44–53 行）：Portal TCP 可达性，复用 `campus_login_lib::network::bound_socket::tcp_connect_bounded`（SO_BINDTODEVICE 物理网卡直连，旁路全量 VPN）。
  - `pub struct CampusProbe`（第 56–63 行）：`source`/`portal_ok`/`on_campus`/`wifi_off`。
  - `pub async fn probe_campus`（第 65–125 行）：核心探测；WiFi 未连接短路（第 86–93 行，2026-10-02 修复）；三层判定 SSID→`/18`→网关/Portal TCP（第 109–117 行）。
  - `pub fn campus_status_message`（第 129–137 行，WiFi 未连接短路优先）；`pub fn portal_host_of`（第 140–149 行，去 scheme/端口/路径）；`pub fn ssid_matches`（第 153–157 行，默认名 `i-wxxy` 特判 `iwxxy-2`/`iwxxy-3`）。
  - `async fn current_wifi_ssid`（第 163–178 行，经 network-bind 插件 `get_wifi_ssid`，JNI 走 `spawn_blocking`）；`pub(crate) async fn probe_campus_with_ssid`（第 185–200 行，SSID 感知版，名称命中直接判在校园网）；`pub fn cache_source_ip`（第 203–210 行，写 `AndroidState.cached_source_ip`）。
  - 命令：`#[tauri::command] pub async fn check_campus_status`（第 216–238 行，强制 `ensure_wifi_bound` 后探测并返回 `onCampusNetwork/currentSsid/campusMessage/...`）；`pub fn request_wifi_ssid_permission`（第 244–249 行，触发 `NEARBY_WIFI_DEVICES` 运行时权限）。
  - 测试：`pick_campus_source_ip` 优先级与排除、`portal_host_of`、`campus_status_message` 短路、`ssid_matches` 语义（第 261–321 行）。

### Protocol commands (login/logout/portal)

### Protocol commands (login/logout/portal)

- `protocol_cmds.rs` — 协议命令面（登录/注销/Portal 探测），复用桌面 `campus_login_lib::auth::protocol`。敏感纪律：日志/错误/事件 payload 不得含 password。
  - `#[tauri::command] pub async fn do_login`（第 10–63 行）：兼容桌面契约（仅传 `adapter`，凭据后端回退）；每次调用先 `ensure_wifi_bound`（第 21 行，此前漏绑导致 WiFi+流量同开时登录走蜂窝）；回退已存凭据（第 23–42 行，运营商名只打长度不打明文，第 37 行）；`run_login`（第 44 行）；手动登录成功清除自动登录熔断、注销保护、置 `was_online`（第 56–61 行）。
  - `pub async fn run_login`（第 66–90 行）：执行体（monitor 自动重登复用），经 `spawn_blocking` 调用同步的 `campus_login_lib::auth::protocol::do_login_with_retry`，源 IP 用缓存的 `cached_adapter_ip`。
  - `#[tauri::command] pub async fn do_logout`（第 94–143 行）：两步注销（`do_logout_with_retry`），设置 60s 注销保护期（`MONITOR.logout_protected_until_ms`，第 140 行，防自动重登立即登回）。
  - `#[tauri::command] pub async fn check_portal_status`（第 147–164 行）：Portal 全量探测（`check_portal_full`，返回 `PortalStatus`）。
  - `fn cached_adapter_ip`（第 166–174 行）；`#[tauri::command] pub fn ping_test`（第 177–179 行，返回 `"pong"`）。
  - `#[tauri::command] pub fn bind_to_wifi`（第 183–196 行，`#[cfg(mobile)]` 调 `campus_network_bind().bind_to_wifi()`）；`accept_wifi_network`（第 201–214 行，让系统接受无互联网 WiFi）；`get_avoid_bad_wifi_status`（第 220–235 行，查 `Settings.Global` 写通道）；`restore_written_settings`（第 240–255 行）。
  - `pub(crate) async fn ensure_wifi_bound`（第 269–338 行）：全链路执行前确保进程绑 WiFi。旁路代理生效时跳过（第 274–279 行，SO_BINDTODEVICE 生效则 `bindProcessToNetwork` 冗余）；绑定成功后清 HTTP 客户端池（fwmark 在 socket 创建时打标，第 294 行）；同网络重复绑定（`path=already_bound`）不清池（第 291 行）；结果记日志不阻断但不静默（第 286–337 行）。
  - `pub(crate) async fn accept_campus_wifi`（第 350–390 行）：绑定成功后让系统接受 captive portal WiFi（反射 hidden API `setAcceptUnvalidated` → 回退 `Settings.Global` 写 `captive_portal_mode=0` + `network_avoid_bad_wifi=0`）。

### Background monitor loop

- `monitor_loop.rs` — 后台监控循环（检测→Portal 探测→掉线自动重登→`emit background-check-result/login-log`），决策逻辑抽纯函数便于 TDD，前台服务保活在插件侧。
  - `pub(crate) static ref MONITOR: MonitorState`（第 16 行，lazy_static，默认 `MonitorState::default()`）。
  - 静态计数器/代际：`OUTBOUND_RESTORE_FAIL_COUNT`/`OUTBOUND_SWITCH_FAIL_COUNT`（第 21、26 行）、`MONITOR_GENERATION`（第 32 行，stop/start 竞速防误清 running 标志）、`LOGIN_ATTEMPT_LOCK`（第 37 行，attempt 段互斥）、`RESTORE_SILENT_PASS`（第 51 行，夜间出站还原后静默期持续放行）。
  - `pub struct MonitorState`（第 53–83 行，派生 `Default`）：`running`、`check_count`、`consecutive_failures`、`reconnect_count`、`last_login_attempt_ms`、`was_online`、`last_result: Mutex<Option<Value>>`、`desired_interval_ms`、`idle_interval_ms`、`logout_protected_until_ms`、`wifi_event_ms`、`notified_online: AtomicU8`（第 76 行，常驻通知状态记忆，0/1/2/3/4）、`scheduled_login_day`/`scheduled_logout_day: AtomicI32`。
  - 常量：`WIFI_EVENT_DELAY_MS = 2500`（第 88 行）、`WIFI_LOST_EVENT_DELAY_MS = 0`（第 93 行）、`WIFI_EVENT_DEBOUNCE_MS = 1000`（第 96 行）。
  - `fn wifi_event_is_burst_start`（第 101–103 行，纯函数，去抖起点判定）；`pub fn effective_interval_ms`（第 110–117 行，纯函数，巡检分档：WiFi 且亮屏走基础间隔，否则闲时间隔）；`pub fn should_attempt_login`（第 121–142 行，纯函数，全量条件显式入参）；`pub fn status_value`（第 145–166 行，给 `get_init_data`/`get_background_status` 的出站形状，展平最近一次检测关键字段）；`pub fn is_running`（第 169–171 行）；`fn epoch_ms`（第 173–178 行）。
  - `fn emit_login_log`（第 180–184 行，emit `login-log`）；`pub(crate) fn notify_system`（第 189–206 行，系统通知，看板娘大图标，`enable_notification=false` 静默）；`fn with_vpn_hint`（第 212–218 行，VPN 接管提示）。
  - `#[tauri::command] pub async fn start_background_check`（第 222–270 行，Rust tokio 循环 + FGS 保活，幂等，运行中仅刷新间隔并保活 FGS，启动 WiFi watcher，代际 token 防竞速）；`pub async fn stop_background_check`（第 273–285 行，先推进代际再清标志）；`start_wifi_watcher`/`stop_wifi_watcher`（第 296–334 行，注册/注销 Kotlin `NetworkCallback` 经 channel 事件）；`handle_wifi_event`（第 339–377 行，去抖+延迟后执行一次 `run_check_once`）；`trigger_background_check`（第 380–388 行）、`get_background_status`（第 391–393 行）。
  - 开机自启/通知开关命令：`get_boot_autostart`/`set_boot_autostart`（第 397–436 行）、`get_notification_enabled`/`set_notification_enabled`（第 440–460 行，均经 `account_cmds::config_io_lock` 与 `save_to` 落盘）。
  - `pub fn run_startup_tasks`（第 472–543 行，启动恢复：`enable_background_check` 拉起后台检测、`night_outbound_restore` 为空时 `restore_avoid_bad_wifi`、质量首测、自动更新检查循环、清理残留 APK、`auto_login_on_start` 并行 spawn，失败发 `startup-failed`）；`async fn probe_with_retry`（第 549–567 行，启动探测 3s 重试一次）。
  - `fn emit_auto_login_result`（第 573–579 行，emit `auto-login-result` 供前端 toast）；`async fn auto_login_on_start`（第 583–644 行，启动自动登录，静默期跳过、绑 WiFi、探测、登录、置 `was_online`、清注销保护）。
  - `#[cfg(mobile)] struct ProbeWindowGuard`（第 649–657 行，drop 释放 WifiLock/WakeLock）；`fn power_state`（第 663–673 行，屏幕交互中+当前网络为 WiFi）；`async fn monitor_tick_loop`（第 675–734 行，tick 循环：定时动作→间隔热更新→分档跳拍→`run_check_once`，代际校验自退）。
  - 夜间出站/运营商切换：`async fn outbound_switch_active`（第 738–743 行）、`latest_settings`（第 748–755 行）、`logout_for_outbound_switch`（第 763–788 行，前置在线检查+注销确认生效判据 `radiusOk || unbindOk`）、`switch_logout_unconfirmed`（第 793–810 行，连续失败上限）、`portal_probe_on_little_cores`（第 816–853 行，专用短命线程绑小核跑 Portal 探测，捕获 reactor context 缺失 panic）、`async fn run_scheduled_actions`（第 867–1077 行，夜间出站切换 Switch/Restore、运营商夜切、定时登录/注销，全部复用 `campus_login_lib` 共享纯函数）。
  - `async fn persist_settings`（第 1082–1105 行，落盘+刷新缓存+自动建号+广播 `config-changed`）；`async fn night_switch_login`（第 1111–1137 行，夜切登录内核，复 `run_login`）；`trigger_wifi_recheck`/`ensure_avoid_bad_wifi`/`restore_avoid_bad_wifi`（第 1143–1232 行，夜间出站切换的系统态 write/restore，`pub(crate)` 收尾）；`verify_night_switch`/`night_switch_verify_round`/`night_switch_check_once`（第 1269–1376 行，夜切后 chkstatus 验证与 uid 核对）。
  - `pub async fn run_check_once`（第 1379–1633 行，单次检测：出站切换态跳过→静默期门控（`RESTORE_SILENT_PASS` 在还原后放行）→探针窗口→绑 WiFi→校园网判定→Portal 探测（绑小核）→三态在线消费（仅 `error_kind.is_none()` 才翻转在线）→状态机→自动登录判定执行（注销保护期+连续失败熔断 `>=5`+`LOGIN_ATTEMPT_LOCK`）→emit `background-check-result`→常驻通知按需更新）。`notify_state`（第 1637–1645 行，状态码 1=在线/2=未连接/4=WiFi 未连接/3=非检测时段）。
  - 测试：通知状态码、`should_attempt_login` 各分支、`wifi_event_is_burst_start`、`effective_interval_ms` 分档与下限、`portal_origin`、`status_value` 展平（第 1654–1769 行）。

### Account commands

- `account_cmds.rs` — 多账号命令面（与桌面 `commands/account.rs` 同构：一账号一 JSON、密码密文落盘、切换合并登录字段），账号文件与主配置同格式（`EncodedSettings`），仅目录不同。
  - `pub async fn config_io_lock`（第 20–22 行，转发 `config_state::config_io_lock`）；`pub struct AccountItem`（第 27–40 行，`id`/`displayName`/`user`/`operator`/`passwordSaved`）；`pub struct AccountResult`（第 42–67 行，含 `ok`/`ok_with_account`/`err` 构造器）。
  - `pub fn sanitize_account_id`（第 72–83 行，账号 id 白名单消毒+32 截断）；`pub fn validate_account_name`（第 86–94 行，1–32 字符文件名校验防路径穿越）；`pub fn accounts_dir`（第 96–102 行，`app_data_dir/accounts`）；`fn read_account_card_info`（第 111–121 行，`password_saved` 走 `config_state::password_saved_at` 的 disk 密文位）；`pub fn list_account_items_sync`（第 126–155 行，同步列举+排序+空显示名兜底 id）。
  - 命令：`list_accounts`（第 158–163 行）、`switch_account`（第 186–227 行，合并登录字段、清夜切恢复目标与出站切换态标记、收尾 `restore_avoid_bad_wifi`）、`save_current_as_account`（第 230–257 行）、`create_account`（第 260–294 行）、`delete_account`（第 334–362 行）、`get_active_account`（第 365–367 行）、`rename_account`（第 370–401 行）。
  - `fn merge_save_as_target`（第 302–331 行，另存合并规则纯函数）、`async fn rename_account_core`（第 405–429 行，校验显示名→存在性→重名检查→只改 displayName）、`fn validate_display_name`（第 434–447 行）、`pub async fn auto_create_account_for_current`（第 451–460 行，R2 自动建号入口）、`async fn auto_create_account_in`（第 473–510 行，自动建号核心：撞库防护、幂等短路、只更新 3 字段保留 displayName）。
  - 测试：账号名消毒、列举过滤、id 消毒边界、显示名校验、自动建号（无凭据跳过/新号/幂等/差异更新/撞库防护）、改名成功与拒绝、另存不串号（第 517–838 行）。

### System & battery settings commands

- `system_cmds.rs` — 聚合初始化与日志命令（与桌面 `commands/system.rs` 同构的安卓子集）。
  - `#[tauri::command] pub async fn get_init_data`（第 6–29 行）：一次性拉全 `config`(已掩码)/`accounts`/`version`/`backgroundStatus` + 桌面专属字段补空默认(`autoLaunch`/`refreshRate`/`adapters: []` 等，防前端 `useInitialDataLoad` 读到 undefined 崩溃)。`backgroundStatus` 取自 `monitor_loop::status_value()`（第 27 行）。
  - `get_logs`（第 32–39 行）、`clear_logs`（第 42–46 行）、`get_log_retention_days`/`set_log_retention_days`（第 49–56 行）、`get_debug_mode`/`set_debug_mode`（第 59–66 行），均转发 `campus_login_lib::infra::logger`。

- `battery_cmds.rs` — 安卓系统设置页跳转命令（平台专属，桌面无此模块），经 foreground-service 插件转发。
  - `pub struct BatteryOptimizationInfo`（第 8–17 行：`ignoring`/`brand`/`hasVendorTarget`）。
  - 命令：`get_battery_optimization_info`（第 20–41 行）、`request_ignore_battery_optimizations`（第 46–61 行）、`open_vendor_battery_settings`（第 65–81 行）、`open_notification_settings`（第 87–102 行）。非 `#[cfg(mobile)]` 一律返回「仅安卓端可用」错误。

### Quality commands & history

- `quality_cmds.rs` — 网络质量命令（复用跨平台 `campus_login_lib::network::quality`，surge_ping ICMP 兜底在安卓静默失败）。
  - 静态：`LATENCY_RUNNING`（第 13 行）、`LATENCY_GENERATION`（第 17 行，代际 token 防竞态）；`async fn run_quality_once`（第 19–49 行，固定 `wlan0` + 缓存源 IP，结果落 `quality_history::append`）。
  - 命令：`check_network_quality`（第 52–54 行）、`start_latency_test`/`stop_latency_test`（第 57–83 行，幂等+代际）；`async fn latency_loop`（第 103–134 行，稳态退避：连续 5 拍 good 及以上翻倍、上限 1800s，波动/失败即恢复基础间隔）；纯函数 `fn next_backoff_interval_ms`（第 95–101 行）；常量 `GOOD_LEVELS`/`BACKOFF_STABLE_STREAK=5`/`BACKOFF_MAX_MS=1_800_000`（第 89–91 行）。
  - 测试：退避翻倍/上限/下限、等级判定（第 140–170 行）。

- `quality_history.rs` — 网络质量历史（`data_dir/quality_history.json`，头插法上限 100，损坏备份重置）。
  - `pub const QUALITY_HISTORY_MAX: usize = 100`（第 12 行）；`pub struct QualityHistoryEntry`（第 14–22 行：`timestamp`/`gateway_latency: Option<i64>`/`external_latency: Option<i64>`/`quality`，负延迟落盘为 null）；`pub fn read`（第 29–42 行）、`pub fn append`（第 52–75 行，tmp+rename 原子写）。
  - 测试：头插法、上限截断、损坏备份重置、序列化形状双端一致（第 101–157 行）。

### Update commands

### Update commands

- `update_cmds.rs` — 更新流（version.json 检查+镜像降级、白名单域名流式下载、APK 安装）。检查/下载复用桌面逻辑，安装改 APK 意图。
  - 常量：`VERSION_FILE`（第 24 行）、`AUTO_CHECK_INTERVAL_SECS = 86400`（第 26 行）、`BACKOFF_RETRY_SECS = [300, 900, 3600]`（第 29 行）、`VERSION_MIRRORS`（第 39–43 行）、`DOWNLOAD_ALLOWED_HOSTS`（第 46–55 行，防 SSRF 白名单）、`MAX_DOWNLOAD_BYTES = 500MB`（第 57 行）、`APK_STALE_SECS = 86400`（第 62 行）。
  - 静态：`UPDATE_LOOP_RUNNING`/`UPDATE_CHECKSUM`（第 16、21 行）、`LAST_CHECK_ERROR`/`LAST_CHECK_TIME_MS`（第 35、37 行）。
  - `pub(crate) fn update_dir`（第 66–71 行，`app_data_dir/files/update`，必须与插件 `installApk`/`FileProvider` 对齐）；`pub struct ReleaseAsset`（第 73–78 行）、`pub struct UpdateInfo`（第 80–94 行，camelCase，含 `has_update`/`lastCheckError`/`lastCheckTime`）、`pub struct DownloadProgress`（第 96–102 行）。
  - `pub fn has_newer_version`（第 105–125 行，逐段数值比较）；`pub(crate) fn http_client`（第 127–136 行，移动端 rustls + connect_timeout/socket 5s/总 30s）；`pub(crate) fn allowed_url`（第 139–159 行，白名单唯一漏斗，人脸下载复用）。
  - `#[tauri::command] pub async fn check_update`（第 164–166 行）→ `check_and_record`（第 198–209 行）→ `check_update_inner`（第 282–333 行，4 源降级，用 `VersionFile`（第 223–229 行）解析 `version.json` 的真实 snake_case 形状，避免误用 camelCase `UpdateInfo` 导致永远「已是最新」）、`fetch_apk_assets`（第 237–280 行，GitHub API 拉资产与 digest，区分 api_ok 三态）。
  - `pub fn start_update_check_loop`（第 340–382 行，启动 5s 后查、之后每 24h、失败 5min→15min→1h 退避重试、有新版本 emit `update-available` + 系统通知）。
  - `#[tauri::command] pub fn get_mirror_urls`（第 385–392 行）；`download_update`（第 401–408 行，外壳原子标志互斥）→ `download_update_inner`（第 410–502 行，流式下载+emit `update-download-progress`+三态 SHA256 校验）；`fn download_tag`（第 506–510 行）、`pub(crate) async fn verify_file_sha256`（第 517–544 行，堆分配 64KB buf 防 WebView JavaBridge 线程栈溢出，非 64 位 hex 显式失败）；`install_update`（第 549–570 行，路径必须位于更新目录内防任意路径）；`pub async fn cleanup_stale_apks`（第 576–610 行，删超 24h 残留 APK）。
  - 测试：版本比较、白名单、UpdateInfo 契约、安装目录含 files 段、清理决策、退避步进、download_tag、version.json 真实形状（第 617–739 行）。

- `login_history.rs` — 登录历史（`data_dir/login-history.json`，头插法上限 100，损坏备份重置；安卓无多适配器，`adapter` 固定 `wlan0`）。
  - `pub const LOGIN_HISTORY_MAX: usize = 100`（第 10 行）；`pub struct LoginHistoryEntry`（第 12–21 行：`time`/`success`/`message`/`adapter`/`user`/`type`）；`pub fn read`（第 28–41 行）、`pub fn append`（第 52–77 行）。
  - 测试：头插法、上限截断、损坏备份、序列化形状对齐桌面（第 90–143 行）。

### Identity gate & self-service / WebVPN

- `identity_gate.rs` — 生物识别验证门（进程内 TTL 时间戳，与桌面 `platform/identity.rs` 同构）。
  - `pub const IDENTITY_VERIFY_TTL_SECS: u64 = 600`（第 8 行）；`static LAST_VERIFY_EPOCH_SECS`（第 6 行）；`pub fn note_identity_verified`（第 18–20 行）、`pub fn identity_verified_recently`（第 23–33 行，从未验证=0/时钟回拨一律过期）；`reset_for_tests`（第 36–38 行）。
  - 测试：从未验证为假、TTL 内外、时钟回拨过期（第 45–65 行）。

- `vpn_cmds.rs` — WebVPN 凭据命令与自助服务自动共存路由支撑（桌面 `commands/vpn.rs` 同构，2026-10-25）。`vpn_password_enc` 语义为「平台加密器密文」（桌面 DPAPI / 安卓 Keystore 互不可解）。
  - `fn has_vpn_credential`（第 26–28 行）、`async fn load_vpn_password`（第 32–38 行，经 `CryptoBridge.decrypt`）、`async fn detect_net_zone`（第 43–50 行，复用 `campus_detect` 探针）、`fn offcampus_hint`（第 53–64 行）、`pub(crate) fn append_hint`（第 67–75 行）。
  - `pub(crate) async fn resolve_self_service_channel`（第 85–110 行，自动共存路由核心：Campus/Unknown→直连，OffCampus+开关+凭据→建/复用 WebVPN 会话）；`vpn_save_credential`（第 115–157 行，设验证门，Keystore 加密落盘+自动开总开关+重置会话）、`vpn_credential_status`（第 161–173 行）、`vpn_clear_credential`（第 178–202 行）、`set_self_service_via_vpn`（第 207–247 行）、`vpn_verify_credential`（第 253–265 行，不落盘仅校验）、`vpn_test_connection`（第 270–284 行）、`async fn run_full_chain`（第 288–305 行，CAS→WebVPN 网关→探活）。
  - 测试：`append_hint`、`offcampus_hint` 语义（第 311–347 行）。

- `self_service_cmds.rs` — 自助服务命令面（七个网络命令包装 `campus_login_lib::self_service`，与桌面同构）。改状态命令设验证门，查询类不设门。
  - `pub struct CommandResult`（第 10–26 行：`success`/`message`/`data`，`ok_msg`/`err` 构造器）；`fn resolve_self_password`（第 30–39 行，MASK/空回退登录密码）；`pub(crate) fn ensure_identity_gate`（第 42–51 行，总开关关放行、TTL 内验证放行）；`fn is_iso_date`（第 196–208 行，YYYY-MM-DD 严格校验）。
  - 命令：`verify_biometric_identity`（第 70–85 行，success 才 `note_identity_verified`）、`bind_operator`（第 88–132 行）、`query_bind_status`（第 135–166 行）、`query_self_dashboard`（第 169–193 行）、`query_self_online_log`（第 211–239 行）、`self_offline_session`（第 242–269 行）、`change_self_password`（第 276–312 行，6–11 位 ASCII 校验）、`reveal_operator_credential`（第 316–345 行，强制验证门）。
  - 测试：自助密码回退、验证门、is_iso_date（第 359–392 行）。

### Face model commands

### Face model commands

- `face_model_cmds.rs` — 人脸识别模型按需下载（APK 不再打包 6 个模型文件 ~8.9MB，首次启用人脸功能时从 GitHub `@vladmandic/human` master 下载到应用私有目录，再经 asset 协议喂给 WebView 内 human）。
  - 常量：`FACE_MODEL_BASE`（第 18 行，raw.githubusercontent 模型目录）、`FACE_MODEL_MIRRORS`（第 20–24 行，前缀代理）、`FACE_MODEL_FILES`（第 26–51 行，(文件名, sha256)，锁定 3.3.6 内容）。
  - `static FACE_DL_RUNNING`（第 54 行，并发互斥）。
  - `pub struct FaceModelFileState`（第 56–63 行：`name`/`size`/`ok`）、`pub struct FaceModelsState`（第 65–72 行：`ready`/`dir`/`files`）、`pub struct FaceModelProgress`（第 74–81 行）。
  - `fn models_dir`（第 86–93 行，`app_data_dir/files/face-models`，须与 `tauri.conf.json` assetProtocol scope 对齐）；`async fn state_inner`（第 96–118 行，六文件就位状态，复用 `verify_file_sha256`）；`face_models_state`（第 121–123 行）、`face_models_download`（第 126–133 行）、`async fn download_models`（第 135–184 行，源排序跟 `update_source`、逐文件增量、SHA256 校验）、`async fn download_one`（第 187–243 行，`.part` 临时文件原子改名）。
  - 测试：mirror url 形式、sources 排序语义（第 250–275 行）。

### CPU affinity (little-core pinning)

- `cpu_affinity.rs` — 大小核拓扑识别与线程绑核（天玑/骁龙 DynamIQ 功耗适配）。
  - `pub fn little_core_ids`（第 8–31 行，解析 `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq` 按频率分簇，最低频簇视为小核，频率差 <30% 视为同构返回空）；`pub fn pin_current_thread_to_little_cores`（第 34–50 行，仅安卓实现、`#[cfg(not(target_os="android"))]` 恒 false，绑核失败静默返回 false）。
  - 测试：host 恒不绑定、同构拓扑返回空（第 57–69 行）。

### Tauri mobile plugins

三个移动插件均为 `#![cfg(mobile)]`，host 编译期整体剔除。标识符见各自 `const PLUGIN_IDENTIFIER` 与 `register_android_plugin`。

- `plugins/network-bind/src/lib.rs` — 把进程网络绑定到 WLAN（`ConnectivityManager.bindProcessToNetwork`，经 netd FwmarkServer 对进程内全部 socket 生效），强制登录流量走 WiFi。
  - `struct CampusNetworkBind<R>`（第 13 行）；方法：`bind_to_wifi`（第 19–21 行，`run_mobile_plugin("bindToWifi")`）、`accept_wifi_network`（第 26–28 行）、`start_wifi_watcher`/`stop_wifi_watcher`（第 32–54 行，注册/注销 `wifiChanged` channel）、`get_wifi_ssid`（第 59–61 行）、`request_wifi_ssid_permission`（第 65–67 行）、`report_wifi_unusable`（第 73–75 行）、`ensure_avoid_bad_wifi`（第 82–84 行）、`restore_avoid_bad_wifi`（第 89–91 行）、`restore_written_settings`（第 96–98 行）、`get_secure_settings_status`（第 104–106 行）。`trait CampusNetworkBindExt`（第 110–118 行）、`init`（第 121–130 行，注册 `com.campuslogin.plugin.networkbind` 的 `NetworkBindPlugin`）。

- `plugins/keystore/src/lib.rs` — AndroidKeyStore AES-GCM 加解密原语（用途与桌面 DPAPI 同构）。
  - `struct CampusKeystore<R>`（第 24 行）；`encrypt`（第 30–33 行，明文→base64(iv+ciphertext)）、`decrypt`（第 36–39 行，密文损坏由 GCM 认证失败拒绝）；`trait CampusKeystoreExt`（第 42–50 行）、`init`（第 52–61 行，注册 `com.campuslogin.plugin.keystore` 的 `KeystorePlugin`）。

- `plugins/foreground-service/src/lib.rs` — 前台服务保活（常驻通知 + WifiLock + PARTIAL_WAKE_LOCK）与开机自启开关，只负责让进程活着，监控/自动重登业务逻辑全在 Rust 侧 `monitor_loop`。
  - `struct CampusMonitorService<R>`（第 27 行）；方法：`start_monitor`/`stop_monitor`（第 33–39 行）、`update_notification`（第 42–44 行）、`begin_probe_window`/`end_probe_window`（第 47–54 行，探针窗口锁）、`get_power_state`（第 58–61 行，`(screen_on, wifi_connected)`）、`set_boot_autostart`/`is_boot_autostart_enabled`（第 64–71 行）、`install_apk`（第 75–77 行，FileProvider content URI）、`get_battery_optimization_info`/`request_ignore_battery_optimizations`/`open_vendor_battery_settings`/`open_notification_settings`（第 80–97 行）。`trait CampusMonitorServiceExt`（第 100–108 行）、`init`（第 110–119 行，注册 `com.campuslogin.plugin.monitorservice` 的 `MonitorServicePlugin`）。

### Frontend shell (sub-module)

安卓端 React 前端壳（`android/frontend`）是体量大、边界独立的子能力，已拆为独立子模块 **[[android-frontend]]** 详述。本文只列其与后端命令面的连接点：
- 命令/Ipc 契约层在 `android/frontend/src/hooks/tauriApi.ts`：桌面专属命令（适配器/DHCP/DNS/导出导入/诊断包等）显式 `Promise.reject` 为 `desktopOnly`（第 14–15 行），桌面专属事件走 `noopListener`（第 18 行），其余命令（如 `getConfig`/`saveConfig`/`checkCampusStatus`/`bindToWifi`/`startBackgroundCheck`/`listAccounts` 等）与 `lib.rs` `generate_handler!` 注册的后端命令一一对应。
- 双外壳入口 `android/frontend/src/App.tsx`：手机外壳（顶部轻 header + 单列卡流 + 底部 5 tab）+ 平板 `TabletShell` 懒加载复用桌面件；崩溃恢复在 `android/frontend/src/main.tsx`（GPU/rAF 心跳丢失重载，`setTimeout` 1s 最多 3 次）。
- `vite.config.ts` 的 `@` 别名指向 `./src`，`package.json` 依赖含 `@tauri-apps/api`、`plugin-biometric`、`plugin-notification`、`plugin-opener`、`@vladmandic/human`。

## Architecture

## Architecture

## Data Flow

## Connections

## Known Issues
