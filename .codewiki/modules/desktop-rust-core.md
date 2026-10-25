---
title: 安卓端 Tauri 后端核心
type: module
source_files:
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/update_cmds.rs
tags:
  - tauri
  - rust
  - ipc
  - config
  - monitor-loop
  - updater
  - desktop
  - android-twin
---

## Overview

本文模块是安卓端（`android/src-tauri`）的 Tauri 后端核心，是整个应用的命令面与状态面骨架：应用装配与 IPC 命令注册（`lib.rs`）、配置的加密落盘与版本迁移（`config_state.rs`）、多账号档案管理（`account_cmds.rs`）、登录/注销/Portal 状态探测（`protocol_cmds.rs`）、后台巡检主循环（`monitor_loop.rs`）、网络质量测试（`quality_cmds.rs`）与 APK 自更新（`update_cmds.rs`）。

七个源文件全部位于 `android/src-tauri/src/` 下；文中引用的 `campus_login_lib::*` 是双端共享的桌面协议核心 crate，安卓端经 Cargo path 依赖单点继承（协议实现单点存在于桌面 crate，安卓禁止复制）。安卓侧各文件的文档注释显式声明与桌面同名模块的对应关系：`protocol_cmds.rs:1-3`（命令名与参数与桌面对齐）、`account_cmds.rs:1-2`（与桌面 `commands/account.rs` 同构）、`config_state.rs:1-3`（与桌面 `config::Config` 对齐）、`monitor_loop.rs:1-3`（启动恢复语义对齐桌面）。

三条贯穿全模块的纪律：

1. **敏感信息唯一出站出口**：密码落盘为密文（`CryptoBridge` → `campus_keystore`，`config_state.rs:214-251`），外发配置恒经 `masked_for_display` 掩码（`config_state.rs:437-446`，非空密码 → `"***"`）；日志/错误/事件 payload 不得携带 password（`protocol_cmds.rs:37-39`），运营商归属只打 `user_len`/`operator_len` 长度、不打原文（`protocol_cmds.rs:37-38`）。

2. **配置读改写全局串行**：`CONFIG_IO_LOCK`（`tokio::Mutex`，`config_state.rs:17-19`）由锁的使用者统一加锁；`account_cmds` 的 `config_io_lock` 是对 `config_state::CONFIG_IO_LOCK` 的转发（`account_cmds.rs:20-22`），防止「读旧配置 → 账号命令落盘 → 旧基线覆盖」的丢更新。

3. **循环生命周期用代际 token 而非布尔**：`MONITOR_GENERATION`（`monitor_loop.rs:32`）与 `LATENCY_GENERATION`（`quality_cmds.rs:17`）在 stop 时先推进代际再清 running 标志，旧循环凭代际失配自然退出，杜绝 stop→start 竞速中旧循环误清新标志。

## Key Components

### 应用装配 lib.rs（141 行）

| 符号 | 行号 | 职责 |
| --- | --- | --- |
| 模块清单（17 个子模块） | `lib.rs:12-28` | campus_detect、android_state、config_state、cpu_affinity、identity_gate、login_history、protocol_cmds、self_service_cmds、vpn_cmds、monitor_loop、account_cmds、system_cmds、quality_cmds、quality_history、update_cmds、face_model_cmds、battery_cmds |
| `run()` | `lib.rs:33-141` | `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 入口 |
| 移动插件链 | `lib.rs:35-42` | `#[cfg(mobile)]` 下注册 campus_network_bind / campus_keystore / campus_monitor_service / biometric（该 crate `#![cfg(mobile)]`，host 编译为空）/ notification，并 `manage(AndroidState::default())`（`lib.rs:42`） |
| host 支路 | `lib.rs:43-44` | 仅 `manage(AndroidState::default())`，无插件 |
| setup 钩子 | `lib.rs:47-66` | 见下文 |
| `invoke_handler` | `lib.rs:68-129` | `generate_handler` 注册 61 条 IPC 命令 |
| `Builder::build` | `lib.rs:130-131` | `unwrap_or_else` 直接 panic |
| 退出护栏 | `lib.rs:138-139` | `RunEvent::ExitRequested` → `api.prevent_exit()` |

setup 细节：日志目录 `app_data_dir/logs`（`lib.rs:56-60`）；`init_logger` 失败必须 `eprintln!("[startup] 日志初始化失败,日志将不可用")`（`lib.rs:56-58`，不静默）；spawn `monitor_loop::run_startup_tasks`（`lib.rs:56`）；spawn `config_state::cleanup_stale_config_tmp` 清理孤儿 tmp（`lib.rs:56-58`）。

命令注册分组（`lib.rs:68-129`）：

