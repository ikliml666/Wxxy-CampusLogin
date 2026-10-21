---
title: android-frontend-auth
type: module
source_files:
  - android/frontend/src/auth/AboutDialog.tsx
  - android/frontend/src/auth/AboutDialogMobile.tsx
  - android/frontend/src/auth/index.ts
  - android/frontend/src/auth/types.ts
  - android/frontend/src/auth/useAuth.ts
tags: [auth, dashboard, about-dialog, tauri-api, mobile-ui]
---

## Overview
本模块 `android-frontend-auth` 负责 Android 端（含桌面与移动端适配）的认证交互、Dashboard 仪表盘展示及关于对话框逻辑。核心功能包括：
1.  **Dashboard 面板**：提供可配置的卡片式布局，支持快捷操作、账号管理、自助服务（在线设备/日志）、网络质量四大类。包含自定义拖拽排序、黑名单持久化、移动端空态合并优化。（注：源码未内联，行号参考已移除）
2.  **认证 Hook (`useAuth`)**：封装登录状态检查、门户跳转及自助服务跳转逻辑。
3.  **关于对话框**：区分桌面端（`AboutDialog`）与移动端（`AboutDialogMobile`），处理版本检查、下载进度监控、安装流程及 Release Notes 渲染。
4.  **类型定义**：定义门户状态、命令结果等基础数据结构。

## Key Components

### `DashboardPanel.tsx`
**核心职责**：提供可配置的 Dashboard 面板，包含快捷操作、账号管理、自助服务、网络质量四大类卡片。支持卡片自定义排序、拖拽交换、黑名单持久化及移动端空态合并优化。

*   **CardId 体系**：内置 `BuiltinCardId` (quickActions, accountManage, selfOnline, selfLog, networkQuality) 与宿主注入的 `string & {}` 泛化 ID。
*   **ExtraCardDef**：宿主自定义卡片接口，含 `render(editing)` 函数，默认位置 `defaultPosition` ('start'/'end')。
*   **布局存储**：使用 `safeStorage` 持久化 `campus-dashboard-layout`；通过 `EXTRA_REMOVED_KEY` 记录被显式删除的 Extra 卡 ID，防止迁移时复活。
*   **QuickActionsCard**：DHCP 续租与获取新 IP。使用 `useAsyncLock` 防抖，低质量网络触发红色光晕动画。
*   **AccountManageCard**：账号切换。复用 `OnlineDeviceStack` 竖排堆叠 UI。严格模式下配合 `mountedRef` 确保状态清理。
*   **NetworkQualityCard**：展示网关/外网延迟。调用 `resolveQualityDisplay` 解析质量等级，渲染 `LatencyPair`。
*   **SelfService Cards (Online/Log)**：
    *   **鉴权掩码**：`useSelfCardReveal` hook 依赖 `useHelloGate` 验证门控。查看需生物识别/密码验证，隐藏无需验证。
    *   **数据获取**：`useSelfCardFetch` hook 封装 `tauriApiWithRetry`，处理锁、挂载检查、错误 Toast。
    *   **SelfOnlineCard**：查询在线设备列表。凭据齐备后启动轮询自动刷新下线状态。支持设备别名编辑、单会话注销。移动端区分水平与垂直堆叠 UI。
    *   **SelfLogCard**：查询今日上网记录，限制数量。
    *   **MergedEmptyCard**：移动端无凭据时，将 selfOnline/selfLog 合并为一张紧凑提示卡。
*   **编辑与拖拽**：自研长按拖拽逻辑。按下后注册全局阻止滚动，移动超阈值取消预备。抬起或移动触发交换相邻卡片，伴随震动反馈。
*   **主组件 DashboardPanel**：
    *   **初始化**：加载布局，合并 Extra 卡，应用黑名单过滤，补全缺失的 start 位 Extra 卡。
    *   **状态订阅**：浅比较订阅 `authStore.bgStatus`, `qualityStore`, `configStore`。计算 `mergeSelfEmpty` 标志。
    *   **CRUD 操作**：添加并清黑名单、移除并加黑名单、仅交换 visible 卡片。
    *   **渲染逻辑**：非编辑模式下，若 `mergeSelfEmpty` 且遇到 `selfOnline`，渲染合并空卡并跳过 `selfLog`。

