---
title: frontend-auth-dashboard
type: module
source_files:
  - android/frontend/src/auth/AboutDialog.tsx
  - android/frontend/src/auth/AboutDialogMobile.tsx
  - android/frontend/src/auth/DashboardPanel.tsx
  - android/frontend/src/auth/index.ts
  - android/frontend/src/auth/types.ts
  - android/frontend/src/auth/useAuth.ts
tags: [认证, 仪表盘, 关于对话框, 移动端适配, 卡片编辑]
---

## Overview
该模块负责校园网认证客户端的核心用户界面展示与交互入口。它包含三个核心部分：
1. **DashboardPanel**：主仪表盘，提供快捷操作、账号管理、网络质量监控及自助服务状态查看。支持自定义卡片布局与编辑模式。
2. **AboutDialog**：桌面端“关于”对话框，处理应用版本检查、更新下载、安装引导及赞助信息。
3. **AboutDialogMobile**：移动端精简版“关于”对话框，针对竖屏优化，提供更新源切换与应用内 APK 安装。

此外，`useAuth` Hook 提供了认证状态管理与门户/自助服务页面的跳转逻辑。

## Key Components

### DashboardPanel (DashboardPanel.tsx)
主容器组件，管理仪表盘的卡片布局、编辑状态及数据渲染。

- **Props**:
  - `accounts`: `AccountItem[]` - 已保存的账号列表。
  - `activeAccount`: `string` - 当前激活账号 ID。
  - `onUpdateConfig`: `(partial: Partial<Config>) => void` - 配置更新回调。
  - `onSwitchAccount`: `(name: string) => Promise<any>` - 切换账号回调。
  - `onDhcpRenew`: `() => Promise<void>` - DHCP 续租回调。
  - `onDhcpReleaseRenew`: `() => Promise<void>` - 释放并重新获取 IP 回调。
  - `onRefreshQuality`: `() => Promise<void>` - 刷新网络质量回调（可选）。
  - `onToggleBackgroundCheck`: `(enabled: boolean, intervalSec: number) => Promise<void>` - 后台检查开关（可选）。
  - `excludeCards`: `CardId[]` - 需排除的卡片 ID 数组（用于移动端过滤 Windows 专属功能）。
  - `mergeSelfServiceEmpty`: `boolean` - 是否合并自助服务空态卡片（移动端优化）。
  - `extraCards`: `ExtraCardDef[]` - 宿主注入的自定义卡片定义。

- **内部状态与逻辑**:
  - `cards`: `CardId[]` - 当前显示的卡片 ID 列表，通过 `safeStorage.get('campus-dashboard-layout')` 持久化。
  - `editing`: `boolean` - 是否处于编辑模式。
  - `handleAddCard(id)`: 添加卡片到布局末尾。若为 extra 卡，从黑名单移除。
  - `handleRemoveCard(id)`: 从布局中移除卡片。若为 extra 卡，加入 `EXTRA_REMOVED_KEY` 黑名单以防重启复活。
  - `handleSwapCard(id, dir)`: 在编辑模式下交换相邻可见卡片位置。
  - `visibleCards`: 经过 `excludeCards` 和 `config.enableNetworkQuality` 过滤后的最终卡片列表。
  - `mergeSelfEmpty` 逻辑: 当无自助凭据且启用合并时，跳过 `selfLog` 卡片，仅渲染 `SelfServiceMergedEmptyCard` 替代 `selfOnline`。

### QuickActionsCard (DashboardPanel.tsx)
快捷操作卡片，包含 DHCP 续租和获取新 IP 功能。

- **Props**:
  - `networkQuality`: `NetworkQuality | null` - 当前网络质量，用于触发危险光效。
  - `onDhcpRenew`, `onDhcpReleaseRenew`: 操作回调。
  - `noAnimation`, `noEnterAnimation`: 控制动画禁用。

