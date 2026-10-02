---
title: 配置模型与持久化
type: concept
tags:
  - 概念
  - 配置
  - 持久化
  - 迁移
  - 原子写
  - config-changed
source_files:
  - tauri-app/src-tauri/src/config/mod.rs
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/config/validate.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/monitor_loop.rs
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/settings/types.ts
  - android/frontend/src/settings/constants.ts
---

# 配置模型与持久化

## Overview

同一份配置在两个平台各有一套模型：桌面 `Config`（`tauri-app/src-tauri/src/config/model.rs:10-172`，60 个字段，serde camelCase）与安卓 `Settings`（`android/src-tauri/src/config_state.rs:15-111`，47 个字段）。两端都把配置加密落盘为 `config.json`：桌面走原子写（`persist.rs:12-43`：纳秒 tmp 名 + `sync_all` + rename 重试），安卓走 `save_file`（`config_state.rs:252-283`：固定名 tmp + rename，无 fsync）。加载时桌面有 lenient 容错层（`validate.rs:188-255`），安卓解密失败直接 `unwrap_or_default` 置空（`config_state.rs:243-248`）。两份结构都未启用 `deny_unknown_fields` 且是容器级 `#[serde(default)]`（`model.rs:9`、`config_state.rs:14`），新增字段对老文件安全。版本号双轨：桌面 `config_version = 5`（`model.rs:306`），安卓 `config_schema_version = 7`（`config_state.rs:180`）。任何写配置路径都会触发 `config-changed` 事件把掩码后的配置回流前端（桌面 `config_cmd.rs:18`、安卓 `config_state.rs:384-391` 与巡检 `monitor_loop.rs:976`）。数据目录下的文件布局：`config.json`、`accounts/<id>.json`、`login-history.json`（`persist.rs:156-158`）、`quality_history.json`（`persist.rs:218`，注意是下划线）、`exports/config-<stamp>.json` 与若干 `*.corrupt-*.bak` 备份。

## 机制说明

### 桌面配置模型

`config/mod.rs` 仅 8 行，`pub use model::Config`（`mod.rs:8`）。常量 `PASSWORD_MASK = "***"`（`model.rs:3`）、`AUTO_DETECT_ADAPTER = "自动检测"`（`model.rs:4`）。

`struct Config`（`model.rs:10-172`）60 个字段，容器级 `#[serde(default)]`（`model.rs:9`）；字段级 `default` 35 个，其余 25 个（user/operator/adapter1/adapter2/adapter1_account/adapter2_account/dual_adapter/auto_login_on_start/auto_exit_after_login/minimize_to_tray/hidden_start/auto_launch/enable_background_check/background_check_interval/auto_login_on_preparation/auto_exit_on_online/theme_mode/enable_notification/active_account/display_name/enable_latency_test/latency_test_interval/custom_theme_color/default_panel/enable_network_quality）靠容器级兜底。已核验的字段锚点：

| 分组 | 字段（行号） |
| --- | --- |
| 凭据/自助 | `password` :12-13、`self_password` :15-16、`self_hello_enabled`(default_true) :20-21、`self_reverify_each_action` :23-24 |
| 运营商/账号 | `operator`、`user`（:10-172 内）；`adapter1/adapter2` 默认 `AUTO_DETECT_ADAPTER`（Default :243） |
| 夜切/出站镜像 | `enable_night_operator_switch` :59-60、`night_operator_restore` :62-63、`enable_night_outbound_switch` :66-67、`outbound_priority` :70-71、`outbound_metric_restore` :75-76、`outbound_disabled_adapters` :81-82、`outbound_standby_route` :86-87、`outbound_manual_hold_day`(i32) :93-94、`night_outbound_restore` :97-98、`dns_optimize_adapters` :102-103 |
| 行为 | `auto_exit_on_online` :104-105、`max_disconnect_reconnect` :164-165、`auto_login_cooldown_secs` :166-167、`skip_sha256_when_missing` :168-169、`lightweight_mode`(default_true) :45-46 |
| 网络 | `skip_ttfb_in_latency`/`skip_content_in_latency`(default_true) :126-129、`portal_url` :130-131、`fixed_gateway` :132-133、`required_network_name`(deserialize_with) :134-135、`enable_network_name_check`(default_true) :136-137、`campus_gateway`(deserialize_with) :138-139、`update_source` :141-142 |
| 校园网时段 | `campus_exit_on_fail`(default_true) :143-144、`campus_exit_start/end_minutes`(default 480/1380) :146-150、`campus_check_start_minutes`(alias `campusCheckStartHour`) :151-152、`campus_check_end_minutes` :154-155 |
| 定时/其他 | `scheduled_login/logout_minutes` :157-161、`log_retention_days` :162-163、`config_version` :170-171 |

反序列化辅助：`deserialize_non_empty_or` :174-184、`deserialize_campus_gateway` :186-191、`deserialize_required_network_name` :193-198。默认值函数 :200-232（`default_true` :200、`default_campus_check_start_minutes=460` :202、`default_campus_check_end_minutes=1380` :206、campus_exit 480/1380 :208/:210、`default_fixed_gateway="10.2.127.254"` :212-214、`default_log_retention_days=7` :216、`default_max_disconnect_reconnect=3` :218、`default_auto_login_cooldown_secs=60` :220、`default_portal_url="http://10.1.99.100"` :222-224、`default_required_network_name="i-wxxy、iwxxy-2、iwxxy-3"` :226-228、`default_campus_gateway="10.2.127.254"` :230-232）。

