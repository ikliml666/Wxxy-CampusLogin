---
title: Android 前端核心模块 (Frontend Core)
type: module
source_files:
  - android/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/network/useDnsSetup.ts
  - android/frontend/src/auth/DashboardPanel.tsx
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/monitor/LatencyTimeline.tsx
  - android/frontend/src/main.tsx
  - android/frontend/src/shared/ConfirmDialog.tsx
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
  - android/frontend/vite.mock.config.ts
tags:
  - architecture
  - conventions
  - error-handling
  - state-management
---

## Overview

本仓库在 Android Tauri 应用开发中形成了一套高度一致的工程化模式。这些模式主要围绕 **Tauri IPC 通信的可靠性**、**Zustand 状态管理的精细化控制**、**Android 系统级资源的降级与适配** 以及 **前端构建环境的 Mock 隔离** 展开。通过将这些模式标准化，项目能够在一个复杂的跨平台（Rust/Kotlin/JS）栈中保持代码的可维护性和稳定性。以下总结了仓库中反复出现的核心模式及其具体实现证据。

## Key Components

### Network Module (`android/frontend/src/network/`)

#### `NetworkPanel` Component
- **文件**: `android/frontend/src/network/NetworkPanel.tsx`
- **行号**: 32-216
- **用途**: 网络设置面板，包含夜间出站开关和 DNS 优化功能。
- **Props**: 
  - `onUpdateConfig`: `(partial: Partial<Config>) => void` (L30)
- **内部状态**:
  - `config`: 通过 `useConfigStore(useShallow(...))` 订阅 (L34)
  - `dnsStatus`, `dnsChecking`: 通过 `useQualityStore` 订阅 (L45-46)
  - `adbCopied`: 剪贴板复制反馈状态 (L58)
  - `secureStatus`: `AvoidBadWifiStatus | null`，Secure Settings 授权状态 (L69)
  - `restoring`: 恢复写入状态 (L70)
- **关键函数**:
  - `refreshSecureStatus`: 获取 Secure Settings 状态 (L71-81)
  - `handleCopyAdbCommand`: 复制 ADB 引导命令 (L61-68)
  - `handleCopyGrantCommand`: 复制授权命令 (L84-91)
  - `handleRestoreWritten`: 恢复 Secure Settings 写入 (L93-113)

#### `useDnsSetup` Hook
- **文件**: `android/frontend/src/network/useDnsSetup.ts`
- **行号**: 24-93
- **用途**: DNS 检测与一键优化逻辑。
- **导出常量**:
  - `ALI_DNS`: Set of recommended Ali DNS addresses (L13)
  - `RECOMMENDED_DNS`: Set of all recommended DNS addresses (L15)
  - `DnsFamily`: Type `'both' | 'ipv4' | 'ipv6'` (L17)
- **内部状态**:
  - `dohEnabling`: DoH 启用中状态 (L28)
  - `dnsFamily`: DNS 优化目标 (L30)
- **关键函数**:
  - `handleCheckDns`: 检查 DNS 状态，判断是否推荐及 DoH 是否启用 (L35-55)
  - `handleSetupDnsDoh`: 执行 DNS 优化，刷新状态 (L57-81)

### Auth & Dashboard Module (`android/frontend/src/auth/`)

#### `DashboardPanel` Component
- **文件**: `android/frontend/src/auth/DashboardPanel.tsx`
- **行号**: 293-436
- **用途**: 仪表盘主面板，包含多种卡片布局。
- **Props**:
  - `accounts`: `AccountItem[]`
  - `activeAccount`: `string`
  - `onUpdateConfig`: `(partial: Partial<Config>) => void`
  - `onSwitchAccount`: `(name: string) => Promise<any>`
  - `onDhcpRenew`: `() => Promise<void>`
  - `onDhcpReleaseRenew`: `() => Promise<void>`
  - `excludeCards`: `CardId[]`
  - `mergeSelfServiceEmpty`: `boolean`
  - `extraCards`: `ExtraCardDef[]`
