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

`frontend-network-config` 模块为安卓前端提供网络设置面板实现，主要包含两大功能卡片：夜间出站切换卡与 DNS 优化卡。模块通过 Tauri IPC（`tauriApiWithRetry`）与后端通信，获取并修改系统设置（如 `WRITE_SECURE_SETTINGS` 权限状态、DNS/DoH 配置）。DNS 优化卡展示各网络适配器的 DNS 服务器列表、质量评级与 DoH 状态，并提供“检测 DNS”和“一键优化”操作。夜间出站切换卡管理校园网恢复时刻配置及 ADB 授权引导。

核心功能：
1. **夜间出站切换**：通过开关 `enableNightOutboundSwitch` 控制，可配置恢复时刻（`nightRestoreMinutes`，范围钳制在 05:00–22:59），引导用户授予 `WRITE_SECURE_SETTINGS` 权限以自动管理 Wi-Fi 策略，并提供手动还原系统设置按钮。
2. **DNS 优化面板**：展示适配器 DNS 详情（含配置文件级 DNS 覆盖警告），质量评级（excellent/good/basic/none），支持 IPv4/IPv6/双栈家族选择，调用检测与一键优化逻辑。
3. **DoH 自动化**：`useDnsSetup` 封装检测与设置逻辑，成功后刷新状态。

## Key Components

### Constants (`android/frontend/src/network/constants.ts`)

`QUALITY_CONFIG` (行1-11) 为 `as const` 只读对象，定义网络质量等级视觉配置，各键对应一个包含以下字段的对象：`label`(中文标签)、`labelKey`(i18n 键)、`color`(文字色)、`bg`(背景色)、`border`(边框色)、`borderBg`(边框背景)、`icon`(图标名)、`hex`(十六进制色)、`activeBars`(信号格数)、`glow`(发光色)。具体等级：

| 键名 | label | color | icon | activeBars | hex |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `excellent` | 极速 | text-emerald-500 | Zap | 5 | #10b981 |
| `great` | 优秀 | text-sky-500 | Zap | 5 | #0ea5e9 |
| `good` | 良好 | text-blue-500 | Activity | 4 | #3b82f6 |
| `fair` | 一般 | text-amber-500 | Activity | 3 | #f59e0b |
| `poor` | 较慢 | text-orange-500 | AlertTriangle | 2 | #f97316 |
| `bad` | 拥堵 | text-rose-500 | AlertTriangle | 1 | #f43f5e |
| `unknown` | 未知 | text-muted-foreground | HelpCircle | 1 | #94a3b8 |
| `disabled` | 已禁用 | text-muted-foreground | Ban | 0 | #94a3b8 |
| `busy` | 检测中 | text-muted-foreground | Loader2 | 0 | #94a3b8 |

> 注意：在当前提供的本模块源码中，`QUALITY_CONFIG` 仅被定义并经由 `index.ts` 导出，尚未被 `NetworkPanel.tsx` 或 `DnsAdapterCard.tsx` 直接引用（质量评级逻辑见 `getDnsQuality` 使用独立 level 字符串）。

### Types (`android/frontend/src/network/types.ts`)

公开导出类型（均带 `export`）：

- **`AdapterStatus`** (行1): 联合类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
- **`Adapter`** (行3-13): 基础适配器信息。
  - `name: string`
  - `ip: string`
  - `wireless: boolean`
  - `guid?: string`
  - `mac: string`
  - `ifIndex: number`
  - `status: AdapterStatus`
  - `linkSpeed?: number` (连接速度 bit/s，0 未知)
- **`AdapterDetail`** (行15-27): 扩展适配器信息，包含上述 `Adapter` 字段并增加：
  - `subnetMask: string`
  - `gateway: string`
  - `dhcpServer: string`
