---
title: desktop-rust-core（桌面端 Tauri 后端核心）
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

七个源文件全部位于 `android/src-tauri/src/` 下；文中引用的 `campus_login_lib::*` 是双端共享的桌面协议核心 crate，安卓端经 Cargo path 依赖单点继承（协议实现单点存在于桌面 crate，安卓禁止复制，见 CodeWiki 的 `decisions/protocol-core-single-source` 与 `concepts/dual-platform-sharing`）。安卓侧各文件的文档注释显式声明与桌面同名模块的对应关系：`protocol_cmds.rs:1-3`（命令名与参数与桌面对齐）、`account_cmds.rs:1-2`（与桌面 `commands/account.rs` 同构）、`config_state.rs:1-3`（与桌面 `config::Config` 对齐）、`monitor_loop.rs`（启动恢复语义对齐桌面 `startup.rs`）。桌面专属实现（出站路由切换的跃点/适配器维度）只按名义边界标注，不复述。

三条贯穿全模块的纪律：

1. **敏感信息唯一出站出口**：密码落盘为密文（`CryptoBridge` → `campus_keystore`，`config_state.rs:204-240`），外发配置恒经 `masked_for_display` 掩码（`config_state.rs:424-430`，非空密码 → `"***"`）；日志/错误/事件 payload 不得携带 password（`protocol_cmds.rs:1-3`），运营商归属只打 `user_len`/`operator_len` 长度、不打原文（`protocol_cmds.rs:36-37`）。

2. **配置读改写全局串行**：`CONFIG_IO_LOCK`（`tokio::Mutex`，`config_state.rs:17-19`）由锁的使用者统一加锁；`account_cmds` 的 `config_io_lock` 是对 `config_state::CONFIG_IO_LOCK` 的转发（`account_cmds.rs:20-22`），防止「读旧配置 → 账号命令落盘 → 旧基线覆盖」的丢更新。

3. **循环生命周期用代际 token 而非布尔**：`MONITOR_GENERATION`（`monitor_loop.rs:30-32`）与 `LATENCY_GENERATION`（`quality_cmds.rs:11-17`）在 stop 时先推进代际再清 running 标志，旧循环凭代际失配自然退出，杜绝 stop→start 竞速中旧循环误清新标志。

## Key Components

### 应用装配 lib.rs（143 行）

| 符号 | 行号 | 职责 |
| --- | --- | --- |
| 模块清单（16 个子模块） | `lib.rs:11-26` | campus_detect、android_state、config_state、cpu_affinity、identity_gate、login_history、protocol_cmds、self_service_cmds、monitor_loop、account_cmds、system_cmds、quality_cmds、quality_history、update_cmds、face_model_cmds、battery_cmds |
| `run()` | `lib.rs:30-133` | `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 入口 |
| 移动插件链 | `lib.rs:33-40` | `#[cfg(mobile)]` 下注册 campus_network_bind / campus_keystore / campus_monitor_service / biometric（该 crate `#![cfg(mobile)]`，host 编译为空）/ notification，并 `manage(AndroidState::default())`（`lib.rs:40`） |
| host 支路 | `lib.rs:41-42` | 仅 `manage(AndroidState::default())`，无插件 |
| setup 钩子 | `lib.rs:45-64` | 见下文 |
| `invoke_handler` | `lib.rs:65-122` | `generate_handler` 注册 55 条 IPC 命令 |
| `Builder::build` | `lib.rs:123-124` | `unwrap_or_else` 直接 panic |
| 退出护栏 | `lib.rs:125-131` | `RunEvent::ExitRequested` → `api.prevent_exit()` |

setup 细节：日志目录 `app_data_dir/logs`（`lib.rs:47-51`）；`init_logger` 失败必须 `eprintln!("[startup] 日志初始化失败,日志将不可用")`（`lib.rs:52-56`，不静默）；spawn `monitor_loop::run_startup_tasks`（`lib.rs:57-58`）；spawn `config_state::cleanup_stale_config_tmp` 清理孤儿 tmp（`lib.rs:59-62`）。

命令注册分组（`lib.rs:66-122`，共 55 条）：

| 模块 | 行号 | 命令 |
| --- | --- | --- |
| protocol_cmds | `lib.rs:67-74` | ping_test、bind_to_wifi、accept_wifi_network、get_avoid_bad_wifi_status、restore_written_settings、do_login、do_logout、check_portal_status |
| campus_detect | `lib.rs:75-76` | check_campus_status、request_wifi_ssid_permission |
| config_state | `lib.rs:77-78` | get_config、save_config |
| self_service_cmds | `lib.rs:79-86` | verify_biometric_identity、bind_operator、query_bind_status、query_self_dashboard、query_self_online_log、self_offline_session、change_self_password、reveal_operator_credential |
| account_cmds | `lib.rs:87-93` | list_accounts、switch_account、rename_account、save_current_as_account、create_account、delete_account、get_active_account |
| system_cmds | `lib.rs:94-100` | get_init_data、get_logs、clear_logs、get_log_retention_days、set_log_retention_days、get_debug_mode、set_debug_mode |
| monitor_loop | `lib.rs:101-108` | start_background_check、stop_background_check、trigger_background_check、get_background_status、get_boot_autostart、set_boot_autostart、get_notification_enabled、set_notification_enabled |
| battery_cmds | `lib.rs:109-112` | 4 条 |
| quality_cmds | `lib.rs:113-115` | 3 条 |
| update_cmds | `lib.rs:116-119` | 4 条 |
| face_model_cmds | `lib.rs:120-121` | 2 条 |

退出护栏：`RunEvent::ExitRequested` → `api.prevent_exit()`（`lib.rs:125-131`）。根因是 tauri #15671：安卓上划掉任务只关 UI，进程随前台服务常驻；不拦住会把常驻服务一起杀掉。

### 配置状态 config_state.rs（827 行）

常量与锁：

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `PASSWORD_MASK` | `config_state.rs:10` | `"***"`，掩码出站 |
| `CONFIG_FILE` | `config_state.rs:11` | `"config.json"` |
| `CONFIG_IO_LOCK` | `config_state.rs:17-19` | `tokio::Mutex`，全局配置写串行 |
| `config_io_lock()` | `config_state.rs:21-24` | 供 `account_cmds` 等转发加锁 |

`Settings` 结构（IPC 契约，camelCase serde，`config_state.rs:26-128`）全字段：

