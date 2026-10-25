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
该模块是 Android 前端应用中的网络设置核心模块，基于 React (Tauri WebView) 构建，使用 Zustand 进行状态管理，并通过 Tauri IPC 与后端 Rust 服务通信。主要职责包括：
1. **DNS 优化与检测**：提供 DNS 质量检查、DoH（DNS over HTTPS）一键优化功能，支持 IPv4 / IPv6 / 双栈目标选择。
2. **适配器可视化**：通过 `DnsAdapterCard` 渲染各网络适配器的 DNS 配置状态、DoH 启用情况及配置文件级 DNS 覆盖警告。
3. **夜间出站切换管理**：处理 Android `WRITE_SECURE_SETTINGS` 权限引导、ADB 命令复制及安全设置还原逻辑，并支持可配置的恢复时刻。
4. **常量与类型定义**：定义网络质量等级样式配置、推荐 DNS 地址集合及适配器 / DNS 相关 TypeScript 接口。

## Key Components

### Constants & Types (`constants.ts`, `types.ts`, `index.ts`)

#### QUALITY_CONFIG
位于 `constants.ts` 第 1-11 行。
导出 `const` 对象（经 `as const` 断言），定义网络质量等级的视觉样式配置。键包含：`excellent`、`great`、`good`、`fair`、`poor`、`bad`、`unknown`、`disabled`、`busy`（共 9 级）。每个键值对象包含以下字段：
- `label`: 中文显示标签（如 '极速'、'优秀'、'良好'、'一般'、'较慢'、'拥堵'、'未知'、'已禁用'、'检测中'）
- `labelKey`: i18n 翻译键（如 `quality.excellent` 等）
- `color`、`bg`、`border`、`borderBg`: Tailwind CSS 类名（如 `text-emerald-500`、`bg-emerald-500/10`）
- `icon`: Lucide 图标名称字符串（`Zap`、`Activity`、`AlertTriangle`、`HelpCircle`、`Ban`、`Loader2`）
- `hex`: 十六进制颜色代码（如 `#10b981`）
- `activeBars`: 信号条激活数量（0–5 整数）
- `glow`: 发光效果 RGBA 值（如 `rgba(16,185,129,0.35)`）