### `index.ts`
**职责**：模块出口文件。
- **导出** (`1-5`)：`DashboardPanel`, `AboutDialog`, `useAuth`，以及 `types` 命名空间。

### `types.ts`
**职责**：定义认证相关数据结构。
- **PortalStatusResult** (`1-6`)：门户状态结果，含 `online`, `message`, `reachable`, `loginAvailable`。
- **CommandResult** (`8-12`)：通用命令返回结构，`success`, `message`, `data`。
- **LoginResult** (`14`)：继承 `CommandResult`。

### `useAuth.ts`
**职责**：封装认证逻辑与外部链接跳转。
- **Hook 实现** (`7-33`)：
    - 从 `useAuthStore` 解构 `isLoggingIn`, `doLogin`, `checkOnline` 等核心方法。
    - `handleOpenPortal` (`20-23`)：打开校园网门户，优先使用 `config.portalUrl`，默认 `http://10.1.99.100`。
    - `handleOpenSelfService` (`25-27`)：固定打开自助服务登录页 `http://10.1.80.200:8080/Self/login/?302=LI`。

### `AboutDialog.tsx` (Desktop)
**职责**：桌面端（横版双栏布局）的“关于”对话框。负责展示应用信息、检查更新、渲染 Release Notes、处理多镜像源下载及安装流程，并包含赞助内嵌页。

*   **常量与类型**：
    *   `GITHUB_REPO`: `'ikliml666/Wxxy-CampusLogin'` (行33)。
    *   `DownloadState`: 枚举类型 `'idle' | 'selecting' | 'downloading' | 'done' | 'error'` (行35)。
    *   `CORE_FEATURES`: 核心优势数据数组，含 `Users`, `Wifi` 图标及 i18n key (行38-41)。
    *   Props: `AboutDialogProps` 包含 `open`, `onClose`, `openExternal`, `onUpdateAvailable`, `initialLatestVersion` 等 (行23-31)。
*   **辅助函数**：
    *   `formatSize(bytes)`: 格式化文件大小 (行43-48)。
    *   `formatSpeed(bytesPerSec)`: 格式化下载速度 (行50-54)。
    *   `renderInlineMarkdown(text)`: 简易 Markdown 渲染器，支持 `**bold**` 和 `` `code` `` (行56-67)。
    *   `renderNotesLines(notes)`: 逐行解析 Release Notes，支持 `###`, `##`, `#` 标题，列表项 `-`/`*`/数字点，表格行 `|`，段落 (行70-96)。注意标题判断顺序从长到短以防错位 (行74-77)。
*   **状态管理 (`useState`)**：
    *   `checking`: 检查中状态 (行101)。
    *   `showSponsor`: 控制右侧栏切换为赞助面板 (行103)。
    *   `updateInfo`: `UpdateInfo` 对象或 null (行104)。
    *   `downloadState`: 当前下载阶段 (行105)。
    *   `progress`: `DownloadProgress` 对象 (行106)。
    *   `mirrors`: `MirrorSource[]` 镜像列表 (行107)。
    *   `downloadError`, `installError`, `checkError`: 各类错误信息 (行108-111)。
    *   `downloadedFile`: 下载完成的本地文件路径 (行110)。
    *   `showMirrorList`, `selectedMirror`: 镜像选择 UI 状态 (行112-113)。
    *   `autoCheckedRef`: 防止重复自动检查 (行117)。