| 字段 | 行号 | 默认值（`Default` impl `config_state.rs:130-200`）/ 说明 |
| --- | --- | --- |
| user | `config_state.rs:30` | 学号/账号 |
| password | `config_state.rs:31` | 运行态明文；落盘时刻意清空（`save_file` `config_state.rs:292-295`） |
| self_hello_enabled | `config_state.rs:32` | default true（135） |
| self_reverify_each_action | `config_state.rs:33` | default false（136） |
| allow_2d_face_verify | `config_state.rs:34-36` | default false（137） |
| operator | `config_state.rs:37` | 运营商后缀（如 `@telecom`，空=校方默认线路） |
| enable_night_operator_switch | `config_state.rs:38-41` | default false（141）；2026-10-02 起默认改回关闭，存量已落盘 true 不迁移（注释 `config_state.rs:139-140`） |
| night_operator_restore | `config_state.rs:42-43` | 运营商夜切恢复位 |
| enable_night_outbound_switch | `config_state.rs:44-47` | default false（143） |
| outbound_priority | `config_state.rs:48-50` | 桌面专属镜像字段，安卓不消费 |
| outbound_metric_restore | `config_state.rs:51-53` | 同上 |
| outbound_disabled_adapters | `config_state.rs:54-56` | 同上 |
| outbound_standby_route | `config_state.rs:57-59` | 同上 |
| night_outbound_restore | `config_state.rs:60-64` | 出站切换态标记，魔法值 `"logged_out"`；非空=切换中，巡检整轮跳过 |
| dns_optimize_adapters | `config_state.rs:65-67` | 桌面专属镜像字段 |
| device_aliases | `config_state.rs:68-72` | `#[serde(default)]`（71），12 位 hex MAC → 别名 |
| auto_login_on_start | `config_state.rs:74` | default true（151） |
| enable_background_check | `config_state.rs:75` | default true（152） |
| background_check_interval | `config_state.rs:76` | default 60_000（155） |
| background_check_idle_interval | `config_state.rs:77-81` | default 300_000（157），闲时 5min |
| auto_login_on_preparation | `config_state.rs:82` | default true（158） |
| max_disconnect_reconnect | `config_state.rs:83` | default 3（159） |
| auto_login_cooldown_secs | `config_state.rs:84` | default 60（160） |
| theme_mode | `config_state.rs:86` | default "dark"（161） |
| enable_notification | `config_state.rs:87` | default true（162） |
| custom_theme_color | `config_state.rs:88` | default "#6366f1"（163） |
| default_panel | `config_state.rs:89` | default "dashboard"（164） |
| display_name | `config_state.rs:90-93` | 当前账号显示名 |
| active_account | `config_state.rs:94` | 当前账号 id |
| enable_boot_autostart | `config_state.rs:95` | default false（167） |
| enable_latency_test | `config_state.rs:97` | default false（168） |
| latency_test_interval | `config_state.rs:98` | default 600_000（170） |
| enable_network_quality | `config_state.rs:99` | default false（174） |
| skip_ttfb_in_latency | `config_state.rs:100` | default true（175） |
| skip_content_in_latency | `config_state.rs:101` | default true（176） |
| portal_url | `config_state.rs:103` | default "http://10.1.99.100"（177） |
| fixed_gateway | `config_state.rs:104` | default "10.2.127.254"（178） |
| required_network_name | `config_state.rs:105` | default "i-wxxy、iwxxy-2、iwxxy-3"（181，2026-10-01 对齐桌面三 SSID；旧单值经 `load_from` 幂等扩为三值 `config_state.rs:332-334`） |
| enable_network_name_check | `config_state.rs:106` | default true（182） |
| campus_gateway | `config_state.rs:107` | default "10.2.127.254"（183） |
| campus_check_start_minutes | `config_state.rs:108-109` | default 460（185，即 7:40） |
| campus_check_end_minutes | `config_state.rs:110-111` | default 1380（188，即 23:00） |
| scheduled_login_minutes | `config_state.rs:112-113` | default 1440（192，哨兵值=禁用） |
| scheduled_logout_minutes | `config_state.rs:114-115` | default 1440（193，哨兵值=禁用） |
| update_source | `config_state.rs:116-118` | default "mirror"（194） |
| log_retention_days | `config_state.rs:120` | default 7（195） |
| config_schema_version | `config_state.rs:121-127` | default 7（197） |

方法与类型：

| 方法/类型 | 行号 | 职责 |
| --- | --- | --- |
| `CryptoBridge` | `config_state.rs:204-207` | encrypt/decrypt 回调（`Arc<dyn Fn>`） |
| `CryptoBridge::from_app` | `config_state.rs:210-240` | mobile → `tauri_plugin_campus_keystore`（211-230）；not(mobile) 恒 Err "Keystore 仅移动端可用"（231-238） |
| `EncodedSettings` | `config_state.rs:242-248` | 磁盘形态 `{settings, passwordCipher}` |
| `load_file` | `config_state.rs:250-263` | 文件不存在 → default（253）；解析失败 Err "配置损坏"（255-256）；解密失败置空不报错（259-261） |
| `password_saved_at` | `config_state.rs:270-276` | 「密码已保存」判据=密文位 passwordCipher 非空；外露字段恒空，用户真机反馈后修复（doc 265-269） |
| `save_file` | `config_state.rs:278-308` | 建目录（279-283）；非空密码加密，失败 eprintln 诊断 + Err 绝不落明文（284-291）；落盘前 `settings.password` 置空（292-295）；pid 唯一 tmp + rename 防陈旧 tmp 竞争（297-306） |
| `cleanup_stale_config_tmp` | `config_state.rs:313-324` | 清 `*.json.<pid>.tmp` 孤儿（startup spawn，`lib.rs:59-62`） |
| `load_from` | `config_state.rs:326-336` | load_file + `migrate_legacy_defaults`（328）+ 旧单值 SSID 幂等扩三值（332-334） |
| `migrate_legacy_defaults` | `config_state.rs:355-417` | v0→v7 一次性迁移（doc 338-354）：v0→v1 间隔 15s→60s（357-359）；legacy(<3) 自动化开关刷开（360-374）；v4 闲时间隔 300s（375-383）；v5 检测终点 0→1380（384-392）；v6 质量间隔 60s→600s（393-401）；v7 夜切默认开 + 定时 0→1440（402-416）；每段落盘失败 log_warn、幂等重迁 |
| `save_to` | `config_state.rs:419-421` | `save_file` 薄封装 |
| `masked_for_display` | `config_state.rs:424-430` | 非空密码 → `"***"`；敏感出站唯一出口 |
| `emit_config_changed` | `config_state.rs:436-439` | emit `config-changed`，payload `{"config": masked}`（订阅方复用掩码，不产生第二出口） |
| `resolve_password_field` | `config_state.rs:442-451` | clear → 空；空串/MASK → 回退已存值；否则新值（与桌面同构语义） |
| `get_config` | `config_state.rs:453-466` | load_from + 刷新 AndroidState.config 缓存 + masked 返回 |
| `save_config` | `config_state.rs:468-499` | 全程持 CONFIG_IO_LOCK（477）→ load_from 基线（483）→ `resolve_password_field`（485-486）→ save_to（487）→ 刷缓存（488-491）→ `auto_create_account_for_current`（494）→ `emit_config_changed`（497） |
| `current_settings` | `config_state.rs:503-520` | 缓存优先（504-509），未命中读盘回填（510-518）；`#[allow(dead_code)]`（502）为历史标注，实际被 protocol_cmds 使用 |

tests：`config_state.rs:523-827`（base64 假加密桥、加密落盘不含明文往返、解密失败置空、SSID 扩三值、默认值业务语义、迁移幂等（迁移后用户改回不被覆盖）、camelCase 形状、掩码出口、密码回退语义与桌面同构、空密码不写密文位、密码已保存判据）。

### 多账号 account_cmds.rs（838 行）

