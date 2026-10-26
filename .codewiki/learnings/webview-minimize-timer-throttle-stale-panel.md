---
title: NetworkPanel 网络适配器与出站管理面板
type: module
source_files:
  - tauri-app/frontend/src/network/NetworkPanel.tsx
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/frontend/src/network/useOutboundManual.ts
  - tauri-app/frontend/src/network/useDnsSetup.ts
  - tauri-app/frontend/src/network/outboundOrder.ts
tags: [前端, Tauri, 网络, DNS, 出站切换, WebView2]
---

## Overview

`NetworkPanel` 是应用的核心网络配置界面，负责展示系统网络适配器列表、管理主/副适配器角色、执行夜间自动出站切换的手动干预（立即切换/还原）、以及提供 DNS/DoH 的一键优化与检测功能。

该模块解决了 WebView2 最小化期间定时器节流导致的状态陈旧问题：在 `NetworkPanel.tsx` 行297-318 的注释与代码中，因最小化时定时器被节流且夜间自动切换多数不改变适配器数量，恢复窗口常看到陈旧出站徽标与列表；故通过 `visibilitychange`/`focus` 事件在窗口恢复时强制同步后端状态（`syncOnRestore` 定义于行301-307，事件绑定 effect 于行308-318，5s 节流去重）。同时处理了适配器拖拽排序、DHCP 续租、禁用/启用等交互。当前后端已不再提供独立禁用适配器接口，所有适配器（含 `disabled` 状态）由 `get_adapters` 统一推送，前端在行224-227 通过 `effectiveDisabledAdapters` 派生过滤。

恢复时刻（`nightRestoreMinutes`）现可配置（行459-486 的 `SettingRow` 与 `Input`），钳制在 `[05:00, 22:59]`（常量 `RESTORE_TIME_MIN_MINUTES` 行60、`RESTORE_TIME_MAX_MINUTES` 行56，默认 `RESTORE_TIME_DEFAULT_MINUTES` 行53），与后端归一化兜底同口径。历史版本的挂起出站切换态（`outboundPendingSwitch`）已移除，后端每拍清理（见行487-493 注释）。

## Key Components

### Frontend: tauri-app/frontend/src/network/NetworkPanel.tsx

**常量:**
- `FOLLOW_CURRENT_ACCOUNT = '__follow__'` (行41): 适配器账号选择哨兵值，标识跟随当前登录账号。
- `NO_BACKUP_ADAPTER = '__none__'` (行42): 无备用适配器哨兵值。
- `RESTORE_TIME_DEFAULT_MINUTES = 450` (行53): 恢复时刻默认值 07:30，与后端 `DEFAULT_RESTORE_START_MINUTES` 同值。
- `RESTORE_TIME_MAX_MINUTES = 1379` (行56): 恢复时刻上限 22:59，必须早于最早切换时刻 23:00。
- `RESTORE_TIME_MIN_MINUTES = 300` (行60): 恢复时刻下限 05:00，产品口径防与凌晨补切窗口冲突。
- `DRAG_LONG_PRESS_MS = 250` (行72): 长按起拖时长 250ms。
- `DRAG_DEAD_ZONE_PX = 8` (行74): 长按等待期内允许位移死区 8px。

**函数:**
- `formatSpeed(bps?: number): string` (行45-49): 连接速度格式化，bit/s → Mbps/Kbps，未知返回空串。
- `formatRestoreTime(minutes: number | undefined): string` (行63-69): 分钟数 → "HH:MM"；0/越界按默认 07:30 显示。

**接口:**
- `NetworkPanelProps` (行36-39): 字段 `adapters: Adapter[]`、`onUpdateConfig: (partial: Partial<Config>) => void`。
- `SortableAdapterRowProps` (行76-88): 字段 `adapter: Adapter`、`isOutboundTarget: boolean`、`isCurrentOutbound: boolean`、`roleLabel?: string`、`onDragEndCommit: () => void`、`onDragStart: () => void`、`children: React.ReactNode`。

**组件:**
- `SortableAdapterRow` (memo, 行95-215): 可拖拽适配器行，实现长按 250ms 起拖（移动超 8px 死区取消，防滚动/点击误触）。行体按下布置定时器（行120-129），按钮区 `stopPropagation` 防拖走整行（行209）。
- `NetworkPanel` (memo, 行217-822): 主面板组件。