`Default for Config` :234-309 关键值：`adapter1=AUTO_DETECT_ADAPTER` :243、`auto_exit_after_login=false` :251、`lightweight_mode=true` :253、`background_check_interval=60000` :258、`enable_night_operator_switch=false` :262（2026-10-02 注释）、`auto_exit_on_online=false` :273、`enable_latency_test=true` :278、`latency_test_interval=600000` :280、`custom_theme_color="#6366f1"` :281、`default_panel=""` :282、`enable_network_quality=true` :283、`scheduled 1440/1440` :299-300、`config_version=5` :306。

掩码出口 `masked_for_display` :315-319 / `mask_in_place` :321-328（空值保留、非空替换 `PASSWORD_MASK`）。契约测试 :331-396（`serde_account_fields_json_names_and_defaults` :367 起，锁 `displayName`/`adapter1Account`/`adapter2Account` camelCase 命名）。

### 桌面原子写与文件布局

`persist.rs`（457 行）：

- `atomic_write` :12-43：tmp 文件名带纳秒时间戳（:14-20，缓解多进程冲突）→ `File::create` :23-24 → `BufWriter` 写入 + flush :26-30 → `sync_all` :31 → `rename` 重试 3 次×100ms :32-39 → 失败清理 tmp 并返回 `Err("重命名临时文件失败（重试3次后）")` :40-42。
- 路径：`get_data_dir` :45-58（`app_data_dir`，回退 `dirs::data_dir().join("campus-login")`）；`get_config_path`→`config.json` :60-62；`get_accounts_dir` :64-66；`get_account_path` :68-71。
- 账号读写：`load_account_config` :75-96（不存在→`Err("账号不存在")`；解密失败→`Err("账号密码解密失败")` :90）；`save_account_config` :101-112（仅 `password` 加密，`self_password` 透传）；`list_account_items` :117-154（过滤 `.` 前缀/空名/非 json，按 id 排序，`display_name` 空或解析失败兜底 id）。
- 登录历史：`LOGIN_HISTORY_LOCK` static Mutex :10；`get_login_history_path`→`login-history.json`（连字符）:156-158；`append_login_history` :160-212（锁串行、损坏 `.bak` 重置、头插、>100 截断 :202-204）。
- 质量历史：`QUALITY_HISTORY_LOCK` :215；`get_quality_history_path`→`quality_history.json`（下划线！）:217-219；`append_quality_history` :223-225；`append_quality_history_to` :227-274（`gatewayLatency`/`externalLatency` 负值落 `null` :257-262，上限 100 :264-266）。
- `save_config_to_disk_encrypted` :276-291（非空即加密；注释明确不排除 `***` MASK 占位符是合法落盘值）。
- 测试 :293-457：`literal_star_password_is_encrypted_on_disk` :305、`empty_password_stays_empty` :323、`quality_history_head_insert_and_shape` :349、`quality_history_caps_at_100` :366、`quality_history_corrupt_backup_and_reset` :380、`list_account_items_missing_dir_is_empty` :396、`list_account_items_fallback_and_filter` :405、`account_config_roundtrip_encrypts_password`(windows) :436。

### 桌面校验与迁移

`validate.rs`（829 行）：

- 校验函数：`validate_username` :10-21（非空/≤64/正则）、`validate_operator` :23-29（仅 `""`/`@telecom`/`@unicom`/`@cmcc`）、`validate_password` :31-39（非空/≤128）、`validate_portal_url` :42-71（http/https + 内网 IPv4/loopback/localhost，禁公网域名，私有 fn）。
- 迁移辅助：`migrate_operator` :73-79（`@ctcc`→`@telecom`、`@cucc`→`@unicom`）；`normalize_portal_url` :81-85（`http://10.1.99.100:801` 或空→`http://10.1.99.100`）。
- 严格校验 `validate_config` :87-183：operator 迁移 :95-96；theme 校验 :100-102；间隔 clamp [10000,3600000] :103-104；portal normalize+校验 :105-106；`fixed_gateway` 必须 IP :107-109；`campus_gateway` 空补默认+IP :110-115；`required_network_name` 空补默认 :116-118；单值 `"i-wxxy"`→三值 :121-123（注释 :119-120）；`log_retention_days>365`→365 :125-127；版本迁移链——v1→v2 小时×60 :130-137、v2→v3 end==0→1380 :141-146、v3→v4（15000→60000、60000→600000）:151-159、v4→v5（夜切 false→true + scheduled 0→1440）:163-174；分钟上界 `min(1439)` 四字段 :175-178、scheduled `min(1440)` :180-181。
- 容错 `validate_config_lenient` :188-255：user/password/operator/color/theme/portal/fixed_gateway/campus_gateway/required_network_name 逐字段回退；三值扩展 :243-245；末尾仍跑严格校验兜底，失败→`Config::default()` :248-254。
- 测试 :259-829：`validate_config_new_defaults_are_v5` :639（锁 config_version=5/auto_exit false/60000/600000/lightweight true/夜切 false/1440）；`迁移_v4夜切默认开_定时禁用值刷哨兵` :654。

### 桌面加载与保存路径

`commands/config_cmd.rs`（364 行）：

