---
title: shared_types
type: module
source_files:
  - tauri-app/frontend/src/shared/types.ts
  - tauri-app/frontend/src/shared/ui-types.ts
  - tauri-app/frontend/src/shared/ui-constants.ts
  - tauri-app/frontend/src/shared/index.ts
  - tauri-app/frontend/src/hooks/tauriApi.ts
  - tauri-app/frontend/src/hooks/useLogToastStore.ts
  - tauri-app/frontend/src/hooks/useThemeStore.ts
  - tauri-app/frontend/src/settings/types.ts
  - tauri-app/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/settings/useSettings.ts
  - tauri-app/frontend/src/components/layout/DockNav.tsx
  - tauri-app/frontend/src/hooks/useAuthStore.ts
  - tauri-app/frontend/src/hooks/useQualityStore.ts
tags: [shared, typescript, types, ui, tauri, frontend]
---

## Overview

`shared_types` 不是一个独立的 npm 包，而是前端 `tauri-app/frontend/src/shared/` 目录下**集中存放的纯类型与常量**模块。它为整个桌面 Tauri 应用提供跨模块复用的 TypeScript 类型（UI 状态枚举、日志/Toast 数据结构、更新检查与下载、镜像源等）以及少数全局常量。

该模块由四个文件组成，全部通过 `shared/index.ts` 这个 barrel 文件以 `export *` 统一再导出，因此业务代码可直接 `import { ... } from '@/shared'`（路径别名解析见 `tsconfig.json`），也可从具体子文件（如 `@/shared/ui-types`）精确导入。

- `types.ts`：后端事件 / 命令载荷的业务数据模型（`UpdateAvailableData`、`UpdateInfo`、`DownloadProgress`、`MirrorSource` 等）。
- `ui-types.ts`：前端 UI 状态枚举与组件级数据结构（`StatusState`、`PanelName`、`ThemeName`、`GlassLevel`、`LogType`、`LogEntry`、`ToastMessage` 等）。
- `ui-constants.ts`：全局 UI 常量（`MAX_LOG_ENTRIES`、`APP_VERSION`、`APP_NAME`、`PASSWORD_MASK`、`NAV_ITEMS`）。
- `index.ts`：仅做聚合再导出（含 `LogPanel` 等组件，但本模块仅关注类型与常量，组件见 links）。

这些类型/常量是"数据契约"：Tauri 后端（`src-tauri/`）的 Rust 命令与事件吐出的 JSON，经 `@tauri-apps/api/core` 的 `invoke` 反序列化为这些类型（见 `tauriApi.ts` 的 `invoke<UpdateInfo>('check_update')` 等），再被各业务 store（zustand）和组件消费。因此它们既是前端内部共享的"单一事实来源"，也是前后端接口的边界定义。

## Key Components

### 文件 `types.ts`（数据模型类型）

`export interface UpdateAvailableData` — 第 1–5 行
- `hasUpdate: boolean`（第 2 行）：是否有可用更新。
- `latestVersion: string`（第 3 行）：最新版本号。
- `releaseNotes?: string`（第 4 行）：发布说明，可选。

`export interface UpdateInfo` — 第 7–17 行
- `hasUpdate: boolean`（第 8 行）
- `latestVersion: string`（第 9 行）
- `releaseNotes: string`（第 10 行）
- `assets: { name: string; url: string; size: number }[]`（第 11 行）：更新资源清单（每项含名称、下载 URL、字节大小）。
- `sha256Checksum?: string`（第 12 行）：校验和，可选。
- `lastCheckError: string | null`（第 14 行）：最近一次更新检查失败的**原因快照**（本次检查前快照；`null` = 此前无失败或上次已成功）。
- `lastCheckTime: number | null`（第 16 行）：最近一次更新检查完成的 **unix 毫秒时间戳**（`null` = 从未检查）。这是自动生成/后端填充的时间字段，前端只读。

`export interface DownloadProgress` — 第 19–24 行
- `downloaded: number`（第 20 行）：已下载字节数。
- `total: number`（第 21 行）：总字节数。
- `speed: number`（第 22 行）：下载速度（字节/秒，约定）。
- `percent: number`（第 23 行）：进度百分比（0–100）。

`export interface MirrorSource` — 第 26–30 行
- `name: string`（第 27 行）：镜像源名称。
- `url: string`（第 28 行）：镜像源地址。
- `description: string`（第 29 行）：描述。

### 文件 `ui-types.ts`（UI 类型与枚举）

