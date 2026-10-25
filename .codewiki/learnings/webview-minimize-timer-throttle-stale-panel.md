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

该模块解决了 WebView2 最小化期间定时器节流导致的状态陈旧问题，通过 `visibilitychange` 和 `focus` 事件在窗口恢复时强制同步后端状态（见 `NetworkPanel.tsx` 行300-318：`syncOnRestore` 函数定义于行301-307，事件绑定 effect 于行308-318）。它同时处理了适配器拖拽排序、DHCP 续租、禁用/启用适配器等复杂交互。当前实现中，后端已不再提供独立的禁用适配器列表接口（旧版 `get_disabled_adapters` 已移除），所有适配器（含 `disabled` 状态）由 `get_adapters` 统一推送，前端在行224-227 通过 `effectiveDisabledAdapters` 派生过滤。

恢复时刻（`nightRestoreMinutes`）现可配置（行457-486），钳制在 `[05:00, 22:59]`（常量 `RESTORE_TIME_MIN_MINUTES` 行60、`RESTORE_TIME_MAX_MINUTES` 行56），与后端归一化兜底同口径。历史版本的挂起出站切换态（`outboundPendingSwitch`）已移除，后端每拍清理（见行489-490 注释）。

## Key Components

### Frontend: NetworkPanel.tsx

**常量:**
- `FOLLOW_CURRENT_ACCOUNT = '__follow__'` (行41): 适配器账号选择的哨兵值，用于标识跟随当前登录账号。
- `NO_BACKUP_ADAPTER = '__none__'` (行42): 无备用适配器哨兵值。
- `RESTORE_TIME_DEFAULT_MINUTES = 450` (行53): 恢复时刻默认值 07:30，与后端 `DEFAULT_RESTORE_START_MINUTES` 同值。
- `RESTORE_TIME_MAX_MINUTES = 1379` (行56): 恢复时刻上限 22:59，必须早于最早切换时刻 23:00。
- `RESTORE_TIME_MIN_MINUTES = 300` (行60): 恢复时刻下限 05:00，产品口径防止与凌晨补切窗口冲突。
- `DRAG_LONG_PRESS_MS = 250` (行72): 长按起拖时长 250ms。
- `DRAG_DEAD_ZONE_PX = 8` (行74): 长按等待期内允许的位移死区 8px。

**函数:**
- `formatSpeed(bps?: number): string` (行45-49): 连接速度格式化，bit/s → 统一 Mbps/Kbps 显示，未知返回空串。
- `formatRestoreTime(minutes: number | undefined): string` (行63-69): 分钟数 → "HH:MM"；0/越界按默认 07:30 显示。

**组件:**
- `SortableAdapterRowProps` 接口 (行76-88): 可拖拽行 props 定义，含 `adapter`, `isOutboundTarget`, `isCurrentOutbound`, `roleLabel`, `onDragEndCommit`, `onDragStart`, `children`。
- `SortableAdapterRow` (memo, 行95-215): 可拖拽适配器行，实现长按 250ms 起拖（移动超 8px 死区取消，防滚动/点击误触）。行体按下布置定时器（行120-129），按钮区 `stopPropagation` 防止拖走整行（行209）。
- `NetworkPanelProps` 接口 (行36-39): 主面板 props：`adapters: Adapter[]`, `onUpdateConfig: (partial: Partial<Config>) => void`。
- `NetworkPanel` (memo, 行217-822): 主面板组件。

