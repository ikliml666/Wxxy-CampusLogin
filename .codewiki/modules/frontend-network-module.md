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

`frontend-network-module` 是安卓前端网络配置模块，负责管理 DNS 设置、避免坏 Wi-Fi（系统安全设置）引导与恢复、以及夜间出站切换功能。该模块提供了展示和管理 DNS 适配器状态的卡片组件，支持 IPv4/IPv6/双栈家族选择，并集成了 DoH (DNS over HTTPS) 的检测与一键优化。模块从 `useConfigStore`、`useQualityStore` 等 Zustand store 订阅状态，底层通过 `tauriApiWithRetry` 调用后端 IPC。

## Key Components

### `constants.ts`
定义网络质量等级的视觉配置常量，虽在当前模块组件中未直接引用，但作为公共常量导出。

*   **`QUALITY_CONFIG`** (行1-11): 常量对象（`as const`），定义 9 种质量状态：`excellent`、`great`、`good`、`fair`、`poor`、`bad`、`unknown`、`disabled`、`busy`。每种包含字段：
    *   `label`: 中文标签（如 '极速'、'优秀'、'良好'、'一般'、'较慢'、'拥堵'、'未知'、'已禁用'、'检测中'）
    *   `labelKey`: i18n 键（如 `quality.excellent`）
    *   `color`、`bg`、`border`、`borderBg`: Tailwind 样式类
    *   `icon`: Lucide 图标名（如 `Zap`、`Activity`、`AlertTriangle`、`HelpCircle`、`Ban`、`Loader2`）
    *   `hex`: 十六进制颜色
    *   `activeBars`: 信号条数量（0-5）
    *   `glow`: 发光 rgba 色

### `types.ts`
定义网络相关 TypeScript 接口与类型。

*   **`AdapterStatus`** (行1): 联合类型 `'disabled' | 'disconnected' | 'enabledNoIp' | 'connected'`。
*   **`Adapter`** (行3-13): 适配器基础信息接口。字段：
    *   `name: string`
    *   `ip: string`
    *   `wireless: boolean`
    *   `guid?: string`
    *   `mac: string`
    *   `ifIndex: number`
    *   `status: AdapterStatus`
    *   `linkSpeed?: number` (连接速度 bit/s，0 未知)
*   **`AdapterDetail`** (行15-27): 适配器详细信息接口。字段：
    *   `name: string`
    *   `ip: string`
    *   `wireless: boolean`
    *   `subnetMask: string`
    *   `gateway: string`
    *   `dhcpServer: string`
    *   `mac: string`
    *   `ifIndex: number`
    *   `status: AdapterStatus`
    *   `linkSpeed?: number`
*   **`DnsServerInfo`** (行29-34): DNS 服务器信息接口。字段：
    *   `address: string`
    *   `dohAvailable: boolean`
    *   `dohEnabled: boolean`
    *   `dohTemplate: string`
*   **`DnsAdapterInfo`** (行36-42): DNS 适配器信息接口。字段：
    *   `name: string`
    *   `dnsSource: string`
    *   `dnsServers: DnsServerInfo[]`
    *   `profileDnsServers: DnsServerInfo[]`
    *   `adapterDnsOverridesProfile: boolean`
*   **`DnsDohStatus`** (行44-49): DNS/DoH 整体状态接口。字段：
    *   `adapters: DnsAdapterInfo[]`
    *   `dohSupported: boolean`
    *   `autoDohEnabled: boolean`
    *   `dnsSource?: string`
*   **`DhcpRenewResult`** (行51-54): DHCP 续租结果接口。字段：
    *   `success: boolean`
    *   `results: { name: string; success: boolean }[]`
*   **`DhcpReleaseRenewResult`** (行56-59): DHCP 释放续租结果接口。字段：
    *   `success: boolean`
    *   `results: { name: string; wireless: boolean; ip: string; regOk: boolean; success: boolean; skipped: boolean; reason: string | null }[]`
*   **`DnsSetupResult`** (行61-68): DNS 设置结果接口。字段：
    *   `success: boolean`
    *   `message: string`
    *   `dnsSuccess?: string[]`
    *   `dnsFailed?: string[]`
    *   `dohAdded?: string[]`
    *   `dohFailed?: string[]`
*   **`EnableAdapterResult`** (行70-73): 启用适配器结果接口。字段：
    *   `success: boolean`
    *   `message?: string`

### `useDnsSetup.ts`
提供 DNS 检测与一键优化的 Hook 逻辑，封装业务规则。