- 启动链：`app/startup.rs:185` `load_config_from_disk_or_default` → `startup.rs:206` `state.config.store(config.clone())` → `startup.rs:207` `network::update_portal_url` → `startup.rs:212` `logger::set_log_retention_days`。
- `load_config_from_disk_or_default` :60-83：Ok→`validate_config_lenient` :62；Err→原文件备份 `config.json.corrupt-<秒>.bak` :66-78 + `Config::default()` :80。`load_config_from_file` :25-58（不存在→default :28-30；`password` 解密失败仅清空 :36-45；`self_password` 同 :47-55）。
- 保存链 `save_config` :221-276：严格 `validate_config` :228；`clear_password==Some(true)`→置空、空/`***` 保留当前值 :238-244，`clear_self_password` 同 :247-252；`update_portal_url` :256、`set_log_retention_days` :259；先落盘 :263 再 `store` :264；R2 `auto_create_account_for_current` + 建号成功补刷托盘 :271-273。`get_config` :215-218（`masked_for_display` :217）。
- 导出/导入：`build_config_export_payload` :91-113（`include_password=true` 时空串/`***`→clear :94-96，否则 encrypt :97-99；wrapper `{type:"campus-login-config",version:1,exportedAt,appVersion,passwordEncrypted,config}` :105-112）；`export_config` :135-150（落 `<data>/exports/config-<stamp>.json` :141-146）；`import_config` :156-207：1MB 防呆 :159-163 → ① JSON 解析 :167-168 → wrapper/裸 Config 兼容 :170-171 → ② 结构 :173-174 → 密码还原先于校验 :178-180（`restore_imported_password_field` :121-131：空/`***`→保留 current；encrypted→decrypt 失败报 `密码密文解密失败（含密码导出仅限本机导入）`）→ 切换态五字段保留本机值 :185-189（`outbound_metric_restore`/`outbound_disabled_adapters`/`outbound_standby_route`/`outbound_manual_hold_day`/`night_outbound_restore`，注释 :181-184 引 k2.8 审计 P1-1）→ ③ 严格校验 :192-195 → 同序落盘 :200-203（`update_portal_url` :200→`set_log_retention_days` :201→`save_config_to_disk_encrypted` :202→`store` :203）→ ④ 落盘失败分列提示。注释引用学习记录 `.codewiki/learnings/mask-placeholder-persisted-as-plaintext`（:116）。
- 公共出口 `save_config_to_disk_encrypted` :9-23：persist 加密落盘 :10-11 → `masked_for_display` 出站 :17 → `notify_config_changed(json!({"config": emit_cfg}))` :18（注释 :13-16：事件体必须 `config` 包裹）→ `refresh_tray_menu_state` :21。调用方：`save_config` :263、`import_config` :202、`switch_account`（`account.rs:56`）、`save_current_as_account`（`account.rs:177`）、`delete_account`（`account.rs:247`）、`rename_account`（`account.rs:306`）、`set_auto_launch`（`system.rs:61`）、`set_notification_enabled`（`system.rs:84`）、`stop_background_check`（`background.rs:20`）、`start/stop_latency_test`（`network_cmd.rs:252`/:264/:277）。自动建号与"非激活账号改名"只写账号档案不触发。
- 测试 :278-364：`masked_for_display_masks_both_password_fields` :288、`export_payload_never_contains_plaintext_password`(windows) :310、`import_password_restore_semantics`(windows) :340。

其余写配置的命令：`system.rs` `set_auto_launch` :44-72（注册表先行，随后更新配置并落盘 :58-61）、`set_notification_enabled` :79-89（落盘 :84）、`set_log_retention_days` :180-184；`network_cmd.rs` `start_latency_test` :243-269（`enable_network_quality` 关闭时强制把 `enable_latency_test` 落盘为 false :250-256，随后 spawn 巡检循环 :258-260、置 true 落盘 :263-266）、`stop_latency_test` :271-282（cancel :273、false 落盘 :276-279）、`check_network_quality` :211-241（结果落质量历史 :237）；`background.rs` `stop_background_check` :13-25（`enable_background_check=false` 落盘 :17-20）。

### 账号与登录历史存储

#### 桌面（`commands/account.rs` 745 行，6 条命令）

- `list_accounts` :7-13；`switch_account` :16-35（spawn_blocking :19-22；成功→`masked_for_display` :28 + `AccountResult::ok_with_account` :29，R4 注释 :26-27）；`perform_switch_account_sync` :42-60（`validate_account_name` :47、`load_account_config` :50、`merge_account_into_config` :52-54、`save_config_to_disk_encrypted` :56）；`merge_account_into_config` :66-82（仅搬 user/password/operator :67-69、`night_operator_restore` 置空 :72、adapter1/adapter2/dual_adapter :73-75、`display_name` 空兜底 safe_name :76-80、`active_account=safe_name` :81）。
- `save_current_as_account` :85-184：若已有 `active_account` 且非同名→先 spawn_blocking 回写旧账号 :102-127（旧档案不存在→以 `display_name=prev_name` 新建 :107-112；其他 Err 透传）；新档案 `password` 置空 :137-138、`active_account=account_name` :139；"另存"（非覆盖同名）时 `display_name` 置空 :142-144；落盘 :145-166；主配置更新 `active_account`/`display_name` :168-172；`save_config_to_disk_encrypted` :176-179；`ok_with_account` :183。`merge_save_as_target` :194-216：existing 存在→仅覆盖 user/password/operator/adapter1/adapter2/dual_adapter/active_account :196-203；`账号不存在|读取账号配置失败|账号密码解密失败`→`account_data.clone()` 宽松回退 :205-213（F3），其他 Err 透传 :214。
- `delete_account` :219-261（remove_file :233；删当前账号→清 `active_account`+落盘+广播 :241-249；结果 `active_account=Some(String::new())` :252-256；`refresh_tray_menu_state` :259）。`get_active_account` :264-267。
- `rename_account` :270-283（业务错误走 IPC `Err(String)` 通道，注释 :278-281）；`perform_rename_account_sync` :289-315（`rename_account_core` :300；改名的是激活账号→同步主配置 `display_name` + 落盘 :302-307；`ok_with_account` + `display_name=Some` :309-312）；`rename_account_core` :319-343（`validate_display_name` :320；重名检查 :331-338，`Err("名称已存在: {new_name}")` :336；只改 `display_name` :340-341）；`validate_display_name` :348-361（trim、1-32 字符、禁控制字符）。
- 自动建号（R2）：`auto_create_account_for_current` :366-376（`config_cmd.rs:271-273` 调用，失败仅 log_warn）；`auto_create_account_in` :389-423（user/password 空→跳过 :390-392；不存在→快照 `display_name=user` 原文 :400、`active_account=id` :401；撞库（existing.user != config.user）→log_warn 跳过 :406-414（F7）；password+operator 全同→幂等跳过 :415-417；差异→仅更新 user/password/operator :418-422）。
- 测试 :425-745：`rename_account_core_success_keeps_filename_and_other_fields` :470-486、`auto_create_is_idempotent_no_write_when_identical` :549-564、撞库 F7 :602-616、解密失败回退 F3 :637-658、`switch_success_result_carries_active_account`(R4) :721-734、`account_item_json_shape` :738-744。