- **方法**:
  - `handleDhcpRenew`: 使用 `useAsyncLock` 防止重复点击，调用 `onDhcpRenew`。
  - `handleGetNewIp`: 使用 `useAsyncLock` 防止重复点击，调用 `onDhcpReleaseRenew`。
  - **UI 特效**: 当网络质量为 'poor' 或 'bad' 时，通过 `useGlowAnimation` 显示红色光晕边框。

### AccountManageCard (DashboardPanel.tsx)
账号管理卡片，使用堆叠卡片 UI 展示多个账号。

- **Props**:
  - `accounts`, `activeAccount`, `onSwitchAccount`.
  - `noAnimation`, `noEnterAnimation`.

- **方法**:
  - `handleSwitchAccount(id)`: 切换目标账号。使用 `setTimeout` 延迟清除切换状态以应对 StrictMode 下的双重挂载问题。
  - **UI 结构**: 复用 `OnlineDeviceStack` 组件，每个账号卡片显示状态点、学号、运营商 Chip、密码保存状态 Chip。非当前账号卡片底部有“切换”按钮。

### NetworkQualityCard (DashboardPanel.tsx)
网络质量监控卡片。

- **Props**:
  - `networkQuality`, `isRefreshingQuality`, `onRefreshQuality`.
  - `noAnimation`, `noEnterAnimation`.

- **逻辑**:
  - 使用 `resolveQualityDisplay` 解析网关延迟和外部延迟。
  - 根据 `QUALITY_CONFIG` 动态显示徽章颜色和文字。
  - 使用 `LatencyPair` 组件展示延迟详情。

### SelfOnlineCard (DashboardPanel.tsx)
在线设备信息卡片，展示当前账号下活跃的设备列表。

- **Props**:
  - `noAnimation`, `noEnterAnimation`.

- **Hooks**:
  - `useSelfCardReveal`: 管理设备明细的掩码/显示状态。依赖 `useHelloGate` 进行生物识别验证。
  - `useSelfCardFetch`: 自动查询在线设备数据。每 60 秒轮询一次以处理设备被踢下线后的自动消失。

- **方法**:
  - `fetchOnline(account)`: 调用 `tauriApiWithRetry.querySelfDashboard` 获取在线列表。
  - `handleOffline(item)`: 注销指定会话。调用 `tauriApiWithRetry.selfOfflineSession`。
  - `handleSaveAlias(mac, name)`: 保存设备别名。调用 `saveConfigDirect` 更新 `deviceAliases`。
  - **UI 逻辑**: 
    - 未验证时显示掩码（圆点）。
    - 平板端使用 `OnlineDeviceStackH`，移动端使用 `OnlineDeviceStack`。
    - 集成 `ConfirmDialog` 用于注销确认。
    - 集成 `DeviceAliasDialog` 用于重命名设备。

### SelfLogCard (DashboardPanel.tsx)
近期上网记录卡片。

- **Props**:
  - `noAnimation`, `noEnterAnimation`.

- **Hooks**:
  - `useSelfCardReveal`: 同上，控制记录明细的掩码。
  - `useSelfCardFetch`: 查询今日上网日志。限制返回最近 5 条 (`SELF_LOG_LIMIT = 5`)。

- **方法**:
  - `fetchLog(account)`: 调用 `tauriApiWithRetry.querySelfOnlineLog`。
  - **UI 逻辑**: 类似 SelfOnlineCard，未验证时掩码时间、时长和流量。

### SelfServiceMergedEmptyCard (DashboardPanel.tsx)
移动端专用的合并空态卡片，当无自助凭据时替代 `selfOnline` 和 `selfLog`。

### EditableCardItem (DashboardPanel.tsx)
编辑模式下的单个卡片包装器，支持长按拖动排序。

- **Props**:
  - `id`, `onRemove`, `removeLabel`, `onSwap`, `children`.