| 模块 | 行号 | 命令 |
| --- | --- | --- |
| protocol_cmds | `lib.rs:69-76` | ping_test、bind_to_wifi、accept_wifi_network、get_avoid_bad_wifi_status、restore_written_settings、do_login、do_logout、check_portal_status |
| campus_detect | `lib.rs:77-78` | check_campus_status、request_wifi_ssid_permission |
| config_state | `lib.rs:79-80` | get_config、save_config |
| self_service_cmds | `lib.rs:81-88` | verify_biometric_identity、bind_operator、query_bind_status、query_self_dashboard、query_self_online_log、self_offline_session、change_self_password、reveal_operator_credential |
| vpn_cmds | `lib.rs:89-94` | vpn_save_credential、vpn_credential_status、vpn_clear_credential、set_self_service_via_vpn、vpn_test_connection、vpn_verify_credential |
| account_cmds | `lib.rs:95-101` | list_accounts、switch_account、rename_account、save_current_as_account、create_account、delete_account、get_active_account |
| system_cmds | `lib.rs:102-108` | get_init_data、get_logs、clear_logs、get_log_retention_days、set_log_retention_days、get_debug_mode、set_debug_mode |
| monitor_loop | `lib.rs:109-116` | start_background_check、stop_background_check、trigger_background_check、get_background_status、get_boot_autostart、set_boot_autostart、get_notification_enabled、set_notification_enabled |
| battery_cmds | `lib.rs:117-120` | get_battery_optimization_info、request_ignore_battery_optimizations、open_vendor_battery_settings、open_notification_settings |
| quality_cmds | `lib.rs:121-123` | check_network_quality、start_latency_test、stop_latency_test |
| update_cmds | `lib.rs:124-127` | check_update、download_update、get_mirror_urls、install_update |
| face_model_cmds | `lib.rs:128-129` | face_models_state、face_models_download |

退出护栏：`RunEvent::ExitRequested` → `api.prevent_exit()`（`lib.rs:138-139`）。根因是 tauri #15671：安卓上划掉任务只关 UI，进程随前台服务常驻；不拦住会把常驻服务一起杀掉。

### 配置状态 config_state.rs（897 行）

常量与锁：

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `PASSWORD_MASK` | `config_state.rs:10` | `"***"`，掩码出站 |
| `CONFIG_FILE` | `config_state.rs:11` | `"config.json"` |
| `CONFIG_IO_LOCK` | `config_state.rs:17-19` | `tokio::Mutex`，全局配置写串行 |
| `config_io_lock()` | `config_state.rs:22-24` | 供 `account_cmds` 等转发加锁 |

`Settings` 结构（IPC 契约，camelCase serde，`config_state.rs:26-136`）全字段：

| 字段 | 行号 | 默认值（`Default` impl `config_state.rs:138-211`）/ 说明 |
| --- | --- | --- |
| user | `config_state.rs:30` | 学号/账号，默认 `""` (141) |
| password | `config_state.rs:31` | 运行态明文；落盘时刻意清空（`save_file` `config_state.rs:304`） |
| self_hello_enabled | `config_state.rs:32` | default true（143） |
| self_reverify_each_action | `config_state.rs:33` | default false（144） |
| allow_2d_face_verify | `config_state.rs:34-36` | default false（145） |
| operator | `config_state.rs:37` | 运营商后缀（如 `@telecom`，空=校方默认线路），默认 `""` (146) |
| enable_night_operator_switch | `config_state.rs:38-41` | default false（149）；2026-10-02 起默认改回关闭，存量已落盘 true 不迁移（注释 `config_state.rs:147-148`） |
| night_operator_restore | `config_state.rs:42-43` | 运营商夜切恢复位，默认 `""` (150) |
| enable_night_outbound_switch | `config_state.rs:44-47` | default false（151） |
| outbound_priority | `config_state.rs:48-50` | 桌面专属镜像字段，安卓不消费，默认 `vec![]` (152) |
| outbound_metric_restore | `config_state.rs:51-53` | 同上，默认 `""` (153) |
| outbound_disabled_adapters | `config_state.rs:54-56` | 同上，默认 `""` (154) |
| outbound_standby_route | `config_state.rs:57-59` | 同上，默认 `""` (155) |
| night_outbound_restore | `config_state.rs:60-64` | 出站切换态标记，魔法值 `"logged_out"`；非空=切换中，巡检整轮跳过，默认 `""` (156) |
| dns_optimize_adapters | `config_state.rs:65-67` | 桌面专属镜像字段，默认 `vec![]` (157) |
| device_aliases | `config_state.rs:68-72` | `#[serde(default)]`（71），12 位 hex MAC → 别名，默认 `HashMap::new()` (158) |
| auto_login_on_start | `config_state.rs:74` | default true（159） |
| enable_background_check | `config_state.rs:75` | default true（160） |
| background_check_interval | `config_state.rs:76` | default 60_000（163） |
| background_check_idle_interval | `config_state.rs:77-81` | default 300_000（165），闲时 5min |
| auto_login_on_preparation | `config_state.rs:82` | default true（166） |
| max_disconnect_reconnect | `config_state.rs:83` | default 3（167） |
| auto_login_cooldown_secs | `config_state.rs:84` | default 60（168） |
| theme_mode | `config_state.rs:86` | default "dark"（169） |
| enable_notification | `config_state.rs:87` | default true（170） |
| custom_theme_color | `config_state.rs:88` | default "#6366f1"（171） |
| default_panel | `config_state.rs:89` | default "dashboard"（172） |
| display_name | `config_state.rs:90-93` | 当前账号显示名，默认 `""` (173) |
| active_account | `config_state.rs:94` | 当前账号 id，默认 `""` (174) |
| enable_boot_autostart | `config_state.rs:95` | default false（175） |
| enable_latency_test | `config_state.rs:97` | default false（176） |
| latency_test_interval | `config_state.rs:98` | default 600_000（178） |
| enable_network_quality | `config_state.rs:99` | default false（182） |
| skip_ttfb_in_latency | `config_state.rs:100` | default true（183） |
| skip_content_in_latency | `config_state.rs:101` | default true（184） |
| portal_url | `config_state.rs:103` | default "http://10.1.99.100"（185） |
| fixed_gateway | `config_state.rs:104` | default "10.2.127.254"（186） |
| required_network_name | `config_state.rs:105` | default "i-wxxy、iwxxy-2、iwxxy-3"（189，2026-10-01 对齐桌面三 SSID；旧单值经 `load_from` 幂等扩为三值 `config_state.rs:343-345`） |
| enable_network_name_check | `config_state.rs:106` | default true（190） |
| campus_gateway | `config_state.rs:107` | default "10.2.127.254"（191） |
| campus_check_start_minutes | `config_state.rs:108-109` | default 460（193，即 7:40） |
| campus_check_end_minutes | `config_state.rs:110-111` | default 1380（196，即 23:00） |
| scheduled_login_minutes | `config_state.rs:112-113` | default 1440（200，哨兵值=禁用） |
| scheduled_logout_minutes | `config_state.rs:114-115` | default 1440（201，哨兵值=禁用） |
| update_source | `config_state.rs:116-118` | default "mirror"（202） |
| log_retention_days | `config_state.rs:120` | default 7（203） |
| vpn_account | `config_state.rs:124` | default `""` (204) |
| vpn_password_enc | `config_state.rs:125` | 密文落盘，默认 `""` (205) |
| self_service_via_vpn | `config_state.rs:126-128` | default true（206） |
| config_schema_version | `config_state.rs:129-135` | default 7（208） |