- **内部状态**:
  - `cards`: `CardId[]`，布局状态 (L315)
  - `editing`: `boolean`，编辑模式状态 (L322)
- **关键函数**:
  - `handleAddCard`: 添加卡片 (L327)
  - `handleRemoveCard`: 移除卡片 (L332)
  - `handleSwapCard`: 交换卡片位置 (L346)

#### `QuickActionsCard` Component
- **文件**: `android/frontend/src/auth/DashboardPanel.tsx`
- **行号**: 118-176
- **用途**: 快速操作卡片，包含 DHCP 续租和获取新 IP。
- **Props**:
  - `networkQuality`: `NetworkQuality | null`
  - `onDhcpRenew`: `() => Promise<void>`
  - `onDhcpReleaseRenew`: `() => Promise<void>`
  - `noAnimation`, `noEnterAnimation`: `boolean`

#### `AccountManageCard` Component
- **文件**: `android/frontend/src/auth/DashboardPanel.tsx`
- **行号**: 178-246
- **用途**: 账号管理卡片，支持切换账号。
- **Props**:
  - `accounts`: `AccountItem[]`
  - `activeAccount`: `string`
  - `onSwitchAccount`: `(name: string) => Promise<any>`
  - `noAnimation`, `noEnterAnimation`: `boolean`

#### `NetworkQualityCard` Component
- **文件**: `android/frontend/src/auth/DashboardPanel.tsx`
- **行号**: 248-291
- **用途**: 网络质量卡片，显示延迟信息。
- **Props**:
  - `networkQuality`: `NetworkQuality | null`
  - `isRefreshingQuality`: `boolean`
  - `onRefreshQuality`: `() => Promise<void>`
  - `noAnimation`, `noEnterAnimation`: `boolean`

#### `SelfOnlineCard` Component
- **文件**: `android/frontend/src/auth/DashboardPanel.tsx`
- **行号**: 346-473
- **用途**: 自助在线信息卡片，支持掩码显示和设备别名。
- **Props**:
  - `noAnimation`, `noEnterAnimation`: `boolean`
- **内部状态**:
  - `offlineTarget`: `SelfOnlineItem | null`
  - `offlineBusy`: `boolean`
  - `aliasTarget`: `SelfOnlineItem | null`
- **关键函数**:
  - `handleSaveAlias`: 保存设备别名 (L373)
  - `fetchOnline`: 获取在线列表 (L383)
  - `handleOffline`: 注销会话 (L396)

#### `SelfLogCard` Component
- **文件**: `android/frontend/src/auth/DashboardPanel.tsx`
- **行号**: 475-547
- **用途**: 自助上网记录卡片。
- **Props**:
  - `noAnimation`, `noEnterAnimation`: `boolean`

#### `EditableCardItem` Component
- **文件**: `android/frontend/src/auth/DashboardPanel.tsx`
- **行号**: 593-664
- **用途**: 编辑模式下的可拖动卡片项。
- **Props**:
  - `id`: `string`
  - `onRemove`: `() => void`
  - `removeLabel`: `string`
  - `onSwap`: `(id: string, dir: -1 | 1) => void`
  - `children`: `React.ReactNode`

#### `useAuth` Hook
- **文件**: `android/frontend/src/auth/useAuth.ts`
- **行号**: 7-28
- **用途**: 认证相关逻辑封装。
- **返回对象**:
  - `isLoggingIn`, `isLoggingOut`, `status`: 来自 `useAuthStore`
  - `doLogin`, `doLogout`, `checkOnline`: 来自 `useAuthStore`
  - `handleOpenPortal`: 打开认证门户 (L15)
  - `handleOpenSelfService`: 打开自助服务页 (L20)

### Monitor Module (`android/frontend/src/monitor/`)

#### `LatencyTimeline` Component
- **文件**: `android/frontend/src/monitor/LatencyTimeline.tsx`
- **行号**: 24-135
- **用途**: 延迟时间线可视化组件。
- **Props**:
  - `totalMs`: `number`
  - `dnsMs`, `tcpMs`, `tlsMs`, `udpMs`, `networkMs`, `ttfbMs`, `contentMs`: `number` (可选)
  - `className`: `string`

