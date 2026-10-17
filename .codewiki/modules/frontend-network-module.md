---
title: frontend-network-module
type: module
source_files:
  - android/frontend/src/network/constants.ts
  - android/frontend/src/network/DnsAdapterCard.tsx
  - android/frontend/src/network/index.ts
  - android/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/network/types.ts
  - android/frontend/src/network/useDnsSetup.ts
tags: [network, dns, wifi, ui, frontend]
---

## Overview

`frontend-network-module` 是前端网络配置模块，负责管理 DNS 设置、Wi-Fi 绑定逻辑以及夜间出站开关。该模块提供了用于展示和管理 DNS 适配器状态的卡片（Cards）和面板（Panels），支持 IPv4/IPv6/双栈选择，并集成了 DoH (DNS over HTTPS) 的检测与一键优化功能。此外，它还处理了 Android 系统级安全设置（Secure Settings）的权限引导与恢复逻辑。

## Key Components

### `constants.ts`
定义了网络质量等级的视觉配置常量。

*   **`QUALITY_CONFIG`** (Line 1-10): 常量对象，定义了从 `excellent` 到 `disabled` 共 9 种网络质量状态的样式属性。
    *   字段包括：`label` (中文标签), `labelKey` (i18n key), `color`, `bg`, `border`, `borderBg`, `icon` (Lucide 图标名), `hex` (颜色值), `activeBars` (信号条数量), `glow` (发光效果)。
    *   状态枚举：`excellent`, `great`, `good`, `fair`, `poor`, `bad`, `unknown`, `disabled`, `busy`。

### `types.ts`
定义了网络相关的 TypeScript 接口和类型。

*   **`AdapterStatus`** (Line 1): 联合类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
*   **`Adapter`** (Line 3-12): 适配器基础信息接口。
    *   字段：`name`, `ip`, `wireless`, `guid?`, `mac`, `ifIndex`, `status`, `linkSpeed?`。
*   **`AdapterDetail`** (Line 14-25): 适配器详细信息接口，继承自 Adapter 并扩展。
    *   新增字段：`subnetMask`, `gateway`, `dhcpServer`。
*   **`DnsServerInfo`** (Line 27-31): DNS 服务器信息接口。
    *   字段：`address`, `dohAvailable`, `dohEnabled`, `dohTemplate`。
*   **`DnsAdapterInfo`** (Line 33-38): DNS 适配器信息接口。
    *   字段：`name`, `dnsSource`, `dnsServers`, `profileDnsServers`, `adapterDnsOverridesProfile`。
*   **`DnsDohStatus`** (Line 40-45): DNS/DoH 整体状态接口。
    *   字段：`adapters`, `dohSupported`, `autoDohEnabled`, `dnsSource?`。
*   **`DhcpRenewResult`** (Line 47-50): DHCP 续租结果接口。
*   **`DhcpReleaseRenewResult`** (Line 52-55): DHCP 释放续租结果接口。
*   **`DnsSetupResult`** (Line 57-63): DNS 设置结果接口。
    *   字段：`success`, `message`, `dnsSuccess?`, `dnsFailed?`, `dohAdded?`, `dohFailed?`。
*   **`EnableAdapterResult`** (Line 65-68): 启用适配器结果接口。

### `useDnsSetup.ts`
提供 DNS 检测与优化的 Hook 逻辑。

*   **`ALI_DNS`** (Line 10): `Set<string>`，包含阿里公共 DNS 地址 (`223.5.5.5`, `223.6.6.6`, `2400:3200::1`, `2400:3200:baba::1`)。
*   **`RECOMMENDED_DNS`** (Line 12): `Set<string>`，合并了阿里 DNS 和腾讯 DNSPod 地址。
*   **`DnsFamily`** (Line 14): 联合类型 `'both' | 'ipv4' | 'ipv6'`。
*   **`useDnsSetup()`** (Line 18-82): React Hook，管理 DNS 状态。
    *   **状态**: `dohEnabling` (boolean, Line 22), `dnsFamily` (DnsFamily, Line 25)。
    *   **方法**:
        *   `handleCheckDns()`: (Line 29-52) 调用 IPC 检查 DNS/DoH 状态，更新 Store，并根据结果记录警告日志。
        *   `handleSetupDnsDoh()`: (Line 54-78) 调用 IPC 执行 DNS/DoH 设置，成功后刷新状态，失败则提示错误。
    *   **返回值**: `{ dnsFamily, setDnsFamily, dohEnabling, handleCheckDns, handleSetupDnsDoh }`。

### `DnsAdapterCard.tsx`
渲染单个 DNS 适配器详细信息的 UI 组件。

*   **`DnsAdapterCard`** (Line 15-66): 函数组件。
    *   Props: `adapter` (DnsAdapterInfo), `autoDohEnabled` (boolean)。
    *   逻辑: 计算 `quality`，渲染适配器名称、徽章（Badge）、DNS 服务器列表、配置文件 DNS 列表及覆盖警告。
*   **`getDnsQuality()`** (Line 68-82): 纯函数，根据适配器信息返回质量等级 `{ level, label }`。
    *   逻辑: 优先使用适配器级 DNS，若为空则回退到配置文件级 DNS。判定是否推荐且 DoH 已启用。
*   **`DnsServerRow`** (Line 84-106): 内部组件，渲染单行 DNS 服务器信息。
    *   Props: `dns` (DnsServerInfo), `detailed` (boolean, default false)。
    *   逻辑: 显示 IP 地址、提供商标签（阿里/腾讯）、DoH 状态图标（绿色对勾/琥珀叉号/灰色叉号）。在 `detailed=true` 时显示 DoH 模板或可用提示。