方法与类型：

| 方法/类型 | 行号 | 职责 |
| --- | --- | --- |
| `CryptoBridge` | `config_state.rs:214-218` | encrypt/decrypt 回调（`Arc<dyn Fn>`） |
| `CryptoBridge::from_app` | `config_state.rs:224-254` | mobile → `tauri_plugin_campus_keystore`（222-241）；not(mobile) 恒 Err "Keystore 仅移动端可用"（242-249） |
| `EncodedSettings` | `config_state.rs:254-259` | 磁盘形态 `{settings, passwordCipher}` |
| `load_file` | `config_state.rs:261-274` | 文件不存在 → default（264）；解析失败 Err "配置损坏"（267）；解密失败置空不报错（270-272） |
| `password_saved_at` | `config_state.rs:281-287` | 「密码已保存」判据=密文位 passwordCipher 非空；外露字段恒空，用户真机反馈后修复（doc 276-280） |
| `save_file` | `config_state.rs:289-319` | 建目录（290-294）；非空密码加密，失败 eprintln 诊断 + Err 绝不落明文（297-302）；落盘前 `settings.password` 置空（304）；pid 唯一 tmp + rename 防陈旧 tmp 竞争（311-317） |
| `cleanup_stale_config_tmp` | `config_state.rs:324-335` | 清 `*.json.<pid>.tmp` 孤儿（startup spawn，`lib.rs:63-65`） |
| `load_from` | `config_state.rs:337-347` | load_file + `migrate_legacy_defaults`（339）+ 旧单值 SSID 幂等扩三值（343-345） |
| `migrate_legacy_defaults` | `config_state.rs:366-428` | v0→v7 一次性迁移（doc 349-365）：v0→v1 间隔 15s→60s（368-370）；legacy(<3) 自动化开关刷开（371-385）；v4 闲时间隔 300s（386-394）；v5 检测终点 0→1380（395-403）；v6 质量间隔 60s→600s（404-412）；v7 夜切默认开 + 定时 0→1440（413-427）；每段落盘失败 log_warn、幂等重迁 |
| `save_to` | `config_state.rs:430-432` | `save_file` 薄封装 |
| `masked_for_display` | `config_state.rs:437-446` | 非空密码 → `"***"`；vpn_password_enc 同打码；敏感出站唯一出口 |
| `emit_config_changed` | `config_state.rs:452-455` | emit `config-changed`，payload `{"config": masked}`（订阅方复用掩码，不产生第二出口） |
| `resolve_password_field` | `config_state.rs:458-467` | clear → 空；空串/MASK → 回退已存值；否则新值（与桌面同构语义） |
| `get_config` | `config_state.rs:470-482` | load_from + 刷新 AndroidState.config 缓存 + masked 返回 |
| `save_config` | `config_state.rs:485-520` | 全程持 CONFIG_IO_LOCK（493）→ load_from 基线（499）→ `resolve_password_field`（501-507）→ save_to（508）→ 刷缓存（509-512）→ `auto_create_account_for_current`（515）→ `emit_config_changed`（518） |
| `current_settings` | `config_state.rs:524-541` | 缓存优先（525-530），未命中读盘回填（531-540）；`#[allow(dead_code)]`（523）为历史标注，实际被 protocol_cmds 使用 |

tests：`config_state.rs:543-897`（base64 假加密桥、加密落盘不含明文往返、解密失败置空、SSID 扩三值、默认值业务语义、迁移幂等（迁移后用户改回不被覆盖）、camelCase 形状、掩码出口、密码回退语义与桌面同构、空密码不写密文位、密码已保存判据）。

### 多账号 account_cmds.rs（838 行）