`infra/state/mod.rs`（325 行）：`ACCOUNT_NAME_RE = ^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$` :60-62、`validate_account_name` :64-72（非空、≤32 字符）、`sanitize_account_id` :77-88（非法字符替换 `_`、截断 32）；`AccountResult` :301-325（`success`/`message`/`activeAccount`/`displayName`/`config: Option<Config>`；`ok_with_account` :319-321）。

#### 安卓（`account_cmds.rs` 769 行，与桌面同构）

- 账号文件与主配置同格式（`EncodedSettings`，仅目录不同，模块注释 :1-2）；`CONFIG_IO_LOCK`（tokio::sync::Mutex）:15-17 + `config_io_lock()` :21-23（所有配置 IO 串行，防主配置并发读写竞态）。`AccountItem {id, displayName}` :29-33（R3 契约）；`AccountResult` :35-48（全字段 `skip_serializing_if = Option::is_none`；ok/ok_with_account/err :51-59）。
- `sanitize_account_id` :65-76、`validate_account_name` :79-87（空/超 32→`账号名称长度需在1-32之间`；正则不过→`账号名称仅允许字母、数字、下划线、中文和连字符`）；`accounts_dir` :89-95；`read_display_name` :99-103（不解密直接读 JSON `settings.displayName`）；`list_account_items_sync` :107-134（显示名空/读失败兜底 id）；`persist_current` :151-162（save_to + 缓存刷新 + masked 返回）。
- `switch_account` :164-206：IO 锁 :177；不存在→err `账号不存在` :178-180；合并 `current_settings` :184 + user/password/operator :185-187；`night_operator_restore` 置空 :190；`night_outbound_restore` 置空 :193；`display_name` 空兜底 id :194-198；`active_account=safe_name` :199；`persist_current` :201；`restore_avoid_bad_wifi` :204。
- `save_current_as_account` :208-236；`merge_save_as_target` :244-273（is_resave 时 `display_name` 置空 :252-254；existing 仅覆盖 user/password/operator/active_account 四字段 :256-262；`final_display` 档案非空优先否则 id :267-271）。`delete_account` :275-304（删当前账号清 `active_account` :294 + `night_outbound_restore` :297 + persist :298 + `restore_avoid_bad_wifi` :300）。`get_active_account` :306-309。
- `rename_account` :311-343（业务错误走 IPC `Err(String)`，注释 :317-322；IO 锁 :327；激活账号同步主配置 `display_name` :332-336）；`rename_account_core` :347-371（重名检查 :362-366，`Err("名称已存在: {new_name}")`；只改 `display_name` :368-369）；`validate_display_name` :376-389。
- 自动建号：`auto_create_account_for_current` :393-402；`auto_create_account_in` :415-452（无凭据跳过 :420-422；撞库跳过 :434-443；幂等短路 :444-446；差异仅更新 user/password/operator :447-451）。
- 测试 :454-769（账号条目 json 形状 :703-709、另存显示名不串号 F2 :714-730、撞库 F7 :751-768）。

#### 安卓登录历史（`login_history.rs` 142 行，与桌面 `append_login_history` 同构）

`LOGIN_HISTORY_LOCK` :8、`LOGIN_HISTORY_MAX=100` :10；`LoginHistoryEntry` :13-21（serde rename `"type"` :19-20）；`history_path`→`login-history.json` :23-25；`read` 损坏→备份 `login-history.json.corrupt-<毫秒>.bak` :34-35 后重置空；`append` :51-76（锁 :52-54、头插 :56-66、adapter 固定 `"wlan0"` :62、`truncate(100)` :67、tmp 固定名 `login-history.json.tmp` + rename :72-74，无 `sync_all`）。测试 :78-142（头插 :90-100、上限截断 :103-113、损坏备份 :116-129、字段形状对齐桌面 :132-141）。

### 安卓配置模型与迁移

`config_state.rs`（765 行）：`PASSWORD_MASK` :10、`CONFIG_FILE="config.json"` :11；`#[serde(rename_all="camelCase", default)]` :14；`struct Settings` :15-111，47 个字段：