*   **关键逻辑与副作用**：
    *   **初始化与缓存处理**：
        *   若存在 `initialLatestVersion` 且无 `updateInfo`，直接构造静态 `UpdateInfo` 对象并标记已检查 (行147-160)。
        *   若打开且未自动检查过，调用 `handleCheckUpdate` (行162-167)。
    *   **`handleCheckUpdate`**: 调用 `api.checkUpdate()`。捕获特定错误：包含 '403' 或 '频率受限' 显示 API 限制提示；'404' 显示未找到；其他显示通用失败 (行121-141)。
    *   **`handleDownload`**:
        *   清理旧监听器，注册 `api.onDownloadProgress` 监听进度 (行185-187)。
        *   调用 `api.downloadUpdate(url)`。成功设置 `downloadedFile` 和 `done` 状态；失败设置 `downloadError` 和 `error` 状态 (行175-200)。
    *   **`ensureFullUpdateInfo`**: 在下载前确保 `updateInfo` 包含 `sha256Checksum` 和完整 `assets`。若缺失则重新调用 `checkUpdate`，失败则返回原缓存对象 (行205-215)。注释指出直接下载可能导致 404 或安装被拒 (行202-204)。
    *   **`handleInstall`**: 调用 `api.installUpdate(downloadedFile, sha256Checksum)`。必须向用户展示错误反馈 (行217-226)。
    *   **`handleQuickDownload`**:
        *   获取镜像列表 `api.getMirrorUrls`。
        *   优先选择非 GitHub 官方源 (行254)。
        *   若获取镜像失败，回退到默认 GitHub URL (行260-264)。
        *   最终调用 `handleDownload` (行241-265)。
    *   **UI 渲染逻辑**：
        *   左侧栏：固定 320px，展示图标、版本、描述、检查按钮、赞助/GitHub 链接 (行278-339)。
        *   右侧栏：根据 `downloadState` 和 `updateInfo?.hasUpdate` 切换视图：
            *   `idle` + 无更新：显示“已是最新”或上次检查失败详情，展示核心优势卡片 (行384-441)。
            *   `idle` + 有更新：展示 `renderNotesLines` 生成的滚动内容区，底部固定“一键下载”按钮和镜像源切换入口 (行444-545)。
            *   `selecting`: 加载动画 (行448-553)。
            *   `downloading`: 进度条、百分比、速度 (行556-582)。
            *   `done`: “安装更新”按钮，安装错误提示 (行585-609)。
            *   `error`: 错误详情，重试按钮 (行612-637)。
        *   赞助页：通过 `showSponsor` 切换，展示微信/支付宝二维码图片 (行345-380)。
        *   镜像下拉菜单：绝对定位向上弹出 (`bottom-full`)，选中高亮 (行511-541)。

### `AboutDialogMobile.tsx` (Mobile)
**职责**：移动端（竖版单栏布局）的“关于”对话框。精简功能，专注于应用内 APK 下载安装，支持配置更新源优先级。

*   **常量与类型**：
    *   `GITHUB_REPO`: `'ikliml666/Wxxy-CampusLogin'` (行30)。
    *   Props: `AboutDialogMobileProps` (行23-28)。
*   **状态管理**：
    *   `checking`, `updateInfo`, `checkError` (行35-37)。
    *   `downloading`, `progress`, `downloadedPath`, `downloadError` (行39-42)。
    *   `updateSource`: 从 `useConfigStore` 读取，默认 `'mirror'` (行44-45)。
    *   `unlistenRef` (行46)。
*   **关键逻辑**：
    *   **`handleCheckUpdate`**: 逻辑同桌面端，处理 403/404 错误 (行50-64)。
    *   **资源判断**: 查找 `.apk` 结尾的资产 `apkAsset`。若无，则提供外链 Releases 按钮 (行68, 247-256)。
    *   **`handleDownload`**:
        *   构建镜像列表：先获取 `api.getMirrorUrls(apkAsset.url)`，若失败则仅保留 GitHub 直连 (行77-78)。
        *   排序：若 `updateSource === 'github'`，GitHub 排第一；否则镜像排第一 (行79-81)。
        *   循环重试：遍历 mirrors，逐个尝试 `api.downloadUpdate`。任一成功即返回；全部失败记录最后错误 (行83-98)。
        *   监听器管理：每次尝试前清理并注册新监听 (行85-89)。
    *   **`handleInstall`**: 调用 `api.installUpdate(downloadedPath)`，无 checksum 参数 (行103-110)。
    *   **UI 渲染**：
        *   顶部：应用图标、名称、版本 (行122-134)。
        *   更新源配置：两个按钮切换 `mirror` / `github`，持久化到 store (行137-159)。
        *   检查更新区域：按钮、错误提示、成功提示 (行162-193)。
        *   有更新时：显示 Notes (纯文本 `whitespace-pre-line`)，下载/安装按钮，进度条 (行194-258)。
        *   项目链接：GitHub 仓库按钮 (行262-269)。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      Parent Container                        │