**核心状态与逻辑（NetworkPanel 组件内）:**
- `effectiveDisabledAdapters` (行224-227): 从 `adapters` 过滤 `status === 'disabled'` 派生。
- `config` (行230): 自订阅 `useConfigStore` 浅比较，避免 App 外壳级联重渲染。
- `accounts` (行231): 账号列表。
- 在线运营商标签 (行233-237): 从 `useAuthStore` 取 `onlineOperator` 等，仅 online 态显示。
- `outboundOrder` (行243-247): 基于 `config.outboundPriority` 与当前 `adapters` 构建完整出站顺序（`buildOutboundOrder`）。
- `outboundOrderKey` (行248): 顺序键，用于 effect 依赖。
- `dragOrder` / `isDraggingRef` / `dragOrderRef` (行249-251): 拖拽期间本地顺序渲染，非拖拽时同步外部。
- 同步 effect (行252-259): 非拖拽中时重置本地态到 `outboundOrder`。
- `displayedOrder` / `orderedAdapters` (行260-263): 展示列表 = 拖拽态或外部顺序映射的适配器对象。
- `handleDragStart` (行266-268): 标记拖拽中，屏蔽外部顺序同步。
- `handleReorder` (行271-275): 拖拽每帧回调，仅写本地态。
- `handleDragEnd` (行279-287): 拖拽结束提交 `outboundPriority` 并解除本地态（顺序未变不落盘）。
- `gettingNewIpAdapter` / `enablingAdapter` (行289-290): 单卡操作 busy 态。
- `ipc = tauriApiWithRetry` (行291): IPC 封装。
- `mountedRef` (行292): StrictMode 下防止异步 setState 丢失。
- `useOutboundManual` 调用 (行295): 提供 `currentOutbound` 及切换/还原动作。
- `lastFocusSyncRef` (行300) 与 `syncOnRestore` (行301-307): 恢复可见性时强制 `refreshAdapterData({force:true})` 和 `refreshCurrentOutbound()`，5s 节流。
- 可见性/焦点 effect (行308-318): 绑定 `visibilitychange` 与 `focus` 到 `syncOnRestore`。
- 挂载对账 effect (行323-325): 挂载时 `refreshAdapterData()` 对齐 store。
- StrictMode mountedRef effect (行327-332): 二次 setup 恢复 `mountedRef`。
- `dnsStatus` / `dnsChecking` (行334-335): 从 `useQualityStore` 取 DNS 状态。
- `refreshAdapters` / `isRefreshingAdapters` (行336-337): 适配器 store 刷新动作。
- `useDnsSetup` 调用 (行339): 提供 DNS 检测/优化/恢复逻辑。
- `dnsAdapterCount` (行341): DNS 结果卡数量。
- `familyTabs` (行342-346): IPv4/IPv6/both 切换标签。
- `dnsAdapterCandidates` (行349-358): DNS 优化目标适配器候选（已选 ∪ 当前有 IP 活跃卡）。
- `toggleDnsAdapter` (行359-362): 切换 DNS 优化目标适配器至 `config.dnsOptimizeAdapters`。
- `handleGetNewIpForAdapter` (行364-379): 单适配器 DHCP 释放/续租（`ipc.dhcpReleaseRenewAdapter`），结果归一化提示并刷新。
- `handleEnableAdapter` (行381-399): 启用禁用适配器（`ipc.enableAdapter`），强制刷新适配器列表。

### Backend: tauri-app/src-tauri/src/commands/network_cmd.rs

**辅助函数:**
- `empty_quality_json(quality: &str)` (行15-17): 构造空质量 JSON。
- `get_campus_gateway(state: &AppState) -> String` (行139-143): 取校园网关配置或默认。
- `wrap_dhcp_result(results: Vec<serde_json::Value>)` (行145-147): 包装 DHCP 结果为 `{success:true, results}`。