**核心状态与逻辑（NetworkPanel 组件内）:**
- `effectiveDisabledAdapters` (行224-227): 从 `adapters` 过滤 `status === 'disabled'` 派生，避免旧版双通道不一致。
- `config` (行230): 自订阅 `useConfigStore` 浅比较，使 App 外壳不再级联重渲染。
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
- `useOutboundManual` (行295): 提供 `currentOutbound` 及 `handleOutboundSwitchNow`/`handleOutboundRestoreNow` 等（逻辑外移 `useOutboundManual.ts`）。
- `lastFocusSyncRef` (行300) 与 `syncOnRestore` (行301-307): 恢复可见性时强制 `refreshAdapterData({force:true})` 和 `refreshCurrentOutbound()`，5s 节流。
- 可见性/焦点 effect (行308-318): 绑定 `visibilitychange` 与 `focus` 到 `syncOnRestore`。
- 挂载对账 effect (行323-325): 挂载时 `refreshAdapterData()` 对齐 store。
- StrictMode mountedRef effect (行327-332): 二次 setup 恢复 `mountedRef`。
- `dnsStatus` / `dnsChecking` (行334-335): 从 `useQualityStore` 取 DNS 状态。
- `refreshAdapters` / `isRefreshingAdapters` (行336-337): 适配器 store 刷新动作。
- `useDnsSetup` (行339): 提供 `handleCheckDns`/`handleSetupDnsDoh`/`handleResetDns` 等（外移 `useDnsSetup.ts`）。
- `dnsAdapterCount` (行341): DNS 结果卡数量。
- `familyTabs` (行342-346): IPv4/IPv6/both 切换标签。
- `dnsAdapterCandidates` (行349-358): DNS 优化目标适配器候选（已选 ∪ 当前有 IP 活跃卡）。
- `toggleDnsAdapter` (行359-362): 切换 DNS 优化目标适配器至 `config.dnsOptimizeAdapters`。
- `handleGetNewIpForAdapter` (行364-379): 单适配器 DHCP 释放/续租（`ipc.dhcpReleaseRenewAdapter`），结果归一化提示并刷新。
- `handleEnableAdapter` (行381-399): 启用禁用适配器（`ipc.enableAdapter`），强制刷新适配器列表。

### Backend: network_cmd.rs

**Tauri 命令 (pub async fn / pub fn):**
- `get_adapters(force: Option<bool>)` (行19-25): 获取适配器列表（缓存或强制刷新），统一推送所有卡（含禁用）。
- `enable_adapter(adapter_name: String)` (行27-36): 启用指定适配器，允许 COM 提权失败降级弹 UAC。
- `get_adapter_details()` (行38-41): 获取适配器详情。
- `outbound_switch_now(app_handle)` (行44-53): 手动立即切换出站，调用 `manual_outbound_switch`。
- `outbound_restore_now(app_handle)` (行56-65): 手动立即还原出站，调用 `manual_outbound_restore`。
- `get_current_outbound_name()` (行69-80): 获取当前真实出站接口别名（默认路由 metric 求和最小者），失败返回 None 软降级。
- `check_campus_status(app_handle)` (行82-102): 检查校园网状态（SSID/有线等）。
- `check_portal_status(adapter_ip, app_handle)` (行104-134): 检查 Portal 状态，含注销保护期判断（行115-122）。
- `dhcp_renew_all(app_handle)` (行146-163): 仅续租主/副适配器中有线者。
- `dhcp_release_renew(app_handle)` (行165-186): MAC 重置 + 释放/续租主/副适配器。
- `dhcp_release_renew_adapter(adapter_name, app_handle)` (行188-204): 单适配器 DHCP 释放/续租。
- `check_network_quality(app_handle)` (行206-236): 网络质量检测，手动检测同样落盘历史。
- `start_latency_test(app_handle)` (行238-264): 启动延迟测试，校验总开关并持久化 `enable_latency_test`。
- `stop_latency_test(app_handle)` (行266-277): 停止延迟测试，取消任务并持久化开关。
- `check_dns_doh_status()` (行279-292): 读取注册表检测 DNS/DoH 状态。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于行287-290 直接返回 `{"adapters": [], "dohSupported": false}`。
- `setup_dns_doh(app_handle, family)` (行294-380): 一键设置 DNS+DoH，依据 `config.dns_optimize_adapters` 名单提权修改。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于行304-308 返回 `{"success": false, "message": "仅支持Windows"}`。
- `reset_dns(app_handle)` (行382-472): 恢复 DNS 自动获取，操作范围同 `setup_dns_doh`。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于行386-390 返回 `CommandResult::err("仅支持Windows")`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                  NetworkPanel (Frontend)                     │
│  tauri-app/frontend/src/network/NetworkPanel.tsx            │
│  ┌─────────────────────┐  ┌──────────────────────────────┐ │
│  │ Adapter List        │  │ DNS Optimization             │ │
│  │ (Reorder.Group)     │  │ (useDnsSetup)                │ │
│  │ • SortableAdapterRow│  │ • familyTabs                 │ │
│  │ • syncOnRestore     │  │ • dnsAdapterCandidates       │ │
│  └─────────┬───────────┘  └──────────────┬───────────────┘ │
│            │ IPC (tauriApiWithRetry)                     │
└────────────┼──────────────────────────────────────────────┘
             ▼
