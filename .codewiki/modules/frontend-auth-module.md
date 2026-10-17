---
title: frontend-auth-module
type: module
source_files:
  - android/frontend/src/auth/AboutDialog.tsx
  - android/frontend/src/auth/AboutDialogMobile.tsx
  - android/frontend/src/auth/index.ts
  - android/frontend/src/auth/types.ts
  - android/frontend/src/auth/useAuth.ts
tags: [authentication, dashboard, ui, about-dialog, mobile]
---

## Overview
`frontend-auth-module` 是校园网登录应用的前端认证与核心交互模块。它封装了用户身份验证的生命周期（登录/登出/在线状态检查），并提供了两个主要的 UI 视图：
1. **DashboardPanel**：主仪表盘，包含快捷操作、账号管理、网络质量监控及自助服务信息。
2. **AboutDialog / AboutDialogMobile**：关于对话框，处理版本更新检查、下载、安装以及应用赞助信息。

该模块通过 `useAuth` Hook 暴露认证状态和操作，并通过 Zustand store (`useAuthStore`, `useConfigStore`) 与全局状态同步。

## Key Components

### `useAuth` (Hook)
- **文件**: `android/frontend/src/auth/useAuth.ts`
- **行号**: 6-35
- **描述**: 组合认证状态和辅助操作的自定义 Hook。
- **导出函数**:
  - `useAuth()`: 返回包含以下属性的对象：
    - `isLoggingIn` (boolean): 是否正在登录。
    - `isLoggingOut` (boolean): 是否正在登出。
    - `status` (object): 当前认证状态对象。
    - `doLogin` (function): 执行登录操作。
    - `doLogout` (function): 执行登出操作。
    - `checkOnline` (function): 检查在线状态。
    - `handleOpenPortal` (function): 打开认证门户页面。
    - `handleOpenSelfService` (function): 打开自助服务页面。

### `DashboardPanel` (Component)
- **文件**: `android/frontend/src/auth/DashboardPanel.tsx`
- **行号**: 480-605
- **描述**: 主仪表盘组件，由多个卡片组成。
- **Props**:
  - `accounts`: `AccountItem[]` - 账号列表。
  - `activeAccount`: `string` - 当前活跃账号 ID。
  - `onUpdateConfig`: `(partial: Partial<Config>) => void` - 配置更新回调。
  - `onSwitchAccount`: `(name: string) => Promise<any>` - 切换账号回调。
  - `onDhcpRenew`: `() => Promise<void>` - DHCP 续租回调。
  - `onDhcpReleaseRenew`: `() => Promise<void>` - DHCP 释放并续租回调。
  - `onRefreshQuality`: `() => Promise<void>` - 刷新网络质量回调。
  - `onToggleBackgroundCheck`: `(enabled: boolean, intervalSec: number) => Promise<void>` - 后台检查开关回调。
  - `excludeCards`: `CardId[]` - 需要排除的卡片 ID 列表。
  - `mergeSelfServiceEmpty`: `boolean` - 是否合并自助服务空态卡片。
  - `extraCards`: `ExtraCardDef[]` - 宿主注入的自定义卡片定义。

#### 内部子组件
- **`QuickActionsCard`** (行 96-156): 快捷操作卡片，包含 DHCP 续租和获取新 IP 按钮。
- **`AccountManageCard`** (行 158-227): 账号管理卡片，显示当前账号和其他可切换账号。
- **`NetworkQualityCard`** (行 229-273): 网络质量卡片，显示网关和外部延迟。
- **`SelfOnlineCard`** (行 333-455): 自助服务在线设备卡片，支持设备别名管理和下线操作。
- **`SelfLogCard`** (行 457-527): 自助服务日志卡片，显示近期上网记录。
- **`SelfServiceMergedEmptyCard`** (行 529-549): 移动端合并的空态卡片。
- **`EditableCardItem`** (行 585-652): 编辑模式下的卡片项，支持长按拖动排序。

#### 工具函数/Hooks
- **`useSelfCardReveal`** (行 275-288): 控制自助服务敏感信息的显示/隐藏，需通过 Hello 门控验证。
- **`useSelfCardFetch`** (行 290-331): 通用的自助服务数据获取 Hook，处理加载、错误和自动重试。
- **`renderCard`** (行 551-578): 根据卡片 ID 渲染对应卡片的工厂函数。
- **`loadLayout`** (行 65-73): 从本地存储加载布局配置。
- **`saveLayout`** (行 75-77): 保存布局配置到本地存储。
- **`loadRemovedExtras`** (行 79-86): 加载被删除的额外卡片黑名单。

### `AboutDialog` (Component)
- **文件**: `android/frontend/src/auth/AboutDialog.tsx`
- **行号**: 1-435
- **描述**: 桌面版关于对话框，包含应用信息、更新检查和下载功能。
- **Props**:
  - `open`: `boolean` - 对话框打开状态。
  - `onClose`: `() => void` - 关闭回调。
  - `openExternal`: `(url: string) => void` - 打开外部链接回调。
  - `onUpdateAvailable`: `(hasUpdate: boolean, latestVersion?: string, releaseNotes?: string) => void` - 更新可用回调。
  - `initialLatestVersion`: `string` - 初始最新版本号。
  - `initialReleaseNotes`: `string` - 初始更新说明。
  - `initialUpdateAvailable`: `boolean` - 初始是否有更新。