#### Type Definitions (`types.ts`)
- `AdapterStatus` (L1): 联合类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`
- `Adapter` (L3-L13): 接口，字段：`name: string`、`ip: string`、`wireless: boolean`、`guid?: string`、`mac: string`、`ifIndex: number`、`status: AdapterStatus`、`linkSpeed?: number`（连接速度 bit/s，0 未知）
- `AdapterDetail` (L15-L27): 接口，继承基础字段并增加 `subnetMask: string`、`gateway: string`、`dhcpServer: string`
- `DnsServerInfo` (L29-L34): 接口，字段：`address: string`、`dohAvailable: boolean`、`dohEnabled: boolean`、`dohTemplate: string`
- `DnsAdapterInfo` (L36-L42): 接口，字段：`name: string`、`dnsSource: string`、`dnsServers: DnsServerInfo[]`、`profileDnsServers: DnsServerInfo[]`、`adapterDnsOverridesProfile: boolean`
- `DnsDohStatus` (L44-L49): 接口，字段：`adapters: DnsAdapterInfo[]`、`dohSupported: boolean`、`autoDohEnabled: boolean`、`dnsSource?: string`
- `DhcpRenewResult` (L51-L54): 接口，字段：`success: boolean`、`results: { name: string; success: boolean }[]`
- `DhcpReleaseRenewResult` (L56-L59): 接口，字段：`success: boolean`、`results: { name: string; wireless: boolean; ip: string; regOk: boolean; success: boolean; skipped: boolean; reason: string | null }[]`
- `DnsSetupResult` (L61-L68): 接口，字段：`success: boolean`、`message: string`、`dnsSuccess?: string[]`、`dnsFailed?: string[]`、`dohAdded?: string[]`、`dohFailed?: string[]`
- `EnableAdapterResult` (L70-L73): 接口，字段：`success: boolean`、`message?: string`

#### Barrel Export (`index.ts`)
- L1: `export { NetworkPanel } from './NetworkPanel'`
- L3: `export * from './types'`
- L4: `export * from './constants'`

#### Recommended DNS Sets (`useDnsSetup.ts`)
- `ALI_DNS` (L11): `Set<string>` 包含阿里公共 DNS 地址：`'223.5.5.5'`、`'223.6.6.6'`、`'2400:3200::1'`、`'2400:3200:baba::1'`
- `TENCENT_DNS` (L12): `Set<string>` 包含腾讯 DNSPod 地址：`'1.12.12.12'`、`'120.53.53.53'`、`'2402:4e00::'`
- `RECOMMENDED_DNS` (L13): `Set<string>` 合并上述两集合，用于「是否推荐 DNS / DoH 是否已启用」判定

### Hooks & Logic (`useDnsSetup.ts`)

#### useDnsSetup
位于 `useDnsSetup.ts` 第 22-88 行。
自定义 Hook，封装 DNS 检测与一键优化逻辑。文件 L17-21 注释明确说明与桌面版差异：**安卓无「恢复自动获取」动作、无 DNS 优化适配器门槛检查（setup 直接执行），busy 锁仅 `dohEnabling` 一态**。
- **State**:
  - `dohEnabling` (L26): `boolean`，表示 DoH 是否正在启用中（锁）
  - `dnsFamily` (L28): `DnsFamily` 类型（`'both' | 'ipv4' | 'ipv6'`），默认 `'both'`
- **Methods**:
  - `handleCheckDns` (L37-L59): 异步回调。调用 `ipc.checkDnsDohStatus()` 获取状态并写入 `useQualityStore.setDnsDohStatus`；若未检测到推荐 DNS 或存在推荐但 DoH 未启用，通过 `useLogToastStore.addLog` 记录 warning。
  - `handleSetupDnsDoh` (L61-L85): 异步回调。调用 `ipc.setupDnsDoh(dnsFamily)` 执行优化；成功则刷新状态并弹 success toast，失败弹 error toast；使用 `mountedRef` 防止卸载后状态更新；finally 重置 `dohEnabling`。
- 返回对象 (L87): `{ dnsFamily, setDnsFamily, dohEnabling, handleCheckDns, handleSetupDnsDoh }`

### UI Components (`DnsAdapterCard.tsx`, `NetworkPanel.tsx`)

#### DnsAdapterCard
位于 `DnsAdapterCard.tsx` 第 13-60 行。
导出函数组件，Props: `{ adapter: DnsAdapterInfo; autoDohEnabled: boolean }`。
- 使用 `useTranslation()`，调用 `getDnsQuality(adapter, autoDohEnabled, t)` 得 `quality`。
- 渲染适配器名 (L21) 及质量 Badge：
  - `excellent` → "DNS+DoH" (L22)
  - `good` → "DNS" (L23)
  - `basic` → `t('network.pendingOptimization')` (L24)
  - `none` → `t('network.notConfigured')` (L25)
- 来源 Badge：`adapter.dnsSource === 'profile'` 显 "perProfileDns" (L26-28)，`'manual'` 显 "perAdapterDns" (L29-31)
- 遍历 `adapter.dnsServers` 渲染 `<DnsServerRow detailed />` (L35-37)；空时显无服务器提示 (L38-40)
- 若 `adapterDnsOverridesProfile && profileDnsServers.length>0` 显 Amber 警告条 (L42-47)
- 若 `profileDnsServers` 非空，渲染配置文件级 DNS 简化列表（`<DnsServerRow />` 不带 detailed）(L49-56)

#### getDnsQuality
位于 `DnsAdapterCard.tsx` 第 63-83 行（导出函数）。
参数：`adapter` 局部类型（含 `dnsSource?`, `dnsServers`, `profileDnsServers?`, `adapterDnsOverridesProfile?`）、`autoDohEnabled: boolean`、`t: (key:string)=>string`。
逻辑：
- `effectiveServers` = `adapterDnsOverridesProfile ? servers : (servers.length>0 ? servers : profileServers)` (L75)
- 若 `effectiveServers.length === 0 || adapter.dnsSource === 'dhcp'` 返回 `{ level: 'none', label: t('network.dnsNotConfigured') }` (L77)
- `hasRecommended` = 含 `RECOMMENDED_DNS` 地址 (L78)
- `dohActive` = `autoDohEnabled ||` 推荐服务器全 `dohEnabled` (L79)
- 返回 `excellent` / `good` / `basic` 三级 (L80-82)

#### DnsServerRow
位于 `DnsAdapterCard.tsx` 第 89-120 行（文件内私有组件）。
Props: `{ dns: { address: string; dohEnabled: boolean; dohAvailable: boolean; dohTemplate: string }; detailed?: boolean }`
- 推荐地址 (`RECOMMENDED_DNS.has`) 绿色字体，并显阿里/腾讯标签 (L94-104)
- 图标：`dohEnabled` → `CheckCircle2` 绿；`dohAvailable` → `XCircle` 琥珀；否则灰 `XCircle` (L105-111)
- `detailed && dohEnabled && dohTemplate` 显模板 (L112-114)
- `detailed && !dohEnabled && dohAvailable` 显 `t('network.dohAvailable')` 提示 (L115-117)

#### NetworkPanel
位于 `NetworkPanel.tsx` 第 47-342 行（`memo` 包裹函数组件）。
Props: `{ onUpdateConfig: (partial: Partial<Config>) => void }`
- 常量：
  - `RESTORE_TIME_DEFAULT_MINUTES = 450` (L29) 对应 07:30
  - `RESTORE_TIME_MAX_MINUTES = 1379` (L32) 对应 22:59
  - `RESTORE_TIME_MIN_MINUTES = 300` (L36) 对应 05:00
  - `formatRestoreTime(minutes?)` (L39-45) 分钟数 → "HH:MM"，越界回落默认
- 状态订阅：
  - `config` 经 `useConfigStore(useShallow(...))` (L51)
  - `dnsStatus` = `useQualityStore(s=>s.dnsDohStatus)` (L62)
  - `dnsChecking` = `useQualityStore(s=>s.dnsChecking)` (L63)
  - `useDnsSetup()` 取 `dnsFamily, setDnsFamily, dohEnabling, handleCheckDns, handleSetupDnsDoh` (L65)
  - 本地 `secureStatus` (L86)、`restoring` (L87)、`adbCopied` (L73)
- Handlers：
  - `handleCopyAdbCommand` (L74-82)：复制 `t('network.avoidBadWifiCommand')` 到剪贴板，成功置 `adbCopied` 2 秒
  - `handleCopyGrantCommand` (L101-109)：复制授权命令 `t('network.avoidBadWifiGrantCommand')`
  - `refreshSecureStatus` (L88-96)：调 `ipc.getAvoidBadWifiStatus()` 写 `secureStatus`，失败留 null
  - `handleRestoreWritten` (L111-130)：调 `ipc.restoreWrittenSettings()`，按结果弹 toast，finally 刷新状态
- UI 结构：
  - **夜间出站切换卡** (L134-243)：开关绑 `config.enableNightOutboundSwitch` (L144-148)；恢复时刻 `Input type="time"` 绑 `formatRestoreTime(config.nightRestoreMinutes)`，禁用当开关关，onChange 钳制到 [300,1379] 分钟写 `nightRestoreMinutes` (L162-181)；`secureStatus?.granted` 显绿成功否显两行命令+复制按钮 (L186-222)；琥珀警告 (L223-226)；还原按钮 (L227-239)
  - **DNS 优化卡** (L245-339)：标题旁 "检测 DNS" 按钮（`dnsChecking` 禁用，显旋转）(L258-274) 与 "一键优化" 按钮（`dohEnabling` 禁用，渐变背景）(L275-292)；`SegmentTabs` 选 `dnsFamily` (L302-306)；状态区：无状态提示 (L308-314)、检测中 (L315-320)、有状态映射 `DnsAdapterCard` (L326-328)；`!dnsStatus.dohSupported` 显琥珀警告 (L329-334)

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                 NetworkPanel (NetworkPanel.tsx)             │
│  ┌──────────────────────┐   ┌──────────────────────────┐    │
│  │ Night Outbound Card  │   │   DNS Optimization Card  │    │
│  │ - Switch             │   │  - SegmentTabs           │    │
│  │ - ADB Commands       │   │  - Check/Setup Buttons   │    │
│  │ - Restore Button     │   │                          │    │
│  └─────────┬────────────┘   └──────────┬───────────────┘    │
│            │                           │                    │
│            ▼                           ▼                    │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                 useDnsSetup Hook                     │   │
│  │  - handleCheckDns / handleSetupDnsDoh               │   │
│  │  - interacts with QualityStore & LogToastStore      │   │
│  └──────────────────────────┬───────────────────────────┘   │
│                             │                               │
│                             ▼                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │           DnsAdapterCard (DnsAdapterCard.tsx)        │   │
│  │  - getDnsQuality (Logic)                            │   │
│  │  - DnsServerRow (Render)                            │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────┬──────────────────────────────────┘
                           │ IPC (tauriApiWithRetry)
                           ▼
                  Tauri Backend API
```