与桌面 `commands/account.rs` 同构：一账号一 JSON（同 `EncodedSettings` 格式，仅目录不同 `accounts/`）、密码密文落盘、切换只合并登录字段。

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `ACCOUNT_NAME_RE` | `account_cmds.rs:12-15` | `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$` |
| `config_io_lock` | `account_cmds.rs:20-22` | 转发 `config_state::CONFIG_IO_LOCK`（锁本体在锁的使用者侧统一） |
| `AccountItem` | `account_cmds.rs:27-40` | 账号卡脱敏展示 `{id, displayName, user, operator, passwordSaved}`（29/30-31/34-35/36-37/38-39）；密码只出布尔 |
| `AccountResult` | `account_cmds.rs:42-55` | `{success, message, active_account, display_name, config}`，Option 全 `skip_serializing_if`；构造器 ok/ok_with_account/err（58-66） |
| `sanitize_account_id` | `account_cmds.rs:72-83` | 白名单外字符 → `_`，截断 32 字符，空入空出（调用方跳过建号），全非法 → 全下划线仍建号 |
| `validate_account_name` | `account_cmds.rs:86-94` | 1-32 字符 + 白名单正则，防路径穿越 |
| `accounts_dir` | `account_cmds.rs:96-102` | `app_data_dir/accounts` |
| `read_account_card_info` | `account_cmds.rs:111-121` | 从账号 JSON 读展示字段不解密；password_saved 走 `password_saved_at` 密文位判据（取 `settings.password` 会恒"未保存"） |
| `list_account_items_sync` | `account_cmds.rs:126-155` | 只认 `.json`（130）；跳过 `.` 开头与空 id（137）；空显示名兜底 id（142）；读取失败 log_warn 兜底（145-148）；按 id 排序（153） |
| `list_accounts` | `account_cmds.rs:157-163` | spawn_blocking 包装 |
| `load_account_file` | `account_cmds.rs:165-170` | 不存在 → `Ok(None)` |
| `persist_current` | `account_cmds.rs:172-183` | save_to + 刷缓存 + masked |
| `switch_account` | `account_cmds.rs:185-227` | 持锁（198）；账号不存在 err（199-201）；合并 user/password/operator（205-208）；清 `night_operator_restore`（211，残留会把 A 运营商恢复到 B）；清 `night_outbound_restore`（214，残留让巡检整轮跳过）；display_name 空兜底 id（215-219）；`restore_avoid_bad_wifi` 收尾（225） |
| `save_current_as_account` | `account_cmds.rs:229-257` | 持锁（242）→ `merge_save_as_target`（247）→ 落盘档案（248）→ 主配置同步 active_account/display_name 防串号（252-255） |
| `create_account` | `account_cmds.rs:259-294` | user 空拒绝（271-273）；重名拒绝（279-281）；仅登录字段+显示名，不动主配置不切换（282-291） |
| `merge_save_as_target` | `account_cmds.rs:302-331` | 纯函数：另存场景（current.active_account != safe_name，307）落盘副本 display_name 置空防串号（310-312）；已存在档案 → 保留非登录字段、登录字段用当前配置覆盖（314-321）；主配置最终显示名=档案非空优先否则兜底 id（325-329） |
| `delete_account` | `account_cmds.rs:333-362` | 持锁（345）→ 删文件（346-348）；删当前账号 → 清 active_account（352）+ 清 night_outbound_restore（355）+ persist（356）+ `restore_avoid_bad_wifi`（358）；非当前 → ok(masked)（361）；见 Known Issues #7 |
| `get_active_account` | `account_cmds.rs:364-367` | |
| `rename_account` | `account_cmds.rs:369-401` | 业务错误走 IPC `Err(String)`（switch 走 `AccountResult::err` 通道，双通道各自适用，与桌面同构，doc 375-380）；激活账号同步 display_name（390-394） |
| `rename_account_core` | `account_cmds.rs:405-429` | 校验显示名（411）→ 存在性（413-415）→ 重名检查排除自身、对方空显示名兜底 id 参与（420-424）→ 只改 displayName 写回（426-427） |
| `validate_display_name` | `account_cmds.rs:434-447` | trim 后 1..=32 字符；控制字符/换行拒绝；空格/emoji/`@` 允许（显示名与内部 id 分离，不复用文件名白名单） |
| `auto_create_account_for_current` | `account_cmds.rs:451-460` | R2 自动建号入口，失败仅 log_warn |
| `auto_create_account_in` | `account_cmds.rs:473-510` | 五规则（doc 462-472）：user/password 空跳过（478-480）；id=sanitize(user)、空跳过（481-484）；不存在 → 以快照建号，display_name=原始 user 文本、文件内 active_account=id（486-491）；已存在且 user 原文不同 → 撞库 log_warn 跳过不覆盖凭据（493-501）；user 一致且 password/operator 一致 → 幂等短路不写盘（502-504），有差异 → 只更新 user/password/operator 三字段保留 display_name（505-509） |

tests：`account_cmds.rs:512-838`（消毒放行/路径穿越拒绝/长度边界、列举过滤排序兜底、sanitize 边界（`@@#`→`___`、截断 32）、显示名边界（33 拒 32 过、emoji 过）、自动建号幂等短路（mtime 不变）、改名核心、`AccountItem` JSON 形状锁（displayName/passwordSaved，无 snake_case 冗余键）、另存显示名不串号/覆盖同 id 保留自定义名、撞库防护（2023.1 与 2023/1 同 id 2023_1，后到跳过））。

### 登录注销与 Portal 探测 protocol_cmds.rs（390 行）

敏感纪律（`protocol_cmds.rs:1-3`）：日志/错误/事件 payload 不得携带 password。

| 函数 | 行号 | 职责 |
| --- | --- | --- |
| `do_login` | `protocol_cmds.rs:10-63` | `ensure_wifi_bound` 先行（21，手动登录此前漏了绑定，WiFi+流量同开默认路由落蜂窝）；user/password 空回退 `current_settings`（23-31）；空凭据 Err "账号与密码不能为空"（32-35）；日志只打 user_len/operator_len（36-37）；operator 空回退配置（38-42）；`run_login`（44）；`login_history::append(..., "manual")`（46-52）；成功清 consecutive_failures/logout_protected_until_ms + 置 was_online=true（53-61）——手动登录是在线权威证据，漏置位会让夜间出站切换前置在线检查误判 |
| `run_login` | `protocol_cmds.rs:66-90` | `cached_adapter_ip`（72）；spawn_blocking 调共享 `campus_login_lib::auth::protocol::do_login_with_retry(user, password, operator, adapter_ip.as_deref(), 3, &is_quitting)`（77-87）；同步函数内部 block_on_http 禁止 async 直调（76） |
| `do_logout` | `protocol_cmds.rs:94-143` | `ensure_wifi_bound`（101）；user 回退（103-107）；空凭据拒绝（108-110）；spawn_blocking `do_logout_with_retry`（Radius+MAC 解绑两步）（113-123）；login_history 落账（125-131）；注销保护期 `logout_protected_until_ms = now + 60_000`（134-141），防后台检测冷却后自动登回 |
| `check_portal_status` | `protocol_cmds.rs:147-164` | `ensure_wifi_bound`（153）；IP 回退缓存（154-157）；spawn_blocking `check_portal_full(&ip, None)`（159-163） |
| `cached_adapter_ip` | `protocol_cmds.rs:166-174` | 读 `AndroidState.cached_source_ip` |
| `ping_test` | `protocol_cmds.rs:176-179` | → "pong" |
| `bind_to_wifi` | `protocol_cmds.rs:183-196` | mobile 插件（184-190）；not(mobile) Err "网络绑定仅安卓端支持"（191-195） |
| `accept_wifi_network` | `protocol_cmds.rs:201-214` | 同构（206-207 / 209-213） |
| `get_avoid_bad_wifi_status` | `protocol_cmds.rs:220-235` | WRITE_SECURE_SETTINGS 授权状态与 avoid_bad_wifi 值/快照（224-228）；not(mobile) Err（230-234） |
| `restore_written_settings` | `protocol_cmds.rs:240-255` | 插件还原写入的系统设置（244-248）；not(mobile) Err（250-254） |
| `ensure_wifi_bound` | `protocol_cmds.rs:269-338` | pub(crate)。`bind_capability()==Supported`（旁路代理 SO_BINDTODEVICE 生效）时跳过（274-279，secure VPN 下会被 netd EPERM 拒绝、打误导警告）；JNI spawn_blocking（280-285）；bound 且 path != "already_bound" 才 `clear_client_pool()`（291-294）——keep-alive 连接的 fwmark 是绑前打的，复用等于没绑；already_bound 不清防启动 1 秒 6 次并发反复重建付 TLS 握手；结果 log_info + eprintln 双写（296-312）；随后 `accept_campus_wifi`（316）；失败 warn 回落默认路由（318-336） |
| `accept_campus_wifi` | `protocol_cmds.rs:350-390` | 三路径：反射 hidden API `setAcceptUnvalidated` → 回退写 Settings.Global（captive_portal_mode=0 + network_avoid_bad_wifi=0）；Kotlin 侧无动作返回 already_validated（351-389） |

