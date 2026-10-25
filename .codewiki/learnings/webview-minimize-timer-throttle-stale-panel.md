---
title: NetworkPanel 网络适配器与出站管理面板
type: module
source_files:
  - tauri-app/frontend/src/network/NetworkPanel.tsx
  - tauri-app/src-tauri/src/commands/network_cmd.rs
tags: [前端, Tauri, 网络, DNS, 出站切换, WebView2]
---

## Overview

`NetworkPanel` 是应用的核心网络配置界面，负责展示系统网络适配器列表、管理主/副适配器角色、执行夜间自动出站切换的手动干预（立即切换/还原）、以及提供 DNS/DoH 的一键优化与检测功能。

该模块解决了 WebView2 最小化期间定时器节流导致的状态陈旧问题，通过 `visibilitychange` 和 `focus` 事件在窗口恢复时强制同步后端状态（见 `NetworkPanel.tsx` 286-307 行）。它同时处理了适配器拖拽排序、DHCP 续租、禁用/启用适配器等复杂交互。当前实现中，后端已不再提供独立的禁用适配器列表接口（旧版 `get_disabled_adapters` 已移除），所有适配器（含 `disabled` 状态）由 `get_adapters` 统一推送，前端在 213-216 行通过 `effectiveDisabledAdapters` 派生过滤。

## Key Components

### Frontend: NetworkPanel.tsx

**常量:**
- `FOLLOW_CURRENT_ACCOUNT = '__follow__'` (`tauri-app/frontend/src/network/NetworkPanel.tsx:39`): 适配器账号选择的哨兵值，用于标识跟随当前登录账号。
- `NO_BACKUP_ADAPTER = '__none__'` (`tauri-app/frontend/src/network/NetworkPanel.tsx:40`): 无备用适配器哨兵值。
- `DRAG_LONG_PRESS_MS = 250` (`tauri-app/frontend/src/network/NetworkPanel.tsx:61`): 长按起拖时长 250ms。
- `DRAG_DEAD_ZONE_PX = 8` (`tauri-app/frontend/src/network/NetworkPanel.tsx:63`): 长按等待期内允许的位移死区 8px。

**函数:**
- `formatSpeed(bps?: number): string` (`tauri-app/frontend/src/network/NetworkPanel.tsx:43-47`): 连接速度格式化，bit/s → 统一 Mbps/Kbps 显示，未知返回空串。
- `parsePendingSwitch(json: string | undefined)` (`tauri-app/frontend/src/network/NetworkPanel.tsx:50-58`): 解析待切换状态 JSON（{target, tag}），空/非法返回 null。

**组件:**
- `SortableAdapterRow` (`tauri-app/frontend/src/network/NetworkPanel.tsx:84-204`): 可拖拽适配器行，实现长按 250ms 起拖（移动超 8px 死区取消，防滚动/点击误触）。Props: `adapter`, `isOutboundTarget`, `isCurrentOutbound`, `roleLabel`, `onDragEndCommit`, `onDragStart`, `children`。
- `NetworkPanel` (`tauri-app/frontend/src/network/NetworkPanel.tsx:206-796`): 主面板组件。Props: `adapters: Adapter[]`, `onUpdateConfig: (partial: Partial<Config>) => void`。

