---
title: 适配器操作范围与解析规则
type: decision
source_files:
  - tauri-app/src-tauri/src/network/adapter.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/frontend/src/network/adapters.ts
  - tauri-app/frontend/src/network/adapters.test.ts
tags: [决策, 网络, 适配器, 双端同源]
---

## Overview

一台机器上可能有多张网卡（虚拟网卡、代理 TUN、蓝牙等），若对所有网卡执行检测/登录/注销/DHCP 等操作类流程，会误伤无关网卡。本决策界定操作类流程的作用域，确保只对用户配置或自动解析出的主/副适配器生效。

核心原则：操作类流程（检测/登录/注销/DHCP 续租）只作用于解析后的主/副适配器，避免误伤其他网卡；`select_adapter`（`adapter.rs:130-142`）明确「不回退到配置范围外的适配器」，与登录/注销的作用范围保持一致（`adapter.rs:133-135`）。前端与后端共用同源解析规则，防止选择器与实际目标分叉。

## Key Components

### 后端核心模块 `tauri-app/src-tauri/src/network/adapter.rs`

模块级注释（`adapter.rs:1-5`）声明：本模块保留适配器选择、登录前以太网 IP 检查职责；适配器发现已迁移到 `network::discovery`，缓存访问到 `network::adapter_cache`，DHCP/MAC 重置到 `network::dhcp`，子网/SSID/网关到 `network::subnet`。

**公开函数：**

- `find_by_name`（`adapter.rs:16-18`）：按名称在适配器切片中查找，返回 `Option<&Adapter>`。
- `find_with_valid_ip`（`adapter.rs:21-23`）：按名称查找且 `ip` 字段非空，返回 `Option<&Adapter>`。
- `find_dual_adapters`（`adapter.rs:26-39`）：根据配置与名称查找主/副适配器（副适配器需开启 `dual_adapter` 且名称非空且不等于主卡），返回 `(Option<&Adapter>, Option<&Adapter>)`。
- `is_secondary_adapter_enabled`（`adapter.rs:44-46`）：判断副适配器是否启用（仅 `dual_adapter` 为 true 且 `adapter2` 非空，不含 `!= 主卡` / `!= 哨兵` 条件）。
- `configured_disabled_adapters`（`adapter.rs:53-64`）：纯函数，筛选用户手选（非空且非 `AUTO_DETECT_ADAPTER`）且处于禁用列表的适配器；`adapter1` 手选即纳入，`adapter2` 需 dual 开启且手选，返回 `Vec<&DisabledAdapter>`。
- `resolve_adapter_names`（`adapter.rs:66-118`）：核心解析函数。配置名有效（非空、非哨兵、在当前列表）则使用配置名，否则自动检测（优先有线有 IP > 任意有 IP > 第一个）；副适配器自动检测时排除主适配器，配置名失效时 `log_warn!` 后降级自动检测（`adapter.rs:82-88`、`adapter.rs:101-112`）。
- `filter_operation_adapters`（`adapter.rs:123-128`）：按解析出的名称过滤，仅保留主/副适配器（副名为空时只留主卡）供检测、优化等操作使用，返回 `Vec<Adapter>`。
- `select_adapter`（`adapter.rs:130-142`）：登录出口语义选择，只取解析主卡的 IP，不回退到配置范围外的适配器；返回 `(String, String)`（IP, 名称）。
- `select_adapter_for_quality`（`adapter.rs:150-159`）：质量检测专用，主适配器无 IP 时回退到任意有 IP 且未被 `is_blacklisted` 命中的适配器（避开 VMnet/TUN 等虚拟口），返回 `(String, String)`。
- `ensure_ethernet_ip_for_login`（`adapter.rs:161-239`）：登录前以太网无 IP 补救，对主/副中有线且无 IP 的适配器执行 `ipconfig /renew`（`adapter.rs:199-209` spawn 命令）。spawn 失败记日志（`adapter.rs:205-208`）而非误报超时，轮询后 kill 子进程（`adapter.rs:217-218`）防残留。

**测试辅助构造与结构体字段：**