### 后台监控 monitor_loop.rs（1710 行）

全局状态与常量：

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `MONITOR` | `monitor_loop.rs:16` | `MonitorState` lazy_static |
| `OUTBOUND_RESTORE_FAIL_COUNT` / `OUTBOUND_RESTORE_MAX_FAILS=3` | `monitor_loop.rs:19-23` | 晨间还原连续失败上限，达上限放弃建态恢复巡检 |
| `OUTBOUND_SWITCH_FAIL_COUNT` / `OUTBOUND_SWITCH_MAX_FAILS=3` | `monitor_loop.rs:24-28` | 注销未确认重试上限 |
| `MONITOR_GENERATION` | `monitor_loop.rs:30-32` | AtomicU64 代际 token，防 stop→start 竞速误清 running |
| `LOGIN_ATTEMPT_LOCK` | `monitor_loop.rs:34-37` | `tokio::sync::Mutex`，attempt 段互斥（判在线 → should_attempt_login → run_login → 计数） |
| `MonitorState` | `monitor_loop.rs:39-69` | 字段见下 |
| `WIFI_EVENT_DELAY_MS=2500` | `monitor_loop.rs:71-74` | 连上等 DHCP/路由稳定 + 风暴合并窗口 |
| `WIFI_LOST_EVENT_DELAY_MS=0` | `monitor_loop.rs:76-79` | 断开即事实 |
| `WIFI_EVENT_DEBOUNCE_MS=1000` | `monitor_loop.rs:81-82` | 风暴去抖 |
| `NIGHT_VERIFY_WAIT_SECS=15` / `RETRY_SECS=5` / `ATTEMPTS=3` / `HTTP_TIMEOUT_SECS=15` | `monitor_loop.rs:1196-1199` | 夜切验证时序（硬编码，见 Known Issues #10） |

`MonitorState` 字段（`monitor_loop.rs:39-69`）：running、check_count、consecutive_failures、reconnect_count（达 max_disconnect_reconnect 暂停，回在线清零）、last_login_attempt_ms、was_online、last_result（`Mutex<Option<Value>>`）、desired_interval_ms（改间隔立即生效）、idle_interval_ms、logout_protected_until_ms（手动注销 60s 保护）、wifi_event_ms（去抖）、notified_online（u8：0 未展示 / 1 在线 / 2 未连接 / 3 非检测时段——仅翻转才重建常驻通知省电）、scheduled_login_day / scheduled_logout_day（AtomicI32 当日序号 `num_days_from_ce`，0=未触发，跨天自然重置）。

纯函数：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `wifi_event_is_burst_start` | `monitor_loop.rs:84-89` | 首事件或距上次 >= 窗口 → 风暴起点 |
| `effective_interval_ms` | `monitor_loop.rs:91-103` | `base.max(5000)`；screen_on && wifi_connected → base，否则 `idle_ms.max(base)` |
| `should_attempt_login` | `monitor_loop.rs:105-128` | online \|\| !on_campus → false；!was_online && !auto_login_on_preparation → false（掉线重连不受自动登录开关管）；reconnect_count >= max → false；millis_since_last_attempt >= cooldown*1000 |
| `status_value` | `monitor_loop.rs:130-152` | isRunning/checkCount/consecutiveFailures/reconnectCount/lastResult/watcherFailed + 展平最近一次检测 online/message/onlineOperator/serverAvailable/onCampusNetwork 到顶层（首轮 emit 先于 WebView 监听建立，getInitData 是唯一可靠启动初值来源） |
| `is_running` | `monitor_loop.rs:155-157` | |
| `notify_state` | `monitor_loop.rs:1578-1586` | (true,\_)→(1,"在线")；(\_,wifi_off)→(4,"WiFi 未连接")；else (2,"未连接")；3 被「非检测时段」占用，不得复用 |
| `portal_origin` | `monitor_loop.rs:1202-1214` | 取 scheme+host 去路径端口与查询；空回退 `default_portal_url`（1205-1209） |
| `expected_operator_display` | `monitor_loop.rs:1217-1223` | 空 → "无锡学院"，其余原样 |

事件与展示辅助：`emit_login_log` → "login-log" 事件（`monitor_loop.rs:166-170`）；`notify_system`（175-192）：enable_notification=false 静默，mascot 为 drawable 资源名作通知大图、资源缺失降级纯文本；`with_vpn_hint`（194-204）：vpn_tun_present 时追加「请在 VPN 中排除本应用或临时关闭 VPN」。

命令生命周期：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `start_background_check` | `monitor_loop.rs:207-256` | 读配置刷 desired_interval_ms（下限 5000）/idle_interval_ms；已 running → 仅刷新间隔 + 重发 start_monitor 保活（Rust 循环活着不代表 FGS 活着，START_STICKY 幂等）；先 FGS start_monitor 成功才置 running（防假"运行中"）；notified_online=0；start_wifi_watcher；gen=MONITOR_GENERATION.fetch_add 后 spawn monitor_tick_loop |
| `stop_background_check` | `monitor_loop.rs:259-271` | 先推进代际再清 running + stop_wifi_watcher + stop_monitor |
| `start/stop_wifi_watcher` | `monitor_loop.rs:274-320` | 建 `tauri::ipc::Channel` 回调 handle_wifi_event；失败置 WATCHER_FAILED（status_value 暴露，退化为纯周期检测） |
| `handle_wifi_event` | `monitor_loop.rs:322-363` | !is_running 兜底；`wifi_event_is_burst_start` 去抖；lost 零延迟；延迟后任务先校验 wifi_event_ms==now 否则过期自杀（风暴内最后事件生效） |
| `trigger_background_check` | `monitor_loop.rs:366-374` | 未运行 Err "后台监控未运行,无法触发检测" |
| `get_background_status` | `monitor_loop.rs:377-379` | → `status_value` |
| `get/set_boot_autostart` | `monitor_loop.rs:383-422` | 插件 is/set_boot_autostart_enabled + 持 config_io_lock 改配置 |
| `get/set_notification_enabled` | `monitor_loop.rs:426-446` | 同款 |

启动与登录编排：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `run_startup_tasks` | `monitor_loop.rs:458-529` | spawn + 500ms 就绪窗；读配置失败 → log_error + emit "startup-failed"{reason} + 尽力 stop_monitor（BootReceiver 可能已拉起 FGS 假通知）；set_log_retention_days；并行 spawn 三链：start_background_check（若 enable_background_check）、质量首测（enable_latency_test → start_latency_test，否则 check_network_quality 单次作首屏来源）、start_update_check_loop（24h 同语义）+ cleanup_stale_apks；auto_login_on_start（若开启）；night_outbound_restore 非空（切换态收尾被杀残留）→ `restore_avoid_bad_wifi` 兜底 |
| `probe_with_retry` | `monitor_loop.rs:535-553` | probe_campus_with_ssid 失败/非 campus 3s 后重试一次，retry Err 回退 first |
| `emit_auto_login_result` | `monitor_loop.rs:559-565` | → "auto-login-result" 事件 |
| `auto_login_on_start` | `monitor_loop.rs:567-612` | user/password 空跳过；ensure_wifi_bound；probe_with_retry；cache_source_ip；!on_campus 跳过；run_login 成功 → 置 was_online + 清注销保护 + 应用内 toast，失败 → 系统通知 with_vpn_hint("mascot_offline") + 应用内 toast |
| `ProbeWindowGuard` | `monitor_loop.rs:617-625` | Drop 调 `end_probe_window`——任何 early return / panic unwind 都释放 WifiLock/WakeLock |
| `power_state` | `monitor_loop.rs:631-641` | 插件 get_power_state 失败保守 (true, true) |
| `monitor_tick_loop` | `monitor_loop.rs:643-702` | 主循环，见下文 |

