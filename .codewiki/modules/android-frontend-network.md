---
title: android-frontend-network
type: module
source_files:
  - android/frontend/src/network/constants.ts
  - android/frontend/src/network/DnsAdapterCard.tsx
  - android/frontend/src/network/index.ts
  - android/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/network/types.ts
  - android/frontend/src/network/useDnsSetup.ts
tags: [android, frontend, network, dns, doh, tauri]
---

## Overview
该模块是 Android 前端应用中的网络设置核心模块，主要职责包括：
1. **DNS 优化与检测**：提供 DNS 质量检查、DoH（DNS over HTTPS）一键优化功能。
2. **适配器可视化**：通过 `DnsAdapterCard` 渲染各网络适配器的 DNS 配置状态、DoH 启用情况及配置文件级 DNS 覆盖警告。
3. **夜间出站切换管理**：处理 Android `WRITE_SECURE_SETTINGS` 权限引导、ADB 命令复制及安全设置还原逻辑。
4. **常量定义**：定义网络质量等级样式配置及推荐 DNS 地址集合。

模块基于 React (Tauri WebView) 构建，使用 Zustand 进行状态管理，并通过 Tauri IPC 与后端 Rust 服务通信。

## Key Components

### Constants & Types (`constants.ts`, `types.ts`)

#### QUALITY_CONFIG
位于 `constants.ts` 第 1-11 行。
定义网络质量等级的视觉样式配置对象。包含以下字段：
- `label`: 中文显示标签（如 '极速', '优秀'）。
- `labelKey`: i18n 翻译键。
- `color`, `bg`, `border`, `borderBg`: Tailwind CSS 类名或颜色值。
- `icon`: Lucide 图标名称（字符串）。
- `hex`: 十六进制颜色代码。
- `activeBars`: 信号条激活数量（0-5）。
- `glow`: 发光效果 RGBA 值。

枚举值包括：`excellent`, `great`, `good`, `fair`, `poor`, `bad`, `unknown`, `disabled`, `busy`。

#### Type Definitions (`types.ts`)
位于 `types.ts` 第 1-73 行。
- `AdapterStatus` (L1): 适配器状态联合类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
- `Adapter` (L3-L13): 基础适配器信息接口，包含 `name`, `ip`, `wireless`, `guid?`, `mac`, `ifIndex`, `status`, `linkSpeed?`。
- `AdapterDetail` (L15-L27): 详细适配器信息，继承基础字段并增加 `subnetMask`, `gateway`, `dhcpServer`。
- `DnsServerInfo` (L29-L34): DNS 服务器信息，包含 `address`, `dohAvailable`, `dohEnabled`, `dohTemplate`。
- `DnsAdapterInfo` (L36-L42): 适配器 DNS 配置信息，包含 `name`, `dnsSource`, `dnsServers`, `profileDnsServers`, `adapterDnsOverridesProfile`。
- `DnsDohStatus` (L44-L49): DNS/DoH 整体状态，包含 `adapters`, `dohSupported`, `autoDohEnabled`, `dnsSource?`。
- `DhcpRenewResult` (L51-L54), `DhcpReleaseRenewResult` (L56-L59), `DnsSetupResult` (L61-L68), `EnableAdapterResult` (L70-L73): IPC 通信结果类型。

#### Recommended DNS Sets (`useDnsSetup.ts`)
位于 `useDnsSetup.ts` 第 11-13 行。
- `ALI_DNS`: Set 类型，包含阿里公共 DNS IPv4/IPv6 地址。
- `RECOMMENDED_DNS`: Set 类型，合并阿里和腾讯 DNS 地址，用于判定是否为推荐 DNS。

### Hooks & Logic (`useDnsSetup.ts`)

#### useDnsSetup
位于 `useDnsSetup.ts` 第 22-87 行。
自定义 Hook，封装 DNS 检测与优化逻辑。
- **State**:
    - `dohEnabling` (L26): 布尔值，表示 DoH 是否正在启用中。
    - `dnsFamily` (L28): `DnsFamily` 类型 ('both' | 'ipv4' | 'ipv6')，默认 'both'。