| 分组 | 字段（行号） |
| --- | --- |
| 凭据/自助 | `user` :17、`password` :18、`self_password` :19、`self_hello_enabled` :20、`self_reverify_each_action` :21、`allow_2d_face_verify` :24（安卓独有）、`operator` :25 |
| 夜切/出站镜像 | `enable_night_operator_switch` :29、`night_operator_restore` :31、`enable_night_outbound_switch` :35、`outbound_priority` :38、`outbound_metric_restore` :41、`outbound_disabled_adapters` :44、`outbound_standby_route` :47、`night_outbound_restore` :52（值恒 `"logged_out"`）、`dns_optimize_adapters` :55（安卓仅镜像不消费） |
| 行为 | `auto_login_on_start` :57、`enable_background_check` :58、`background_check_interval` :59、`background_check_idle_interval` :64（闲时巡检，安卓独有）、`auto_login_on_preparation` :65、`max_disconnect_reconnect` :66、`auto_login_cooldown_secs` :67 |
| 界面 | `theme_mode` :69、`enable_notification` :70、`custom_theme_color` :71、`default_panel` :72、`display_name` :76（R3）、`active_account` :77、`enable_boot_autostart` :78（安卓独有） |
| 质量 | `enable_latency_test` :80、`latency_test_interval` :81、`enable_network_quality` :82、`skip_ttfb_in_latency` :83、`skip_content_in_latency` :84 |
| 网络 | `portal_url` :86、`fixed_gateway` :87、`required_network_name` :88、`enable_network_name_check` :89、`campus_gateway` :90、`campus_check_start_minutes` :92、`campus_check_end_minutes` :94、`scheduled_login_minutes` :96、`scheduled_logout_minutes` :98（1440=禁用哨兵） |
| 其他 | `update_source` :101（mirror 默认）、`log_retention_days` :103、`config_schema_version` :110 |

`Default` :113-183 关键值：`self_hello_enabled=true` :119、`allow_2d_face_verify=false` :121、`enable_night_operator_switch=false` :125（2026-10-02 注释 :123-124）、`enable_night_outbound_switch=false` :127、`auto_login_on_start=true` :134、`enable_background_check=true` :135、`background_check_interval=60_000` :138、`background_check_idle_interval=300_000` :140、`theme_mode="dark"` :144、`custom_theme_color="#6366f1"` :146、`default_panel="dashboard"` :147、`enable_boot_autostart=false` :150、`enable_latency_test=false` :151、`latency_test_interval=600_000` :153、`enable_network_quality=false` :157（注释 :154-156）、`portal_url="http://10.1.99.100"` :160、`fixed_gateway="10.2.127.254"` :161、`required_network_name="i-wxxy、iwxxy-2、iwxxy-3"` :164（2026-10-01 对齐注释 :162-163）、`campus_check 460/1380` :168/:171、`scheduled 1440/1440` :175-176、`update_source="mirror"` :177、`log_retention_days=7` :178、`config_schema_version=7` :180。

- 加密桥 `CryptoBridge` :186-223（`from_app` :193；mobile 经 `tauri_plugin_campus_keystore` :196；非 mobile 恒 Err）。`EncodedSettings` :226-232（settings/password_cipher/self_password_cipher）。
- `load_file` :234-250（无文件→default :237；解密失败 `unwrap_or_default` 置空 :243-248）。`save_file` :252-283（非空才加密，失败 Err 绝不落明文 :261-272；settings 内密码置空 :274；tmp=`with_extension("json.tmp")` 固定名 :279、write :280、rename :281，无 `sync_all`）。
- `load_from` :285-295（`migrate_legacy_defaults` :287；`required_network_name=="i-wxxy"`→三值 :291-293，幂等）。`migrate_legacy_defaults` :314-366（doc :297-313；legacy<3 :315-330 强刷六开关 :320-325 + quality false :326-328 + 置 3 落盘 :329-330；<4 idle :332-338；<5 end 0→1380 :339-345；<6 60_000→600_000 :346-352；<7 夜切 false→true :353-356 + scheduled 0→1440 :357-362 + 置 7 落盘 :363-365；每档迁移即落盘，失败静默）。
- `save_to` :368-370；`masked_for_display` :372-382；`emit_config_changed` :384-391（emit `config-changed`，`json!({"config": masked})`）。
- 命令面：`get_config` :405-418（`load_from` :412、更新 AndroidState 缓存 :413-416、masked 出站 :417）；`save_config` :420-453（`resolve_password_field` 处理两个密码字段 :434-440、`save_to` :441、缓存 :442-445、`auto_create_account_for_current` :448、`emit_config_changed` :451）；`resolve_password_field` :394-403（clear→`""`；空/`***`→保留 current；否则透传，与桌面同构）；`current_settings` :456-474（缓存优先，未命中读盘回填）。
- 巡检侧（`monitor_loop.rs` 1607 行）：`persist_settings` :955-978（`save_to` 失败 emit_login_log :964-966；缓存 :968-971；`auto_create_account_for_current` :972；注释 :973-975 夜切改的 operator/restore 须即时推前端；`emit_config_changed` :976）。`status_value` :118-138（json :120-126 + 展平 :130-136；注释 :127-129 首轮 emit 先于 WebView 监听建立，`getInitData` 是唯一可靠启动初值来源）。`set_boot_autostart` :361-383（mobile 走 `tauri_plugin_campus_monitor_service` :362-368；IO 锁 :369；写配置 :376-377）；`set_notification_enabled` :392-407（IO 锁 :393；写配置 :400-401）。夜切/出站/定时判定纯函数在 `campus_login_lib::config::*` 共享 crate，安卓消费点：`monitor_loop.rs:759`（`outbound_switch::evaluate_night_outbound`）、`:856`（`night_switch::evaluate_night_switch`）、`:904`/`:924`（`schedule::should_fire_scheduled_action`）；出站切换态（`night_outbound_restore` 非空，判定 :617-624）下巡检整轮跳过 :1255-1263（注释 :1255-1259：闸在 `run_check_once` 共用体，`run_scheduled_actions` 不经过；:1260 判定、:1262 return）。
- 测试 :476-765：默认值_业务语义 :590-621、迁移_旧默认15s升60s :624-668、迁移_非旧默认值保持不动 :671-682、迁移_v5质量间隔 :685-713、camelCase 契约 :716-724、掩码出口 :727-736、密码回退语义 :739-750、空密码不写密文位 :753-764。

#### 安卓前端类型与默认值