#### 内部逻辑
- **`handleCheckUpdate`** (行 83-100): 检查更新，处理 API 限制和错误。
- **`handleDownload`** (行 116-136): 下载更新包，监听进度。
- **`handleInstall`** (行 143-152): 安装更新包。
- **`handleQuickDownload`** (行 162-186): 快速下载，自动选择镜像源。
- **`ensureFullUpdateInfo`** (行 138-148): 确保更新信息完整，特别是 SHA256 校验和。
- **`renderNotesLines`** (行 53-80): 解析 Markdown 格式的更新说明。
- **`formatSize`** (行 33-38): 格式化文件大小。
- **`formatSpeed`** (行 40-45): 格式化下载速度。

### `AboutDialogMobile` (Component)
- **文件**: `android/frontend/src/auth/AboutDialogMobile.tsx`
- **行号**: 1-200
- **描述**: 移动版关于对话框，简化为竖版布局，支持 APK 下载和安装。
- **Props**:
  - `open`: `boolean`
  - `onClose`: `() => void`
  - `openExternal`: `(url: string) => void`
  - `onUpdateAvailable`: `(hasUpdate: boolean, latestVersion?: string, releaseNotes?: string) => void`

#### 内部逻辑
- **`handleCheckUpdate`** (行 36-52): 检查更新。
- **`handleDownload`** (行 60-88): 下载 APK，支持镜像源重试。
- **`handleInstall`** (行 90-98): 安装 APK。

### Types & Exports
- **文件**: `android/frontend/src/auth/types.ts`
- **行号**: 1-15
- **接口**:
  - `PortalStatusResult`: 门户状态结果。
  - `CommandResult`: 命令执行结果。
  - `LoginResult`: 登录结果，继承自 `CommandResult`。

- **文件**: `android/frontend/src/auth/index.ts`
- **行号**: 1-8
- **导出**:
  - `DashboardPanel`
  - `AboutDialog`
  - `useAuth`
  - `types` 中的所有类型

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      App Shell                             │
│  (Manages Route, Global State, Window Resize)               │
└───────────────┬───────────────────────────────┬─────────────┘
                │                               │
                ▼                               ▼
┌──────────────────────────────┐ ┌──────────────────────────────┐
│       DashboardPanel         │ │     AboutDialog/Mobile       │
│  (android/frontend/src/auth/ │ │     (android/frontend/src/   │
│   DashboardPanel.tsx)        │ │    auth/AboutDialog*.tsx)    │
│                              │ │                              │
│  - QuickActionsCard          │ │  - Update Check              │
│  - AccountManageCard         │ │  - Download Manager          │
│  - NetworkQualityCard        │ │  - Sponsor Info              │
│  - SelfOnlineCard            │ │                              │
│  - SelfLogCard               │ │                              │
└───────────────┬──────────────┘ └───────────────┬──────────────┘
                │                               │
                ▼                               ▼
┌─────────────────────────────────────────────────────────────┐
│                   Shared Hooks & Stores                      │
│  - useAuth (auth/useAuth.ts)                                │
│  - useAuthStore (zustand)                                   │
│  - useConfigStore (zustand)                                 │
│  - tauriApiWithRetry (tauri hooks)                          │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Authentication**:
   - `useAuth` hook calls `api.doLogin` or `api.doLogout`.
   - Status updates are pushed to `useAuthStore`.
   - `DashboardPanel` subscribes to `useAuthStore` to display login status and handle account switching.

2. **Dashboard Updates**:
   - `DashboardPanel` receives `accounts` and `activeAccount` as props from the parent.
   - `AccountManageCard` uses these props to render the current account and switch options.
   - `QuickActionsCard` triggers network operations (DHCP renew) via callbacks passed from the parent.
   - `NetworkQualityCard` fetches latency data using `useQualityStore`.

3. **Self-Service Data**:
   - `SelfOnlineCard` and `SelfLogCard` use `useSelfCardFetch` to query backend APIs.
   - Sensitive data is masked by default and revealed only after passing the `useHelloGate` verification.
   - Data is refreshed periodically (every 60s for online devices) or on demand.

4. **Update Process**:
   - `AboutDialog` triggers `api.checkUpdate`.
   - If an update is available, it displays release notes.
   - User initiates download via `handleDownload` or `handleQuickDownload`.
   - Progress is monitored via `api.onDownloadProgress`.
   - Upon completion, user clicks install, triggering `api.installUpdate`.

## Connections

- [[AppShell]]: Parent component that renders `DashboardPanel` and `AboutDialog`.
- [[useAuthStore]]: Zustand store managing authentication state, used by `useAuth` and `DashboardPanel`.
- [[useConfigStore]]: Zustand store managing app configuration, used by `DashboardPanel` and `AboutDialogMobile`.
- [[tauriApi]]: Tauri API wrapper providing native functionality like file I/O, process management, and external URL opening.