`monitor_tick_loop`（`monitor_loop.rs:643-702`）：interval 下限 5000；启动对账——当日 scheduled_logout_minutes 已过 → 置 scheduled_logout_day 消耗额度不执行（防冷启动首拍立即注销，「用户刚开监控就被踢下线」），定时登录不消耗；每拍顺序：代际校验（!is_running \|\| gen 失配 → break）→ `run_scheduled_actions`（置于分档静默期之前，定时动作常在静默期）→ 间隔热更新（want != current 重建 tick；tokio interval 重建后首 tick 立即返回要吃掉一次）→ power_state + `effective_interval_ms` 分档跳拍 → `run_check_once`；退出时代际匹配才清 running。

出站切换辅助：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `outbound_switch_active` | `monitor_loop.rs:706-711` | night_outbound_restore 非空；读取失败按未切换（宁多巡检不漏检） |
| `latest_settings` | `monitor_loop.rs:716-723` | 切换/还原写盘前重读最新配置，防 stale 快照覆盖 |
| `logout_for_outbound_switch` | `monitor_loop.rs:731-756` | !was_online → 跳过请求直接 true 并清 OUTBOUND_SWITCH_FAIL_COUNT；do_logout 后 `radiusOk \|\| unbindOk` 判离线生效（只看 success 会把 (false,true) 误判失败导致掉线重连 → 再注销整夜摆动）；生效清计数 + was_online=false |
| `switch_logout_unconfirmed` | `monitor_loop.rs:761-778` | fetch_add 计数；达 3 上限按「断网规律」建态 true，否则 false 下一拍重试 |
| `portal_probe_on_little_cores` | `monitor_loop.rs:784-821` | 专用短命裸线程（不占 tokio/spawn_blocking 共享池）+ `cpu_affinity::pin_current_thread_to_little_cores` + `Handle::try_current` 线程内 enter（裸线程无 reactor context 会 panic "there is no reactor running"，2026-09-11 真机每拍必现）（798-799）+ catch_unwind 兜底转 Err（803-806）；线程创建失败降级 spawn_blocking（809-819） |

`run_scheduled_actions`（fn `monitor_loop.rs:835-1037`，doc 823-834）编排：

- 读配置失败静默 return（836-839）。
- 出站动作判定（846-864）：`outbound_active = !night_outbound_restore.is_empty()`（848）；调共享编排 `evaluate_outbound_action`（`campus_login_lib::config::outbound_switch`，856-863）。
- Switch 分支（866-886）：`logout_for_outbound_switch`（869）→ latest_settings 置 "logged_out"（872-873）→ persist（874）→ outbound_active=true → `ensure_avoid_bad_wifi`（881）→ `trigger_wifi_recheck`（882）→ emit。
- Restore 分支（887-924）：`night_switch_login`（890）→ 清 OUTBOUND_RESTORE_FAIL_COUNT（891）→ 置空标记（892-893）→ persist（894）→ `restore_avoid_bad_wifi`（898）；失败达 OUTBOUND_RESTORE_MAX_FAILS → 放弃清标记恢复巡检（901-923）。
- None（925）；出站动作后重读配置 `latest_settings`（929），防 stale 快照复活标记。
- 运营商夜切 `evaluate_night_switch_gated`（939-980）：SwitchToCampus → restore=operator、operator=""（953-955）→ persist（957）→ `night_switch_login`（958）→ spawn `verify_night_switch`（960-963）；Restore → operator=take(restore)（967-968）→ persist（970）→ login（971）→ verify（972-975）。
- 双禁用早退（scheduled_login_minutes/logout_minutes == SCHEDULED_DISABLED_MINUTES=1440）（981-984）。
- 定时判定 `evaluate_scheduled_actions`（`campus_login_lib::config::schedule` 共享纯函数）（993-1001），传 outbound_active=拍后值——切换态下定时登录跳过但不消耗当日标记（过点补触发语义要求还原成功后仍能补登）。
- fire_login（1002-1014）：置 scheduled_login_day（1003）→ emit → `auto_login_on_start`（1013）。
- fire_logout（1016-1036）：置 scheduled_logout_day（1017）→ emit → `do_logout`（1028-1035）。

夜切与系统设置：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `persist_settings` | `monitor_loop.rs:1042-1065` | save_to → 刷缓存 → `auto_create_account_for_current`（1059）→ `emit_config_changed`（1063）；失败 return false 调用方下一拍重试 |
| `night_switch_login` | `monitor_loop.rs:1071-1097` | 空凭据 emit + false（1072-1075）；ensure_wifi_bound（1077）；`run_login`（1079）；成功 → was_online=true + 清保护期（1084-1089，登录是在线权威证据）；无校园网探测闸（断网窗口探测不通过会被 auto_login_on_start 的闸无声吞掉），仅保留空凭据防御 |
| `trigger_wifi_recheck` | `monitor_loop.rs:1103-1124` | spawn_blocking `report_wifi_unusable`（1106-1109）——应用侧唯一合法「让出站口切蜂窝」通道（改跃点属桌面能力）；reported=false → warn + reason（1114-1120） |
| `ensure_avoid_bad_wifi` | `monitor_loop.rs:1129-1165` | WRITE_SECURE_SETTINGS（经 adb pm grant 授权后生效）写 network_avoid_bad_wifi=1；插件记原值快照晨间还原；ensured+changed → log_info 原值（1137-1149）；未授权/失败 warn（1151-1163） |
| `restore_avoid_bad_wifi` | `monitor_loop.rs:1168-1192` | 按快照还原（1170-1174）；restored → log_info（1176-1182）；无快照幂等静默（1183-1184） |
| `verify_night_switch` | `monitor_loop.rs:1229-1283` | 独立 tokio 任务：入口出站切换态守卫（1232-1234）；sleep NIGHT_VERIFY_WAIT_SECS（1235）；读配置失败静默（1236-1239）；operator 漂移守卫（1241-1243，用户手动改走则跳过）；`night_switch_verify_round` Ok → emit success return（1244-1247）；复验前再查出站切换态（1252-1254，等待期间进入切换态则「注销→复登」会打断等待窗口）；emit warning（1256）；重读配置 + operator 复查（1257-1263）；do_logout（注销失败仅 warning 继续）（1264-1267）；`night_switch_login`（1268）；再等 15s（1269）；复验 Ok → success（1270-1271）；最终不符 → notify_system "夜切验证失败" / "夜切后在线账号与期望运营商不符, 请手动检查" / mascot_alert（1272-1281） |
| `night_switch_verify_round` | `monitor_loop.rs:1287-1304` | 复用 `update_cmds::http_client`（1292）；NIGHT_VERIFY_ATTEMPTS 次重试间隔 NIGHT_VERIFY_RETRY_SECS（1294-1302）；收敛 Err(last_err) |
| `night_switch_check_once` | `monitor_loop.rs:1307-1336` | GET `{portal_origin}/drcom/chkstatus?callback=dr1003`（1313-1316）；timeout HTTP_TIMEOUT（1319）；共享纯函数 `night_switch::parse_chkstatus` + `uid_matches`（1326-1335）；不符 Err 含观测 uid（1329-1333） |

`run_check_once`（fn `monitor_loop.rs:1339-1574`，doc 1338，单次检测全编排，周期拍/WiFi 事件/手动触发三入口共用）：