- **Methods**:
    - `handleCheckDns` (L37-L59): 异步函数。调用 `ipc.checkDnsDohStatus()` 获取状态并更新 Store。若未检测到推荐 DNS 或 DoH 未启用，记录 Warning Log。
    - `handleSetupDnsDoh` (L61-L85): 异步函数。调用 `ipc.setupDnsDoh(dnsFamily)` 执行优化。成功后刷新状态并提示 Success Toast；失败提示 Error Toast。包含 `mountedRef` 防止卸载后状态更新。

### UI Components (`DnsAdapterCard.tsx`, `NetworkPanel.tsx`)

#### DnsAdapterCard
位于 `DnsAdapterCard.tsx` 第 13-60 行。
展示单个网络适配器的 DNS 配置卡片。
- **Props**: `adapter: DnsAdapterInfo`, `autoDohEnabled: boolean`。
- **Sub-components**:
    - `getDnsQuality` (L63-L83): 内部导出函数，计算适配器 DNS 质量等级（'none', 'basic', 'good', 'excellent'）。逻辑：若无有效服务器或 DHCP 源则返回 'none'；若有推荐 DNS 且 DoH 启用则 'excellent'；有推荐 DNS 但 DoH 未启用则 'good'；否则 'basic'。
    - `DnsServerRow` (L89-L119): 私有组件，渲染单行 DNS 服务器信息。`detailed` 参数控制是否显示 DoH 模板和“可启用”提示。
- **UI Features**:
    - 显示适配器名称。
    - 根据 `getDnsQuality` 结果显示 Badge（DNS+DoH, DNS, Pending Optimization, Not Configured）。
    - 显示来源 Badge（Per Profile DNS, Per Adapter DNS）。
    - 警告提示：若 `adapterDnsOverridesProfile` 为真，显示 Amber 警告条。
    - 配置文件级 DNS 列表（仅显示地址和图标，不显示 DoH 详情）。

#### NetworkPanel
位于 `NetworkPanel.tsx` 第 25-288 行。
主面板组件，包含两个主要区块：夜间出站切换卡和 DNS 优化卡。
- **Props**: `onUpdateConfig: (partial: Partial<Config>) => void`。
- **State Management**:
    - `config`: 从 `useConfigStore` 浅比较获取。
    - `dnsStatus`, `dnsChecking`: 从 `useQualityStore` 获取。
    - `secureStatus`: 本地 State，存储 `AvoidBadWifiStatus`。
    - `restoring`: 本地 State，控制还原按钮加载态。
    - `adbCopied`: 本地 State，控制复制按钮反馈。
- **Handlers**:
    - `handleCopyAdbCommand` (L52-L60): 复制“避免坏 WiFi” ADB 命令到剪贴板。
    - `handleCopyGrantCommand` (L79-L87): 复制授予权限的 ADB 命令。
    - `refreshSecureStatus` (L66-L74): 调用 `ipc.getAvoidBadWifiStatus()` 刷新状态。
    - `handleRestoreWritten` (L89-L108): 调用 `ipc.restoreWrittenSettings()` 还原被修改的安全设置。
- **UI Structure**:
    - **Night Outbound Switch Card** (L113-L188):
        - 开关控件绑定 `enableNightOutboundSwitch`。
        - 若 `secureStatus.granted` 为真，显示绿色成功提示。
        - 否则，显示两行 ADB 命令（授权命令 + 替代命令），每行配有复制按钮。
        - 底部显示琥珀色警告信息及“还原安全设置”按钮。
    - **DNS Optimization Card** (L192-L285):
        - 标题栏右侧包含“检测 DNS”和“一键优化”按钮。
        - “检测 DNS”按钮在 `dnsChecking` 时禁用并显示旋转图标。
        - “一键优化”按钮在 `dohEnabling` 时禁用。
        - `SegmentTabs` 选择 IPv4/IPv6/Both。
        - 状态显示区域：
            - 无状态：提示点击检测。
            - 检测中：显示旋转图标。
            - 有状态：遍历 `dnsStatus.adapters` 渲染 `DnsAdapterCard`。
            - 若 `!dnsStatus.dohSupported`，显示 Amber 警告。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     NetworkPanel (NetworkPanel.tsx)          │