**Tauri 命令 (pub async fn / pub fn):**
- `get_adapters(force: Option<bool>)` (行19-25): 获取适配器列表（缓存或强制刷新），统一推送所有卡（含禁用）。
- `enable_adapter(adapter_name: String)` (行27-36): 启用指定适配器，允许 COM 提权失败降级弹 UAC。
- `get_adapter_details()` (行38-41): 获取适配器详情。
- `outbound_switch_now(app_handle: AppHandle)` (行44-53): 手动立即切换出站，调用 `manual_outbound_switch`。
- `outbound_restore_now(app_handle: AppHandle)` (行56-65): 手动立即还原出站，调用 `manual_outbound_restore`。
- `get_current_outbound_name()` (行69-80): 获取当前真实出站接口别名（默认路由 metric 求和最小者），失败返回 None 软降级。
- `check_campus_status(app_handle: AppHandle)` (行82-105): 检查校园网状态（SSID/有线等），返回逐卡判定明细。
- `check_portal_status(adapter_ip: String, app_handle: AppHandle)` (行107-137): 检查 Portal 状态，含注销保护期判断（行118-125）。
- `dhcp_renew_all(app_handle: AppHandle)` (行149-166): 仅续租主/副适配器中有线者。
- `dhcp_release_renew(app_handle: AppHandle)` (行168-189): MAC 重置 + 释放/续租主/副适配器。
- `dhcp_release_renew_adapter(adapter_name: String, app_handle: AppHandle)` (行191-207): 单适配器 DHCP 释放/续租。
- `check_network_quality(app_handle: AppHandle)` (行209-239): 网络质量检测，手动检测同样落盘历史。
- `start_latency_test(app_handle: AppHandle, _state: State<'_, AppState>)` (行241-267): 启动延迟测试，校验总开关并持久化 `enable_latency_test`。
- `stop_latency_test(app_handle: AppHandle, state: State<'_, AppState>)` (行269-280): 停止延迟测试，取消任务并持久化开关。
- `check_dns_doh_status()` (行282-295): 读取注册表检测 DNS/DoH 状态。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于行290-293 直接返回 `{"adapters": [], "dohSupported": false}`。
- `setup_dns_doh(app_handle: tauri::AppHandle, family: Option<String>)` (行297-383): 一键设置 DNS+DoH，依据 `config.dns_optimize_adapters` 名单提权修改。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于行307-311 返回 `{"success": false, "message": "仅支持Windows"}`。
- `reset_dns(app_handle: tauri::AppHandle)` (行385-476): 恢复 DNS 自动获取，操作范围同 `setup_dns_doh`。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于行389-392 返回 `CommandResult::err("仅支持Windows")`。

### Frontend: tauri-app/frontend/src/network/useOutboundManual.ts

- `useOutboundManual(adaptersCount: number)` (行14-96): 当前出站查询/轮询与手动切换/还原。
  - `mountedRef` (行17): StrictMode 保护。
  - `currentOutbound` / `switchingOutbound` / `restoringOutbound` 状态 (行21-23)。
  - StrictMode effect (行25-30): 恢复 `mountedRef`。
  - `refreshCurrentOutbound` (行32-39): 调用 `ipc.getCurrentOutboundName?.()` 更新状态。
  - 挂载/数量变化 effect (行41-43) 与 60s 轮询 effect (行44-47)。
  - `handleOutboundSwitchNow` (行50-70): 调用 `ipc.outboundSwitchNow?.()`，完成后刷新适配器与出站。
  - `handleOutboundRestoreNow` (行73-93): 调用 `ipc.outboundRestoreNow?.()`，完成后刷新。

### Frontend: tauri-app/frontend/src/network/useDnsSetup.ts

- `ALI_DNS` (行11), `TENCENT_DNS` (行12), `RECOMMENDED_DNS` (行13): DNS 推荐地址集合。
- `DnsFamily` 类型 (行15): `'both' | 'ipv4' | 'ipv6'`。
- `useDnsSetup(dnsOptimizeAdaptersCount: number)` (行23-124): DNS 检测/优化/恢复逻辑。
  - `mountedRef` (行26), `dohEnabling`/`dnsResetting`/`dnsFamily` 状态 (行27-30)。
  - StrictMode effect (行32-37)。
  - `handleCheckDns` (行39-61): 调用 `ipc.checkDnsDohStatus()` 并更新 store。
  - `handleSetupDnsDoh` (行63-91): 调用 `ipc.setupDnsDoh(dnsFamily)`，成功后刷新状态。
  - `handleResetDns` (行93-121): 调用 `ipc.resetDns()`，成功后刷新状态。

### Frontend: tauri-app/frontend/src/network/outboundOrder.ts

- `buildOutboundOrder(priority: string[] | undefined, detected: string[], wirelessNames?: string[]): string[]` (行11-30): 构建出站顺序，priority 非空时过滤保留并追加新卡；空时无线优先或退化发现顺序。

## Architecture

