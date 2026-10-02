---
title: 适配器操作范围只作用于主/副适配器（UI 展示遍历全部）
type: decision
source_files:
  - tauri-app/src-tauri/src/network/adapter.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/frontend/src/network/adapters.ts
  - tauri-app/frontend/src/network/adapters.test.ts
tags: [决策, 网络, 适配器, 双端同源]
---

## 背景

一台机器上可能有多张网卡（虚拟网卡、代理 TUN、蓝牙等），若对所有网卡执行检测/登录/注销/DHCP 等操作类流程，会误伤无关网卡。

## 决策

- 操作类流程（检测/登录/注销/DHCP 续租与释放重连）**只作用于** `resolve_adapter_names`（`tauri-app/src-tauri/src/network/adapter.rs:66-118`）解析出的主/副适配器；范围过滤由 `filter_operation_adapters`（`adapter.rs:123-128`）完成：按名称保留主/副（副名为空时只留主卡），其余适配器不参与操作；
- UI 展示类**遍历全部**：`get_adapters`（`tauri-app/src-tauri/src/commands/network_cmd.rs:20-25`）返回全量列表；适配器详情（`:44-46`）、禁用列表（`:28-30`）、DNS/DoH 状态读取（`:285-297`）与校园网状态检查 `check_campus_status`（`:88-107`，经 `get_adapters_force` 对全量列表做 SSID/网关判断 `:94-95`）都不裁剪范围；
- 检测/登录/注销的作用点（当前代码锚点）：
  - 登录/注销：`auth/service.rs:133、252` 与 `commands/login.rs:21` 先 `resolve_adapter_names` 再只操作解析结果；
  - 后台巡检/自动重登：`monitor/background_check.rs:42、85`、`monitor/auto_auth.rs:285、302` 同样 resolve + filter；
  - 网络质量检测：`select_adapter`（`adapter.rs:130-142`）只从解析出的**主适配器**取 IP，不回退到配置范围外的适配器（调用点 `network_cmd.rs:228`、`monitor/latency.rs:103`）；
  - 登录前以太网无 IP 补救 `ensure_ethernet_ip_for_login`（`adapter.rs:144-222`，调用点 `auth/service.rs:106`）：在主/副中筛出**有线且无 IP** 者逐个 `ipconfig /renew`，spawn 失败记日志而非误报续租超时（`:187-192`），轮询 5s 后 kill 子进程防残留（`:196-202`）；
  - 自助服务源 IP 绑定：`resolve_campus_bind_addr`（`commands/self_service.rs:27-33`）复用同源规则（`:30` resolve、`:31` `find_with_valid_ip` 取主卡 IP），保证自助请求从校园网侧发出；