1. `check_count`+=1（1340）；读配置失败 emit error return（1341-1347）。
2. 出站切换态闸（1349-1357）：night_outbound_restore 非空 → log_debug return——闸放此处因三入口共用；`run_scheduled_actions` 不过此闸（晨间还原照常）。
3. 静默期门控（1359-1383）：campus_check_start_minutes>0 且 `minutes_now < start` 或 `(end>start && minutes_now>=end)`（1363-1369）→ 跳过；notified_online.swap(3) != 3 才 update_notification "监控运行中 · 已暂停检测(非检测时段)"（1375-1379）——常驻通知不能停在上一拍"在线"（2026-09-13 真机反馈）。
4. 探针窗口（1385-1392）：`ProbeWindowGuard`，本轮持 WifiLock + 唤醒锁，窗口外释放（省电）。
5. `ensure_wifi_bound`（1394-1396）。
6. 校园网判定（1398-1411）：`probe_campus_with_ssid`（SSID 感知，2026-09-20）（1401）；`cache_source_ip`（1404）；Err → emit "校园网检测失败" + return（1407-1410）。
7. Portal 探测三态短路（1413-1424）：(Some(src), true) → `portal_probe_on_little_cores`（1421）；(Some, false) → Err "不在校园网,跳过 Portal 探测"（1422）；(None, \_) → Err "WiFi 未连接,跳过 Portal 探测"（1423）——2026-10-02 离网拍 11s→3s。
8. 三态消费（1429-1445）：error_kind=None（确定判定）才翻转 online（1441-1444）；Unknown（已在线页面特征失配）/Failed（探测超时）保持 prev_online——Portal 已在线页面特征（GBK 中文乱码、严格串匹配）间歇失配即误判掉线（真机 2026-09-09）；!on_campus 强制 false（1444）——WiFi 断开后"在线"钉死（2026-09-13）。
9. 状态机（1447-1457）：online → 清 reconnect_count/consecutive_failures/logout_protected（1448-1452）；`was_online.swap(online)`（1453）；was_online && !online && on_campus → notify_system 掉线通知（1455-1457）。
10. 自动登录双闸（1459-1523）：logout_protected（60s）（1466）+ failures_capped（consecutive_failures >= 5 熔断）（1467）+ `should_attempt_login`（1468-1479）；should && 凭据非空 → `LOGIN_ATTEMPT_LOCK` 互斥段（1483）：记 last_login_attempt_ms、reconnect_count+1（1484-1486）；达上限仅通知不 run_login（1487-1489）；run_login 成功 → login_history("auto")（1497-1503）+ 应用内 toast（1506）；协议失败/执行失败 → consecutive_failures+1 + 系统通知（1509/1516）。
11. payload（1525-1560）：serverAvailable/loginAvailable/online/message（在线="在线"、wifi_off="WiFi 未连接"、否则 portal_message）/onlineOperator（uid → `operator_suffix_from_uid` 徽标，双端同批特性安卓此前漏了后端字段，1527-1534）/timestamp/checkCount/isRunning/onCampusNetwork/campusMessage/currentSsid/sourceIp/autoLogin → 存 last_result + emit "background-check-result"（1556-1560）。
12. 常驻通知（1562-1573）：`notify_state(online, wifi_off)` 翻转才 update_notification（1567-1572）。

tests：`monitor_loop.rs:1588-1710`（notify_state 三态码、should_attempt_login 全分支、wifi_event_is_burst_start、巡检分档含下限 5s、portal_origin、expected_operator_display、status_value 展平含 onlineOperator）。

### 网络质量 quality_cmds.rs（170 行）

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `LATENCY_RUNNING` + `LATENCY_GENERATION` | `quality_cmds.rs:11-17` | AtomicBool + 代际 token |
| `run_quality_once` | `quality_cmds.rs:19-49` | 复用共享 `campus_login_lib::network::quality::check_network_quality_async("wlan0", &adapter_ip, skip_ttfb, skip_content, &fixed_gateway, is_quitting, Some(app), false)`；`quality_history::append` 落盘供趋势回溯 |
| `check_network_quality` | `quality_cmds.rs:52-54` | 单次检测命令 |
| `start_latency_test` | `quality_cmds.rs:57-74` | `LATENCY_RUNNING.swap(true)` 幂等；读配置失败必须复位标志（62-68）——否则标志卡死所有后续 start 被吞 |
| `stop_latency_test` | `quality_cmds.rs:76-83` | 先推进代际再清标志（80-81） |
| `GOOD_LEVELS` | `quality_cmds.rs:89` | `["excellent","great","good"]` |
| `BACKOFF_STABLE_STREAK` | `quality_cmds.rs:90` | 5 |
| `BACKOFF_MAX_MS` | `quality_cmds.rs:91` | 1_800_000（30min） |
| `next_backoff_interval_ms` | `quality_cmds.rs:95-101` | 纯函数：streak % 5 == 0 → `current*2.min(MAX).max(base)`，否则不变 |
| `latency_loop` | `quality_cmds.rs:103-134` | base=`latency_test_interval.max(10_000)`（104）；先检测后 sleep（107-109，原 interval 重建首 tick 立即到期）；循环头查 LATENCY_RUNNING+代际（111-113）；run_quality_once（114）；GOOD_LEVELS 含 → streak+1+退避（116-118）；波动且 streak>=5 → log_info 恢复（119-122）；else 清零（123-126）；sleep 循环尾（127）；退出仅代际匹配才清标志（131-133） |

稳态退避动机（`quality_cmds.rs:85-88`）：每 60s 打 12+ 外网目标是后台功耗/流量主力，`decisions/android-keepalive-fgs-architecture` 的「未实施备查」项，2026-09-20 落地。

tests：`quality_cmds.rs:136-170`（退避翻倍封顶、未达 5 不变、不低于 base、等级判定）。

### 自更新 update_cmds.rs（739 行）

常量与全局：

| 符号 | 行号 | 说明 |
| --- | --- | --- |
| `UPDATE_LOOP_RUNNING` | `update_cmds.rs:15-16` | 自动检查循环幂等 |
| `UPDATE_CHECKSUM` | `update_cmds.rs:17-21` | `Mutex<Option<(String,String)>>`，记账 (release tag, digest)；(tag, "") 哨兵=API 在线无 digest → 下载端拒绝；API 不可用不记账 → 下载端放行但 emit update-unverified；digest 携带 tag 防跨版本误配 |
| `VERSION_FILE` | `update_cmds.rs:24` | raw.githubusercontent.com version.json |
| `AUTO_CHECK_INTERVAL_SECS` | `update_cmds.rs:26` | 24*3600 |
| `BACKOFF_RETRY_SECS` | `update_cmds.rs:27-29` | [5min, 15min, 1h] |
| `LAST_CHECK_ERROR` | `update_cmds.rs:31-35` | 最近检查失败原因（原实现静默吞 Err，现经 lastCheckError 带出） |
| `LAST_CHECK_TIME_MS` | `update_cmds.rs:36-37` | |
| `VERSION_MIRRORS` | `update_cmds.rs:38-43` | ghfast.top / gh-proxy.com / ghproxy.net 前缀 |
| `DOWNLOAD_ALLOWED_HOSTS` | `update_cmds.rs:45-55` | 8 域名白名单（github.com、api.github.com、objects.githubusercontent.com、raw.githubusercontent.com、release-assets.githubusercontent.com、ghfast.top、gh-proxy.com、ghproxy.net），防 SSRF |
| `MAX_DOWNLOAD_BYTES` | `update_cmds.rs:57` | 500 MiB |
| `APK_STALE_SECS` | `update_cmds.rs:59-62` | 24h |
| `update_dir` | `update_cmds.rs:64-71` | `app_data_dir/files/update`——下载/安装/插件三处约定集中 |
| `DOWNLOAD_RUNNING` | `update_cmds.rs:398` | 下载互斥 |