与桌面 `commands/account.rs` 同构：一账号一 JSON（同 `EncodedSettings` 格式，仅目录不同 `accounts/`）、密码密文落盘、切换只合并登录字段。

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `ACCOUNT_NAME_RE` | `account_cmds.rs:13-15` | `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$` |
| `config_io_lock` | `account_cmds.rs:20-22` | 转发 `config_state::CONFIG_IO_LOCK`（锁本体在锁的使用者侧统一） |
| `AccountItem` | `account_cmds.rs:27-40` | 账号卡脱敏展示 `{id, displayName, user, operator, passwordSaved}`（29/30-31/34-35/36-37/38-39）；密码只出布尔 |
| `AccountResult` | `account_cmds.rs:42-55` | `{success, message, active_account, display_name, config}`，Option 全 `skip_serializing_if`；构造器 ok/ok_with_account/err（57-67） |
| `sanitize_account_id` | `account_cmds.rs:72-83` | 白名单外字符 → `_`，截断 32 字符，空入空出（调用方跳过建号），全非法 → 全下划线仍建号 |
| `validate_account_name` | `account_cmds.rs:86-94` | 1-32 字符 + 白名单正则，防路径穿越 |
| `accounts_dir` | `account_cmds.rs:96-102` | `app_data_dir/accounts` |
| `read_account_card_info` | `account_cmds.rs:111-121` | 从账号 JSON 读展示字段不解密；password_saved 走 `password_saved_at` 密文位判据（119） |
| `list_account_items_sync` | `account_cmds.rs:126-155` | 只认 `.json`（130）；跳过 `.` 开头与空 id（137）；空显示名兜底 id（142）；读取失败 log_warn 兜底（145-148）；按 id 排序（153） |
| `list_accounts` | `account_cmds.rs:158-163` | spawn_blocking 包装 |
| `load_account_file` | `account_cmds.rs:165-170` | 不存在 → `Ok(None)` |
| `persist_current` | `account_cmds.rs:172-183` | save_to + 刷缓存 + masked |
| `switch_account` | `account_cmds.rs:211-253` | 持锁（198）；账号不存在 err（199-201）；合并 user/password/operator（206-208）；清 `night_operator_restore`（211，残留会把 A 运营商恢复到 B）；清 `night_outbound_restore`（214，残留让巡检整轮跳过）；display_name 空兜底 id（215-219）；`restore_avoid_bad_wifi` 收尾（225） |
| `save_current_as_account` | `account_cmds.rs:229-257` | 持锁（242）→ `merge_save_as_target`（247）→ 落盘档案（248）→ 主配置同步 active_account/display_name 防串号（252-255） |
| `create_account` | `account_cmds.rs:259-294` | user 空拒绝（271-273）；重名拒绝（279-281）；仅登录字段+显示名，不动主配置不切换（283-291） |
| `merge_save_as_target` | `account_cmds.rs:302-331` | 纯函数：另存场景（current.active_account != safe_name，307）落盘副本 display_name 置空防串号（310-312）；已存在档案 → 保留非登录字段、登录字段用当前配置覆盖（314-321）；主配置最终显示名=档案非空优先否则兜底 id（325-329） |
| `delete_account` | `account_cmds.rs:334-363` | 持锁（345）→ 删文件（346-348）；删当前账号 → 清 active_account（352）+ 清 night_outbound_restore（355）+ persist（356）+ `restore_avoid_bad_wifi`（358）；非当前 → ok(masked)（361） |
| `get_active_account` | `account_cmds.rs:365-368` | |
| `rename_account` | `account_cmds.rs:369-401` | 业务错误走 IPC `Err(String)`（381）；激活账号同步 display_name（390-394） |
| `rename_account_core` | `account_cmds.rs:405-429` | 校验显示名（411）→ 存在性（413-415）→ 重名检查排除自身、对方空显示名兜底 id 参与（420-424）→ 只改 displayName 写回（426-427） |
| `validate_display_name` | `account_cmds.rs:434-447` | trim 后 1..=32 字符；控制字符/换行拒绝；空格/emoji/`@` 允许 |
| `auto_create_account_for_current` | `account_cmds.rs:451-460` | R2 自动建号入口，失败仅 log_warn |
| `auto_create_account_in` | `account_cmds.rs:473-510` | 五规则（doc 462-472）：user/password 空跳过（478-480）；id=sanitize(user)、空跳过（481-484）；不存在 → 以快照建号，display_name=原始 user 文本、文件内 active_account=id（486-491）；已存在且 user 原文不同 → 撞库 log_warn 跳过不覆盖凭据（493-501）；user 一致且 password/operator 一致 → 幂等短路不写盘（502-504），有差异 → 只更新 user/password/operator 三字段保留 display_name（505-509） |

tests：`account_cmds.rs:512-838`（消毒放行/路径穿越拒绝/长度边界、列举过滤排序兜底、sanitize 边界、显示名边界、自动建号幂等短路、改名核心、`AccountItem` JSON 形状锁、另存显示名不串号/覆盖同 id 保留自定义名、撞库防护）。

### 登录注销与 Portal 探测 protocol_cmds.rs（390 行）

敏感纪律（`protocol_cmds.rs:1-3`）：日志/错误/事件 payload 不得携带 password。

