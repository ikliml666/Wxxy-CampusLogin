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

该模块解决了 WebView2 最小化期间定时器节流导致的状态陈旧问题，通过 `visibilitychange` 和 `focus` 事件在窗口恢复时强制同步后端状态。它同时处理了适配器拖拽排序、DHCP 续租、禁用/启用适配器等复杂交互。

## Key Components

### Frontend: NetworkPanel.tsx

**常量:**
- `ALI_DNS`: 阿里云 DNS IP 集合 (`tauri-app/frontend/src/network/NetworkPanel.tsx:36`)
- `TENCENT_DNS`: 腾讯云 DNS IP 集合 (`tauri-app/frontend/src/network/NetworkPanel.tsx:37`)
- `RECOMMENDED_DNS`: 推荐 DNS 合集 (`tauri-app/frontend/src/network/NetworkPanel.tsx:38`)
- `FOLLOW_CURRENT_ACCOUNT`: 适配器账号选择的哨兵值 `'__follow__'` (`tauri-app/frontend/src/network/NetworkPanel.tsx:41`)
- `DRAG_LONG_PRESS_MS`: 长按起拖时长 250ms (`tauri-app/frontend/src/network/NetworkPanel.tsx:55`)
- `DRAG_DEAD_ZONE_PX`: 长按死区 8px (`tauri-app/frontend/src/network/NetworkPanel.tsx:58`)

**函数:**
- `formatSpeed(bps)`: 格式化连接速度为 Mbps/Kbps (`tauri-app/frontend/src/network/NetworkPanel.tsx:44`)
- `getDnsQuality(adapter, autoDohEnabled)`: 计算单个适配器的 DNS 质量等级 (`tauri-app/frontend/src/network/NetworkPanel.tsx:495`)

**组件:**
- `SortableAdapterRow`: 可拖拽的适配器行组件，实现长按起拖逻辑 (`tauri-app/frontend/src/network/NetworkPanel.tsx:74`)
  - Props: `adapter`, `isOutboundTarget`, `isCurrentOutbound`, `roleLabel`, `onDragEndCommit`, `onDragStart`, `children`
- `NetworkPanel`: 主面板组件 (`tauri-app/frontend/src/network/NetworkPanel.tsx:135`)
  - Props: `adapters: Adapter[]`, `onUpdateConfig: (partial: Partial<Config>) => void`

**核心状态与逻辑:**
- `effectiveDisabledAdapters`: 从 `adapters` 派生的禁用列表，避免双通道不一致 (`tauri-app/frontend/src/network/NetworkPanel.tsx:147`)
- `outboundOrder` / `dragOrder`: 出站优先级排序，支持拖拽重排 (`tauri-app/frontend/src/network/NetworkPanel.tsx:170-207`)
- `currentOutbound`: 当前真实出站接口名称，通过 `getCurrentOutboundName` 获取 (`tauri-app/frontend/src/network/NetworkPanel.tsx:215-224`)
- `syncOnRestore`: 恢复可见性时的状态同步逻辑，含 5s 节流 (`tauri-app/frontend/src/network/NetworkPanel.tsx:240-246`)
- `handleCheckDns`: 检测 DNS/DoH 状态 (`tauri-app/frontend/src/network/NetworkPanel.tsx:260`)
- `handleSetupDnsDoh`: 一键设置 DNS+DoH (`tauri-app/frontend/src/network/NetworkPanel.tsx:278`)
- `handleResetDns`: 重置 DNS 为自动获取 (`tauri-app/frontend/src/network/NetworkPanel.tsx:308`)
- `handleGetNewIpForAdapter`: DHCP 释放/续租 (`tauri-app/frontend/src/network/NetworkPanel.tsx:338`)
- `handleEnableAdapter`: 启用禁用适配器 (`tauri-app/frontend/src/network/NetworkPanel.tsx:354`)
- `handleOutboundSwitchNow`: 手动触发立即切换出站 (`tauri-app/frontend/src/network/NetworkPanel.tsx:375`)
- `handleOutboundRestoreNow`: 手动触发立即还原出站 (`tauri-app/frontend/src/network/NetworkPanel.tsx:396`)

### Backend: network_cmd.rs