**核心状态与逻辑（NetworkPanel 组件内）:**
- `effectiveDisabledAdapters` (`tauri-app/frontend/src/network/NetworkPanel.tsx:213-216`): 从 `adapters` 过滤 `status === 'disabled'` 派生，避免旧版双通道不一致。
- `outboundOrder` (`tauri-app/frontend/src/network/NetworkPanel.tsx:232-237`): 基于 `config.outboundPriority` 与当前 `adapters` 构建完整出站顺序。
- `dragOrder` / `isDraggingRef` (`tauri-app/frontend/src/network/NetworkPanel.tsx:238-249`): 拖拽期间本地顺序渲染，非拖拽时同步外部。
- `handleDragStart` (`tauri-app/frontend/src/network/NetworkPanel.tsx:255-257`): 标记拖拽中，屏蔽外部顺序同步。
- `handleReorder` (`tauri-app/frontend/src/network/NetworkPanel.tsx:260-264`): 拖拽每帧回调，仅写本地态。
- `handleDragEnd` (`tauri-app/frontend/src/network/NetworkPanel.tsx:268-276`): 拖拽结束提交 `outboundPriority` 并解除本地态。
- `syncOnRestore` (`tauri-app/frontend/src/network/NetworkPanel.tsx:289-307`): 恢复可见性时强制 `refreshAdapterData({force:true})` 和 `refreshCurrentOutbound()`，5s 节流（`lastFocusSyncRef` 于 289 行）。
- `dnsAdapterCandidates` (`tauri-app/frontend/src/network/NetworkPanel.tsx:338-347`): DNS 优化目标适配器候选（已选 ∪ 当前有 IP 活跃卡）。
- `toggleDnsAdapter` (`tauri-app/frontend/src/network/NetworkPanel.tsx:348-351`): 切换 DNS 优化目标适配器至 `config.dnsOptimizeAdapters`。
- `handleGetNewIpForAdapter` (`tauri-app/frontend/src/network/NetworkPanel.tsx:353-368`): 单适配器 DHCP 释放/续租（`ipc.dhcpReleaseRenewAdapter`），结果归一化提示并刷新。
- `handleEnableAdapter` (`tauri-app/frontend/src/network/NetworkPanel.tsx:370-388`): 启用禁用适配器（`ipc.enableAdapter`），强制刷新适配器列表。
- `useOutboundManual` (`tauri-app/frontend/src/network/NetworkPanel.tsx:284`): 提供当前出站 `currentOutbound` 及切换/还原逻辑 `handleOutboundSwitchNow`/`handleOutboundRestoreNow`（已外移至 `useOutboundManual.ts`）。
- `useDnsSetup` (`tauri-app/frontend/src/network/NetworkPanel.tsx:328`): 提供 DNS 检测/优化/恢复 `handleCheckDns`/`handleSetupDnsDoh`/`handleResetDns`（已外移至 `useDnsSetup.ts`）。

### Backend: network_cmd.rs

**Tauri 命令 (pub async fn / pub fn):**
- `get_adapters(force: Option<bool>)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:19-25`): 获取适配器列表（缓存或强制刷新），当前统一推送所有卡（含禁用）。
- `enable_adapter(adapter_name: String)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:27-36`): 启用指定适配器，允许 COM 提权失败降级弹 UAC。
- `get_adapter_details()` (`tauri-app/src-tauri/src/commands/network_cmd.rs:38-41`): 获取适配器详情。
- `outbound_switch_now(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:45-53`): 手动立即切换出站，调用 `manual_outbound_switch`。
- `outbound_restore_now(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:57-65`): 手动立即还原出站，调用 `manual_outbound_restore`。
- `get_current_outbound_name()` (`tauri-app/src-tauri/src/commands/network_cmd.rs:70-80`): 获取当前真实出站接口别名（默认路由 metric 求和最小者），失败返回 None 软降级。
- `check_campus_status(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:83-102`): 检查校园网状态（SSID/有线等）。
- `check_portal_status(adapter_ip, app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:106-134`): 检查 Portal 状态，含注销保护期判断（115-122行）。
- `dhcp_renew_all(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:147-163`): 仅续租主/副适配器中有线者。
- `dhcp_release_renew(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:166-186`): MAC 重置 + 释放/续租主/副适配器。
- `dhcp_release_renew_adapter(adapter_name, app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:189-204`): 单适配器 DHCP 释放/续租。
- `check_network_quality(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:207-236`): 网络质量检测，手动检测同样落盘历史。
- `start_latency_test(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:239-264`): 启动延迟测试，校验总开关并持久化 `enable_latency_test`。
- `stop_latency_test(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:267-277`): 停止延迟测试，取消任务并持久化开关。
- `check_dns_doh_status()` (`tauri-app/src-tauri/src/commands/network_cmd.rs:280-292`): 读取注册表检测 DNS/DoH 状态。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于 287-290 行直接返回 `{"adapters": [], "dohSupported": false}`。
- `setup_dns_doh(app_handle, family)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:295-379`): 一键设置 DNS+DoH，依据 `config.dns_optimize_adapters` 名单提权修改。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于 304-308 行返回 `{"success": false, "message": "仅支持Windows"}`。
- `reset_dns(app_handle)` (`tauri-app/src-tauri/src/commands/network_cmd.rs:383-472`): 恢复 DNS 自动获取，操作范围同 `setup_dns_doh`。非 Windows 平台通过 `#[cfg(not(target_os = "windows"))]` 于 386-390 行返回 `CommandResult::err("仅支持Windows")`。

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
   - 后端 `get_adapters` (`network_cmd.rs:211-217`) 定期轮询或通过事件推送所有适配器（含 `disabled`）。
   - 前端 `NetworkPanel` 订阅 `useAdapterStore`，在挂载 (`NetworkPanel.tsx:206-208`)、恢复可见性 `syncOnRestore` (`NetworkPanel.tsx:206-224`) 时调用 `refreshAdapterData`。
   - `effectiveDisabledAdapters` (`NetworkPanel.tsx:213-216`) 直接从 `adapters` 过滤生成，确保 UI 一致性（旧版独立拉取通道已废弃）。