┌─────────────────────────────────────────────────────────────┐
│              Tauri Commands (Rust)                           │
│  network_cmd.rs                                             │
│  • get_adapters / enable_adapter                            │
│  • outbound_switch_now / outbound_restore_now              │
│  • check_dns_doh_status / setup_dns_doh / reset_dns        │
└────────────┬────────────────────────────────────────────────┘
             ▼
┌─────────────────────────────────────────────────────────────┐
│                  System Network API                         │
│  • Windows Registry (DNS Config)                            │
│  • Routing Table (Best Route)                               │
│  • DHCP Client                                              │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **适配器列表同步**:
   - 后端 `get_adapters` (行19-25) 定期轮询或通过事件推送所有适配器（含 `disabled`）。
   - 前端 `NetworkPanel` 订阅 `useAdapterStore`，在挂载 (行323-325)、恢复可见性 `syncOnRestore` (行301-307 及绑定 effect 308-318) 时调用 `refreshAdapterData`。
   - `effectiveDisabledAdapters` (行224-227) 直接从 `adapters` 过滤生成，确保 UI 一致性（旧版独立拉取通道已废弃）。

2. **出站切换**:
   - 用户点击 "立即切换" -> `useOutboundManual` 的 `handleOutboundSwitchNow` (行295 引用) -> `ipc.outboundSwitchNow` -> 后端 `outbound_switch_now` (行44-53) -> `manual_outbound_switch`。
   - 后端修改路由 metric 后，前端通过 `refreshAdapterData({force:true})` 和 `refreshCurrentOutbound()` 刷新 UI（在 `syncOnRestore` 行305-306 强制触发，解决 WebView2 最小化节流问题）。
   - 恢复时刻可配置：用户修改 `nightRestoreMinutes` (行469-483) 经 `onUpdateConfig` 落盘。

3. **DNS 优化**:
   - 用户选择目标适配器 -> `toggleDnsAdapter` (行359-362) 更新 `config.dnsOptimizeAdapters`。
   - 点击 "一键优化" -> `useDnsSetup` 的 `handleSetupDnsDoh` -> `ipc.setupDnsDoh(family)` -> 后端 `setup_dns_doh` (行294-380)。
   - 后端依据 `dns_optimize_adapters` 名单 (行318-323) 及 `family` 参数，通过管理员权限或 Helper 进程修改注册表 DNS 设置。

## Connections

- [[learnings/webview-minimize-timer-throttle-stale-panel]]: 本文自身，记录 WebView2 最小化导致状态陈旧的根因及 `syncOnRestore` 解决方案（见 `NetworkPanel.tsx` 行300-318）。
- [[settings/config-store]]: `NetworkPanel` 通过 `useConfigStore` 读写 `adapter1`, `adapter2`, `dnsOptimizeAdapters`, `outboundPriority`, `enableNightOutboundSwitch`, `nightRestoreMinutes` 等配置（行230-231, 243-247, 359-362, 449-454, 469-483）。
- [[monitor/scheduled]]: 包含 `manual_outbound_switch` 和 `manual_outbound_restore` 的后端实现逻辑（`network_cmd.rs` 行48, 60 调用）。

## Known Issues

- **非 Windows 平台 DNS 功能受限**：在 `tauri-app/src-tauri/src/commands/network_cmd.rs` 中，DNS 相关命令 `check_dns_doh_status` (行287-290)、`setup_dns_doh` (行304-308)、`reset_dns` (行386-390) 均通过 `#[cfg(not(target_os = "windows"))]` 宏在非 Windows 平台直接短路返回不支持错误，仅 Windows 平台具备实际注册表操作逻辑。
- **旧版双通道废弃**：原后端 `get_disabled_adapters` 命令已移除，禁用适配器列表现由前端基于 `get_adapters` 全量返回过滤派生（`NetworkPanel.tsx` 行224-227），如后端行为回退需排查适配器推送通道。
- **恢复时刻钳制**：前端将 `nightRestoreMinutes` 钳制在 `[300, 1379]` 分钟（行477-480），后端另有归一化兜底；若后端返回值越界，显示层按默认 450 展示（行63-69）。