类型：

| 类型 | 行号 | 字段 |
| --- | --- | --- |
| `ReleaseAsset` | `update_cmds.rs:73-78` | {name, url, size} |
| `UpdateInfo` | `update_cmds.rs:80-94` | {has_update(83), latest_version(84), release_notes(85), assets(86), sha256_checksum(87), last_check_error(88-91), last_check_time(92-93)}，camelCase + default |
| `DownloadProgress` | `update_cmds.rs:96-102` | {downloaded, total, speed, percent} |
| `VersionFile` | `update_cmds.rs:219-229` | version.json 真实形状 snake_case {version(225-226), notes(227-228)}——不能复用 UpdateInfo（camelCase）反序列化，否则 has_update 恒 false（≤v2.3.6 真实缺陷）(219-222) |

函数：

| 函数 | 行号 | 说明 |
| --- | --- | --- |
| `has_newer_version` | `update_cmds.rs:104-125` | 去 v 前缀逐段数值比较，短版补 0，预发布后缀段解析 0 |
| `http_client` | `update_cmds.rs:127-136` | rustls（避免 openssl 交叉编译）；connect_timeout 5s + timeout 30s（129-130）——被阻断地址 TCP 挂起，不设连接超时吃满 30s，镜像加速检查「极慢」主因 |
| `allowed_url` | `update_cmds.rs:138-159` | 白名单唯一漏斗（人脸模型下载复用，勿复制） |
| `check_update` | `update_cmds.rs:163-166` | 命令 → `check_and_record` |
| `now_epoch_ms` / `last_check_error_snapshot` / `last_check_time_snapshot` | `update_cmds.rs:168-173` / `175-177` / `179-182` | 快照助手 |
| `backoff_delay_secs` | `update_cmds.rs:184-193` | 0 → 24h；1..=3 → 档位；>3 → 24h |
| `check_and_record` | `update_cmds.rs:195-209` | 检查 + 写 LAST_CHECK_ERROR/LAST_CHECK_TIME_MS，手动自动统一 |
| `version_urls` | `update_cmds.rs:211-217` | mirror_first 按 update_source 排序，另一侧降级兜底 |
| `fetch_apk_assets` | `update_cmds.rs:231-280` | GitHub API `releases/tags/v{latest}`（238）；timeout 10s（241）；UA Wxxy-CampusLogin（244）；过滤 .apk（262）；digest "sha256:" 剥前缀（265-271）；返回 (assets, checksum, api_ok)——api_ok 区分「API 可用无 digest」与「API 不可用」（审计 B5）（232-236）；assets 空 → 前端降级外链 Releases |
| `check_update_inner` | `update_cmds.rs:282-333` | 4 源按序降级（290-331）；空 version 跳过该源（296-299）；`has_newer_version(env!("CARGO_PKG_VERSION"), latest)`（300-301）；三态记账（309-315）；UpdateInfo 带快照（324-325） |
| `start_update_check_loop` | `update_cmds.rs:335-382` | swap 幂等（341）；5s 后首查（348）；成功 fail_count=0，has_update → emit update-available + notify_system "发现新版本" mascot_update（350-365）；失败 log_warn + fail_count+=1（367-372）；wait=backoff_delay_secs（374）；退避用尽复位（375-378） |
| `get_mirror_urls` | `update_cmds.rs:384-392` | GitHub + 3 镜像 JSON |
| `download_update` | `update_cmds.rs:400-408` | swap 互斥（402-406），inner 后复位 |
| `download_update_inner` | `update_cmds.rs:410-502` | allowed_url（411）；文件名 URL 尾段、".." 拒绝、兜底 campus-login-{ver}.apk（422-428）；目录约定必须与 MonitorServicePlugin.installApk 对齐——app_data_dir=dataDir 必须拼 files 段否则 "APK 文件不存在"（v2.3.9 真机实测）（430-433）；流式写盘（449-474）；超 500MB 写入前判定+删文件（451-461）；200ms 节流 emit update-download-progress（460-473）；SHA256 三态校验（476-500） |
| `download_tag` | `update_cmds.rs:504-510` | `/releases/download/<tag>/` 提取，镜像前缀无关 |
| `verify_file_sha256` | `update_cmds.rs:512-544` | pub(crate) 人脸模型复用；非 64 位 hex → Err（525-527，原实现 true 静默放行，审计 B5）；缓冲区必须 vec 堆分配 65536——async fn 栈数组并入 future，tauri 在 JavaBridge 线程（栈小）构造 → SIGSEGV（2026-09-20 tombstone）（531-534） |
| `install_update` | `update_cmds.rs:546-570` | canonicalize 限更新目录内（556-561）；`campus_monitor_service().install_apk` 走 FileProvider（562-564）；不能立刻删 APK——install_apk 返回只代表安装器 UI 拉起，content URI 异步读取，删早撞 v2.3.9 竞态（565-568）；收尾 cleanup_stale_apks |
| `cleanup_stale_apks` | `update_cmds.rs:572-610` | DOWNLOAD_RUNNING 跳过（581-584）；只删 .apk（590-596）且 mtime 超 APK_STALE_SECS（597-603） |

tests：`update_cmds.rs:612-739`（版本比较、白名单、UpdateInfo camelCase 契约、安装目录含 files 段、清理残留、退避步进、download_tag、version_json 真实形状回归护栏）。

## Architecture

```text
lib.rs run() (lib.rs:30-133)
 ├─ setup: run_startup_tasks(monitor_loop.rs:458) + cleanup_stale_config_tmp(config_state.rs:313)
 ├─ invoke_handler 55 条命令 (lib.rs:65-122)
 └─ ExitRequested → prevent_exit (lib.rs:125-131, tauri #15671)
      ├─ config_state ── CONFIG_IO_LOCK(:17) + CryptoBridge(:204) → config.json
      ├─ account_cmds ── accounts/*.json（转发同一把锁）
      ├─ protocol_cmds ── campus_login_lib::auth::{protocol,portal}
      ├─ monitor_loop: monitor_tick_loop(:643) → run_scheduled_actions(:835)
      │        └─ run_check_once(:1339) → protocol_cmds → 协议核心
      ├─ quality_cmds ── campus_login_lib::network::quality
      └─ update_cmds ── GitHub API/镜像 HTTP → campus_monitor_service 装 APK
```

装配四决定性细节：

1. `AndroidState` 在 mobile 与 host 两条支路都 manage（`lib.rs:40` / `lib.rs:41-42`），故所有命令体内 `app.state::<AndroidState>()` 无需 cfg 配对。
2. 移动插件整段 `#[cfg(mobile)]`（`lib.rs:33-40`），host 编译期消除；调用点不成对门控——host 上注册的命令在运行期返回 Err 字符串（如 `protocol_cmds.rs:191-195`）。
3. 退出护栏（`lib.rs:125-131`）拦的是「最后窗口关闭 → 进程退出」事件：安卓语义里划掉任务只关 UI，进程随前台服务常驻（tauri #15671 根因）。
4. 日志初始化失败必须可见 eprintln（`lib.rs:52-56`）——静默等于日志系统永远缺失且无人知晓。

## Data Flow

### 启动恢复链（run_startup_tasks，monitor_loop.rs:458-529）