| 函数 | 行号 | 职责 |
| --- | --- | --- |
| `do_login` | `protocol_cmds.rs:10-63` | `ensure_wifi_bound` 先行（21）；user/password 空回退 `current_settings`（23-31）；空凭据 Err（32-35）；日志只打 user_len/operator_len（37）；operator 空回退配置（38-42）；`run_login`（44）；`login_history::append(..., "manual")`（46-52）；成功清 consecutive_failures/logout_protected_until_ms + 置 was_online=true（56-61） |
| `run_login` | `protocol_cmds.rs:66-90` | `cached_adapter_ip`（72）；spawn_blocking 调共享 `campus_login_lib::auth::protocol::do_login_with_retry(user, password, operator, adapter_ip.as_deref(), 3, &is_quitting)`（77-87） |
| `do_logout` | `protocol_cmds.rs:94-143` | `ensure_wifi_bound`（101）；user 回退（103-107）；空凭据拒绝（108-110）；spawn_blocking `do_logout_with_retry`（113-123）；login_history 落账（125-131）；注销保护期 `logout_protected_until_ms = now + 60_000`（134-141） |
| `check_portal_status` | `protocol_cmds.rs:147-164` | `ensure_wifi_bound`（153）；IP 回退缓存（154-157）；spawn_blocking `check_portal_full(&ip, None)`（159-163） |
| `cached_adapter_ip` | `protocol_cmds.rs:166-174` | 读 `AndroidState.cached_source_ip` |
| `ping_test` | `protocol_cmds.rs:177-179` | → "pong" |
| `bind_to_wifi` | `protocol_cmds.rs:183-196` | mobile 插件（184-190）；not(mobile) Err（191-195） |
| `accept_wifi_network` | `protocol_cmds.rs:201-214` | 同构（202-208 / 209-213） |
| `get_avoid_bad_wifi_status` | `protocol_cmds.rs:220-235` | WRITE_SECURE_SETTINGS 授权状态与 avoid_bad_wifi 值/快照（224-228）；not(mobile) Err（230-234） |
| `restore_written_settings` | `protocol_cmds.rs:240-255` | 插件还原写入的系统设置（244-248）；not(mobile) Err（250-254） |
| `ensure_wifi_bound` | `protocol_cmds.rs:269-338` | pub(crate)。`bind_capability()==Supported`（274-279）时跳过；JNI spawn_blocking（280-285）；bound 且 path != "already_bound" 才 `clear_client_pool()`（291-294）；结果 log_info + eprintln 双写（298-312）；随后 `accept_campus_wifi`（316）；失败 warn 回落默认路由（318-336） |
| `accept_campus_wifi` | `protocol_cmds.rs:350-390` | 三路径：反射 hidden API `setAcceptUnvalidated` → 回退写 Settings.Global；Kotlin 侧无动作返回 already_validated（350-389） |

### 后台监控 monitor_loop.rs（1710 行）

全局状态与常量：

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `MONITOR` | `monitor_loop.rs:16` | `MonitorState` lazy_static |
| `OUTBOUND_RESTORE_FAIL_COUNT` / `OUTBOUND_RESTORE_MAX_FAILS=3` | `monitor_loop.rs:21-23` | 晨间还原连续失败上限 |
| `OUTBOUND_SWITCH_FAIL_COUNT` / `OUTBOUND_SWITCH_MAX_FAILS=3` | `monitor_loop.rs:26-28` | 注销未确认重试上限 |
| `MONITOR_GENERATION` | `monitor_loop.rs:32` | AtomicU64 代际 token |
| `LOGIN_ATTEMPT_LOCK` | `monitor_loop.rs:37` | `tokio::sync::Mutex`，attempt 段互斥 |
| `MonitorState` | `monitor_loop.rs:39-69` | running (42), check_count (43), consecutive_failures (44), reconnect_count (46), last_login_attempt_ms (47), was_online (48), last_result (49), desired_interval_ms (52), idle_interval_ms (54), logout_protected_until_ms (56), wifi_event_ms (58), notified_online (62), scheduled_login_day (66), scheduled_logout_day (68) |
| `WIFI_EVENT_DELAY_MS=2500` | `monitor_loop.rs:74` | 连上等 DHCP/路由稳定 |
| `WIFI_LOST_EVENT_DELAY_MS=0` | `monitor_loop.rs:79` | 断开即事实 |
| `WIFI_EVENT_DEBOUNCE_MS=1000` | `monitor_loop.rs:82` | 风暴去抖 |
| `NIGHT_VERIFY_WAIT_SECS=15` 等 | `monitor_loop.rs:1196-1199` | 夜切验证时序 |

纯函数：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `wifi_event_is_burst_start` | `monitor_loop.rs:87-89` | 首事件或距上次 >= 窗口 → 风暴起点 |
| `effective_interval_ms` | `monitor_loop.rs:96-103` | `base.max(5000)`；screen_on && wifi_connected → base，否则 `idle_ms.max(base)` |
| `should_attempt_login` | `monitor_loop.rs:107-128` | online \|\| !on_campus → false；!was_online && !auto_login_on_preparation → false；reconnect_count >= max → false；millis_since_last_attempt >= cooldown*1000 |
| `status_value` | `monitor_loop.rs:131-152` | isRunning/checkCount/consecutiveFailures/reconnectCount/lastResult/watcherFailed + 展平最近检测 online/message/onlineOperator/serverAvailable/onCampusNetwork |
| `is_running` | `monitor_loop.rs:155-157` | |
| `notify_state` | `monitor_loop.rs:1578-1586` | (true,\_)→(1,"在线")；(\_,wifi_off)→(4,"WiFi 未连接")；else (2,"未连接")；3 被「非检测时段」占用 |
| `portal_origin` | `monitor_loop.rs:1202-1214` | 取 scheme+host 去路径；空回退 `default_portal_url` |
| `expected_operator_display` | `monitor_loop.rs:1217-1223` | 空 → "无锡学院"，其余原样 |

事件与展示辅助：`emit_login_log` → "login-log" 事件（`monitor_loop.rs:166-170`）；`notify_system`（175-192）；`with_vpn_hint`（198-204）。