## Data Flow
1. **初始化与状态订阅**：`NetworkPanel` 挂载时通过 `useConfigStore` 浅订阅配置 (L51)，通过 `useQualityStore` 订阅 `dnsDohStatus` / `dnsChecking` (L62-63)；`useDnsSetup` 初始化 `dohEnabling`、`dnsFamily` (L26-L28)；`useEffect` 调 `refreshSecureStatus` 查权限 (L97-99)。
2. **DNS 检测流程**：用户点 "检测 DNS" → `handleCheckDns` (L37-59) → `ipc.checkDnsDohStatus()` → `setDnsDohStatus` 写 store → 检查推荐/DoH 状态加日志 → `NetworkPanel` 重渲 `DnsAdapterCard` 列表。
3. **DNS 优化流程**：点 "一键优化" → `handleSetupDnsDoh` (L61-85) 设 `dohEnabling=true` → `ipc.setupDnsDoh(dnsFamily)` → 成功刷状态+success toast，失败 error toast → finally 解索。
4. **适配器卡片渲染**：`NetworkPanel` 遍历 `dnsStatus.adapters` 传 `adapter` 与 `autoDohEnabled` 给 `DnsAdapterCard` (L326-328) → 内部 `getDnsQuality` 评级 → 渲染 Badge / `DnsServerRow` (detailed) → 若覆盖显警告 → 若有 `profileDnsServers` 简列。
5. **夜间切换配置**：开关改 `enableNightOutboundSwitch`；恢复时刻 Input 改 `nightRestoreMinutes` 钳制 [300,1379] (L170-178)；复制 ADB 命令静默处理；还原按钮调 `ipc.restoreWrittenSettings` 后刷 `secureStatus`。