│  (Decides Desktop vs Mobile based on viewport/device)        │
└───────────────┬───────────────────────────────┬──────────────┘
                │                               │
                ▼                               ▼
┌──────────────────────────┐      ┌──────────────────────────┐
│   AboutDialog.tsx        │      │   AboutDialogMobile.tsx  │
│   (Desktop, Dual-pane)   │      │   (Mobile, Single-pane)  │
│                          │      │                          │
│  - Check Update          │      │  - Check Update          │
│  - Download via Mirror   │      │  - Retry Mirrors Loop    │
│  - Install with Checksum │      │  - Install w/o Checksum  │
│  - Sponsor Panel         │      │  - Source Config Toggle  │
└───────────────┬──────────┘      └──────────────┬───────────┘
                │                               │
                ▼                               ▼
┌──────────────────────────────────────────────────────────────┐
│                  Tauri API Layer                             │
│  api.checkUpdate() / api.downloadUpdate() / api.installUpdate│
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                     DashboardPanel                           │
│                                                              │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ QuickActions│  │ AccountManage│  │ SelfServiceCards │   │
│  │ (DHCP)      │  │ (Switch)     │  │ (Online/Log)     │   │
│  └─────────────┘  └──────────────┘  └──────────────────┘   │
│                                                              │
│  ┌──────────────────┐  ┌──────────────────┐                │
│  │ NetworkQuality   │  │ EditableCardItem │                │
│  │ (Latency)        │  │ (Drag & Drop)    │                │
│  └──────────────────┘  └──────────────────┘                │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. 入口层
*   `index.ts` 导出 `DashboardPanel` 和 `useAuth` 供上层容器或路由使用。
*   `DashboardPanel` 接收 `DashboardPanelProps`，其中包含 `accounts`, `activeAccount`, `onSwitchAccount`, `onDhcpRenew` 等回调。

### 2. 状态管理层
*   `DashboardPanel` 内部通过 `useConfigStore`, `useAuthStore`, `useQualityStore` 直接订阅全局状态。
*   `useAuth` Hook 也订阅 `useAuthStore` 和 `useConfigStore`，提供高层 API 给 UI 组件（如按钮点击）。

### 3. 数据流细节
*   **配置更新**：`DashboardPanel` 内的 `SelfOnlineCard` 保存别名时，调用 `saveConfigDirect` 写入 `config.deviceAliases`。
*   **API 调用**：
    *   `SelfOnlineCard` / `SelfLogCard` -> `useSelfCardFetch` -> `tauriApiWithRetry.querySelfDashboard` / `querySelfOnlineLog`。
    *   `QuickActionsCard` -> `onDhcpRenew` / `onDhcpReleaseRenew` (由父组件传入，通常绑定到底层 Tauri 命令)。
    *   `useAuth.handleOpenPortal` -> `api.openExternal`。
    *   `AboutDialog` / `AboutDialogMobile` -> `api.checkUpdate`, `api.downloadUpdate`, `api.installUpdate`。
*   **布局持久化**：`DashboardPanel` 的 `cards` 状态变化触发 `useEffect` 调用 `saveLayout`，写入 `safeStorage`。

### 4. 组件嵌套
*   `DashboardPanel` -> `renderCard` -> `QuickActionsCard` / `AccountManageCard` / `SelfOnlineCard` / `SelfLogCard` / `NetworkQualityCard`。
*   `AccountManageCard` / `SelfOnlineCard` -> `OnlineDeviceStack` / `OnlineDeviceStackCard`。
*   `SelfOnlineCard` -> `ConfirmDialog` (注销确认), `DeviceAliasDialog` (重命名)。

## Connections
*   [[android-frontend-config]]: `DashboardPanel` 和 `AboutDialogMobile` 均依赖 `useConfigStore` 获取配置。
*   [[android-frontend-store]]: `useAuth` 和 `DashboardPanel` 均订阅 `useAuthStore`。
*   [[android-frontend-tauri-api]]: 所有 API 调用均通过 `tauriApiWithRetry` 实例进行。

## Known Issues