- **手势逻辑**:
  - `LONG_PRESS_MS = 350`: 长按 350ms 激活拖拽。
  - `handlePointerDown`: 启动计时器。若点击前移动超过 10px 则取消（视为滚动）。
  - `handlePointerMove`: 激活后监听移动距离，累计越过卡片高度一半时触发 `onSwap`。
  - `preventRef`: 激活后阻止 `touchmove` 默认行为，防止浏览器接管滚动。
  - **视觉反馈**: 激活时放大并添加阴影，使用 `framer-motion` 的 `layout` 属性实现平滑过渡。

### AboutDialog (AboutDialog.tsx)
桌面端关于对话框。

- **Props**:
  - `open`, `onClose`, `openExternal`, `onUpdateAvailable`, `initialLatestVersion`, `initialReleaseNotes`, `initialUpdateAvailable`.

- **状态**:
  - `checking`: 是否正在检查更新。
  - `showSponsor`: 是否显示赞助面板。
  - `updateInfo`: 更新检查结果 (`UpdateInfo`)。
  - `downloadState`: `'idle' | 'selecting' | 'downloading' | 'done' | 'error'`。
  - `progress`: 下载进度 (`DownloadProgress`)。
  - `mirrors`: 镜像源列表。
  - `selectedMirror`: 选中的镜像源 URL。

- **方法**:
  - `handleCheckUpdate`: 调用 `api.checkUpdate()`，处理 403/404 错误，通知父组件。
  - `handleDownload(url)`: 开始下载，注册 `onDownloadProgress` 监听，处理完成或错误。
  - `ensureFullUpdateInfo`: 确保 `updateInfo` 包含完整的 `sha256Checksum` 和 `assets`，必要时重新请求。
  - `handleInstall`: 调用 `api.installUpdate(downloadedFile, sha256Checksum)`。
  - `handleQuickDownload`: 一键下载流程：获取镜像列表 -> 选择首选非 GitHub 源 -> 开始下载。
  - `renderNotesLines`: 自定义 Markdown 渲染器，支持标题、列表、表格行和内联代码/粗体。

### AboutDialogMobile (AboutDialogMobile.tsx)
移动端关于对话框。

- **Props**:
  - `open`, `onClose`, `openExternal`, `onUpdateAvailable`.

- **状态**:
  - `updateSource`: `'mirror' | 'github'`，从 `useConfigStore` 读取。
  - `downloading`, `progress`, `downloadedPath`, `downloadError`.

- **方法**:
  - `handleCheckUpdate`: 同桌面端，但更简洁。
  - `handleDownload`: 根据 `updateSource` 调整镜像重试顺序。`github` 优先直连，`mirror` 优先镜像。循环尝试直到成功或全部失败。
  - `handleInstall`: 调用 `api.installUpdate(downloadedPath)`。
  - **UI 逻辑**: 若 Release 无 APK 资产，显示外链到 GitHub Releases 的按钮。

### useAuth (useAuth.ts)
认证状态 Hook。

- **返回值**:
  - `isLoggingIn`, `isLoggingOut`, `status`, `doLogin`, `doLogout`, `checkOnline`: 来自 `useAuthStore`。
  - `handleOpenPortal`: 打开认证门户页面 (`config.portalUrl` 或默认 `http://10.1.99.100`)。
  - `handleOpenSelfService`: 打开自助服务页面 (`http://10.1.80.200:8080/Self/login/?302=LI`)。