- **`DnsAdapterInfo`** (行36-42): 适配器级 DNS 信息。
  - `name: string`
  - `dnsSource: string`
  - `dnsServers: DnsServerInfo[]` (结构见下)
  - `profileDnsServers: DnsServerInfo[]`
  - `adapterDnsOverridesProfile: boolean`
- **`DnsDohStatus`** (行44-49): DoH 状态汇总。
  - `adapters: DnsAdapterInfo[]`
  - `dohSupported: boolean`
  - `autoDohEnabled: boolean`
  - `dnsSource?: string`
- **`DhcpRenewResult`** (行51-54): DHCP 续租结果。
  - `success: boolean`
  - `results: { name: string; success: boolean }[]`
- **`DhcpReleaseRenewResult`** (行56-59): DHCP 释放/续租详细结果。
  - `success: boolean`
  - `results: { name: string; wireless: boolean; ip: string; regOk: boolean; success: boolean; skipped: boolean; reason: string | null }[]`
- **`DnsSetupResult`** (行61-68): DNS 设置结果。
  - `success: boolean`
  - `message: string`
  - `dnsSuccess?: string[]`
  - `dnsFailed?: string[]`
  - `dohAdded?: string[]`
  - `dohFailed?: string[]`
- **`EnableAdapterResult`** (行70-73): 适配器启用结果。
  - `success: boolean`
  - `message?: string`

模块内部（未导出）接口 **`DnsServerInfo`** (行29-34) 被 `DnsAdapterInfo` 使用：
  - `address: string`
  - `dohAvailable: boolean`
  - `dohEnabled: boolean`
  - `dohTemplate: string`

### useDnsSetup Hook (`android/frontend/src/network/useDnsSetup.ts`)

- **`ALI_DNS`** (行11): `Set<string>` 阿里公共 DNS 地址：`['223.5.5.5', '223.6.6.6', '2400:3200::1', '2400:3200:baba::1']`。
- **`TENCENT_DNS`** (行12): `Set<string>` 腾讯 DNSPod 地址：`['1.12.12.12', '120.53.53.53', '2402:4e00::']`。
- **`RECOMMENDED_DNS`** (行13): `Set<string>` 合并上述两组，用于推荐判定。
- **`DnsFamily`** (行15): 类型 `'both' | 'ipv4' | 'ipv6'`。
- **`useDnsSetup()`** (行22-88): 自定义 Hook，返回对象 `{ dnsFamily, setDnsFamily, dohEnabling, handleCheckDns, handleSetupDnsDoh }`。
  - 内部状态：`dohEnabling` (行26), `dnsFamily` 默认 `'both'` (行28)。
  - **`handleCheckDns`** (行37-59): 异步，置 `useQualityStore.setDnsChecking(true)`；调用 `ipc.checkDnsDohStatus()`；成功更新 `setDnsDohStatus`，并根据是否含推荐 DNS 或 DoH 未启用写日志；失败清状态；finally 置 false。
  - **`handleSetupDnsDoh`** (行61-85): 异步，置 `dohEnabling(true)`；调用 `ipc.setupDnsDoh(dnsFamily)`；成功弹成功 Toast 并独立刷新状态；失败弹错误 Toast；finally 置 false。

### DnsAdapterCard Component (`android/frontend/src/network/DnsAdapterCard.tsx`)