### 1. StrictMode 兼容性
*   `AccountManageCard` 和 `useSelfCardFetch` 均使用 `mountedRef` 并在 `useEffect` 中重置。这是为了应对 React 18 StrictMode 下 useEffect 执行两次导致的 `setSwitchingAccount` 在 cleanup 后被再次 set 但 ref 已失效的问题。

### 2. Extra 卡生命周期管理
*   **黑名单机制**：内置卡删除后可随时重新添加；宿主注入的 Extra 卡一旦删除，ID 会被加入 `EXTRA_REMOVED_KEY`。重启或布局迁移时，即使代码中定义了新的 Extra 卡，若其 ID 在黑名单中也不会自动恢复，除非用户重新手动添加（此时从黑名单移除）。这解决了“旧布局不存在”与“用户主动删除”无法区分的歧义。
*   **位置优先级**：Extra 卡根据 `defaultPosition` 插入到内置卡之前或之后，且优先于 DEFAULT_LAYOUT 中的顺序。

### 3. 移动端密度优化
*   `mergeSelfServiceEmpty`：当 props 开启且无凭据时，`selfOnline` 和 `selfLog` 两张卡合并为一张 `SelfServiceMergedEmptyCard`。注意：此合并仅在**非编辑模式**且**无凭据**时生效；编辑模式下始终渲染两张占位卡。

### 4. 拖拽交互的非标准实现
*   `EditableCardItem` 未使用 Framer Motion 的 drag 功能，因为 WebView 环境下 pointer capture 失效。
*   **滚动冲突解决**：长按激活后，通过 `document.addEventListener('touchmove', prevent, { passive: false })` 全局拦截滚动，直到松开手指或移动距离过短。
*   **阈值判定**：移动超过卡片高度的一半才触发交换，避免误触。

### 5. 自助服务数据刷新策略
*   **首次查询**：依赖 `account` 引用变化触发 `useEffect`。
*   **持续监控**：`SelfOnlineCard` 在凭据存在时，额外启动 `setInterval` 每 60 秒轮询 `refetch()`，以捕捉服务端踢下线的状态变更，而非仅依赖用户手动刷新。

### 6. 安全与隐私
*   **掩码显示**：`SelfOnlineCard` 和 `SelfLogCard` 在未验证状态下，IP、MAC、时间等敏感信息显示为圆点。
*   **Hello Gate**：查看明文必须通过 `useHelloGate` 的生物识别/密码验证，验证通过后全局解锁当前会话的可见性。

### 7. 桌面端缓存一致性风险
*   `ensureFullUpdateInfo` 的存在表明初始传入的 `updateInfo` (可能来自缓存) 可能缺少 `sha256Checksum` 和 `assets`。直接下载会导致 404 或后端拒绝安装。该函数强制刷新以确保完整性，失败时仍返回残缺对象，此时安装会报错。

### 8. 桌面端 Release Notes 渲染
*   `renderNotesLines` 手动解析 Markdown，对多级标题 (`###`, `##`, `#`) 统一渲染为 `<h3>`，需注意判断顺序。表格行若不含分隔符行 (`---`) 则不作为表头跳过，但代码中 `isHeader` 判断后若为假仍渲染为 flex 布局单元格，而非标准 table 标签。

### 9. 移动端镜像重试机制
*   `handleDownload` 在移动端采用同步循环重试策略，依次尝试每个镜像源，直到成功或全部失败。这比桌面端的“选一个最好的”更健壮，适合网络不稳定的移动环境。

### 10. 移动端更新源配置
*   移动端特有功能：允许用户在 `mirror` (镜像加速) 和 `github` (官方直连) 之间切换优先级，并持久化存储。桌面端无此 UI，默认使用 `handleQuickDownload` 中的自动优选逻辑。

### 11. 移动端 APK 缺失兜底
*   若 Release 中没有 `.apk` 资产，移动端不提供下载按钮，而是提供跳转到 GitHub Releases 页面的外链按钮。

### 12. 安装反馈
*   桌面端 `handleInstall` 明确注释此前静默失败，现增加 `installError` 状态以提供用户可见反馈。

### 13. 监听器清理
*   两文件均使用 `unlistenRef` 管理 Tauri 事件监听。在开始新下载前必须清理旧监听，并在 `finally` 或失败时清理，防止内存泄漏或重复触发。