`export type StatusState` — 第 1 行：`'loading' | 'online' | 'offline' | 'error'`，在线状态机的取值集合（被 `useAuthStore.ts` 第 4 行导入用于认证状态）。

`export type PanelName` — 第 2 行：`'dashboard' | 'account' | 'selfservice' | 'network' | 'monitor' | 'quality' | 'settings' | 'log' | 'speedtest'`，导航面板标识。被 `App.tsx`（第 24 行）、`DockNav.tsx`（第 1、52、56 行）、`settings/types.ts`（第 1、65 行 `defaultPanel: PanelName | ''`）消费。

`export type ThemeName` — 第 3 行：`'default' | 'vibrant' | 'forest' | 'midnight' | 'ocean' | 'cherry' | 'custom'`，主题名。被 `useThemeStore.ts`（第 3 行）、`settings/constants.ts`（第 2、85–95 行）、`useSettings.ts`（第 9 行）使用，校验白名单见 `VALID_THEMES`。

`export type GlassLevel` — 第 5 行：`'off' | 'standard' | 'ultra'`，玻璃质感三档（对齐 Apple Liquid Glass / FlClash）。被 `useThemeStore.ts`（第 3 行）、`settings/constants.ts`（第 2、103–109 行）使用，校验白名单见 `VALID_GLASS_LEVELS`。

`export type LogType` — 第 6 行：`'info' | 'success' | 'error' | 'warning'`，日志/Toast 类型。被 `useLogToastStore.ts`（第 2 行）、`useEventListeners.ts`（第 3 行）、`LogPanel.tsx`、`ToastContainer.tsx` 使用。

`export interface LogEntry` — 第 8–15 行
- `id: string`（第 9 行）：日志条目 id（由 `useLogToastStore` 的 `logIdCounter` 自增字符串生成，第 45 行）。
- `time: string`（第 10 行）：发生时间（本地时间字符串，第 33 行 `toLocaleTimeString`）。
- `message: string`（第 11 行）
- `type: LogType`（第 12 行）
- `count?: number`（第 14 行）：连续重复折叠的累计次数（`undefined` 表示只出现一次；去重折叠逻辑见 `useLogToastStore.ts` 第 39–43 行）。

`export interface ToastMessage` — 第 17–29 行
- `id: string`（第 18 行）：toast id（`toastIdCounter` 自增，第 63 行）。
- `title: string`（第 19 行）
- `description?: string`（第 20 行）
- `type: LogType`（第 21 行）
- `duration?: number`（第 22 行）：显示时长（毫秒，默认 4000，带 action 的默认 8000，见 `useLogToastStore.ts` 第 53、86 行）。
- `mascot?: 'portrait' | 'celebrate' | 'offline' | 'alert' | 'update'`（第 24 行）：看板娘变体，缺省按 `type` 取默认变体（`ToastContainer.tsx` 第 16–21 行 `TOAST_MASCOTS`）。
- `action?: { label: string; onClick: () => void }`（第 25–28 行）：操作按钮（如"取消退出"）。

`export interface AdapterDisabledWarningData` — 第 31–34 行
- `name: string`（第 32 行）
- `message: string`（第 33 行）
- 由后端事件 `adapter-disabled-warning` 经 `tauriApi.ts` 第 81、210 行 `onAdapterDisabledWarning` 推送。

`export interface AutoExitCountdownData` — 第 36–39 行
- `delay: number`（第 37 行）
- `shortcut: string`（第 38 行）
- 由后端事件 `auto-exit-countdown` 经 `tauriApi.ts` 第 115、256 行 `onAutoExitCountdown` 推送。

`export interface SaveConfigResult` — 第 41–45 行
- `success: boolean`（第 42 行）
- `message?: string`（第 43 行）
- `data?: Record<string, unknown>`（第 44 行）
- `saveConfig` 命令的返回体（`tauriApi.ts` 第 9、41、179 行），`tauriApiWithRetry` 仅对该方法做指数退避重试（第 314–320 行）。

### 文件 `ui-constants.ts`（常量）

`export const MAX_LOG_ENTRIES = 300` — 第 1 行：日志环形缓冲上限。`useLogToastStore.ts` 第 3、46–48、129 行据此裁剪。

`export const APP_VERSION = '2.4.3'` — 第 2 行：应用版本号。

`export const APP_NAME = '校园网登录助手'` — 第 3 行：应用名。

`export const PASSWORD_MASK = '***'` — 第 4 行：密码掩码（导出配置时掩码明文密码）。