- `make_test_config`（`adapter.rs:246-313`）：构造完整 `Config` 测试实例。字段全量如下：
  `user`, `password`, `self_hello_enabled`, `self_reverify_each_action`, `operator`, `adapter1`, `adapter2`, `adapter1_account`, `adapter2_account`, `display_name`, `device_aliases`, `dual_adapter`, `auto_login_on_start`, `auto_exit_after_login`, `minimize_to_tray`, `lightweight_mode`, `hidden_start`, `auto_launch`, `enable_background_check`, `background_check_interval`, `auto_login_on_preparation`, `enable_night_operator_switch`, `night_operator_restore`, `enable_night_outbound_switch`, `night_restore_minutes`, `outbound_priority`, `outbound_metric_restore`, `outbound_disabled_adapters`, `outbound_standby_route`, `outbound_manual_hold_day`, `outbound_pending_switch`, `night_outbound_restore`, `dns_optimize_adapters`, `auto_exit_on_online`, `theme_mode`, `enable_notification`, `active_account`, `enable_latency_test`, `latency_test_interval`, `custom_theme_color`, `default_panel`, `enable_network_quality`, `skip_ttfb_in_latency`, `skip_content_in_latency`, `portal_url`, `fixed_gateway`, `required_network_name`, `enable_network_name_check`, `campus_gateway`, `campus_exit_on_fail`, `campus_exit_start_minutes`, `campus_exit_end_minutes`, `campus_check_start_minutes`, `campus_check_end_minutes`, `scheduled_login_minutes`, `scheduled_logout_minutes`, `log_retention_days`, `max_disconnect_reconnect`, `auto_login_cooldown_secs`, `skip_sha256_when_missing`, `update_source`, `vpn_account`, `vpn_password_enc`, `self_service_via_vpn`, `config_version`。

- `make_test_adapter`（`adapter.rs:316-332`）：构造 `Adapter` 测试实例。字段全量：`name: String`, `ip: String`, `wireless: bool`, `guid: String`, `mac: String`, `if_index: i32` (测试中赋 1), `status: AdapterStatus`, `link_speed: i32` (测试中赋 0)。

- `make_disabled`（`adapter.rs:369-375`）：构造 `DisabledAdapter` 测试实例。字段全量：`name: String`, `status: AdapterStatus`, `description: String`。

**单元测试方法（锁行为）：**

- `resolve_adapter_names_falls_back_when_config_name_missing`（`adapter.rs:335-345`）
- `resolve_adapter_names_uses_config_when_present`（`adapter.rs:347-357`）
- `resolve_adapter_names_auto_detect_prefers_wired_with_ip`（`adapter.rs:359-368`）
- `configured_disabled_adapters_manual_only`（`adapter.rs:378-420`）
- `quality_select_uses_primary_when_ip_present`（`adapter.rs:422-431`）
- `quality_select_falls_back_to_active_when_primary_offline`（`adapter.rs:433-443`）
- `quality_select_fallback_skips_virtual_adapters`（`adapter.rs:445-456`）
- `quality_select_returns_empty_when_nothing_active`（`adapter.rs:458-467`）

### 后端 Tauri 命令 `tauri-app/src-tauri/src/commands/network_cmd.rs`