## Connections
- [[android-frontend-settings]]: `NetworkPanel` 接收 `onUpdateConfig` 回调（L23, L146, L172）直接修改 Settings 模块管理的 `Config`
- [[android-frontend-hooks-quality]]: 依赖 `useQualityStore` 存储 DNS 检测结果（`dnsDohStatus`, `dnsChecking`）及设置方法
- [[android-frontend-hooks-log-toast]]: 使用 `useLogToastStore` 记录日志和弹出 Toast 提示（`addLog`, `addToast`）
- [[android-backend-network]]: 通过 `tauriApiWithRetry` 调用后端 IPC：`checkDnsDohStatus`、`setupDnsDoh`、`getAvoidBadWifiStatus`、`restoreWrittenSettings`

## Known Issues
1. **StrictMode 双重渲染防护**：`NetworkPanel.tsx` (L55-60) 与 `useDnsSetup.ts` (L30-35) 均使用 `mountedRef` 防止 React StrictMode 开发模式双次 setup/cleanup 导致异步回调在卸载后 setState 被丢弃，属必要防御非 Bug。
2. **剪贴板权限限制**：`handleCopyAdbCommand` (L74-82) 与 `handleCopyGrantCommand` (L101-109) 中 `navigator.clipboard.writeText` 可能被 WebView 限制，捕获异常静默，仅成功时置 `adbCopied` 提供视觉反馈。
3. **DoH 支持检测**：`NetworkPanel` 中若 `!dnsStatus.dohSupported` 显示琥珀警告 (L329-334)；后端不支持时即使配推荐 DNS 也无法一键启用 DoH。
4. **恢复时刻边界钳制**：UI 层 `RESTORE_TIME_MIN_MINUTES=300` (05:00) 比后端纯 `>0` 更紧 (L33-36)，避免凌晨补切窗口互顶；后端另有归一化兜底，输入 onChange 钳制逻辑见 L170-178。