│  ┌──────────────────────┐   ┌──────────────────────────┐    │
│  │ Night Outbound Card  │   │   DNS Optimization Card  │    │
│  │ - Switch             │   │  - SegmentTabs           │    │
│  │ - ADB Commands       │   │  - Check/Setup Buttons   │    │
│  │ - Restore Button     │   │                          │    │
│  └──────────┬───────────┘   └──────────┬───────────────┘    │
│             │                          │                    │
│             ▼                          ▼                    │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                  useDnsSetup Hook                     │   │
│  │  - handleCheckDns / handleSetupDnsDoh                │   │
│  │  - interacts with QualityStore & LogToastStore       │   │
│  └──────────────────────────┬───────────────────────────┘   │
│                             │                               │
│                             ▼                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              DnsAdapterCard (DnsAdapterCard.tsx)      │   │
│  │  - getDnsQuality (Logic)                             │   │
│  │  - DnsServerRow (Render)                             │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────┬──────────────────────────────────┘
                           │ IPC
                           ▼
                  Tauri Backend API
```

## Data Flow

1. **初始化与状态订阅**:
   - `NetworkPanel` 挂载时，通过 `useConfigStore` 订阅配置，通过 `useQualityStore` 订阅 DNS 状态 (`dnsDohStatus`, `dnsChecking`)。
   - `useDnsSetup` Hook 初始化 `dohEnabling` 和 `dnsFamily` 状态。

2. **DNS 检测流程**:
   - 用户点击“检测 DNS”按钮触发 `handleCheckDns`。
   - `handleCheckDns` 调用 `tauriApiWithRetry.checkDnsDohStatus()`。
   - 获取结果后，更新 `useQualityStore.setDnsDohStatus(status)`。
   - 同时检查是否存在推荐 DNS 但 DoH 未启用的情况，若是，通过 `useLogToastStore.addLog` 添加警告日志。
   - `NetworkPanel` 监听到 `dnsStatus` 变化，重新渲染 `DnsAdapterCard` 列表。

3. **DNS 优化流程**:
   - 用户点击“一键优化”按钮触发 `handleSetupDnsDoh`。
   - 设置 `dohEnabling = true`。
   - 调用 `tauriApiWithRetry.setupDnsDoh(dnsFamily)`。
   - 若成功，调用 `ipc.checkDnsDohStatus()` 刷新最新状态至 Store，并弹出 Success Toast。
   - 若失败，弹出 Error Toast。
   - Finally 块重置 `dohEnabling = false`。

4. **适配器卡片渲染**:
   - `NetworkPanel` 遍历 `dnsStatus.adapters`，将每个 `adapter` 和全局 `autoDohEnabled` 传入 `DnsAdapterCard`。
   - `DnsAdapterCard` 内部调用 `getDnsQuality` 计算当前适配器的质量等级。
   - 根据等级渲染不同的 Badge 和颜色。
   - 遍历 `adapter.dnsServers` 渲染 `DnsServerRow`，若 `detailed=true` 则显示 DoH 模板和可用性提示。
   - 若 `adapter.adapterDnsOverridesProfile` 为真，渲染覆盖警告。
   - 若存在 `profileDnsServers`，渲染配置文件级 DNS 列表（简化视图）。

## Connections
- [[android-frontend-settings]]: `NetworkPanel` 接收 `onUpdateConfig` 回调，直接修改 Settings 模块管理的配置。
- [[android-frontend-hooks-quality]]: 依赖 `useQualityStore` 存储 DNS 检测结果。
- [[android-frontend-hooks-log-toast]]: 使用 `useLogToastStore` 记录日志和弹出 Toast 提示。
- [[android-backend-network]]: 通过 Tauri IPC 调用后端网络操作接口。

## Known Issues
1. **StrictMode 双重渲染防护**:
   - 在 `NetworkPanel.tsx` (L33-L38) 和 `useDnsSetup.ts` (L30-L35) 中，均使用了 `mountedRef` 来防止 React StrictMode 下开发模式的双次 setup/cleanup 导致异步回调在组件卸载后尝试更新状态。这是必要的防御性编程，非 Bug。
2. **剪贴板权限限制**:
   - 在 `NetworkPanel.tsx` (L52-L60, L79-L87) 中，`navigator.clipboard.writeText` 可能被 WebView 环境限制。代码捕获了异常并静默处理，仅在成功时更新 `adbCopied` 状态以提供视觉反馈。
3. **DoH 支持检测**:
   - `DnsAdapterCard` (L275-L279) 显示全局 `dohSupported` 标志。如果后端报告不支持 DoH，即使配置了推荐 DNS，用户也无法通过一键优化启用 DoH。