- `empty_quality_json`（`network_cmd.rs:15-17`）：构造空质量 JSON 辅助。
- `get_adapters`（`network_cmd.rs:19-25`）：返回全量列表（UI 展示遍历全部，不裁剪），`force` 参数决定 `get_adapters_force` 或 `get_adapters_cached`。
- `enable_adapter`（`network_cmd.rs:27-36`）：用户显式点名启用单卡，`validate_adapter_name` 校验（`network_cmd.rs:30`），允许 COM 提权失败降级弹 UAC。
- `get_adapter_details`（`network_cmd.rs:38-41`）：全量适配器详情缓存读取。
- `outbound_switch_now`（`network_cmd.rs:45-54`）：「立即切换出站」手动触发，绕过自动切换退避闸。
- `outbound_restore_now`（`network_cmd.rs:57-66`）：「立即还原」手动触发。
- `get_current_outbound_name`（`network_cmd.rs:70-81`）：当前真实出站接口别名（软降级），查询失败返回 `None`。
- `check_campus_status`（`network_cmd.rs:83-106`）：校园网状态检查，使用 `get_adapters_force` 对全量列表判断。
- `check_portal_status`（`network_cmd.rs:107-137`）：Portal 状态探测（只读，不含凭据）；注销保护期内直接返回离线（`network_cmd.rs:118-125`）。
- `get_campus_gateway`（`network_cmd.rs:139-143`）：读取配置或默认校园网关辅助。
- `wrap_dhcp_result`（`network_cmd.rs:145-147`）：包装 DHCP 结果为 `{"success": true, "results": ...}`。
- `dhcp_renew_all`（`network_cmd.rs:150-167`）：经 `resolve_adapter_names`（`network_cmd.rs:156`）与 `filter_operation_adapters`（`network_cmd.rs:157-158`）限定主/副，仅续租其中有线卡（`dhcp_renew_wired_only`）。
- `dhcp_release_renew`（`network_cmd.rs:168-189`）：同上限定主/副（`network_cmd.rs:179-180`），MAC 重置 + 释放/续租（`dhcp_release_renew_all`）。
- `dhcp_release_renew_adapter`（`network_cmd.rs:192-208`）：单卡显式操作（`dhcp_release_renew_single`），不受主/副约束，校验名（`network_cmd.rs:192`）。
- `check_network_quality`（`network_cmd.rs:210-240`）：调用 `select_adapter_for_quality`（`network_cmd.rs:226`）获取出口，无 IP 返回 `unknown`；总开关关闭返回 `disabled`（`network_cmd.rs:214-216`）。
- `start_latency_test`（`network_cmd.rs:242-268`）：延迟测试启动。历史缺陷修复：总开关 `enable_network_quality` 关闭时定时测试不应运行，此处直接返回并落盘 false（`network_cmd.rs:248-254`）；持久化开关避免重启丢失（`network_cmd.rs:242-246, 261-264`）。
- `stop_latency_test`（`network_cmd.rs:270-281`）：取消任务并持久化关闭开关。
- `check_dns_doh_status`（`network_cmd.rs:283-296`）：DNS/DoH 状态读取；非 Windows 返回空结构（`network_cmd.rs:283-290`）。
- `setup_dns_doh`（`network_cmd.rs:297-383`）：独立于主/副范围，目标读配置 `dns_optimize_adapters`（`network_cmd.rs:318-326`），过滤存在性 + 黑名单；非 Windows 直接返回仅支持 Windows（`network_cmd.rs:307-311`）；空/全不可用给出契约文案（`network_cmd.rs:328-338`）。
- `reset_dns`（`network_cmd.rs:386-476`）：恢复 DNS 自动获取，目标同 `dns_optimize_adapters`（`network_cmd.rs:386-397`）；非 Windows 返回仅支持 Windows（`network_cmd.rs:386-390`）；空/全不可用给出契约文案（`network_cmd.rs:386-393`）。

### 前端同源解析 `tauri-app/frontend/src/network/adapters.ts`

- `AUTO_DETECT_ADAPTER`（`adapters.ts:3`）：哨兵常量 `'自动检测'`。
- `AdapterScope` 接口（`adapters.ts:5-9`）：字段全量 `adapter1: string`, `adapter2: string`, `dualAdapter: boolean`。
- `resolveAdapterNames`（`adapters.ts:15-29`）：与后端同源规则。配置名有效（非空、非哨兵、在当前列表）用配置名；否则自动检测（优先有线有 IP、排除已选主卡 `exclude`），`dualAdapter` 关闭时 `secondary` 恒为空。返回 `{ primary, secondary }`。

### 前端测试锁 `tauri-app/frontend/src/network/adapters.test.ts`

- `makeAdapter`（`adapters.test.ts:5-15`）：测试夹具，默认构造有 IP 有线网卡。覆盖 `Adapter` 字段：`name`, `ip`, `wireless`, `mac`, `ifIndex`, `status` 及 `...overrides`。
- `describe('resolveAdapterNames')`（`adapters.test.ts:22-68`）：6 组用例锁行为：
  - 配置名有效直用（`adapters.test.ts:23-28`）
  - 自动检测优先有线有 IP 且排除主卡（`adapters.test.ts:30-42`）
  - 配置名失效降级（`adapters.test.ts:44-50`）
  - dual 关闭 secondary 空（`adapters.test.ts:52-57`）
  - 仅剩无 IP 卡允许降级选中（`adapters.test.ts:59-62`）
  - 空列表返回空名（`adapters.test.ts:64-67`）