### `NetworkPanel.tsx`
主面板组件，整合夜间开关、DNS 优化等功能。

*   **`NetworkPanel`** (Line 21-232): 记忆化组件 (`memo`)。
    *   Props: `onUpdateConfig`。
    *   **状态**:
        *   `config`: 从 `useConfigStore` 获取 (Line 24)。
        *   `dnsStatus`, `dnsChecking`: 从 `useQualityStore` 获取 (Line 27-28)。
        *   `adbCopied`: 控制复制按钮反馈 (Line 38)。
        *   `secureStatus`: 存储 "避免坏 Wi-Fi" 安全设置状态 (Line 45)。
        *   `restoring`: 控制恢复按钮加载状态 (Line 46)。
    *   **Effect**:
        *   Line 29-34: StrictMode 兼容，确保 `mountedRef` 正确重置。
        *   Line 53-56: 初始化时刷新 `secureStatus`。
    *   **Handlers**:
        *   `handleCopyAdbCommand()`: (Line 39-47) 复制避免坏 Wi-Fi 命令。
        *   `refreshSecureStatus()`: (Line 48-56) 调用 IPC `getAvoidBadWifiStatus`。
        *   `handleCopyGrantCommand()`: (Line 58-66) 复制授权命令。
        *   `handleRestoreWritten()`: (Line 68-85) 调用 IPC 恢复写入的设置，并刷新状态。
    *   **UI 结构**:
        *   **夜间出站开关卡** (Line 88-156): 包含 Switch 控件、权限引导文本、ADB 命令复制按钮、恢复按钮。
        *   **DNS 优化卡** (Line 158-232): 包含标题、操作按钮（检测/优化）、IPv4/IPv6 切换标签 (`SegmentTabs`)、DNS 状态展示区（调用 `DnsAdapterCard` 列表）。

### `index.ts`
模块入口文件。

*   **Exports** (Line 1-6): 导出 `NetworkPanel`, `types`, `constants`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      NetworkPanel                           │
│                     (NetworkPanel.tsx)                      │
│                                                             │
│  ┌──────────────────────┐   ┌──────────────────────────┐   │
│  │ Night Outbound Card  │   │     DNS Optimization     │   │
│  │ - ADB Command Copy   │   │      Card                │   │
│  │ - Secure Status      │   │ - Family Tabs            │   │
│  │ - Restore Action     │   │ - DnsAdapterCard List    │   │
│  └──────────────────────┘   └──────────┬───────────────┘   │
│                                        │                   │
│                                ┌───────▼────────┐          │
│                                │ DnsAdapterCard │          │
│                                │(DnsAdapterCard │          │
│                                │    .tsx)       │          │
│                                └───────┬────────┘          │
│                                        │                   │
│                                ┌───────▼────────┐          │
│                                │ useDnsSetup    │          │
│                                │   (Hook)       │          │
│                                └───────┬────────┘          │
│                                        │                   │
│                                ┌───────▼────────┐          │
│                                │ tauriApiWith   │          │
│                                │    Retry       │          │
│                                └────────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

1.  **NetworkPanel** 作为顶层容器，管理全局配置和安全状态。
2.  **DNS 优化区域** 依赖 **useDnsSetup** Hook 进行业务逻辑处理（检测/设置）。
3.  **DnsAdapterCard** 负责具体适配器的可视化展示，它不直接调用 API，而是通过父组件传入的状态数据渲染。
4.  所有底层 IPC 通信均通过 **tauriApiWithRetry** 进行，确保重试机制。

## Data Flow

1.  **初始化**:
    *   `NetworkPanel` 挂载，读取 `useConfigStore` 中的 `enableNightOutboundSwitch`。
    *   调用 `ipc.getAvoidBadWifiStatus()` 获取安全设置状态 `secureStatus`。
    *   初始化 `dnsFamily` 为 `'both'`。

2.  **DNS 检测流程**:
    *   用户点击 "Check DNS" -> 触发 `handleCheckDns`。
    *   `useQualityStore.setDnsChecking(true)`。
    *   调用 `ipc.checkDnsDohStatus()`。
    *   成功：`useQualityStore.setDnsDohStatus(status)`，UI 更新显示 `DnsAdapterCard` 列表。
    *   失败/完成：`useQualityStore.setDnsChecking(false)`。

3.  **DNS 优化流程**:
    *   用户点击 "One Click Optimize" -> 触发 `handleSetupDnsDoh`。
    *   `setDohEnabling(true)`。
    *   调用 `ipc.setupDnsDoh(dnsFamily)`。
    *   成功：Toast 提示成功，再次调用 `ipc.checkDnsDohStatus()` 刷新状态。
    *   失败：Toast 提示错误。
    *   完成：`setDohEnabling(false)`。

4.  **夜间开关与安全设置**:
    *   切换 Switch -> `onUpdateConfig({ enableNightOutboundSwitch: checked })`。
    *   若无权限 (`secureStatus.granted === false`)，显示 ADB 命令供用户复制。
    *   若有写入的历史记录，显示 "Restore" 按钮，点击后调用 `ipc.restoreWrittenSettings()`。

## Connections

*   [[useConfigStore]]: 提供全局配置，如 `enableNightOutboundSwitch`。
*   [[useQualityStore]]: 共享 DNS 检测状态 (`dnsDohStatus`, `dnsChecking`)。
*   [[useLogToastStore]]: 用于弹出 Toast 通知和记录日志。
*   [[tauriApi]]: 底层 IPC 通信封装。
*   [[SegmentTabs]]: 用于 IPv4/IPv6 切换的 UI 组件。
*   [[AnimatedCard]]: 用于面板入场动画的 UI 组件。