`export const NAV_ITEMS` — 第 7–17 行：导航项数组（只读 `as const`），每项 `{ id: 'dashboard' as const, labelKey: 'nav.dashboard', icon: 'LayoutDashboard', shortcut: '1' }`。`id` 取值与 `PanelName` 一一对应（注意 `'selfservice'` 有对应项但无 `shortcut` 显示差异略）；`DockNav.tsx` 第 20 行导入渲染导航，`settings/constants.ts` 第 1、111–114 行据此生成 `DEFAULT_PANEL_OPTIONS`。

### 文件 `index.ts`（barrel）

第 1–12 行：依次导出组件（`LogPanel`、`ConfirmDialog`、`AnimatedNumber`、`RefreshButton`、`SegmentTabs`、`ToastContainer`、`FluidBackground`、`ErrorBoundary`）以及 `export * from './types'`、`export * from './ui-types'`、`export * from './ui-constants'`。业务代码据此从 `@/shared` 一次性拿到类型与常量；注意 `App.tsx` 第 17–23 行刻意**按文件直接导入组件**（而非经 barrel），以避免静态引入 `LogPanel` 等低频面板模块拖入主 bundle。

## Architecture

该模块位于前端的"契约层"：上游是 Tauri 后端命令/事件（`src-tauri`），下游是 zustand store 与 React 组件。类型与常量在此单向流动、被多处汇聚复用。

```text
┌──────────────────────┐   invoke<X>('cmd')    ┌──────────────────────────┐
│  src-tauri (Rust)   │ ────────────────────► │  hooks/tauriApi.ts       │
│  commands & events  │  events via listen()  │  (类型来自 @/shared)      │
└──────────────────────┘ ◄─────────────────── └───────────┬──────────────┘
                                                         │ 反序列化为 shared 类型
                              ┌──────────────────────────┼──────────────────────────┐
                              ▼                          ▼                          ▼
                    ┌───────────────────┐   ┌────────────────────┐   ┌────────────────────┐
                    │ useLogToastStore │   │ useThemeStore      │   │ useAuthStore       │
                    │ LogEntry/Toast…  │   │ ThemeName/Glass…   │   │ StatusState        │
                    └─────────┬─────────┘   └─────────┬──────────┘   └─────────┬──────────┘
                              ▼                       ▼                        ▼
                    ┌───────────────────┐   ┌────────────────────┐   ┌────────────────────┐
                    │ ToastContainer /  │   │ settings/constants │   │ App / DockNav      │
                    │ LogPanel          │   │ ThemeName 白名单   │   │ PanelName 导航     │
                    └───────────────────┘   └────────────────────┘   └────────────────────┘

  共享契约层: shared/types.ts · shared/ui-types.ts · shared/ui-constants.ts
  经 shared/index.ts 统一再导出 → 各 store/组件 import { ... } from '@/shared'
```

## Data Flow

1. **后端 → 命令返回体**：`tauriApi.ts` 第 129–135、268–272 行通过 `invoke<UpdateInfo>('check_update')`、`invoke<MirrorSource[]>('get_mirror_urls', { githubUrl })` 等把后端 JSON 反序列化成 `UpdateInfo` / `MirrorSource` 等类型。
2. **后端 → 事件流**：后端事件经 `createEventListener<T>`（`tauriApi.ts` 第 146 行）强类型化：
   - `update-available` → `UpdateAvailableData`（第 136、274 行）
   - `update-download-progress` → `DownloadProgress`（第 135、273 行）
   - `adapter-disabled-warning` → `AdapterDisabledWarningData`（第 81、210 行）
   - `auto-exit-countdown` → `AutoExitCountdownData`（第 115、256 行）
3. **store 落库**：`useLogToastStore.ts` 第 28–136 行把 `LogType`/`LogEntry`/`ToastMessage` 作为内部状态：
   - `addLog`（第 32–51 行）生成 `LogEntry`，对连续重复消息做折叠并累加到 `count`（第 39–43 行），超过 `MAX_LOG_ENTRIES` 时裁剪（第 46–48 行）。
   - `addToast` / `addToastWithAction`（第 53–109 行）生成 `ToastMessage`，维护最多 `MAX_TOASTS = 4` 个、按 `title` 去重（第 13、62 行）、非前台跳过（第 57 行）。