`android/frontend/src/settings/types.ts`（94 行）：`Config` 接口 :9-73（`allow2dFaceVerify` :18、夜切/出站镜像字段 :21-33、`enableBootAutostart` :36、`backgroundCheckIdleInterval` :40、`enableLatencyTest` :45、`latencyTestInterval` :46、`enableNetworkQuality` :49、`requiredNetworkName` :54、`enableNetworkNameCheck` :55、`campusGateway` :56、`updateSource` :58、`campusCheckStart/EndMinutes` :60/:62、`scheduledLogin/LogoutMinutes` :64/:66、`autoLoginCooldownSecs` :68、`logRetentionDays` :69、`configSchemaVersion` :70、`displayName?` :72）；`AutoLaunchResult` :75-78；`InitData` :80-94（含 `isAutoStart` :89）。

`android/frontend/src/settings/constants.ts`（93 行）：`DEFAULT_CONFIG` :5-59（`selfHelloEnabled=true` :9、夜切 `false` :13、`enableNightOutboundSwitch=false` :15、`autoLoginOnStart=true` :22、`enableBackgroundCheck=true` :25、`backgroundCheckInterval=60000` :27、idle `300000` :29、theme `dark` :31、`enableLatencyTest=true` :34、`latencyTestInterval=600000` :36、`defaultPanel=''` :38、`enableNetworkQuality=true` :39、`portalUrl=http://10.1.99.100` :42、`fixedGateway=10.2.127.254` :43、`requiredNetworkName='i-wxxy'` :44、`campusGateway=10.2.127.254` :46、campusCheck `460/1380` :48/:50、scheduled `1440` :52-53、`configSchemaVersion=7` :58）；`ISP_OPTIONS` :61-66、`operatorLabelKey` :73-76（空串运营商=无锡学院 `__default__`）、`THEME_OPTIONS` :78-86、`DEFAULT_PANEL_OPTIONS` :90-93。

### 配置变更如何触发前端更新

事件链（桌面）：

```
任意写配置路径
  → save_config_to_disk_encrypted            commands/config_cmd.rs:9-23
  → 加密落盘                                  persist.rs:276-291
  → notify_config_changed(json!({"config": masked}))   config_cmd.rs:18
  → infra/command_context.rs:44-45
  → infra/events.rs:107  emit_config_changed("config-changed")
  → 前端 tauriApi.ts:226  onConfigChanged（createEventListener）
  → useEventListeners.ts:354-375 处理器（mergeConfigFromBackend(data.config) :360，
      注释 :357-359 历史缺陷 updateConfigLocal 全量替换；
      activeAccount 同步 :364-368、listAccounts 刷新 :369-372，覆盖托盘切账号路径）
  → useConfigStore.mergeConfigFromBackend    useConfigStore.ts:110-117
      （逐字段合并，跳过 dirtyFields 中的字段 :113-115）
```

前端写配置队列（`useConfigStore.ts` 220 行）：

| 机制 | 行号/值 |
| --- | --- |
| `DIRTY_FAILURE_LIMIT` | :24（连续失败 3 次，:150-160 计数；达阈值放弃脏标记允许后端重同步并告警 :153-156） |
| dirtyFields 登记 | `updateConfig` :69-72 / `updateConfigLocal` :101-104 |
| pending 合并 + MASK 保护 | :74-82（`password` 明文优先于 `***` :76-78） |
| debounce 保存 | `updateConfig` :83-93（500ms :93，失败 toast :90）；`customThemeColor` 同步 useThemeStore :94 |
| `saveConfigDirect` | :128-171（成功清脏 :134-137；password/selfPassword 非空→sync*Saved(true) :140-145；in-flight 仅清自己引用 :163-170） |
| `hasPendingConfig` | :183-188（pending 或 in-flight 任一存在） |
| `flushPendingConfig` | :190-220（MASK 密码从增量剔除 :199-202；store 中 `***` 原样发后端识别为"保留" :203-204；flush 发出的保存纳入返回值 :206-213；`Promise.allSettled` 合流 :215-218） |

窗口关闭保护：`useEventListeners.ts:50-75`（`hasPendingConfig` :51 → `preventDefault` :52 → `flushPendingConfig` :55 → `Promise.race` 限时 2s :61-64 → close :69；无 pending 直接 `flushPendingConfig()` :71）。

启动初值：`useInitialDataLoad.ts`（173 行）——`getInitData` :34 → `cfg={...DEFAULT_CONFIG,...initData.config}` :37 → `password===***`→`syncPasswordSaved(true)` :38-39、非空非掩码→false :40-42，`selfPassword` 同 :43-45 → `initTheme` :48 → 面板恢复（quality 被禁则跳过）:50-59 → `isAutoStart&&hiddenStart` 不 showWindow :61-65 → adapters 空兜底 `getAdapters(false)` :67-76 → backgroundStatus 注入 useAuthStore :78-91 → accounts/activeAccount :102-106 → GPU/刷新率 :110-124 → DNS DoH 检查 + 推荐名单告警 :126-147（`RECOMMENDED_DNS` 223.5.5.5/223.6.6.6/1.12.12.12/120.53.53.53 :133）→ 注释 :149-150 说明前端不再主动 `checkNetworkQuality`（后端 latency loop 统一管理）→ `dnsPromise.catch` :152 → `configLoaded=true` :157；任一步失败降级 `DEFAULT_CONFIG`+`configLoaded` :158-166。

保存重试仅此一路：`tauriApiWithRetry` 包装 `saveConfig`（`tauriApi.ts:280-286`，:285；注释 :282-284 说明仅 saveConfig 保留重试），`withRetry` 指数退避（maxRetries=2、baseDelay=500ms，:262-278；`isRetryableError` timeout/network/fetch/connection :250-260）。