- **`DnsAdapterCard`** (行13-60): 导出函数组件。Props: `{ adapter: DnsAdapterInfo; autoDohEnabled: boolean }`。使用 `useTranslation`，调用 `getDnsQuality` 计算 `quality`。渲染适配器名、质量 Badge（excellent→DNS+DoH, good→DNS, basic→待优化, none→未配置）、来源 Badge（profile→每配置文件DNS, manual→每适配器DNS）。遍历 `adapter.dnsServers` 渲染 `DnsServerRow` (detailed=true)；若无服务器显示提示；若 `adapterDnsOverridesProfile` 且存在 profile DNS 显示覆盖警告；之后分隔线渲染 `profileDnsServers` 行 (detailed=false)。
- **`getDnsQuality`** (行63-83): 导出函数。参数 `(adapter: {dnsSource?, dnsServers, profileDnsServers?, adapterDnsOverridesProfile?}, autoDohEnabled: boolean, t: (key)=>string)`。依据 `effectiveServers`（适配器覆盖则用适配器，否则适配器非空用适配器，否则用配置文件）判定：空或 dhcp 源 → `none`；有推荐且 (autoDohEnabled 或 所有推荐服务器 dohEnabled) → `excellent`；有推荐 → `good`；其他 → `basic`。返回 `{level, label}`。
- **`DnsServerRow`** (行89-120): 内部函数。Props `{ dns: {address,dohEnabled,dohAvailable,dohTemplate}, detailed?: boolean }`。显示地址（推荐绿色）、供应商标签（阿里/腾讯）、DoH 状态图标（启用绿勾、可用黄叉、不可用灰叉）；detailed 时显示 dohTemplate 或“可启用”提示。

### NetworkPanel Component (`android/frontend/src/network/NetworkPanel.tsx`)

- **`RESTORE_TIME_DEFAULT_MINUTES`** (行29): 常量 `450`（07:30），后端同值回落。
- **`RESTORE_TIME_MAX_MINUTES`** (行32): 常量 `1379`（22:59），须早于切换时刻 23:00。
- **`RESTORE_TIME_MIN_MINUTES`** (行36): 常量 `300`（05:00），产品口径下限。
- **`formatRestoreTime`** (行39-45): 函数，分钟数 → "HH:MM"，越界则默认 450。
- **`NetworkPanel`** (行47-342): `memo` 组件，Props `{ onUpdateConfig: (partial: Partial<Config>) => void }`。
  - 从 `useConfigStore` (行51) 浅比较取 `config`；`dnsStatus`/`dnsChecking` 来自 `useQualityStore` (行62-63)；`useDnsSetup` 提供 `dnsFamily,setDnsFamily,dohEnabling,handleCheckDns,handleSetupDnsDoh` (行65)。
  - 内部状态：`adbCopied` (行73), `secureStatus` (行86), `restoring` (行87)。
  - **`handleCopyAdbCommand`** (行74-82): 复制 `t('network.avoidBadWifiCommand')` 到剪贴板，成功置 `adbCopied` 2秒。
  - **`refreshSecureStatus`** (行88-96): 调用 `ipc.getAvoidBadWifiStatus()` 更新 `secureStatus`，失败置 null。
  - **`handleCopyGrantCommand`** (行101-109): 复制授权命令。
  - **`handleRestoreWritten`** (行111-130): 调用 `ipc.restoreWrittenSettings()`，根据结果弹 Toast，最后刷新状态。
  - UI 结构：
    - 夜间出站切换卡 (行134-243)：开关绑定 `config.enableNightOutboundSwitch`；恢复时刻 `Input type="time"` 绑定 `config.nightRestoreMinutes` 并钳制；权限 granted 显示成功否则显示两个命令代码块与复制按钮；警告块；还原按钮。
    - DNS 优化卡 (行245-339)：标题旁“检测 DNS”与“一键优化”按钮；家族 `SegmentTabs`；空态/加载态/适配器列表（`DnsAdapterCard`）；不支持 DoH 警告。

### Index (`android/frontend/src/network/index.ts`)