2. **出站切换**:
   - 用户点击 "立即切换" -> `useOutboundManual` 的 `handleOutboundSwitchNow` (`NetworkPanel.tsx:284`) -> `ipc.outboundSwitchNow` -> 后端 `outbound_switch_now` (`network_cmd.rs:45-53`) -> `manual_outbound_switch`。
   - 后端修改路由 metric 后，前端通过 `refreshAdapterData({force:true})` 和 `refreshCurrentOutbound()` 刷新 UI。
   - 恢复可见性时，`syncOnRestore` 强制触发上述刷新，解决 WebView2 最小化节流问题。

3. **DNS 优化**:
   - 用户选择目标适配器 -> `toggleDnsAdapter` (`NetworkPanel.tsx:348-351`) 更新 `config.dnsOptimizeAdapters`。
   - 点击 "一键优化" -> `useDnsSetup` 的 `handleSetupDnsDoh` -> `ipc.setupDnsDoh(family)` -> 后端 `setup_dns_doh` (`network_cmd.rs:295-379`)。
   - 后端依据 `dns_optimize_adapters` 名单 (`network_cmd.rs:297-302`) 及 `family` 参数，通过管理员权限或 Helper 进程修改注册表 DNS 设置。

## Connections

- [[learnings/webview-minimize-timer-throttle-stale-panel]]: 核心关联文章，记录 WebView2 最小化导致状态陈旧的根因及 `syncOnRestore` 解决方案（见 `NetworkPanel.tsx:290-308`）。
- [[settings/config-store]]: `NetworkPanel` 通过 `useConfigStore` 读写 `adapter1`, `adapter2`, `dnsOptimizeAdapters`, `outboundPriority`, `enableNightOutboundSwitch` 等配置（219-220行, 232-236行, 348-351行）。
- [[monitor/scheduled]]: 包含 `manual_outbound_switch` 和 `manual_outbound_restore` 的后端实现逻辑（`network_cmd.rs` 48行, 60行调用）。

## Known Issues

- **非 Windows 平台 DNS 功能受限**：在 `tauri-app/src-tauri/src/commands/network_cmd.rs` 中，DNS 相关命令 `check_dns_doh_status` (287-290行)、`setup_dns_doh` (304-308行)、`reset_dns` (386-390行) 均通过 `#[cfg(not(target_os = "windows"))]` 宏在非 Windows 平台直接短路返回不支持错误，仅 Windows 平台具备实际注册表操作逻辑。
- **旧版双通道废弃**：原后端 `get_disabled_adapters` 命令已移除，禁用适配器列表现由前端基于 `get_adapters` 全量返回过滤派生（`NetworkPanel.tsx:213-216`），如后端行为回退需排查适配器推送通道。