安卓双通道：安卓 `useConfigStore.ts`（220 行）与桌面逐行同构（机制行号一致），api 为 `tauriApiWithRetry`（:7、:12）。命令路径 `get_config`/`save_config`（`config_state.rs:405-453`）→ `save_to`（:252-283）→ `emit_config_changed`（:384-391）；巡检路径 `persist_settings`（`monitor_loop.rs:955-978`）同样 emit（:976），且先 `auto_create_account_for_current`（:972）。`get_config` 读取时也刷新 AndroidState 缓存（:413-416），`current_settings`（:456-474）让同步读路径拿到缓存。

## 2026-09-20 增补

- 轻量模式 `lightweight_mode` 默认 true（`model.rs:45-46`/:253）。
- 默认值调整：`auto_exit_after_login=false`（:251）、`auto_exit_on_online=false`（:273）、`background_check_interval=60000`（:258）、`latency_test_interval=600000`（:280）、`enable_latency_test=true`（:278）、`enable_network_quality=true`（:283）。
- 桌面版本迁移 v3→v4（`validate.rs:151-159`）、v4→v5（:163-174：夜切 false→true + scheduled 0→1440），`config_version=5`（`model.rs:306`）。
- 安卓版本迁移 v5→v6（`config_state.rs:346-352`）、v6→v7（:353-365），`config_schema_version=7`（:180）。
- 2026-10-02 夜切新装默认回 false：桌面（`model.rs:262`）、安卓（`config_state.rs:125`）；存量用户经 v4→v5 / v6→v7 迁移仍刷 true。
- 2026-09-27 `required_network_name` 三值化：桌面默认三值（`model.rs:226-228`）、安卓默认三值（`config_state.rs:164`）、lenient/迁移单值扩展（`validate.rs:121-123`、`config_state.rs:291-293`）。
- 出站字段组（`model.rs:70-103`）：`outbound_manual_hold_day` 手动切换当日保持标记，防自动判定叠加（`model.rs:93-94`，i32）；安卓镜像字段 `config_state.rs:38-52`；出站切换态下安卓巡检整轮跳过（`monitor_loop.rs:1260-1263`）。
- 质量历史落盘：`persist.rs:215-274`（`quality_history.json`）+ `network_cmd.rs:211-241`（`check_network_quality` 落盘 :237）。
- 配置导出/导入：`config_cmd.rs:91-207`（密码还原 :178-180、切换态五字段保留本机 :185-189、严格校验 :192-195、四分列失败提示）。
- DNS 优化名单 `dns_optimize_adapters`：桌面消费（`network_cmd.rs:284-478`：`setup_dns_doh` :299-385 / `reset_dns` :387-478）；安卓仅镜像不消费（`config_state.rs:55`）。
- latency 开关持久化修复：`start/stop_latency_test` 落盘 `enable_latency_test`（`network_cmd.rs:243-282`）。

## 关键约束

1. 所有写配置路径必须经 `save_config_to_disk_encrypted`（`config_cmd.rs:9-23`）——它同时负责加密落盘、`config-changed` 广播与托盘刷新，绕过会导致前端不同步。
2. 校验只有严格 `validate_config`（`validate.rs:87-183`）一条权威路径；lenient（:188-255）仅用于读盘容错，保存/导入必须严格（`config_cmd.rs:228`/:192-195）。
3. 保存顺序固定：先落盘成功才更新内存 state（`config_cmd.rs:263-264`；导入同序 :202-203）。
4. 密码出站一律 `masked_for_display`（`model.rs:315-319`）；只有显式 `include_password` 导出才解密（`config_cmd.rs:91-113`），`***` 占位符是合法落盘值（注释 `config_cmd.rs:13-16`，回归测试 :310）。
5. 配置文件永不包含明文密码：桌面 `persist.rs:276-291`（非空即加密）；安卓 `config_state.rs:252-283`（非空才加密、失败 Err 绝不落明文；空密码不产生密文位，测试 :753-764）。
6. 桌面 `config_version=5`（`model.rs:306`）与安卓 `config_schema_version=7`（`config_state.rs:180`）是两套独立版本线，迁移链 `validate.rs:130-181` / `config_state.rs:314-366` 逐级生效；安卓每档迁移即落盘（失败静默）。
7. 登录历史与质量历史上限 100 条（`persist.rs:202-204`/:227-274；`login_history.rs:10`/:67），新事件头插。
8. 安卓所有配置 IO 经 `CONFIG_IO_LOCK` 串行（`account_cmds.rs:15-23`）；桌面为进程内静态 Mutex（多进程场景失效，见 Known Issues）。
9. 安卓账号档案只保存凭据与账号态字段；切换/删除账号时清 `night_outbound_restore`（出站切换态不跨账号残留）。
10. camelCase 是前后端唯一契约（serde `rename_all`；契约测试 `model.rs:367`、`config_state.rs:716-724`）。
11. 1440 是定时任务"禁用"哨兵（`scheduled_*_minutes`），0 是历史遗留的未配置值（迁移时刷成 1440）。
12. 前端 dirtyFields 机制保证用户编辑不被后端回流覆盖（`useConfigStore.ts:69-72`/:110-117）；dirty 字段在后端合并时被跳过（:113-115）。

## Data Flow