spawn + 500ms 就绪窗 → 读配置失败 → log_error + emit "startup-failed"{reason} + 尽力 stop_monitor（BootReceiver 可能已拉起 FGS 假通知）→ set_log_retention_days → 并行 spawn 三链：①`start_background_check`（enable_background_check，2026-09-09 提速改为并行）②质量首测（enable_latency_test → `start_latency_test`，否则 `check_network_quality` 单次——首屏数据来源）③`start_update_check_loop` + `cleanup_stale_apks` → `auto_login_on_start`（若开启；probe_with_retry 后 on_campus 才登录）→ night_outbound_restore 非空（切换态收尾被杀残留）→ `restore_avoid_bad_wifi` 兜底。

### 巡检单拍（run_check_once，monitor_loop.rs:1339-1574）

周期拍（`monitor_tick_loop` `monitor_loop.rs:643-702`）、WiFi 事件（`handle_wifi_event` `monitor_loop.rs:322-363`）、手动触发（`trigger_background_check` `monitor_loop.rs:366-374`）三入口共用同一编排。顺序：出站切换态闸（1349-1357，run_scheduled_actions 不过此闸）→ 静默期门控（1359-1383）→ 探针窗口 ProbeWindowGuard（1385-1392）→ ensure_wifi_bound（1394-1396）→ 校园网判定 + 源 IP 缓存（1398-1411）→ Portal 三态短路（1413-1424，避免空转）→ 三态消费 error_kind=None 才翻转（1429-1445，状态点每拍打灰又变绿误报根治）→ 状态机在线清零（1447-1457）→ 自动重登双闸 + LOGIN_ATTEMPT_LOCK 互斥（1459-1523）→ payload emit "background-check-result"（1525-1560，onlineOperator 由 uid 后缀推导）→ 常驻通知按翻转重建（1562-1573）。

### 配置写路径

`save_config`（`config_state.rs:468-499`）与 `persist_settings`（`monitor_loop.rs:1042-1065`）是同构的两条写链：持 CONFIG_IO_LOCK → `load_from` 基线 → merge/`resolve_password_field` → `save_to` 落盘 → 刷新 AndroidState.config 缓存 → `auto_create_account_for_current` → `emit_config_changed`（掩码 payload）。emit 必不可少：夜切 operator/restore 即时改写若不推前端，UI 停在旧运营商，用户下次改设置会用旧快照覆盖回去。

### 更新下载与校验三态

检查：`check_update_inner`（`update_cmds.rs:282-333`）4 源按序降级（GitHub + 3 镜像 version.json），`has_newer_version` 数值比较。`UPDATE_CHECKSUM` 三态记账把「完整性未校验」从静默变可见——tag 匹配非空 digest 强制比对（不匹配删文件报错）；tag 匹配空 digest 拒绝安装；无记账/tag 不匹配放行但 emit update-unverified + log_warn（`update_cmds.rs:476-500`）。下载：`allowed_url` 白名单漏斗 + 流式写盘 + 500MB 上限 + 200ms 节流进度。安装：`install_update`（`update_cmds.rs:546-570`）canonicalize 限目录 + install_apk（FileProvider），不立刻删 APK（v2.3.9 竞态），孤儿包由 `cleanup_stale_apks`（24h）收尾。

## Connections

- [[campus-detect]]：校园网判定与 SSID 感知探针来源（`run_check_once` `monitor_loop.rs:1398-1411`、`auto_login_on_start` `monitor_loop.rs:567-612`）
- [[android-state]]：cached_source_ip / config 缓存宿主（`AndroidState`，`lib.rs:40` manage）
- [[login-history]]：manual/auto 登录注销历史落账（`protocol_cmds.rs:46-52`、`125-131`；`monitor_loop.rs:1497-1503`）
- [[quality-history]]：质量趋势落盘（`run_quality_once` `quality_cmds.rs:19-49`）
- [[system-cmds]]：get_init_data 聚合启动初值（`status_value` 唯一可靠来源 `monitor_loop.rs:130-152`）
- [[self-service-cmds]]：自助大厅与实名核身命令面（`lib.rs:79-86` 注册）
- [[identity-gate]]：人脸/生物识别门控（self_service 前置）
- [[battery-cmds]] / [[face-model-cmds]]：电池策略与人脸模型下载（face 下载复用 `update_cmds::allowed_url` 与 `verify_file_sha256`，`update_cmds.rs:138-159`、`512-544`）
- 共享协议核心 `campus_login_lib`（Cargo path 依赖单点继承，禁复制）：`auth::protocol`（登录注销）、`auth::portal`（Portal 探测/chkstatus）、`network::quality`（质量检测）、`config::{schedule, night_switch, outbound_switch, scheduled_tick}`（定时/夜切/出站共享编排纯函数）、`infra::logger`
- 决策文章：`decisions/night-outbound-switch`（夜切整体设计）、`decisions/android-keepalive-fgs-architecture`（FGS 保活架构，quality 稳态退避为其落地项）

## Known Issues

1. **AccountItem.password_saved 恒 false（已修复）**：旧实现读 `settings.password`（落盘时刻意清空）判保存态，恒 false。现走 `password_saved_at` 密文位判据（`config_state.rs:270-276`、`account_cmds.rs:111-121`）。
2. **Settings::default 与 v7 迁移夜切默认不一致（已通过注释定案）**：`enable_night_operator_switch` 默认 2026-10-02 起改回关闭（`config_state.rs:141`，注释 139-140），存量已落盘 true 不迁移、由用户自行关闭；迁移侧 v7 刷开仅作用于旧默认场景（`config_state.rs:402-416`）。
3. **night_outbound_restore 魔法字符串 "logged_out" 状态位**：空串/非空即状态机，误写非空值会静默压住整轮巡检（run_check_once 闸，`monitor_loop.rs:1349-1357`）；三处写入（`monitor_loop.rs:872-873`、`892-893`）与读取（`outbound_switch_active` `monitor_loop.rs:706-711`）无类型保护。
4. **桌面专属字段镜像不消费**：outbound_priority/outbound_metric_restore/outbound_disabled_adapters/outbound_standby_route/dns_optimize_adapters（`config_state.rs:48-59`、`65-67`）仅为 camelCase 契约镜像；安卓出站切换走 night_outbound_restore + 系统 network_avoid_bad_wifi（`monitor_loop.rs:1129-1192`），状态机不互通。
5. **host 编译能力编译期禁用**：移动插件（`lib.rs:33-40`）与 cfg(mobile) 分支（`config_state.rs:231-238`、`protocol_cmds.rs:191-255` 等）在 host 上恒 Err；`cargo test`（host）只覆盖纯函数与文件 IO，移动路径需真机/交叉编译验证。
6. **#[allow(dead_code)] 历史遗留标注**：`current_settings`（`config_state.rs:502`）等处仍带标注；实际已被 run_check_once/monitor_tick_loop/protocol_cmds 调用，勿误删。
7. **delete_account 不清 night_operator_restore**：删当前账号只清 active_account（`account_cmds.rs:352`）与 night_outbound_restore（`account_cmds.rs:355`），运营商恢复位残留——若夜切恢复窗口恰逢账号删除，恢复动作会把已删账号的 ISP 写回主配置。
8. **do_login operator 回退时机**：operator_len 记录于回退配置值之前（`protocol_cmds.rs:36-42`），排障看到 0 实际用了配置值。
9. **MAX_DOWNLOAD_BYTES 与 content_length 无联动**：响应无 Content-Length 时 `DownloadProgress.total` 为 0，percent 恒 0（`update_cmds.rs:449-474` 进度 emit 处），进度条停在 0% 但下载正常。
10. **夜切验证时序硬编码**：NIGHT_VERIFY_*（`monitor_loop.rs:1196-1199`）不可配，弱网整轮可达数十秒，且与巡检/定时动作并发（`verify_night_switch` 独立任务，`monitor_loop.rs:1229-1283`）。