- DHCP 续租/释放重连仍走主/副范围：`dhcp_renew_all`（`network_cmd.rs:152-168`，resolve `:158` → filter `:159-160` → `dhcp_renew_wired_only` `:161`，只续租其中**有线**卡）；`dhcp_release_renew`（`:171-191`，resolve `:181` → filter `:182-183`，MAC 重置 + 释放/续租同样只限主/副）；单卡按钮 `dhcp_release_renew_adapter`（`:194-209`，`validate_adapter_name` 校验 `:196` 后走 `dhcp_release_renew_single` `:202`）与启用网卡 `enable_adapter`（`:33-41`，`validate_adapter_name` `:35`）都是用户显式点名的单适配器操作，不受主/副范围约束；
- 后台状态快照 `get_background_status_value`（`commands/background.rs:46-69`）只对解析出的主/副构建状态条目（resolve `:58`），属展示快照而非操作过滤；
- 前端 `network/adapters.ts::resolveAdapterNames`（`tauri-app/frontend/src/network/adapters.ts:15-30`）与后端**同源规则**：配置名有效（非空、非哨兵 `AUTO_DETECT_ADAPTER`＝"自动检测"（`adapters.ts:3`）、在当前列表中）→ 用配置名；否则自动检测——有线有 IP > 任意有 IP > 第一个（`adapters.ts:16-19`；后端同序 `adapter.rs:68-75`，配置名不在列表时后端 warn 后降级自动检测 `adapter.rs:82-88`），副适配器自动检测时额外排除主适配器（`adapters.ts:16-19` 的 `exclude`；后端 `adapter.rs:92-95`），`dualAdapter` 关闭时 secondary 恒为空（`adapters.ts:25-27`）。改任一侧必须同步另一侧，`adapters.test.ts`（6 组用例，`tauri-app/frontend/src/network/adapters.test.ts:22-68`）锁行为：配置名有效直用（`:23-28`）、自动检测优先有线有 IP 且排除主卡（`:30-42`）、配置名失效降级（`:44-50`）、dual 关闭 secondary 空（`:52-57`）、仅剩无 IP 卡允许降级选中（`:59-62`）、空列表返回空名（`:64-67`）；
- **2026-09-27 修订：DNS 设置/恢复摘出主/副范围**。一键优化 DNS（`setup_dns_doh`，`network_cmd.rs:300-385`）与恢复 DNS（`reset_dns`，`:388-478`）的目标改为配置字段 `dns_optimize_adapters`（`config/model.rs:103`，默认空 `:270`；前端 `settings/types.ts:46` `dnsOptimizeAdapters: string[]`，默认 `[]` `constants.ts:34`）——DNS 优化卡内 chip 多选显式名单（`NetworkPanel.tsx:912`，切换 `:323-325`），可多选、可含非主/副卡；空名单操作时提示先选择，名单内卡不在当前适配器列表或命中黑名单则跳过（后端过滤 `network_cmd.rs:323-328`、`:403-409`，注意：过滤条件只有**存在性 + 黑名单**，不判断"已断开"——无 IP 卡仍在列表中不会被跳过），全不可用报「请重新选择」（`:330-340`、`:411-418`）。两命令的提权路径一致：管理员直接执行（`:342-344`、`:420-443`），非管理员经 `--helper`（`dns` / `clear_dns`）提权（`:346-382`、`:445-475`），30s 超时。DHCP 续租（`dhcp_renew_all`/`dhcp_release_renew`）与检测/登录/注销仍走主/副范围，本修订不涉及。

## 理由

旧文档未记录。代码注释给出的动机：检测、优化等操作只作用于解析结果，避免误伤主/副之外的网卡；`select_adapter` 明确「不回退到配置范围外的适配器」，与登录/注销的作用范围保持一致（`adapter.rs:120-122`、`:133-135`）。

## 备选方案

旧文档未记录。

## 影响与约束

- 改适配器解析规则必须两端同步，且靠 `adapters.test.ts` 锁行为；前端消费方为登录/注销选择器（`DashboardPanel.tsx:22、147`、`DockNav.tsx:28、430`），后端消费方为登录/注销/巡检/自动重登/自助服务（见上）。
- 注意解析结果在安卓恒为空（安卓前端无 adapters 概念，列表恒空、`getAdapters` 必 reject——`android/frontend/src/hooks/useAuthStore.ts:52`、`android/frontend/src/hooks/tauriApi.ts:203-204`），消费方需容忍；安卓端没有复刻 `resolveAdapterNames`。
- DNS 名单为空/全不可用的双态文案是产品契约（「先选择」vs「请重新选择」），前端按钮同样在空名单时拦截（`NetworkPanel.tsx:351、381`）。
- 副适配器「启用」语义独立成函数 `is_secondary_adapter_enabled`（`adapter.rs:44-46`）：仅判断 dual_adapter 开关 + adapter2 非空，不含「≠主卡 / ≠哨兵」条件，调用方需自行附加（同文件的 `configured_disabled_adapters` `:53-64` 是其使用示例）。

## Connections

[[protocol-core-single-source]]、[[adapter-visibility-cache-staleness]]
