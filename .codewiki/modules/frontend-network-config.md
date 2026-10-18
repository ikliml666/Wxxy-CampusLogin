---
title: frontend-network-config
type: module
source_files:
  - android/frontend/src/network/constants.ts
  - android/frontend/src/network/DnsAdapterCard.tsx
  - android/frontend/src/network/index.ts
  - android/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/network/types.ts
  - android/frontend/src/network/useDnsSetup.ts
tags: [network, dns, doh, config, ui, tauri]
---

## Overview

`frontend-network-config` 模块负责前端网络设置面板的实现，主要处理 DNS 适配器的配置显示、质量评级以及 DoH (DNS over HTTPS) 的优化流程。该模块与后端 IPC 接口交互，获取当前网络连接状态和 DNS 信息，并提供用户界面进行“检测 DNS”和“一键优化”操作。

核心功能包括：
1. **夜间出站切换卡**：管理 Android `WRITE_SECURE_SETTINGS` 权限，引导用户通过 ADB 授予权限以自动管理 Wi-Fi 策略（避免坏网）。
2. **DNS 优化面板**：展示各网络适配器（Adapter）的 DNS 服务器列表、DoH 支持状态及质量评级（如“极速”、“优秀”）。
3. **DoH 自动化**：提供一键启用 DoH 的功能，并刷新状态以验证结果。

## Key Components

### Constants (`android/frontend/src/network/constants.ts`)

定义了网络质量的视觉配置常量 `QUALITY_CONFIG` (第 1-10 行)。每个等级包含标签、颜色代码、图标名称等 UI 属性。

| 键名 | Label | Color | Icon | Active Bars |
| :--- | :--- | :--- | :--- | :--- |
| `excellent` | 极速 | Emerald | Zap | 5 |
| `great` | 优秀 | Sky | Zap | 5 |
| `good` | 良好 | Blue | Activity | 4 |
| `fair` | 一般 | Amber | Activity | 3 |
| `poor` | 较慢 | Orange | AlertTriangle | 2 |
| `bad` | 拥堵 | Rose | AlertTriangle | 1 |
| `unknown` | 未知 | Muted | HelpCircle | 1 |
| `disabled` | 已禁用 | Muted | Ban | 0 |
| `busy` | 检测中 | Muted | Loader2 | 0 |

### Types (`android/frontend/src/network/types.ts`)

定义了模块涉及的数据结构：

*   **`AdapterStatus`** (第 1 行): 枚举类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
*   **`Adapter`** (第 3-12 行): 基础适配器信息，包含 `name`, `ip`, `wireless`, `mac`, `ifIndex`, `status`, `linkSpeed`。
*   **`AdapterDetail`** (第 14-26 行): 扩展适配器信息，增加 `subnetMask`, `gateway`, `dhcpServer`。
*   **`DnsServerInfo`** (第 28-32 行): DNS 服务器详情，包含 `address`, `dohAvailable`, `dohEnabled`, `dohTemplate`。
*   **`DnsAdapterInfo`** (第 34-39 行): 适配器级 DNS 信息，包含 `dnsSource`, `dnsServers`, `profileDnsServers`, `adapterDnsOverridesProfile`。
*   **`DnsDohStatus`** (第 41-45 行): DoH 状态汇总，包含 `adapters`, `dohSupported`, `autoDohEnabled`。
*   **`DhcpRenewResult`** (第 47-49 行): DHCP 续租结果。
*   **`DhcpReleaseRenewResult`** (第 51-53 行): DHCP 释放/续租详细结果。
*   **`DnsSetupResult`** (第 55-60 行): DNS 设置结果，区分成功/失败的 DNS 和 DoH 列表。
*   **`EnableAdapterResult`** (第 62-65 行): 适配器启用结果。

### useDnsSetup Hook (`android/frontend/src/network/useDnsSetup.ts`)

管理 DNS 检测和优化的业务逻辑。