命令生命周期：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `start_background_check` | `monitor_loop.rs:208-256` | 读配置刷 desired/idle；已 running → 仅刷新间隔 + 重发 start_monitor；先 FGS 成功才置 running；start_wifi_watcher；gen=MONITOR_GENERATION.fetch_add 后 spawn monitor_tick_loop |
| `stop_background_check` | `monitor_loop.rs:259-271` | 先推进代际再清 running + stop_wifi_watcher + stop_monitor |
| `start/stop_wifi_watcher` | `monitor_loop.rs:282-320` | 建 Channel 回调 handle_wifi_event；失败置 WATCHER_FAILED |
| `handle_wifi_event` | `monitor_loop.rs:325-363` | !is_running 兜底；去抖；lost 零延迟；延迟后校验 wifi_event_ms==now |
| `trigger_background_check` | `monitor_loop.rs:366-374` | 未运行 Err |
| `get_background_status` | `monitor_loop.rs:377-379` | → `status_value` |
| `get/set_boot_autostart` | `monitor_loop.rs:383-422` | 插件 + 持 config_io_lock 改配置 |
| `get/set_notification_enabled` | `monitor_loop.rs:426-446` | 同款 |

启动与登录编排：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `run_startup_tasks` | `monitor_loop.rs:458-529` | spawn + 500ms；读配置失败 → log_error + emit "startup-failed" + 尽力 stop_monitor；set_log_retention_days；并行 spawn 三链：start_background_check、质量首测、start_update_check_loop + cleanup_stale_apks；auto_login_on_start；night_outbound_restore 非空 → `restore_avoid_bad_wifi` |
| `probe_with_retry` | `monitor_loop.rs:535-553` | probe_campus_with_ssid 失败/非 campus 3s 后重试 |
| `emit_auto_login_result` | `monitor_loop.rs:559-565` | → "auto-login-result" |
| `auto_login_on_start` | `monitor_loop.rs:570-612` | 空凭据跳过；ensure_wifi_bound；probe_with_retry；cache_source_ip；!on_campus 跳过；run_login 成功 → 置 was_online + 清保护 + toast |
| `ProbeWindowGuard` | `monitor_loop.rs:617-625` | Drop 调 `end_probe_window` |
| `power_state` | `monitor_loop.rs:631-641` | 插件 get_power_state 失败保守 (true, true) |
| `monitor_tick_loop` | `monitor_loop.rs:643-702` | 主循环：启动对账；每拍代际校验 → run_scheduled_actions → 间隔热更新 → 分档跳拍 → run_check_once |

出站切换辅助：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `outbound_switch_active` | `monitor_loop.rs:706-711` | night_outbound_restore 非空 |
| `latest_settings` | `monitor_loop.rs:716-723` | 重读最新配置防 stale |
| `logout_for_outbound_switch` | `monitor_loop.rs:731-756` | !was_online → 跳过请求；do_logout 后 `radiusOk \|\| unbindOk` 判离线生效 |
| `switch_logout_unconfirmed` | `monitor_loop.rs:761-778` | 计数达上限按断网规律建态 |
| `portal_probe_on_little_cores` | `monitor_loop.rs:784-821` | 专用裸线程 + 绑小核 + reactor enter + catch_unwind |

`run_scheduled_actions`（`monitor_loop.rs:835-1037`）：出站动作判定 `evaluate_outbound_action`（856-863）；Switch 分支（866-886）；Restore 分支（887-924）；运营商夜切 `evaluate_night_switch_gated`（943-980）；双禁用早退（982-984）；定时判定 `evaluate_scheduled_actions`（993-1001）；fire_login/fire_logout（1002-1036）。

夜切与系统设置：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `persist_settings` | `monitor_loop.rs:1042-1065` | save_to → 刷缓存 → auto_create_account_for_current → emit_config_changed |
| `night_switch_login` | `monitor_loop.rs:1071-1097` | 空凭据 false；ensure_wifi_bound；run_login；成功置 was_online |
| `trigger_wifi_recheck` | `monitor_loop.rs:1103-1124` | spawn_blocking report_wifi_unusable |
| `ensure_avoid_bad_wifi` | `monitor_loop.rs:1129-1165` | 写 network_avoid_bad_wifi=1 |
| `restore_avoid_bad_wifi` | `monitor_loop.rs:1168-1192` | 按快照还原 |
| `verify_night_switch` | `monitor_loop.rs:1229-1283` | 独立任务：出站切换态守卫；sleep；复验 |
| `night_switch_verify_round` | `monitor_loop.rs:1287-1304` | 复用 update_cmds::http_client |
| `night_switch_check_once` | `monitor_loop.rs:1307-1336` | GET chkstatus + uid_matches |

`run_check_once`（`monitor_loop.rs:1339-1574`）：出站切换态闸（1354-1357）→ 静默期门控（1363-1383）→ 探针窗口（1388-1392）→ ensure_wifi_bound（1396）→ 校园网判定（1401-1411）→ Portal 三态短路（1420-1424）→ 三态消费 error_kind=None 才翻转（1441-1445）→ 状态机（1448-1453）→ 自动重登双闸 + LOGIN_ATTEMPT_LOCK（1463-1523）→ payload emit "background-check-result"（1535-1560）→ 常驻通知按翻转重建（1564-1573）。

tests：`monitor_loop.rs:1588-1710`

