---
title: 两端配置字段集不是子集关系，而是交集 + 双向差集
type: learning
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/config_state.rs
tags: [教训, 双端, 配置, 同步]
---

## 现象

新增配置字段时无法照抄某一端——按"双端同步"的直觉去抄会抄错。

## 根因

两端字段集不是子集关系。实读当前代码（2026-10-02 后，行号按源码「行号| 」前缀重新标注）：桌面 `Config` 共 60 个字段（`tauri-app/src-tauri/src/config/model.rs:10-176`，容器级 `#[serde(default)]` 在行9 兜底），安卓 `Settings` 共 47 个字段（`android/src-tauri/src/config_state.rs:28-128`，`#[serde(rename_all = "camelCase", default)]` 在行27）。分解：

- **交集 43 个字段**（按字段存在于两端结构体计，camelCase JSON 名一致）。
- **桌面独有 17 个**：

| 字段 | 源码位置 | 说明 |
| --- | --- | --- |
| `adapter1` | model.rs:23 | 主适配器 |
| `adapter2` | model.rs:24 | 副适配器 |
| `adapter1_account` | model.rs:27-28 | 设备级账号绑定，切账号合并排除（见 [[adapter-account-binding]]） |
| `adapter2_account` | model.rs:30-31 | 同上 |
| `dual_adapter` | model.rs:32-33 | 双适配器开关 |
| `auto_exit_after_login` | model.rs:36-37 | 登录后退出（2026-09-20 起默认 false，见 model.rs:254） |
| `minimize_to_tray` | model.rs:38-39 | 窗口最小化到托盘 |
| `lightweight_mode` | model.rs:42-43 | 轻量化模式，注释明言"桌面专属平台能力，安卓 Settings 无此字段" |
| `hidden_start` | model.rs:44-45 | 隐藏启动 |
| `auto_launch` | model.rs:46-47 | 开机自启 |
| `outbound_manual_hold_day` | model.rs:92-93 | 夜切「立即切换」当日保持标记，安卓端不消费 |
| `auto_exit_on_online` | model.rs:108-109 | 上线后退出（2026-09-20 起默认 false，见 model.rs:277） |
| `campus_exit_on_fail` | model.rs:147-148 | 非校园网自动退出生效开关 |
| `campus_exit_start_minutes` | model.rs:150-151 | 退出时段起点 |
| `campus_exit_end_minutes` | model.rs:153-154 | 退出时段终点 |
| `skip_sha256_when_missing` | model.rs:172-173 | 缺失 sha256 跳过 |
| `config_version` | model.rs:174-175 | 桌面配置版本（默认 5，见 model.rs:310） |

- **安卓独有 4 个**：

| 字段 | 源码位置 | 说明 |
| --- | --- | --- |
| `allow_2d_face_verify` | config_state.rs:36 | 2D 人脸验证开关（默认 false） |
| `background_check_idle_interval` | config_state.rs:81 | 闲时巡检间隔（默认 300_000，见 config_state.rs:157） |
| `enable_boot_autostart` | config_state.rs:95 | 开机自启（安卓专属，默认 false） |
| `config_schema_version` | config_state.rs:127 | 安卓配置 schema 版本（默认 7，见 config_state.rs:197） |

交集相对旧版编译的增量主因是**夜切/出站切换镜像字段族 9 件双端同加**（字段存在但多数不消费）：

`enable_night_operator_switch` (model.rs:56-57 / config_state.rs:41)、`night_operator_restore` (model.rs:59-60 / config_state.rs:43)、`enable_night_outbound_switch` (model.rs:63-64 / config_state.rs:47)、`outbound_priority` (model.rs:69-70 / config_state.rs:50)、`outbound_metric_restore` (model.rs:74-75 / config_state.rs:53)、`outbound_disabled_adapters` (model.rs:80-81 / config_state.rs:56)、`outbound_standby_route` (model.rs:85-86 / config_state.rs:59)、`night_outbound_restore` (model.rs:96-97 / config_state.rs:64)、`dns_optimize_adapters` (model.rs:101-102 / config_state.rs:67)。

注意**字段存在 ≠ 字段消费**：这族镜像字段多数在安卓端"仅镜像配置结构"不消费（config_state.rs 各字段注释明确：outbound_priority 行50、outbound_metric_restore 行53、outbound_disabled_adapters 行56、outbound_standby_route 行59、dns_optimize_adapters 行67 均注明安卓端不消费）；`night_outbound_restore` 反向——桌面落盘但桌面端不消费（model.rs:95-98 注明"桌面端不消费此字段"），是安卓切换态标记。

历史脉络：2026-09-13 双端同加 `scheduled_login_minutes`/`scheduled_logout_minutes`（禁用哨兵 1440，model.rs:161-165 / config_state.rs:113-115）；2026-09-14 双端同加 `display_name`（model.rs:118-119 / config_state.rs:93）、桌面另加 `adapter1_account`/`adapter2_account` 设备级字段——后者是"通用改进里仍含平台专属字段"的实例，serde 契约有测试锁（model.rs:367-396）；2026-10-01 安卓 `required_network_name` 对齐桌面三 SSID 名单（config_state.rs:181 ↔ model.rs:230-232，且安卓载入时旧单值扩三值见 config_state.rs:332-334）；2026-10-02 双端同改夜切开关默认回关（model.rs:265 / config_state.rs:141，存量已落盘 true 不迁移）。

另有两套易混编号：`config_version`（桌面，当前默认 5，model.rs:310，迁移在 validate.rs）与 `config_schema_version`（安卓，当前默认 7，config_state.rs:197，迁移 `migrate_legacy_defaults` 在 config_state.rs:355-417）语义不同却名字相近。交集字段的默认值也不全同：`default_panel`（安卓 `"dashboard"` config_state.rs:164 vs 桌面空串 model.rs:286）、`enable_latency_test`（桌面 true model.rs:282 vs 安卓 false config_state.rs:168）、`enable_network_quality`（桌面 true model.rs:287 vs 安卓 false config_state.rs:174）。其余如 `background_check_interval` 两端均为 60000（model.rs:261 / config_state.rs:155），`latency_test_interval` 两端均为 600000（model.rs:284 / config_state.rs:170），`campus_check_end_minutes` 两端均为 1380（model.rs:300 / config_state.rs:188），`scheduled_login_minutes` 两端均为 1440（model.rs:303 / config_state.rs:192）。

## 解决

"双端同步"按**双向增量同步**执行：先判断字段属于两端通用还是平台专属，再决定是否两端各加一份。通用字段（如夜切族）双端同加但注明消费侧差异；平台专属字段（如 `lightweight_mode` 仅桌面、`allow_2d_face_verify` 仅安卓）只加一端。

## 教训

① 新增配置字段时对照 `config/model.rs` 与 `config_state.rs` 两份定义，别假设一端是另一端的超集（现状 60 vs 47）；② 改默认值要走后端 `Settings::default` +（安卓）schema 迁移（`migrate_legacy_defaults`），且两端默认值允许合理分叉（质量检测、闲时巡检间隔等按平台省电策略各自取值）；③ 不要混淆 `config_version`（桌面，v5）与 `config_schema_version`（安卓，v7），两套版本号各自独立演进、迁移阶梯互不相同；④ 镜像字段"存在但不消费"是常态（安卓镜像桌面夜切状态、桌面忽略安卓标记），判断字段语义必须读字段注释，不能只看字段名在不在两端。

## Connections

[[dual-tree-sync-human-discipline]]、[[android-interval-default-schema-migration]]、[[config-missing-field-load-failure]]、[[adapter-account-binding]]