### Types (types.ts)
- `PortalStatusResult`: 门户状态结果 (`online`, `message`, `reachable`, `loginAvailable`)。
- `CommandResult`: 通用命令结果 (`success`, `message`, `data`)。
- `LoginResult`: 继承自 `CommandResult`。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                       App Shell                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                   DashboardPanel                         │ │
│  │  ┌─────────────┐ ┌──────────────┐ ┌──────────────────┐ │ │
│  │  │QuickActions │ │AccountManage │ │NetworkQuality    │ │ │
│  │  └─────────────┘ └──────────────┘ └──────────────────┘ │ │
│  │  ┌─────────────┐ ┌──────────────┐                      │ │
│  │  │SelfOnline   │ │SelfLog       │                      │ │
│  │  └─────────────┘ └──────────────┘                      │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                  AboutDialog / Mobile                    │ │
│  │  ┌─────────────┐      ┌──────────────┐                 │ │
│  │  │ Check Update│ ──►  │ Download     │                 │ │
│  │  └─────────────┘      └──────────────┘                 │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
         ▲                        ▲
         │                        │
┌────────┴────────┐      ┌───────┴────────┐
│ useAuthStore    │      │ tauriApiWithRetry│
│ (Zustand)       │      │ (Tauri Backend)  │
└─────────────────┘      └────────────────┘
```

## Data Flow

1. **初始化与布局加载**:
   - `DashboardPanel` 挂载时，从 `safeStorage` 读取 `campus-dashboard-layout`。
   - 结合 `DEFAULT_LAYOUT` 和 `extraCards` 的 `defaultPosition` 计算初始卡片顺序。
   - 处理 `EXTRA_REMOVED_KEY` 黑名单，防止已删除的自定义卡片复活。

2. **账号与状态同步**:
   - `DashboardPanel` 接收 `accounts` 和 `activeAccount` props。
   - `AccountManageCard` 监听账号变化，渲染堆叠卡片。
   - `useAuthStore` 提供全局登录状态，影响 `QuickActionsCard` 等组件的行为（如禁用按钮）。

3. **自助服务数据流**:
   - `SelfOnlineCard` 和 `SelfLogCard` 使用 `useSelfCardReveal` 和 `useSelfCardFetch`。
   - 当配置了 `user` 和 `passwordSaved` 时，自动发起首次查询。
   - `SelfOnlineCard` 额外启动 60s 定时器轮询 `refetch`，确保设备下线后 UI 实时更新。
   - 用户点击“眼睛”图标触发 `useHelloGate` 验证，通过后解除掩码。

4. **更新流程 (AboutDialog)**:
   - 打开对话框时，若存在缓存结果 (`initialLatestVersion`) 则直接使用，否则调用 `api.checkUpdate()`。
   - 检测到更新时，显示 `releaseNotes`。
   - 用户点击“一键下载”，`handleQuickDownload` 获取镜像列表，选择最优源（非 GitHub）开始下载。
   - 下载过程中通过 `onDownloadProgress` 更新进度条。
   - 下载完成后，`downloadState` 变为 `done`，显示安装按钮。
   - 点击安装，调用 `api.installUpdate`，传入文件路径和 SHA256 校验和。

5. **移动端更新流程 (AboutDialogMobile)**:
   - 用户可在 UI 中选择 `updateSource` (`mirror` 或 `github`)。
   - 下载时，根据源类型调整镜像重试顺序。
   - 若 Release 无 APK，降级为打开 GitHub Releases 网页。

## Connections

- [[frontend-auth-store]]: `useAuthStore` 管理认证状态，被 `useAuth` 和 `DashboardPanel` 引用。
- [[frontend-config-store]]: `useConfigStore` 管理配置（包括 `updateSource`, `deviceAliases`, `enableNetworkQuality`），被 `AboutDialogMobile` 和 `DashboardPanel` 引用。
- [[frontend-tauri-api]]: `tauriApiWithRetry` 提供所有后端通信（检查更新、下载、安装、查询自助服务），被 `AboutDialog`、`AboutDialogMobile` 和 `DashboardPanel` 引用。
- [[frontend-shared-ui]]: 使用 `@/components/ui/*` 中的基础组件（Button, Dialog, Card 等）。
- [[frontend-account-self-service]]: `SelfOnlineCard` 和 `SelfLogCard` 的逻辑与自助服务页紧密相关，共享 `useHelloGate` 和 API 调用路径。