```text
┌─────────────────────────────────────────────────────────┐
│ NetworkPanel (Frontend) - NetworkPanel.tsx              │
│ ┌──────────────────┐  ┌──────────────────────────────┐ │
│ │ Adapter List     │  │ DNS Optimization (useDnsSetup)│ │
│ │ syncOnRestore    │  │ dnsAdapterCandidates         │ │
│ └────────┬─────────┘  └──────────────┬───────────────┘ │
│          │ IPC (tauriApiWithRetry)                  │
└──────────┼────────────────────────────────────────────┘
           ▼
┌─────────────────────────────────────────────────────────┐
│ Tauri Commands (Rust) - network_cmd.rs                   │
│ get_adapters/enable_adapter/outbound_*/check_*/setup_dns│
└──────────┬────────────────────────────────────────────────┘
           ▼
┌─────────────────────────────────────────────────────────┐
│ System Network API (Registry/Routing/DHCP)              │
└─────────────────────────────────────────────────────────┘
```

## Data Flow

1. **适配器列表同步**:
   - 后端 `get_adapters` (行19-25) 定期轮询或事件推送所有适配器（含 `disabled`）。
   - 前端 `NetworkPanel` 订阅 `useAdapterStore`，在挂载 (行323-325)、恢复可见性 `syncOnRestore` (行301-307 及绑定 effect 308-318) 时调用 `refreshAdapterData`。
   - `effectiveDisabledAdapters` (行224-227) 直接从 `adapters` 过滤生成，确保 UI 一致性。

2. **出站切换**:
   - 用户点击 "立即切换" -> `useOutboundManual` 的 `handleOutboundSwitchNow` (行50-70) -> `ipc.outboundSwitchNow` -> 后端 `outbound_switch_now` (行44-53) -> `manual_outbound_switch`。
   - 后端修改路由 metric 后，前端通过 `refreshAdapterData({force:true})` 和 `refreshCurrentOutbound()` 刷新 UI（在 `syncOnRestore` 行305-306 强制触发，解决 WebView2 最小化节流问题）。
   - 恢复时刻可配置：用户修改 `nightRestoreMinutes` (行469-483) 经 `onUpdateConfig` 落盘。

3. **DNS 优化**:
   - 用户选择目标适配器 -> `toggleDnsAdapter` (行359-362) 更新 `config.dnsOptimizeAdapters`。
   - 点击 "一键优化" -> `useDnsSetup` 的 `handleSetupDnsDoh` -> `ipc.setupDnsDoh(family)` -> 后端 `setup_dns_doh` (行297-383)。
   - 后端依据 `dns_optimize_adapters` 名单 (行318-323) 及 `family` 参数，通过管理员权限或 Helper 进程修改注册表 DNS 设置。

## Connections

- [[learnings/webview-minimize-timer-throttle-stale-panel]]: 本文自身，记录 WebView2 最小化导致状态陈旧的根因及 `syncOnRestore` 解决方案（见 `NetworkPanel.tsx` 行297-318）。
- [[settings/config-store]]: `NetworkPanel` 通过 `useConfigStore` 读写 `adapter1`, `adapter2`, `dnsOptimizeAdapters`, `outboundPriority`, `enableNightOutboundSwitch`, `nightRestoreMinutes` 等配置（行230-231, 243-247, 359-362, 449-454, 469-483）。
- [[monitor/scheduled]]: 包含 `manual_outbound_switch` 和 `manual_outbound_restore` 的后端实现逻辑（`network_cmd.rs` 行48, 60 调用）。

## Known Issues

- **非 Windows 平台 DNS 功能受限**：在 `network_cmd.rs` 中，DNS 相关命令 `check_dns_doh_status` (行290-293)、`setup_dns_doh` (行307-311)、`reset_dns` (行389-392) 均通过 `#[cfg(not(target_os = "windows"))]` 宏在非 Windows 平台直接短路返回不支持错误，仅 Windows 平台具备实际注册表操作逻辑。
- **旧版双通道废弃**：原后端独立禁用适配器列表接口已移除，禁用适配器列表现由前端基于 `get_adapters` 全量返回过滤派生（`NetworkPanel.tsx` 行224-227），如后端行为回退需排查适配器推送通道。
- **恢复时刻钳制**：前端将 `nightRestoreMinutes` 钳制在 `[300, 1379]` 分钟（行477-480），后端另有归一化兜底；若后端返回值越界，显示层按默认 450 展示（行63-69）。