*   **`ALI_DNS`** (第 15 行): 阿里 DNS IP 集合 `['223.5.5.5', '223.6.6.6', '2400:3200::1', '2400:3200:baba::1']`。
*   **`RECOMMENDED_DNS`** (第 18 行): 推荐 DNS 集合（阿里 + 腾讯）。
*   **`useDnsSetup()`** (第 22-90 行):
    *   **State**: `dohEnabling` (是否正在启用 DoH), `dnsFamily` ('both' | 'ipv4' | 'ipv6')。
    *   **`handleCheckDns`** (第 33-53 行): 调用 `ipc.checkDnsDohStatus()` 获取状态，更新 `useQualityStore`。如果未使用推荐 DNS 或 DoH 未启用，记录警告日志。
    *   **`handleSetupDnsDoh`** (第 55-82 行): 调用 `ipc.setupDnsDoh(dnsFamily)` 执行优化。成功后重新检查状态并更新 Store。失败时弹出错误 Toast。
    *   **返回值**: `{ dnsFamily, setDnsFamily, dohEnabling, handleCheckDns, handleSetupDnsDoh }`。

### DnsAdapterCard Component (`android/frontend/src/network/DnsAdapterCard.tsx`)

渲染单个适配器的 DNS 信息卡片。

*   **`DnsAdapterCard`** (第 17-65 行):
    *   Props: `adapter: DnsAdapterInfo`, `autoDohEnabled: boolean`。
    *   计算 `quality` 级别（调用内部 `getDnsQuality`）。
    *   显示适配器名称、质量 Badge（DNS+DoH / DNS / 待优化 / 未配置）、来源 Badge（配置文件 / 手动）。
    *   遍历 `adapter.dnsServers` 渲染 `DnsServerRow`。
    *   如果 `adapter.adapterDnsOverridesProfile` 为真且存在配置文件 DNS，显示覆盖警告。
    *   如果存在 `profileDnsServers`，在分隔线后渲染配置文件级 DNS。
*   **`getDnsQuality`** (第 68-82 行):
    *   根据 `effectiveServers`（优先适配器级，否则配置文件级）判断质量。
    *   无服务器或 DHCP 源 -> `none`。
    *   有推荐 DNS 且 DoH 激活 -> `excellent`。
    *   有推荐 DNS 但 DoH 未激活 -> `good`。
    *   其他 -> `basic`。
*   **`DnsServerRow`** (第 88-112 行):
    *   Props: `dns: DnsServerInfo`, `detailed?: boolean`。
    *   显示 IP 地址、供应商标签（阿里/腾讯）、DoH 状态图标（启用/可用/不可用）。
    *   `detailed=true` 时显示 DoH 模板 URL 或“可启用”提示。

### NetworkPanel Component (`android/frontend/src/network/NetworkPanel.tsx`)

主面板组件，整合夜间开关和 DNS 优化 UI。

*   **`NetworkPanel`** (第 20-205 行):
    *   **Props**: `onUpdateConfig: (partial: Partial<Config>) => void`。
    *   **State**:
        *   `config`: 从 `useConfigStore` 获取。
        *   `adbCopied`: 控制复制按钮反馈。
        *   `secureStatus`: `AvoidBadWifiStatus` 快照，用于判断权限状态。
        *   `restoring`: 恢复设置时的加载态。
    *   **Effects**:
        *   `mountedRef` 管理 StrictMode 下的生命周期。
        *   `refreshSecureStatus` (第 68-76 行): 调用 `ipc.getAvoidBadWifiStatus()` 初始化权限状态。
    *   **Handlers**:
        *   `handleCopyAdbCommand` (第 43-51 行): 复制避免坏网的 ADB 命令。
        *   `handleCopyGrantCommand` (第 78-86 行): 复制授权 ADB 命令。
        *   `handleRestoreWritten` (第 88-105 行): 调用 `ipc.restoreWrittenSettings()` 还原被修改的系统设置，并刷新状态。
    *   **UI Structure**:
        *   **夜间出站切换卡** (第 110-165 行):
            *   开关控制 `enableNightOutboundSwitch`。
            *   若 `secureStatus?.granted`，显示绿色成功提示。
            *   否则，显示两个代码块（授权命令和替代命令）及复制按钮。
            *   底部显示恢复系统设置的按钮。
        *   **DNS 优化卡** (第 167-205 行):
            *   标题栏包含“检测 DNS”和“一键优化”按钮，分别绑定 `handleCheckDns` 和 `handleSetupDnsDoh`。
            *   `SegmentTabs` 选择 IPv4/IPv6/Both。
            *   根据 `dnsChecking` 和 `dnsStatus` 显示空状态、加载状态或适配器列表。
            *   若 `!dnsStatus.dohSupported`，显示黄色警告。