## Architecture

```text
┌────────────────────────┐      ┌─────────────────────────┐
│ Frontend               │      │ Backend (adapter.rs)    │
│ resolveAdapterNames    │◄────►│ resolve_adapter_names   │
│ (adapters.ts:15-29)    │同源  │ (adapter.rs:66-118)     │
└────────────────────────┘      └───────────┬─────────────┘
                                            │ filter_operation_adapters
                                            │ (adapter.rs:123-128)
                                            ▼
┌────────────────────────┐      ┌─────────────────────────┐
│ network_cmd.rs         │      │ select_adapter*         │
│ dhcp_renew_all         │◄─────│ (adapter.rs:130-159)    │
│ (network_cmd.rs:149-166)│     └─────────────────────────┘
└────────────────────────┘
```

## Data Flow

1. 前端 `resolveAdapterNames`（`adapters.ts:15-29`）与后端 `resolve_adapter_names`（`adapter.rs:66-118`）基于同一规则将配置映射为具体主/副适配器名称。
2. Tauri 命令（如 `dhcp_renew_all` `network_cmd.rs:150-167`）先调用 `resolve_adapter_names` 取得名称，再调用 `filter_operation_adapters`（`adapter.rs:123-128`）裁剪出操作目标列表。
3. 操作执行限定于裁剪后列表：`dhcp_renew_wired_only` 仅续租有线（`network_cmd.rs:159`）；`select_adapter` / `select_adapter_for_quality` 提供登录/测速出口 IP（`adapter.rs:130-159`）。
4. DNS 优化路径（`setup_dns_doh` `network_cmd.rs:386-472`、`reset_dns` `network_cmd.rs:386-476`）不跟随主/副解析，改读 `dns_optimize_adapters` 配置名单并过滤黑名单，独立于上述数据流。
5. 质量检测异步回退：当主卡离线，`select_adapter_for_quality` 回退至非黑名单活跃卡（`adapter.rs:150-159`），经 `check_network_quality_async` 测速（`network_cmd.rs:232`）。

## Connections

[[protocol-core-single-source]]、[[adapter-visibility-cache-staleness]]

## Known Issues

- **`ensure_ethernet_ip_for_login` 历史缺陷修复**：原实现 spawn 失败被 `if let Ok` 静默吞掉仍走 poll 误报超时，现 `adapter.rs:204-208` 记录 `log_warn!` 并返回 `None`；轮询结束后 kill 子进程（`adapter.rs:217-218`）防 `ipconfig` 悬挂残留，kill 失败仅记录不致命。
- **`select_adapter` 作用域收敛**：原实现配置名无 IP 时会静默选中任意有 IP 的外部适配器，现统一经 `resolve_adapter_names` 解析且只在解析主卡取 IP，不回退（`adapter.rs:133-135`）。
- **`start_latency_test` 总开关校验**：历史缺陷为运行路径不校验总开关，关总开关后定时测试仍做全量外网检测；现 `network_cmd.rs:242-248` 拦截并返回，且同步落盘 `enable_latency_test = false`。
- **`start_latency_test` 开关持久化**：历史缺陷为仅前端本地更新重启即回退，现 `network_cmd.rs:242-246` 与 `network_cmd.rs:242-245` 经 `save_config_to_disk_encrypted` 落盘。
- **DNS 优化范围分叉**：DNS 操作不再跟随适配器设置卡的主/副解析，改为显式 `dns_optimize_adapters` 名单（`network_cmd.rs:318-326`、`network_cmd.rs:396-407`）；过滤条件只有存在性 + 黑名单，不判断已断开（无 IP 卡仍在列表中不会被跳过），空/全不可用双态文案是产品契约（`network_cmd.rs:328-338`、`network_cmd.rs:409-416`）。
- **副适配器启用语义**：`is_secondary_adapter_enabled`（`adapter.rs:44-46`）仅判断 `dual_adapter && !adapter2.is_empty()`，调用方（如 `configured_disabled_adapters` `adapter.rs:59-60`）需自行附加「!= 主卡 / != 哨兵」条件。
