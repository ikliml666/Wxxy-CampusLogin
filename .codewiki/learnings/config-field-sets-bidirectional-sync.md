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

两端字段集不是子集关系。实读当前代码（2026-10-02，行号已验证）：桌面 `Config` 共 60 个字段（`tauri-app/src-tauri/src/config/model.rs:10-172`，容器级 `#[serde(default)]` 兜底），安卓 `Settings` 共 47 个字段（`android/src-tauri/src/config_state.rs:15-111`，`#[serde(rename_all = "camelCase", default)]`）。分解：

- **交集 43 个字段**（按字段存在于两端结构体计，camelCase JSON 名一致）。
- **桌面独有 17 个**：双适配器三件 `adapter1`/`adapter2`/`dual_adapter`（model.rs:26-27、35-36）；适配器账号绑定两件 `adapter1_account`/`adapter2_account`（model.rs:28-34，设备级字段，切账号合并明确排除，见 [[adapter-account-binding]]）；退出策略两件 `auto_exit_after_login`/`auto_exit_on_online`（model.rs:39-40、104-105）；窗口/托盘形态四件 `minimize_to_tray`/`lightweight_mode`/`hidden_start`/`auto_launch`（model.rs:41-50，`lightweight_mode` 注释明言"桌面专属平台能力，安卓 Settings 无此字段"）；夜切「立即切换」当日保持标记 `outbound_manual_hold_day`（model.rs:88-94，安卓端不消费）；`campus_exit_*` 三件（model.rs:143-150）；`skip_sha256_when_missing`（model.rs:168-169）；`config_version`（model.rs:170-171）。
- **安卓独有 4 个**：`allow_2d_face_verify`（config_state.rs:22-24）、`background_check_idle_interval`（config_state.rs:60-64）、`enable_boot_autostart`（config_state.rs:78）、`config_schema_version`（config_state.rs:104-110）。

交集相对上次编译（34 个）的增量主因是**夜切/出站切换镜像字段族 9 件双端同加**：`enable_night_operator_switch`、`night_operator_restore`、`enable_night_outbound_switch`、`outbound_priority`、`outbound_metric_restore`、`outbound_disabled_adapters`、`outbound_standby_route`、`night_outbound_restore`、`dns_optimize_adapters`（model.rs:57-103 / config_state.rs:26-55）。注意**字段存在 ≠ 字段消费**：这族镜像字段多数在安卓端"仅镜像配置结构"不消费（config_state.rs:36-55 各字段注释）；`night_outbound_restore` 反向——桌面落盘但桌面端不消费，是安卓切换态标记（model.rs:95-98 / config_state.rs:48-52）。

历史脉络：2026-09-13 双端同加 `scheduled_login_minutes`/`scheduled_logout_minutes`（禁用哨兵 1440）；2026-09-14 双端同加 `display_name`、桌面另加 `adapter1_account`/`adapter2_account` 设备级字段——后者是"通用改进里仍含平台专属字段"的实例，serde 契约有测试锁（model.rs:364-395）；2026-10-01 安卓 `required_network_name` 对齐桌面三 SSID 名单（config_state.rs:162-164 ↔ model.rs:226-228）；2026-10-02 双端同改夜切开关默认回关（model.rs:260-262 / config_state.rs:123-125）。

另有两套易混编号：`config_version`（桌面，当前默认 5，model.rs:306，迁移在 validate.rs）与 `config_schema_version`（安卓，当前默认 7，config_state.rs:180，迁移 `migrate_legacy_defaults` 在 config_state.rs:314-366）语义不同却名字相近。交集字段的默认值也不全同：`default_panel`（安卓 `"dashboard"`、桌面空串，config_state.rs:147 vs model.rs:282）、`enable_latency_test`（桌面 true / 安卓 false，model.rs:278 vs config_state.rs:151）、`enable_network_quality`（桌面 true / 安卓 false，model.rs:283 vs config_state.rs:157）。

## 解决

"双端同步"按**双向增量同步**执行：先判断字段属于两端通用还是平台专属，再决定是否两端各加一份。

## 教训

① 新增配置字段时对照 `config/model.rs` 与 `config_state.rs` 两份定义，别假设一端是另一端的超集（现状 60 vs 47）；② 改默认值要走后端 `Settings::default` +（安卓）schema 迁移，且两端默认值允许合理分叉（质量检测、闲时巡检间隔等按平台省电策略各自取值）；③ 不要混淆 `config_version`（桌面，v5）与 `config_schema_version`（安卓，v7），两套版本号各自独立演进、迁移阶梯互不相同；④ 镜像字段"存在但不消费"是常态（安卓镜像桌面夜切状态、桌面忽略安卓标记），判断字段语义必须读字段注释，不能只看字段名在不在两端。

## Connections

[[dual-tree-sync-human-discipline]]、[[android-interval-default-schema-migration]]、[[config-missing-field-load-failure]]、[[adapter-account-binding]]