### 网络质量 quality_cmds.rs（170 行）

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `LATENCY_RUNNING` + `LATENCY_GENERATION` | `quality_cmds.rs:12-17` | AtomicBool + 代际 token |
| `run_quality_once` | `quality_cmds.rs:19-49` | 复用共享 `check_network_quality_async`；`quality_history::append` 落盘 |
| `check_network_quality` | `quality_cmds.rs:52-54` | 单次检测命令 |
| `start_latency_test` | `quality_cmds.rs:57-74` | `LATENCY_RUNNING.swap(true)` 幂等；读配置失败复位标志 |
| `stop_latency_test` | `quality_cmds.rs:77-83` | 先推进代际再清标志 |
| `GOOD_LEVELS` | `quality_cmds.rs:89` | `["excellent","great","good"]` |
| `BACKOFF_STABLE_STREAK` | `quality_cmds.rs:90` | 5 |
| `BACKOFF_MAX_MS` | `quality_cmds.rs:91` | 1_800_000 |
| `next_backoff_interval_ms` | `quality_cmds.rs:95-101` | 纯函数：streak % 5 == 0 → 翻倍封顶 |
| `latency_loop` | `quality_cmds.rs:103-134` | base=`latency_test_interval.max(10_000)`；先检测后 sleep；代际校验；退出仅代际匹配才清标志 |

tests：`quality_cmds.rs:136-170`

### 自更新 update_cmds.rs（739 行）

常量与全局：

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `UPDATE_LOOP_RUNNING` | `update_cmds.rs:16` | 自动检查循环幂等 |
| `UPDATE_CHECKSUM` | `update_cmds.rs:21` | `Mutex<Option<(String,String)>>` 记账 (tag, digest) |
| `VERSION_FILE` | `update_cmds.rs:24` | raw.githubusercontent.com version.json |
| `AUTO_CHECK_INTERVAL_SECS` | `update_cmds.rs:26` | 24*3600 |
| `BACKOFF_RETRY_SECS` | `update_cmds.rs:29` | [5min, 15min, 1h] |
| `LAST_CHECK_ERROR` | `update_cmds.rs:35` | 最近检查失败原因 |
| `LAST_CHECK_TIME_MS` | `update_cmds.rs:37` | |
| `VERSION_MIRRORS` | `update_cmds.rs:39-43` | 3 镜像前缀 |
| `DOWNLOAD_ALLOWED_HOSTS` | `update_cmds.rs:46-55` | 8 域名白名单 |
| `MAX_DOWNLOAD_BYTES` | `update_cmds.rs:57` | 500 MiB |
| `APK_STALE_SECS` | `update_cmds.rs:62` | 24h |
| `update_dir` | `update_cmds.rs:66-71` | `app_data_dir/files/update` |
| `DOWNLOAD_RUNNING` | `update_cmds.rs:398` | 下载互斥 |

类型：

| 类型 | 行号 | 字段 |
| --- | --- | --- |
| `ReleaseAsset` | `update_cmds.rs:73-78` | {name, url, size} |
| `UpdateInfo` | `update_cmds.rs:80-94` | {has_update (83), latest_version (84), release_notes (85), assets (86), sha256_checksum (87), last_check_error (91), last_check_time (93)} |
| `DownloadProgress` | `update_cmds.rs:96-102` | {downloaded, total, speed, percent} |
| `VersionFile` | `update_cmds.rs:223-229` | snake_case {version (226), notes (228)} |

函数：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `has_newer_version` | `update_cmds.rs:105-125` | 去 v 前缀逐段数值 |
| `http_client` | `update_cmds.rs:127-136` | rustls；connect_timeout 5s + timeout 30s |
| `allowed_url` | `update_cmds.rs:139-159` | 白名单唯一漏斗 |
| `check_update` | `update_cmds.rs:164-166` | → check_and_record |
| `backoff_delay_secs` | `update_cmds.rs:187-193` | 退避步进 |
| `check_and_record` | `update_cmds.rs:198-209` | 写 LAST_CHECK_ERROR/TIME |
| `version_urls` | `update_cmds.rs:211-217` | mirror_first 排序 |
| `fetch_apk_assets` | `update_cmds.rs:237-280` | GitHub API；返回 (assets, checksum, api_ok) |
| `check_update_inner` | `update_cmds.rs:282-333` | 4 源降级；三态记账 |
| `start_update_check_loop` | `update_cmds.rs:340-382` | 24h 循环 + 退避 |
| `get_mirror_urls` | `update_cmds.rs:385-392` | 镜像 JSON |
| `download_update` | `update_cmds.rs:401-408` | swap 互斥 |
| `download_update_inner` | `update_cmds.rs:410-502` | 流式写盘 + SHA256 三态校验 |
| `download_tag` | `update_cmds.rs:506-510` | 提取 release tag |
| `verify_file_sha256` | `update_cmds.rs:517-544` | pub(crate) 人脸复用；非 64 hex → Err；vec 缓冲 |
| `install_update` | `update_cmds.rs:549-570` | canonicalize 限目录 + install_apk |
| `cleanup_stale_apks` | `update_cmds.rs:576-610` | 超龄删 .apk |

tests：`update_cmds.rs:612-739`

## Architecture

```text
lib.rs run() (lib.rs:33-141)
 ├─ setup: run_startup_tasks(monitor_loop.rs:458) + cleanup_stale_config_tmp(config_state.rs:324)
 ├─ invoke_handler 61 条命令 (lib.rs:68-129)
 └─ ExitRequested → prevent_exit (lib.rs:137-138, tauri #15671)
      ├─ config_state ── CONFIG_IO_LOCK(:17) + CryptoBridge(:214) → config.json
      ├─ account_cmds ── accounts/*.json（转发同一把锁）
      ├─ protocol_cmds ── campus_login_lib::auth::{protocol,portal}
      ├─ monitor_loop: monitor_tick_loop(:643) → run_scheduled_actions(:835)
      │        └─ run_check_once(:1339) → protocol_cmds → 协议核心
      ├─ quality_cmds ── campus_login_lib::network::quality
      └─ update_cmds ── GitHub API/镜像 HTTP → campus_monitor_service 装 APK
```