### Index (`android/frontend/src/network/index.ts`)

导出公共 API：
*   `NetworkPanel` (第 1 行)
*   `types` (第 3 行)
*   `constants` (第 4 行)

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      NetworkPanel                           │
│                  (NetworkPanel.tsx)                         │
│                                                             │
│  ┌──────────────────────┐   ┌──────────────────────────┐   │
│  │ Night Outbound Switch│   │      DNS Optimization    │   │
│  │                      │   │                          │   │
│  │ [Switch]             │   │ [Check DNS] [Optimize]   │   │
│  │ [ADB Commands]       │   │ [SegmentTabs: IPv4/6]    │   │
│  │ [Restore Btn]        │   │                          │   │
│  └──────────────────────┘   │  ┌────────────────────┐  │   │
│                              │  │  DnsAdapterCard    │  │   │
│                              │  │  (per adapter)     │  │   │
│                              │  │  ┌──────────────┐  │  │   │
│                              │  │  │ DNS Server   │  │  │   │
│                              │  │  │ Row x N      │  │  │   │
│                              │  │  └──────────────┘  │  │   │
│                              │  └────────────────────┘  │   │
│                              └──────────────────────────┘   │
│                                                             │
│  Uses:                                                      │
│  ├── useConfigStore (config state)                         │
│  ├── useQualityStore (dnsDohStatus, dnsChecking)           │
│  ├── useDnsSetup (logic for check/setup)                   │
│  └── tauriApiWithRetry (IPC calls)                         │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Initialization**:
    *   `NetworkPanel` mounts, reads `config` from `useConfigStore`.
    *   Calls `ipc.getAvoidBadWifiStatus()` to populate `secureStatus`.
    *   Initializes `dnsFamily` default to `'both'` in `useDnsSetup`.

2.  **DNS Check Flow**:
    *   User clicks "Check DNS" -> triggers `handleCheckDns`.
    *   Sets `dnsChecking` to true in `useQualityStore`.
    *   Calls `ipc.checkDnsDohStatus()`.
    *   On success: Updates `dnsDohStatus` in `useQualityStore`. Checks if recommended DNS/DoH is active; logs warnings if not.
    *   On failure: Clears status.
    *   Sets `dnsChecking` to false.

3.  **DNS Optimization Flow**:
    *   User clicks "One Click Optimize" -> triggers `handleSetupDnsDoh`.
    *   Sets `dohEnabling` to true.
    *   Calls `ipc.setupDnsDoh(dnsFamily)`.
    *   On success: Shows success toast. Calls `ipc.checkDnsDohStatus()` again to refresh the view.
    *   On failure: Shows error toast with extracted message.
    *   Sets `dohEnabling` to false.

4.  **Rendering**:
    *   `NetworkPanel` listens to `dnsStatus` and `dnsChecking`.
    *   If `dnsStatus` exists, maps over `dnsStatus.adapters`.
    *   For each adapter, renders `DnsAdapterCard`.
    *   `DnsAdapterCard` calculates quality via `getDnsQuality` and renders `DnsServerRow` for each server.

## Connections

*   [[settings]]: `NetworkPanel` interacts with global configuration via `useConfigStore`.
*   [[quality-monitor]]: `NetworkPanel` consumes DNS status from `useQualityStore` which is likely updated by background tasks or other network modules.
*   [[tauri-api]]: All backend communication goes through `tauriApiWithRetry`.
*   [[ui-components]]: Uses shared UI components like `AnimatedCard`, `Badge`, `Switch`, `SegmentTabs`.