4. **UI 渲染**：`ToastContainer.tsx` 消费 `toasts: ToastMessage[]`，按 `type` 取 `TOAST_MASCOTS` 默认看板娘（第 16–21 行）；`LogPanel.tsx` 渲染日志区域（其 props 的 `addToast` 签名本身就是 `'info'|'success'|'error'|'warning'`，与 `LogType` 保持一致，第 40 行）。
5. **配置驱动 UI 枚举**：`Config.defaultPanel: PanelName | ''`（`settings/types.ts` 第 65 行）与 `Config` 主题/玻璃字段经 `useThemeStore`（`useThemeStore.ts` 第 10–17 行）驱动 `ThemeName`/`GlassLevel`，并由 `settings/constants.ts` 的 `VALID_THEMES`/`VALID_GLASS_LEVELS` 做合法性校验（第 95、109 行）。
6. **导航**：`DockNav.tsx` 用 `NAV_ITEMS`（`ui-constants.ts` 第 7 行）渲染 `PanelName` 导航项，`App.tsx` 的 `activePanel`（`useAdapterStore`）即 `PanelName` 类型。

## Connections

- [[tauriApi]]：`hooks/tauriApi.ts` 是这些类型的主要"落地处"——`invoke<>`/`createEventListener<T>` 的泛型参数全部来自本模块（第 9、81、115、129–135、210、256、268–274 行）。
- [[useLogToastStore]]：`hooks/useLogToastStore.ts` 直接持有并构造 `LogType`/`LogEntry`/`ToastMessage`，并依赖 `MAX_LOG_ENTRIES` 常量（第 2–3、32–136 行）。
- [[useThemeStore]]：`hooks/useThemeStore.ts` 消费 `ThemeName`/`GlassLevel`（第 3、10–17 行）。
- [[useAuthStore]]：`hooks/useAuthStore.ts` 导入 `StatusState` 描述登录状态（第 4 行）。
- [[useQualityStore]]：版本更新标志 `updateAvailable`/`latestVersion` 由 `settings`/`quality` 模块根据 `UpdateInfo`/`UpdateAvailableData` 写入。
- [[settings]]：`settings/types.ts` 的 `Config.defaultPanel` 用 `PanelName`，`settings/constants.ts` 用 `NAV_ITEMS` 生成默认面板选项并用 `ThemeName`/`GlassLevel` 白名单校验，`useSettings.ts` 用 `ThemeName`。
- [[DockNav]]：`components/layout/DockNav.tsx` 用 `PanelName` + `NAV_ITEMS` 渲染底部导航。
- [[App]]：`App.tsx` 导入 `PanelName` 并驱动 `activePanel` 切换。
- [[account]] / [[monitor]] / [[network]]：这些模块各自定义自己的领域类型（`Adapter`、`BackgroundStatus`、`NetworkQuality`、`AccountItem` 等），与本模块的"共享 UI/数据模型类型"互补，不在本模块内。

## Known Issues

- **`StatusState` 仅被声明、未在本模块外显式见枚举消费处完全覆盖**：经检索，`StatusState` 在 `useAuthStore.ts` 第 4 行被导入，但认证状态的具体取值绑定在 `useAuthStore` 内部；若需确认每个枚举值的使用面，应去 [[useAuthStore]] 进一步核对（本模块只负责定义类型，不产生状态）。
- **`NAV_ITEMS` 中 `'selfservice'` 项无独立 `shortcut` 矛盾点**：`ui-constants.ts` 第 10 行的 `selfservice` 项没有像其他面板那样带数字快捷键（`shortcut` 字段缺失），其 `id` 仍与 `PanelName` 的 `'selfservice'` 对齐，但快捷键提示仅在 `dashboard`/`account`/`network`/`monitor`/`quality`/`speedtest`/`settings`/`log` 项出现，可能意味着自助服务面板没有快捷键，需向 UI 侧确认是否为有意设计。
- **`UpdateInfo.lastCheckTime`/`lastCheckError` 是后端快照而非前端生成**：第 14、16 行注释明确这两个字段是"本次检查前快照 / 上次已完成时间"，前端不应写入，仅用于展示上次检查状态；误用为主动状态源会导致逻辑错误。
- **`SaveConfigResult` 仅有命令返回语义**：`tauriApi.ts` 第 41、179 行，`saveConfig` 是唯一被 `withRetry` 包裹的方法（第 314–320 行），其余高频命令（如 `checkPortalStatus`、`checkNetworkQuality`）因有后端兜底而刻意不重试；不要贸然给其它 `invoke` 增加重试，以免放大流量。
- 无禁用/注释掉的导出：本模块所有类型与常量均处于启用状态（`index.ts` 第 10–12 行全部 `export *`）。