## Data Flow

### 启动恢复链（run_startup_tasks，monitor_loop.rs:458-529）

spawn + 500ms 就绪窗 → 读配置失败 → log_error + emit "startup-failed" + 尽力 stop_monitor → set_log_retention_days → 并行 spawn 三链：①`start_background_check` ②质量首测 ③`start_update_check_loop` + `cleanup_stale_apks` → `auto_login_on_start` → night_outbound_restore 非空 → `restore_avoid_bad_wifi` 兜底。

### 巡检单拍（run_check_once，monitor_loop.rs:1339-1574）

周期拍、WiFi 事件、手动触发三入口共用。顺序：出站切换态闸 → 静默期门控 → 探针窗口 → ensure_wifi_bound → 校园网判定 + 源 IP 缓存 → Portal 三态短路 → 三态消费 error_kind=None 才翻转 → 状态机 → 自动重登双闸 + LOGIN_ATTEMPT_LOCK → payload emit "background-check-result" → 常驻通知按翻转重建。

### 配置写路径

`save_config`（`config_state.rs:485-520`）与 `persist_settings`（`monitor_loop.rs:1042-1065`）同构：持 CONFIG_IO_LOCK → `load_from` 基线 → merge/`resolve_password_field` → `save_to` 落盘 → 刷缓存 → `auto_create_account_for_current` → `emit_config_changed`。

### 更新下载与校验三态

检查：`check_update_inner`（`update_cmds.rs:282-333`）4 源降级。`UPDATE_CHECKSUM` 三态记账（309-315）。下载：`allowed_url` 白名单 + 流式写盘 + 500MB 上限 + 200ms 节流。安装：`install_update` canonicalize 限目录 + install_apk，不立刻删 APK，孤儿包由 `cleanup_stale_apks`（24h）收尾。

## Connections

- [[campus-detect]]：校园网判定与 SSID 感知探针来源（`run_check_once` `monitor_loop.rs:1401`、`auto_login_on_start` `monitor_loop.rs:570`）
- [[android-state]]：cached_source_ip / config 缓存宿主（`AndroidState`，`lib.rs:42` manage）
- [[login-history]]：manual/auto 登录注销历史落账（`protocol_cmds.rs:46-52`、`125-131`；`monitor_loop.rs:1497-1503`）
- [[quality-history]]：质量趋势落盘（`run_quality_once` `quality_cmds.rs:19-49`）
- [[system-cmds]]：get_init_data 聚合启动初值（`status_value` `monitor_loop.rs:131-152`）
- [[self-service-cmds]]：自助大厅与实名核身命令面（`lib.rs:81-88` 注册）
- [[identity-gate]]：人脸/生物识别门控
- [[battery-cmds]] / [[face-model-cmds]]：电池策略与人脸模型下载（face 下载复用 `update_cmds::allowed_url` 与 `verify_file_sha256`，`update_cmds.rs:139-159`、`517-544`）
- 共享协议核心 `campus_login_lib`：`auth::protocol`、`auth::portal`、`network::quality`、`config::{schedule, night_switch, outbound_switch, scheduled_tick}`、`infra::logger`
- 决策文章：`decisions/night-outbound-switch`、`decisions/android-keepalive-fgs-architecture`

## Known Issues

1. **AccountItem.password_saved 判据已修复**：走 `password_saved_at` 密文位（`config_state.rs:281-287`、`account_cmds.rs:111-121`）。
2. **Settings::default 与 v7 迁移夜切默认不一致（定案）**：`enable_night_operator_switch` 默认 2026-10-02 起关闭（`config_state.rs:149`），存量已落盘 true 不迁移；迁移侧 v7 刷开仅旧默认（`config_state.rs:413-427`）。
3. **night_outbound_restore 魔法字符串状态位**：空串/非空即状态机，误写非空值会静默压住整轮巡检（`run_check_once` 闸 `monitor_loop.rs:1354-1357`）；三处写入（`872-873`、`892-893`）与读取（`706-711`）无类型保护。
4. **桌面专属字段镜像不消费**：outbound_priority 等（`config_state.rs:48-59`、`65-67`）仅为契约镜像；安卓出站切换走 night_outbound_restore + 系统 network_avoid_bad_wifi（`monitor_loop.rs:1129-1192`）。
5. **host 编译能力编译期禁用**：移动插件（`lib.rs:35-42`）与 cfg(mobile) 分支在 host 上恒 Err；`cargo test`（host）只覆盖纯函数与文件 IO。
6. **#[allow(dead_code)] 历史遗留**：`current_settings`（`config_state.rs:524`）仍带标注；实际已被调用。
7. **delete_account 不清 night_operator_restore**：删当前账号只清 active_account 与 night_outbound_restore（`account_cmds.rs:352-355`），运营商恢复位残留。
8. **do_login operator 回退时机**：operator_len 记录于回退配置值之前（`protocol_cmds.rs:37-42`）。
9. **MAX_DOWNLOAD_BYTES 与 content_length 无联动**：响应无 Content-Length 时进度 percent 恒 0（`update_cmds.rs:421-473`）。
10. **夜切验证时序硬编码**：NIGHT_VERIFY_*（`monitor_loop.rs:1196-1199`）不可配，弱网整轮可达数十秒。