```
                桌面                                              安卓
 ┌───────────────────────────────────────┬───────────────────────────────────────────┐
 │ 磁盘: config.json / accounts/<id>.json│ 磁盘: config.json / accounts/<id>.json    │
 │       login-history.json             │       login-history.json                  │
 │       quality_history.json           │                                           │
 │       exports/config-*.json / *.bak  │                                           │
 └──────────────┬────────────────────────┴───────────────────┬───────────────────────┘
        atomic_write persist.rs:12-43                save_file config_state.rs:252-283
        （纳秒 tmp+fsync+rename 重试）               （固定名 tmp+rename，无 fsync）
                │                                            │
                ▼                                            ▼
   AppState.config (model.rs:10-172, 60 字段)      AndroidState 缓存 (Settings, 47 字段)
                ▲                                            ▲
     store（先落盘后内存 config_cmd.rs:263-264）    persist_current/save 后回填 (:442-445)
                │                                            │
   命令层 config_cmd/account/system/background/  命令层 config_state/account_cmds +
          network_cmd                            巡检 persist_settings (monitor_loop.rs:955-978)
                │                                            │
                └────────────┐              ┌────────────────┘
                             ▼              ▼
              save_config_to_disk_encrypted / save_to + emit_config_changed
                             │（config-changed 事件，体为 {"config": masked}）
                             ▼
              前端 useConfigStore（onConfigChanged → mergeConfigFromBackend，
              dirtyFields 保护；写回走 updateConfig → 500ms 防抖 → save_config 命令）
```

读路径：启动时桌面 `startup.rs:185` → lenient 解析（备份损坏文件 :66-78）；安卓 `get_config` :405-418（缓存 + 迁移 + masked 出站）。前端启动初值唯一可靠来源是 `getInitData`（`useInitialDataLoad.ts:34`，安卓侧 `monitor_loop.rs:127-129` 注释同样强调）。

## Connections

- [[concepts/desktop-config|桌面配置命令面]] — Config 模型与 get_config/save_config 命令细节
- [[concepts/security-model|安全模型]] — 密钥管理与密码加密通道
- [[concepts/android-backend|安卓后端架构]] — Settings/CryptoBridge 与命令面
- [[concepts/ipc-command-surface|IPC 命令面]] — 命令清单与参数契约
- [[concepts/desktop-frontend-hooks|桌面前端 hooks]] — useConfigStore/useInitialDataLoad 消费侧
- [[concepts/dual-platform-sharing|双平台共享逻辑]] — campus_login_lib 共享纯函数
- [[concepts/background-check-and-auto-login|后台检测与自动登录]] — 巡检循环消费配置
- [[concepts/desktop-account-selfservice|桌面账号自助管理]] — 账号命令与主配置联动
- [[concepts/lightweight-mode-desktop|轻量模式]] — lightweight_mode 字段的语义
- [[concepts/account-display-name-id-separation|显示名与 id 分离]] — R3 契约
- [[concepts/account-switch-ui-state-desync|账号切换 UI 状态]] — R4 契约与 config-changed 同步

## Known Issues

1. **双端字段集非子集**：交集 43 个；桌面独有 17（adapter1/adapter2/dual_adapter/adapter1_account/adapter2_account/minimize_to_tray/hidden_start/auto_launch/lightweight_mode/auto_exit_after_login/auto_exit_on_online/campus_exit_on_fail/campus_exit_start_minutes/campus_exit_end_minutes/outbound_manual_hold_day/skip_sha256_when_missing/config_version）；安卓独有 4（allow_2d_face_verify/background_check_idle_interval/enable_boot_autostart/config_schema_version）。互导 JSON 时多余字段被静默丢弃（无 `deny_unknown_fields`）。
2. **版本号双轨**：`config_version=5`（`model.rs:306`）与 `config_schema_version=7`（`config_state.rs:180`），跨平台互导不会触发对方迁移链。
3. **安卓原子写弱于桌面**：`save_file` tmp 固定名 `config.json.tmp` 且无 `sync_all`（`config_state.rs:279-281`）；桌面是纳秒 tmp + fsync + rename 重试（`persist.rs:12-43`）。进程被杀时安卓可能留下半写文件。
4. **桌面多进程写竞争**：静态 Mutex 仅进程内有效，多实例并发写仅靠纳秒 tmp 名降低冲突概率，rename 重试 3 次后仍失败会报错（`persist.rs:32-42`）。
5. **安卓无校验层**：`load_from` 只做迁移不做字段校验（`config_state.rs:285-295`），非法值直接进内存；桌面有 lenient + 严格双层。
6. **default 混用**：桌面 35 个字段级 `#[serde(default)]` + 容器级（`model.rs:9`），安卓全容器级（`config_state.rs:14`）；serde 对"显式 null"与"缺字段"处理不同，靠 `deserialize_non_empty_or`（`model.rs:174-184`）兜住部分场景。
7. **`default_panel` 默认值不一致**：安卓 `"dashboard"`（`config_state.rs:147`），桌面空串（`model.rs:282`）。
8. **config-changed 是全量广播**：无增量事件，每次写配置都带全量 masked 配置（`config_cmd.rs:18`、`config_state.rs:384-391`、`monitor_loop.rs:976`）；前端靠 dirtyFields 防覆盖、靠 `***` 占位符识别"保留当前密码"（`useConfigStore.ts:203-204`）。
9. **安卓前端 DEFAULT_CONFIG 与后端默认不一致**：`enableLatencyTest` true（`constants.ts:34`）vs 后端 false（`config_state.rs:151`）；`enableNetworkQuality` true（`:39`）vs false（`:157`）；`requiredNetworkName` 单值 `'i-wxxy'`（`:44`）vs 三值（`:164`）；另有兜底差异 `defaultPanel ''`（`:38`）vs `"dashboard"`（`:147`）。首帧短暂不一致，由 `getInitData` 合并纠正（`useInitialDataLoad.ts:37`，失败降级 :158-166）。
10. **夜切开关迁移语义**：新装默认 false（`model.rs:262`、`config_state.rs:125`），存量旧版本经 v4→v5 / v6→v7 迁移刷成 true（`validate.rs:163-174`、`config_state.rs:353-356`），两类用户默认行为不同。