**命令:**
- `get_adapters(force)`: 获取适配器列表 (`tauri-app/src-tauri/src/commands/network_cmd.rs:18`)
- `get_disabled_adapters()`: 获取禁用适配器列表 (`tauri-app/src-tauri/src/commands/network_cmd.rs:26`)
- `enable_adapter(adapter_name)`: 启用指定适配器 (`tauri-app/src-tauri/src/commands/network_cmd.rs:31`)
- `get_adapter_details()`: 获取适配器详情 (`tauri-app/src-tauri/src/commands/network_cmd.rs:41`)
- `outbound_switch_now(app_handle)`: 手动立即切换出站 (`tauri-app/src-tauri/src/commands/network_cmd.rs:47`)
- `outbound_restore_now(app_handle)`: 手动立即还原出站 (`tauri-app/src-tauri/src/commands/network_cmd.rs:58`)
- `get_current_outbound_name()`: 获取当前真实出站接口名 (`tauri-app/src-tauri/src/commands/network_cmd.rs:69`)
- `dhcp_release_renew_adapter(adapter_name, app_handle)`: 单适配器 DHCP 续租 (`tauri-app/src-tauri/src/commands/network_cmd.rs:135`)
- `check_dns_doh_status()`: 检测 DNS/DoH 状态 (`tauri-app/src-tauri/src/commands/network_cmd.rs:178`)
- `setup_dns_doh(app_handle, family)`: 一键设置 DNS+DoH (`tauri-app/src-tauri/src/commands/network_cmd.rs:186`)
- `reset_dns(app_handle)`: 重置 DNS 自动获取 (`tauri-app/src-tauri/src/commands/network_cmd.rs:233`)

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      NetworkPanel (Frontend)                │
│  ┌─────────────────────┐  ┌──────────────────────────────┐ │
│  │  Adapter List       │  │      DNS Optimization        │ │
│  │  (Reorder.Group)    │  │  ┌──────────┐ ┌──────────┐  │ │
│  │  • SortableRow      │  │  │ Check DNS│ │ Optimize │  │ │
│  │  • Outbound Badge   │  │  └──────────┘ └──────────┘  │ │
│  │  • Actions (IP/En.) │  │  • Family Tabs               │ │
│  └─────────┬───────────┘  │  • Adapter Chips               │ │
│            │              └──────────────────────────────┘ │
│            │ IPC Calls                                     │
└────────────┼───────────────────────────────────────────────┘
             ▼
┌─────────────────────────────────────────────────────────────┐
│                   Tauri Commands (Rust)                     │
│  • get_adapters / enable_adapter                            │
│  • outbound_switch_now / outbound_restore_now               │
│  • check_dns_doh_status / setup_dns_doh / reset_dns         │
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
   - 后端 `get_adapters` 定期轮询或通过 `onAdaptersChanged` 事件推送。
   - 前端 `NetworkPanel` 订阅 `useAdapterStore`，在挂载、列表变化、或恢复可见性时调用 `refreshAdapterData`。
   - `effectiveDisabledAdapters` 直接从 `adapters` 过滤生成，确保 UI 一致性。

2. **出站切换**:
   - 用户点击 "立即切换" -> `handleOutboundSwitchNow` -> `ipc.outboundSwitchNow` -> 后端 `manual_outbound_switch`。
   - 后端修改路由 metric 后，前端通过 `refreshAdapterData({force:true})` 和 `refreshCurrentOutbound()` 刷新 UI。
   - 恢复可见性时，`syncOnRestore` 强制触发上述刷新，解决 WebView2 最小化节流问题。

3. **DNS 优化**:
   - 用户选择目标适配器 -> `toggleDnsAdapter` 更新 `config.dnsOptimizeAdapters`。
   - 点击 "一键优化" -> `handleSetupDnsDoh` -> `ipc.setupDnsDoh(family)`。
   - 后端根据 `family` (ipv4/ipv6/both) 和选定的适配器名单，通过管理员权限或 Helper 进程修改注册表 DNS 设置。
   - 优化完成后，前端自动调用 `checkDnsDohStatus` 刷新状态显示。

## Connections

- [[learnings/webview-minimize-timer-throttle-stale-panel]]: 详细记录了 WebView2 最小化导致状态陈旧的根因及 `syncOnRestore` 解决方案。
- [[settings/config-store]]: `NetworkPanel` 通过 `useConfigStore` 读写 `adapter1`, `adapter2`, `dnsOptimizeAdapters` 等配置。
- [[monitor/scheduled]]: 包含 `manual_outbound_switch` 和 `manual_outbound_restore` 的后端实现逻辑。