- 行1: `export { NetworkPanel } from './NetworkPanel'`
- 行3: `export * from './types'`
- 行4: `export * from './constants'`

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      NetworkPanel                           │
│                  (NetworkPanel.tsx)                         │
│                                                             │
│  ┌──────────────────────┐   ┌──────────────────────────┐   │
│  │ Night Outbound Switch│   │      DNS Optimization    │   │
│  │  (lines 134-243)     │   │      (lines 245-339)     │   │
│  │ [Switch]             │   │ [Check DNS][Optimize]   │   │
│  │ [Restore Time Input] │   │ [SegmentTabs IPv4/6]    │   │
│  │ [ADB Commands]       │   │  ┌────────────────────┐  │   │
│  │ [Restore Btn]        │   │  │  DnsAdapterCard    │  │   │
│  └──────────────────────┘   │  │  (DnsAdapterCard)  │  │   │
│                              │  │  ┌──────────────┐  │  │   │
│                              │  │  │ DnsServerRow │  │  │   │
│                              │  │  └──────────────┘  │  │   │
│                              │  └────────────────────┘  │   │
│ ┌──────────────┐  ┌──────────────────────────────────┐   │
│ │ useDnsSetup  │  │ getDnsQuality (DnsAdapterCard)   │   │
│ │ (hook)       │  └──────────────────────────────────┘   │
│ └──────────────┘                                          │
│  Uses:                                                      │
│  ├── useConfigStore (config)                               │
│  ├── useQualityStore (dnsDohStatus, dnsChecking)          │
│  ├── tauriApiWithRetry (IPC)                               │
│  └── constants QUALITY_CONFIG (exported, unused here)     │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Initialization**:
   - `NetworkPanel` 挂载 (行47)，从 `useConfigStore` 读取 `config` (行51)。
   - 调用 `refreshSecureStatus` (行88-96) 获取 `AvoidBadWifiStatus` 写入 `secureStatus`。
   - `useDnsSetup` 初始化 `dnsFamily='both'` (行28)，`dohEnabling=false`。

2. **DNS Check Flow**:
   - 用户点击“检测 DNS” → `handleCheckDns` (useDnsSetup 行37) → `useQualityStore.setDnsChecking(true)`。
   - 调用 `ipc.checkDnsDohStatus()`，成功则 `setDnsDohStatus(status)`；根据 `RECOMMENDED_DNS` 判断写警告日志。
   - 失败清状态；finally `setDnsChecking(false)`。
   - `NetworkPanel` 因 `dnsStatus`/`dnsChecking` 变化重渲染，列表映射 `DnsAdapterCard` (行326)。

3. **DNS Optimization Flow**:
   - 点击“一键优化” → `handleSetupDnsDoh` (行61) 置 `dohEnabling=true`，调 `ipc.setupDnsDoh(dnsFamily)`。
   - 成功弹 Toast，内部再调 `checkDnsDohStatus` 刷新；失败弹错误。
   - finally `dohEnabling=false`。

4. **Night Switch Flow**:
   - 开关 `onCheckedChange` → `onUpdateConfig({enableNightOutboundSwitch})` (行146)。
   - 恢复时刻 `onChange` 解析 time，钳制到 [300,1379] 后 `onUpdateConfig({nightRestoreMinutes})` (行172-177)。
   - 复制命令按钮调用 `handleCopyAdbCommand` / `handleCopyGrantCommand`。
   - `handleRestoreWritten` 调 `ipc.restoreWrittenSettings` 并刷新 `secureStatus`。

5. **Rendering**:
   - `DnsAdapterCard` 通过 `getDnsQuality` (行63) 算质量，渲染 `DnsServerRow` 每行；detailed 控制显示模板。

## Connections

- [[settings]]: `NetworkPanel` 通过 `useConfigStore` 读写全局配置（如 `enableNightOutboundSwitch`, `nightRestoreMinutes`）。
- [[quality-monitor]]: 消费/更新 `useQualityStore` 的 `dnsDohStatus` 与 `dnsChecking`，可能与后台质量任务联动。
- [[tauri-api]]: 所有 IPC 调用经 `tauriApiWithRetry`（方法 `getAvoidBadWifiStatus`, `restoreWrittenSettings`, `checkDnsDohStatus`, `setupDnsDoh`）。
- [[ui-components]]: 复用 `AnimatedCard`, `Badge`, `Switch`, `SegmentTabs`, `Button`, `Input`, `Label` 等共享组件。
