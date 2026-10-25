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

## 背景

一台机器上可能有多张网卡（虚拟网卡、代理 TUN、蓝牙等），若对所有网卡执行检测/登录/注销/DHCP 等操作类流程，会误伤无关网卡。本决策界定操作类流程的作用域，确保只对用户配置或自动解析出的主/副适配器生效。

## 决策

核心原则与全量公开 API 如下（行号一律以提供的源码「行号| 」前缀为准）：

### 后端核心解析与选择（`tauri-app/src-tauri/src/network/adapter.rs`）

- `find_by_name` (`adapter.rs:16-18`)：按名称在适配器切片中查找，返回 `Option<&Adapter>`。
- `find_with_valid_ip` (`adapter.rs:21-23`)：按名称查找且 `ip` 字段非空。
- `find_dual_adapters` (`adapter.rs:26-39`)：根据配置与名称查找主/副适配器（副适配器需开启 `dual_adapter` 且名称非空且不等于主卡）。
- `is_secondary_adapter_enabled` (`adapter.rs:44-46`)：判断副适配器是否启用（仅 `dual_adapter` 为 true 且 `adapter2` 非空，不含 `!= 主卡` / `!= 哨兵` 条件）。
- `configured_disabled_adapters` (`adapter.rs:53-64`)：纯函数，筛选用户手选（非空且非 `AUTO_DETECT_ADAPTER`）且处于禁用列表的适配器；`adapter1` 手选即纳入，`adapter2` 需 dual 开启且手选，返回 `Vec<&DisabledAdapter>`。
- `resolve_adapter_names` (`adapter.rs:66-118`)：核心解析函数。配置名有效（非空、非哨兵、在当前列表）则使用配置名，否则自动检测（优先有线有 IP > 任意有 IP > 第一个）；副适配器自动检测时排除主适配器，配置名失效时 warn 后降级自动检测（主卡降级见 `adapter.rs:82-88`，副卡降级见 `adapter.rs:101-112`）。
- `filter_operation_adapters` (`adapter.rs:123-128`)：按解析出的名称过滤，仅保留主/副适配器（副名为空时只留主卡）供检测、优化等操作使用。
- `select_adapter` (`adapter.rs:130-142`)：登录出口语义选择，只取解析主卡的 IP，不回退到配置范围外的适配器。
- `select_adapter_for_quality` (`adapter.rs:150-159`)：质量检测专用，主适配器无 IP 时回退到任意有 IP 且未被 `discovery::is_blacklisted` 命中的适配器（避开 VMnet/TUN 等虚拟口，见 `adapter.rs:155`）。
- `ensure_ethernet_ip_for_login` (`adapter.rs:161-239`)：登录前以太网无 IP 补救，对主/副中有线且无 IP 的适配器执行 `ipconfig /renew`。spawn 失败记日志（`adapter.rs:206`）而非误报超时，轮询后 kill 子进程（`adapter.rs:217-218`）防残留。

测试构造类型：测试中 `make_test_adapter` (`adapter.rs:312-328`) 构造 `Adapter`，字段含 `name`, `ip`, `wireless`, `guid`, `mac`, `if_index`, `status`, `link_speed`；`make_test_config` (`adapter.rs:246-310`) 构造完整 `Config`（含 `adapter1`, `adapter2`, `dual_adapter` 等）。后端单测 `adapter.rs:322-454` 覆盖解析降级、双端手选、质量回退等（如 `resolve_adapter_names_falls_back_when_config_name_missing` 行331、`quality_select_falls_back_to_active_when_primary_offline` 行429）。

### 前端同源解析（`tauri-app/frontend/src/network/adapters.ts`）

- `AUTO_DETECT_ADAPTER` (`adapters.ts:3`)：哨兵常量 `'自动检测'`。
- `AdapterScope` 接口 (`adapters.ts:5-9`)：字段 `adapter1: string`, `adapter2: string`, `dualAdapter: boolean`。
- `resolveAdapterNames` (`adapters.ts:15-30`)：与后端同源规则。配置名有效（非空、非哨兵、在当前列表）用配置名；否则自动检测（优先有线有 IP、排除已选主卡 `exclude`），`dualAdapter` 关闭时 `secondary` 恒为空。

### Tauri 命令与作用域（`tauri-app/src-tauri/src/commands/network_cmd.rs`）