*   **`ALI_DNS`** (行11): 导出 `Set<string>`，包含阿里公共 DNS 地址：`223.5.5.5`, `223.6.6.6`, `2400:3200::1`, `2400:3200:baba::1`。
*   **`TENCENT_DNS`** (行12): 内部 `Set<string>`（未导出），包含腾讯 DNSPod 地址：`1.12.12.12`, `120.53.53.53`, `2402:4e00::`。
*   **`RECOMMENDED_DNS`** (行13): 导出 `Set<string>`，合并 `ALI_DNS` 与 `TENCENT_DNS`，用于推荐 DNS 判定。
*   **`DnsFamily`** (行15): 导出类型 `'both' | 'ipv4' | 'ipv6'`。
*   **`useDnsSetup()`** (行22-88): React Hook，管理 DNS 优化状态与动作。
    *   状态：`dohEnabling` (行26, `useState(false)`)，`dnsFamily` (行28, `useState<DnsFamily>('both')`)。
    *   Effect (行30-35): StrictMode 下重置 `mountedRef`。
    *   方法 `handleCheckDns` (行37-59): 调用 `ipc.checkDnsDohStatus()`，更新 `useQualityStore` 的 `dnsChecking` 与 `dnsDohStatus`，并根据结果写日志。
    *   方法 `handleSetupDnsDoh` (行61-85): 调用 `ipc.setupDnsDoh(dnsFamily)`，成功后 Toast 并刷新状态，失败报错。
    *   返回 (行87): `{ dnsFamily, setDnsFamily, dohEnabling, handleCheckDns, handleSetupDnsDoh }`。
    *   注释指明安卓无「恢复自动获取」动作、无 DNS 优化适配器门槛检查（行18-21）。

### `DnsAdapterCard.tsx`
渲染单个 DNS 适配器详情的 UI 组件（安卓简单列表布局）。

*   **`DnsAdapterCard`** (行13-60): 导出函数组件。
    *   Props: `{ adapter: DnsAdapterInfo; autoDohEnabled: boolean }` (行13)。
    *   逻辑：调用 `getDnsQuality` (行15) 计算质量徽章；映射 `adapter.dnsServers` 到 `DnsServerRow` (行35-37)；显示配置文件级 DNS (行49-55) 及适配器覆盖配置警告 (行42-47)。
*   **`getDnsQuality`** (行63-83): 导出纯函数，参数 `(adapter: {dnsSource?; dnsServers; profileDnsServers?; adapterDnsOverridesProfile?}, autoDohEnabled: boolean, t)`。根据有效 DNS 服务器判定等级：`none`/`excellent`/`good`/`basic` (行77-82)。
*   **`DnsServerRow`** (行89-120): 内部函数组件，Props `{ dns: {address; dohEnabled; dohAvailable; dohTemplate}, detailed?: boolean }`。渲染地址、供应商标签（阿里/腾讯）、DoH 状态图标；`detailed=true` 时显示 DoH 模板或“可启用”提示 (行112-117)。

### `NetworkPanel.tsx`
主面板组件，整合夜间出站开关、避免坏 Wi-Fi 引导、DNS 优化。

*   **`RESTORE_TIME_DEFAULT_MINUTES`** (行29): 常量 `450`（07:30），与后端默认同值。
*   **`RESTORE_TIME_MAX_MINUTES`** (行32): 常量 `1379`（22:59）。
*   **`RESTORE_TIME_MIN_MINUTES`** (行36): 常量 `300`（05:00）。
*   **`formatRestoreTime`** (行39-45): 函数，分钟数转 "HH:MM"，越界回落默认。
*   **`NetworkPanel`** (行47-342): `memo` 组件，Props `{ onUpdateConfig: (partial: Partial<Config>) => void }` (行22-24)。
    *   状态订阅：`config` (行51, `useConfigStore` shallow)，`dnsStatus` (行62), `dnsChecking` (行63, `useQualityStore`)，`dnsFamily` 等来自 `useDnsSetup` (行65)；本地状态 `adbCopied` (行73), `secureStatus` (行86), `restoring` (行87)。
    *   Effect: 行55-60 重置 `mountedRef`；行97-99 初始刷新 `secureStatus`。
    *   Handlers:
        *   `handleCopyAdbCommand` (行74-82): 复制 `t('network.avoidBadWifiCommand')` 到剪贴板。
        *   `refreshSecureStatus` (行88-96): 调用 `ipc.getAvoidBadWifiStatus()`。
        *   `handleCopyGrantCommand` (行101-109): 复制授权命令。
        *   `handleRestoreWritten` (行111-130): 调用 `ipc.restoreWrittenSettings()` 并 Toast。
    *   UI 结构：
        *   夜间出站开关卡 (行134-243)：含 `Switch` (行144)、恢复时刻 `Input` (行162-181, 当 `!config.enableNightOutboundSwitch` 时 `disabled` 置灰)、安全设置引导与还原按钮。
        *   DNS 优化卡 (行245-339)：含检测/优化按钮、`SegmentTabs` 家族切换 (行302)、`DnsAdapterCard` 列表 (行326-328)、DoH 不支持警告 (行329-334)。