### Shared Module (`android/frontend/src/shared/`)

#### `ConfirmDialog` Component
- **文件**: `android/frontend/src/shared/ConfirmDialog.tsx`
- **行号**: 13-53
- **用途**: 通用确认对话框。
- **Props**:
  - `open`: `boolean`
  - `title`: `string`
  - `message`: `string`
  - `onConfirm`: `() => void`
  - `onCancel`: `() => void`
  - `confirmLabel`: `string` (可选)
  - `confirmVariant`: `'default' | 'destructive'` (默认 `'destructive'`)

### Main Entry (`android/frontend/src/`)

#### `main.tsx`
- **文件**: `android/frontend/src/main.tsx`
- **行号**: 1-125
- **用途**: 应用入口，初始化主题、崩溃恢复机制。
- **关键函数**:
  - `initTheme`: 初始化主题 (L38)
  - `setupCrashRecovery`: 设置崩溃恢复机制 (L50)

### Keystore Plugin (`android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/`)

#### `KeystorePlugin` Class
- **文件**: `android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt`
- **行号**: 26-85
- **用途**: Android Keystore 插件，提供加密和解密功能。
- **Companion Object**:
  - `KEY_ALIAS`: `"campus_login_master"` (L31)
  - `IV_LEN`: `12` (L32)
  - `TAG`: `"CampusKeystore"` (L33)
- **Methods**:
  - `getOrCreateKey`: 获取或创建 AES 密钥 (L35)
  - `encrypt`: 加密命令 (L47)
  - `decrypt`: 解密命令 (L60)

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     main.tsx (Entry)                        │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │ initTheme()  │   │ setupCrashReco.  │   │ AppWrapper  │ │
│  └──────────────┘   └──────────────────┘   └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     DashboardPanel                          │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │ QuickActions │   │ AccountManage    │   │ SelfOnline  │ │
│  └──────────────┘   └──────────────────┘   └─────────────┘ │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │ SelfLog      │   │ NetworkQuality   │   │ EditableCard│ │
│  └──────────────┘   └──────────────────┘   └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     NetworkPanel                            │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │ NightSwitch  │   │ DnsOptimization  │   │ SecureState │ │
│  └──────────────┘   └──────────────────┘   └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     useDnsSetup Hook                        │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │ handleCheck  │   │ handleSetup      │   │ dnsFamily   │ │
│  └──────────────┘   └──────────────────┘   └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Tauri IPC / Keystore                      │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │ tauriApi     │   │ KeystorePlugin   │   │ LatencyTime │ │
│  │ WithRetry    │   │ (Encrypt/Decrypt)│   │ Line        │ │
│  └──────────────┘   └──────────────────┘   └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Initialization**: `main.tsx` initializes theme and crash recovery, then renders `App`.
2.  **Dashboard Rendering**: `DashboardPanel` loads layout from storage, renders cards based on `cards` state.
3.  **Card Interaction**: User interacts with cards (e.g., switch account, check DNS).
4.  **State Updates**: Zustand stores (`useAuthStore`, `useConfigStore`, `useQualityStore`) are updated via hooks.
5.  **IPC Communication**: `tauriApiWithRetry` wraps Tauri commands, handling retries and errors.
6.  **Security**: Sensitive data is encrypted/decrypted using `KeystorePlugin`.
7.  **Feedback**: UI updates reflect state changes, including loading states and error messages.

## Connections

- [[main.tsx]] -> [[DashboardPanel]]
- [[DashboardPanel]] -> [[NetworkPanel]]
- [[NetworkPanel]] -> [[useDnsSetup]]
- [[useDnsSetup]] -> [[tauriApiWithRetry]]
- [[KeystorePlugin]] -> [[DashboardPanel]] (via secure data handling)
- [[LatencyTimeline]] -> [[DashboardPanel]] (via network quality card)
- [[ConfirmDialog]] -> [[DashboardPanel]] (via self online card offline action)
- [[vite.mock.config.ts]] -> [[main.tsx]] (development environment)