- `get_adapters` (`network_cmd.rs:19-25`)：返回全量列表（UI 展示遍历全部，不裁剪）。
- `enable_adapter` (`network_cmd.rs:27-36`)：用户显式点名启用单卡，`validate_adapter_name` 校验 (`network_cmd.rs:30`)。
- `get_adapter_details` (`network_cmd.rs:38-41`)：全量适配器详情缓存读取。
- `outbound_switch_now` (`network_cmd.rs:45-54`) / `outbound_restore_now` (`network_cmd.rs:45-54`)：出站手动切换/还原。
- `get_current_outbound_name` (`network_cmd.rs:70-81`)：当前真实出站接口别名（软降级）。
- `check_campus_status` (`network_cmd.rs:83-102`)：校园网状态检查，使用 `get_adapters_force` 对全量列表做判断。
- `check_portal_status` (`network_cmd.rs:104-134`)：Portal 状态探测（只读，不含凭据）。
- `get_campus_gateway` (`network_cmd.rs:136-140`) / `wrap_dhcp_result` (`network_cmd.rs:142-144`)：辅助工具函数。
- `dhcp_renew_all` (`network_cmd.rs:147-164`)：经 `resolve_adapter_names` (`network_cmd.rs:153`) 与 `filter_operation_adapters` (`network_cmd.rs:154`) 限定主/副，仅续租其中有线卡 (`dhcp_renew_wired_only` 调用见 `network_cmd.rs:156`)。
- `dhcp_release_renew` (`network_cmd.rs:165-186`)：同上限定主/副 (`network_cmd.rs:176-177`)，MAC 重置 + 释放/续租 (`dhcp_release_renew_all` 调用见 `network_cmd.rs:179`)。
- `dhcp_release_renew_adapter` (`network_cmd.rs:189-205`)：单卡显式操作，不受主/副约束（`validate_adapter_name` 校验见 `network_cmd.rs:191`，执行见 `network_cmd.rs:189`）。
- `check_network_quality` (`network_cmd.rs:206-236`)：调用 `select_adapter_for_quality` (`network_cmd.rs:223`) 获取出口，无 IP 返回 `unknown` (`network_cmd.rs:226-228`)。
- `start_latency_test` (`network_cmd.rs:239-265`) / `stop_latency_test` (`network_cmd.rs:239-250`)：延迟测试生命周期管理。
- `check_dns_doh_status` (`network_cmd.rs:280-293`)：DNS/DoH 状态读取。
- `setup_dns_doh` (`network_cmd.rs:383-469`) 与 `reset_dns` (`network_cmd.rs:383-473`)：独立于主/副范围，目标读配置 `dns_optimize_adapters` (`network_cmd.rs:383-388` 与 `network_cmd.rs:383-389`)，过滤存在性 + 黑名单，空/全不可用给出产品契约文案 (`network_cmd.rs:383-393` 与 `network_cmd.rs:383-390`)。

### 架构与数据流

```text
┌────────────────────────┐      ┌─────────────────────────┐
│ Frontend               │      │ Backend (adapter.rs)    │
│ resolveAdapterNames    │◄────►│ resolve_adapter_names   │
│ (adapters.ts:15-30)    │同源  │ (adapter.rs:66-118)     │
└────────────────────────┘      └───────────┬─────────────┘
                                            │ filter_operation_adapters
                                            │ (adapter.rs:123-128)
                                            ▼
┌────────────────────────┐      ┌─────────────────────────┐
│ network_cmd.rs         │      │ select_adapter*         │
│ dhcp_renew_all         │◄─────│ (adapter.rs:130-159)    │
│ (cmd:146-163)          │      └─────────────────────────┘
└────────────────────────┘
```

### 测试锁行为（`tauri-app/frontend/src/network/adapters.test.ts`）

- `makeAdapter` (`adapters.test.ts:5-15`)：测试夹具，默认构造有 IP 有线网卡。
- `describe('resolveAdapterNames')` (`adapters.test.ts:22-68`)：6 组用例锁行为：配置名有效直用 (`:23-28`)、自动检测优先有线有 IP 且排除主卡 (`:30-42`)、配置名失效降级 (`:44-50`)、dual 关闭 secondary 空 (`:52-57`)、仅剩无 IP 卡允许降级选中 (`:59-62`)、空列表返回空名 (`:64-67`)。后端单测 `adapter.rs:330-462` 同步锁行为。

## 理由

操作类流程（检测/登录/注销/DHCP 续租）只作用于解析后的主/副适配器，避免误伤其他网卡；`select_adapter` 明确「不回退到配置范围外的适配器」，与登录/注销的作用范围保持一致（`adapter.rs:120-122` 注释、`adapter.rs:133-135` 注释）。前端与后端共用同源解析规则，防止选择器与实际目标分叉。

## 备选方案

旧文档未记录。

## 影响与约束

- 改适配器解析规则必须两端（前端 `adapters.ts` / 后端 `adapter.rs`）同步，且靠 `adapters.test.ts` (`adapters.test.ts:22-68`) 与 `adapter.rs` 单元测试 (`adapter.rs:330-462`) 锁行为。
- DNS 名单（`dns_optimize_adapters`）为空/全不可用的双态文案是产品契约（「先选择」vs「请重新选择」），后端在 `network_cmd.rs:325-335` 和 `network_cmd.rs:406-413` 拦截。注意过滤条件只有存在性 + 黑名单（见 `network_cmd.rs:321` 与 `network_cmd.rs:402`），不判断已断开（无 IP 卡仍在列表中不会被跳过）。
- 副适配器「启用」语义独立成 `is_secondary_adapter_enabled` (`adapter.rs:44-46`)，调用方（如 `configured_disabled_adapters` `adapter.rs:59-60`）需自行附加「!= 主卡 / != 哨兵」条件。
- `ensure_ethernet_ip_for_login` 的 DHCP 续租已修复 spawn 失败被静默吞掉的历史缺陷（`adapter.rs:205-208` 注释与 `adapter.rs:206` 日志），且轮询结束后 kill 子进程防残留（`adapter.rs:217-218`）。

## Connections

[[protocol-core-single-source]]、[[adapter-visibility-cache-staleness]]