### `index.ts`
模块入口。

*   **Exports** (行1-4): 导出 `NetworkPanel` (行1)，`types` (行3)，`constants` (行4)。

## Architecture

```text
┌─────────────────────────────────────────────────────┐
│ NetworkPanel (NetworkPanel.tsx)                     │
│  ├─ NightOutboundCard (行134-243)                    │
│  └─ DnsOptimizationCard (行245-339)                 │
└───────────────┬───────────────────────┬─────────────┘
                │                       │
        ┌───────▼────────┐      ┌───────▼─────────┐
        │ DnsAdapterCard │      │ useDnsSetup     │
        │ (DnsAdapterCard│      │ (useDnsSetup.ts)│
        │   .tsx)        │      └───────┬─────────┘
        └───────┬────────┘              │
                │                      │
                └──────────┬───────────┘
                          ▼
                 tauriApiWithRetry (IPC)
```

1. `NetworkPanel` 顶层管理配置与安全状态，渲染两张卡片。
2. DNS 优化卡依赖 `useDnsSetup` Hook 处理检测/优化逻辑。
3. `DnsAdapterCard` 仅按父组件传入的 `adapter` 与 `autoDohEnabled` 渲染，不直接调用 API。
4. 所有 IPC 经由 `tauriApiWithRetry` 重试封装。

## Data Flow

1. **初始化**:
   - `NetworkPanel` 挂载 (行47)，从 `useConfigStore` 读 `config` (行51)。
   - 调用 `refreshSecureStatus` (行88) -> `ipc.getAvoidBadWifiStatus()` 填充 `secureStatus` (行91)。
   - `useDnsSetup` 初始化 `dnsFamily='both'` (行28)。

2. **DNS 检测流程**:
   - 用户点击检测 (行261 `handleCheckDns`) -> `useQualityStore.setDnsChecking(true)` (行38)。
   - `ipc.checkDnsDohStatus()` (行40)，成功则 `setDnsDohStatus(status)` (行41)。
   - 空状态显示提示 (行308-314)；结果映射 `DnsAdapterCard` (行326-328)。

3. **DNS 优化流程**:
   - 点击一键优化 (行278 `handleSetupDnsDoh`) -> `setDohEnabling(true)` (行62)。
   - `ipc.setupDnsDoh(dnsFamily)` (行64)，成功 Toast (行67) 并重新检测刷新 (行70)。
   - 结束 `setDohEnabling(false)` (行83)。

4. **夜间开关与恢复时刻**:
   - 切换 `Switch` (行144) -> `onUpdateConfig({ enableNightOutboundSwitch: checked })` (行146)。
   - 恢复时刻 `Input` (行162) 修改时钳制到 [300,1379] 分钟 (行173-176)，开关关闭时 `disabled` (行166)。
   - 无权限时显示 adb 命令复制 (行195-220)；有历史写入则显示还原按钮 (行229-238) 调用 `ipc.restoreWrittenSettings()`。

## Connections

* [[useConfigStore]]: 提供全局配置 `enableNightOutboundSwitch`、`nightRestoreMinutes` 等。
* [[useQualityStore]]: 共享 DNS 状态 `dnsDohStatus`、`dnsChecking`。
* [[useLogToastStore]]: 弹出 Toast 与日志（如行118、行67）。
* [[tauriApi]]: 底层 IPC 封装 `tauriApiWithRetry`，方法含 `getAvoidBadWifiStatus`、`restoreWrittenSettings`、`checkDnsDohStatus`、`setupDnsDoh`。
* [[SegmentTabs]]: IPv4/IPv6/双栈切换 UI（行302）。
* [[AnimatedCard]]: 面板卡片容器（行135、246）。
* [[lib/utils]]: `cn`、`extractErrorMessage` 工具（行9、行3）。
* [[react-i18next]]: 国际化 `useTranslation`（多文件）。
